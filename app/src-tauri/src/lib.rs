//! Delta's GUI backend. Reads keyboards over VIA (read-only), streams which keys are
//! held, listens to mice and knob turns without grabbing anything, and loads the
//! Delta config. The window is Svelte (`app/src`).

use delta_core::paths::Paths;
use delta_core::{Config, import};
use delta_via::keycode::{Keycode, decode};
use delta_via::{Definition, Via, discover};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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
    /// Input devices that already have a listener thread (they run for the app's life).
    listening: Mutex<HashSet<PathBuf>>,
    /// evdev key code a knob turn sends → which keyboard, knob and direction.
    knob_keys: Arc<Mutex<HashMap<u16, KnobTurn>>>,
}

#[derive(Clone, Serialize)]
struct KnobTurn {
    /// The keyboard's VIA (hidraw) path, as the UI knows it.
    path: String,
    encoder: u8,
    direction: &'static str,
}

#[derive(Clone, Serialize)]
struct InputEvent {
    path: String,
    #[serde(flatten)]
    event: delta_input::Event,
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
                remember_knob_keys(&state, &path, &encoders);
                state.keyboards.lock().unwrap().insert(path, Open { via, rows: def.rows, cols: def.cols });
            }
            Err(e) => info.error = Some(e.to_string()),
        }
        info.definition = Some(def);
        out.push(info);
    }
    Ok(out)
}

/// Note which evdev keys each knob turn sends, so the knob listener can light it up.
fn remember_knob_keys(state: &AppState, path: &str, encoders: &[Vec<(u16, u16)>]) {
    let mut keys = state.knob_keys.lock().unwrap();
    keys.retain(|_, turn| turn.path != path);
    for layer in encoders {
        for (i, (ccw, cw)) in layer.iter().enumerate() {
            for (code, direction) in [(ccw, "left"), (cw, "right")] {
                if let Some(evdev) = qmk_to_evdev(*code) {
                    keys.entry(evdev).or_insert(KnobTurn { path: path.into(), encoder: i as u8, direction });
                }
            }
        }
    }
}

/// QMK keycode → Linux key code, for the keys knobs usually send: F13–F24 and volume.
fn qmk_to_evdev(code: u16) -> Option<u16> {
    match code {
        0x68..=0x73 => Some(183 + (code - 0x68)), // KC_F13..KC_F24 → KEY_F13..KEY_F24
        0xA8 => Some(113),                        // KC_AUDIO_MUTE → KEY_MUTE
        0xA9 => Some(115),                        // KC_AUDIO_VOL_UP → KEY_VOLUMEUP
        0xAA => Some(114),                        // KC_AUDIO_VOL_DOWN → KEY_VOLUMEDOWN
        _ => None,
    }
}

/// Start a passive listener on `path` (once per device). Never grabs it.
fn listen_once(
    app: &AppHandle,
    path: PathBuf,
    mut handle: impl FnMut(&AppHandle, delta_input::Event) + Send + 'static,
) {
    if !app.state::<AppState>().listening.lock().unwrap().insert(path.clone()) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = delta_input::listen(&path, |event| {
            handle(&app, event);
            true
        });
        // Device gone: allow a later re-listen
        app.state::<AppState>().listening.lock().unwrap().remove(&path);
    });
}

/// One physical pointing device, which may expose several input nodes.
#[derive(Serialize)]
struct Mouse {
    name: String,
    vendor_id: u16,
    product_id: u16,
    paths: Vec<String>,
}

/// Hardware mice for the device tabs, one per USB ID. Pointer nodes belonging to a
/// VIA keyboard (its mouse keys) are left out: they're part of that keyboard.
/// Some devices expose several nodes for one piece of hardware (the Steam
/// Controller Puck has one per controller slot); they're grouped into one entry.
#[tauri::command]
fn mice() -> Vec<Mouse> {
    let keyboards: HashSet<(u16, u16)> = discover().iter().map(|d| (d.vendor_id, d.product_id)).collect();
    let mut out: Vec<Mouse> = Vec::new();
    for d in delta_input::list() {
        if !d.pointer || d.virtual_device || keyboards.contains(&(d.vendor_id, d.product_id)) {
            continue;
        }
        let path = d.path.display().to_string();
        match out.iter_mut().find(|m| (m.vendor_id, m.product_id) == (d.vendor_id, d.product_id)) {
            Some(m) => m.paths.push(path),
            None => out.push(Mouse {
                name: d.name.strip_suffix(" Mouse").unwrap_or(&d.name).to_string(),
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                paths: vec![path],
            }),
        }
    }
    out
}

/// Stream a mouse's buttons and wheel as `input` events, from each of its nodes.
/// Motion is never read.
#[tauri::command]
fn watch_mouse(app: AppHandle, paths: Vec<String>) {
    for path in paths {
        let label = path.clone();
        listen_once(&app, PathBuf::from(&path), move |app, event| {
            let _ = app.emit("input", InputEvent { path: label.clone(), event });
        });
    }
}

/// Light up knobs when they turn. Turns aren't switches, so VIA can't report them;
/// they arrive as key presses, from xremap's output device (xremap holds the
/// keyboard) or from the keyboard itself when nothing grabs it. Only the knob's own
/// keys are forwarded: typing never leaves this listener.
#[tauri::command]
fn watch_knobs(app: AppHandle, state: State<'_, AppState>) {
    let ids: HashSet<(u16, u16)> = state
        .keyboards
        .lock()
        .unwrap()
        .keys()
        .filter_map(|p| discover().into_iter().find(|d| d.path.display().to_string() == *p))
        .map(|d| (d.vendor_id, d.product_id))
        .collect();
    for dev in delta_input::list() {
        let ours = ids.contains(&(dev.vendor_id, dev.product_id));
        if !(ours || dev.name == "xremap") {
            continue;
        }
        let knob_keys = state.knob_keys.clone();
        listen_once(&app, dev.path, move |app, event| {
            if let delta_input::Event::Button { code, pressed: true, .. } = event
                && let Some(turn) = knob_keys.lock().unwrap().get(&code).cloned()
            {
                let _ = app.emit("knob", turn);
            }
        });
    }
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

/// Validate and save the config edited in the app. deltad (if running) notices the
/// file change and applies it within a second.
#[tauri::command]
fn save_config(config: Config) -> Result<ConfigInfo, Vec<String>> {
    let paths = Paths::from_env();
    match config.save(&paths.config) {
        Ok(()) => Ok(ConfigInfo {
            config: Some(config),
            source: paths.config.display().to_string(),
            legacy: false,
            problems: vec![],
        }),
        Err(delta_core::config::ConfigError::Invalid(p)) => Err(p.0),
        Err(e) => Err(vec![e.to_string()]),
    }
}

/// KDE global shortcuts, for the "KDE shortcut" action picker.
#[tauri::command]
async fn shortcuts() -> Result<Vec<delta_core::system::Shortcut>, String> {
    delta_core::system::list_shortcuts().await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct AppSuggestion {
    /// What xremap matches on (the window class), e.g. `org.kde.konsole`, `firefox`.
    class: String,
    name: String,
}

/// Installed apps, for the "only in this app" picker: window class from each
/// .desktop file's StartupWMClass, or its desktop ID.
#[tauri::command]
fn app_suggestions() -> Vec<AppSuggestion> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        "/usr/share/applications".to_string(),
        "/var/lib/flatpak/exports/share/applications".to_string(),
        format!("{home}/.local/share/applications"),
        format!("{home}/.local/share/flatpak/exports/share/applications"),
    ];
    let mut out: Vec<AppSuggestion> = Vec::new();
    for dir in dirs {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".desktop")) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let field = |key: &str| {
                text.lines()
                    .skip_while(|l| l.trim() != "[Desktop Entry]")
                    .take_while(|l| !l.starts_with('[') || l.trim() == "[Desktop Entry]")
                    .find_map(|l| l.strip_prefix(key)?.strip_prefix('=').map(str::trim))
            };
            if field("NoDisplay") == Some("true") || field("Type") != Some("Application") {
                continue;
            }
            let class = field("StartupWMClass").unwrap_or(id).to_lowercase();
            if !out.iter().any(|a| a.class == class) {
                out.push(AppSuggestion { class, name: field("Name").unwrap_or(id).to_string() });
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// deltad's status, or None when it isn't running (before the cutover).
#[tauri::command]
async fn deltad_status() -> Option<HashMap<String, String>> {
    let conn = zbus::Connection::session().await.ok()?;
    let reply = conn
        .call_method(Some("org.seglectic.Delta"), "/org/seglectic/Delta", Some("org.seglectic.Delta1"), "Status", &())
        .await
        .ok()?;
    reply.body().deserialize().ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            keyboards,
            watch_keys,
            config,
            mice,
            watch_mouse,
            watch_knobs,
            save_config,
            shortcuts,
            app_suggestions,
            deltad_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running Delta");
}
