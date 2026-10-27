//! Delta's GUI backend. Reads keyboards over VIA (read-only), streams which keys are
//! held, and loads the Delta config. The window is Svelte (`app/src`).

use delta_core::paths::Paths;
use delta_core::{Config, import};
use delta_via::keycode::{Keycode, decode};
use delta_via::{Definition, Via, discover};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// How often held keys are polled while the window is open (~60 Hz).
const POLL: Duration = Duration::from_millis(16);

/// An open keyboard plus its matrix size, shared by commands and the key watcher so
/// only one request is in flight on the device at a time.
struct Open {
    via: Via,
    rows: u8,
    cols: u8,
}

#[derive(Default)]
struct AppState {
    keyboards: Mutex<HashMap<String, Open>>,
    /// Set to false to stop the running key watcher.
    watching: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Serialize)]
struct KeyboardInfo {
    path: String,
    device_name: String,
    vendor_id: u16,
    product_id: u16,
    protocol: Option<u16>,
    definition: Option<Definition>,
    /// `[layer][row][col]`
    layers: Vec<Vec<Vec<Keycode>>>,
    /// `[layer][knob]` → (left, right)
    encoders: Vec<Vec<(Keycode, Keycode)>>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct Pressed {
    path: String,
    keys: Vec<(u8, u8)>,
}

#[derive(Serialize)]
struct ConfigInfo {
    config: Option<Config>,
    /// Where it came from.
    source: String,
    /// True when shown from the legacy delta.conf because delta.toml doesn't exist yet.
    legacy: bool,
    problems: Vec<String>,
}

/// Every VIA keyboard, with its layout and full keymap.
#[tauri::command]
async fn keyboards(state: State<'_, AppState>) -> Result<Vec<KeyboardInfo>, String> {
    let paths = Paths::from_env();
    let mut out = Vec::new();
    for device in discover() {
        let path = device.path.display().to_string();
        let mut info = KeyboardInfo {
            path: path.clone(),
            device_name: device.name.clone(),
            vendor_id: device.vendor_id,
            product_id: device.product_id,
            protocol: None,
            definition: None,
            layers: vec![],
            encoders: vec![],
            error: None,
        };
        let def = match Definition::load(device.vendor_id, device.product_id, &paths.layouts, &paths.layout_cache, true)
        {
            Ok(def) => def,
            Err(e) => {
                info.error = Some(e.to_string());
                out.push(info);
                continue;
            }
        };

        let read = || -> Result<_, delta_via::ViaError> {
            let mut via = Via::open(&device)?;
            let protocol = via.protocol_version()?;
            let keymap = via.keymap(def.rows, def.cols)?;
            let encoders = via.encoders(keymap.len() as u8, def.encoder_count())?;
            Ok((via, protocol, keymap, encoders))
        };
        match read() {
            Ok((via, protocol, keymap, encoders)) => {
                let name = |c: u16| decode(c, &def.custom_keycodes);
                info.protocol = Some(protocol);
                info.layers =
                    keymap.iter().map(|l| l.iter().map(|r| r.iter().map(|c| name(*c)).collect()).collect()).collect();
                info.encoders =
                    encoders.iter().map(|l| l.iter().map(|(ccw, cw)| (name(*ccw), name(*cw))).collect()).collect();
                state.keyboards.lock().unwrap().insert(path, Open { via, rows: def.rows, cols: def.cols });
            }
            Err(e) => info.error = Some(e.to_string()),
        }
        info.definition = Some(def);
        out.push(info);
    }
    Ok(out)
}

/// Start streaming `pressed` events for one keyboard (stops any previous stream).
#[tauri::command]
fn watch_keys(app: AppHandle, state: State<'_, AppState>, path: String) {
    let running = Arc::new(AtomicBool::new(true));
    if let Some(old) = state.watching.lock().unwrap().replace(running.clone()) {
        old.store(false, Ordering::Relaxed);
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let mut last: Option<Vec<(u8, u8)>> = None;
        while running.load(Ordering::Relaxed) {
            let keys = {
                let mut boards = state.keyboards.lock().unwrap();
                let Some(open) = boards.get_mut(&path) else { break };
                let (rows, cols) = (open.rows, open.cols);
                open.via.pressed(rows, cols)
            };
            match keys {
                Ok(keys) if last.as_ref() != Some(&keys) => {
                    let _ = app.emit("pressed", Pressed { path: path.clone(), keys: keys.clone() });
                    last = Some(keys);
                }
                Ok(_) => {}
                Err(e) => {
                    let _ = app.emit("keyboard-error", e.to_string());
                    state.keyboards.lock().unwrap().remove(&path);
                    break;
                }
            }
            std::thread::sleep(POLL);
        }
    });
}

/// The Delta config: delta.toml if it exists, otherwise a preview of the legacy
/// delta.conf (nothing is written).
#[tauri::command]
fn config() -> ConfigInfo {
    let paths = Paths::from_env();
    let source = paths.config.display().to_string();
    if let Ok(text) = std::fs::read_to_string(&paths.config) {
        return match Config::from_toml(&text) {
            Ok(config) => {
                let problems = config.validate().err().map(|p| p.0).unwrap_or_default();
                ConfigInfo { config: Some(config), source, legacy: false, problems }
            }
            Err(e) => ConfigInfo { config: None, source, legacy: false, problems: vec![e.to_string()] },
        };
    }
    match std::fs::read_to_string(&paths.legacy_conf) {
        Ok(text) => {
            let imported = import::import_conf(&text);
            ConfigInfo {
                config: Some(imported.config),
                source: paths.legacy_conf.display().to_string(),
                legacy: true,
                problems: imported.warnings,
            }
        }
        Err(_) => ConfigInfo {
            config: None,
            source,
            legacy: false,
            problems: vec!["no delta.toml yet; run `delta import`".into()],
        },
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![keyboards, watch_keys, config])
        .run(tauri::generate_context!())
        .expect("error while running Delta");
}
