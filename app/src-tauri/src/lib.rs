//! Delta's GUI backend. Reads keyboards over VIA (read-only), streams which keys are
//! held, listens to mice and knob turns without grabbing anything, and loads the
//! Delta config. The window is Svelte (`app/src`).

use delta_core::install::{self, Owner, Permissions};
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
/// Check the board's base layer every this many key polls (~300 ms).
const LAYER_EVERY: u32 = 20;

/// An open keyboard plus its matrix size, shared by commands and the key watcher so
/// only one request is in flight on the device at a time.
struct Open {
    via: Via,
    rows: u8,
    cols: u8,
    /// Keychron per-key RGB: LED count and matrix → LED map, read on first use
    /// (`Some(None)`: the firmware doesn't have it).
    leds: Option<Option<Leds>>,
}

/// LED count and matrix `[row][col]` → LED index.
type Leds = (u8, Vec<Vec<Option<u8>>>);

#[derive(Default)]
struct AppState {
    /// What installing the bundled deltad did at start (released builds only).
    install: Mutex<Option<Result<install::Sync, String>>>,
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
    /// The layer the board is on (Keychron's Mac/Windows switch); None when its
    /// firmware doesn't say.
    base_layer: Option<u8>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct BaseLayer {
    path: String,
    layer: u8,
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
            base_layer: None,
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
            let base_layer = via.keychron_layers()?.map(|l| l.0);
            Ok((via, protocol, keymap, encoders, base_layer))
        };
        match read() {
            Ok((via, protocol, keymap, encoders, base_layer)) => {
                info.base_layer = base_layer;
                let name = |c: u16| decode(c, &def.custom_keycodes);
                info.protocol = Some(protocol);
                info.layers =
                    keymap.iter().map(|l| l.iter().map(|r| r.iter().map(|c| name(*c)).collect()).collect()).collect();
                info.encoders =
                    encoders.iter().map(|l| l.iter().map(|(ccw, cw)| (name(*ccw), name(*cw))).collect()).collect();
                remember_knob_keys(&state, &path, &encoders);
                state.keyboards.lock().unwrap().insert(path, Open { via, rows: def.rows, cols: def.cols, leds: None });
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
async fn mice() -> Vec<Mouse> {
    list_mice()
}

fn list_mice() -> Vec<Mouse> {
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
        // The Mac/Windows switch, checked every LAYER_EVERY polls
        let mut last_layer: Option<u8> = None;
        let mut tick = 0u32;
        while running.load(Ordering::Relaxed) {
            let (keys, layer) = {
                let mut boards = state.keyboards.lock().unwrap();
                let Some(open) = boards.get_mut(&path) else { break };
                let (rows, cols) = (open.rows, open.cols);
                let layer = tick.is_multiple_of(LAYER_EVERY).then(|| open.via.keychron_layers().ok().flatten()).flatten();
                (open.via.pressed(rows, cols), layer)
            };
            tick = tick.wrapping_add(1);
            if let Some((base, _)) = layer
                && last_layer.replace(base) != Some(base)
            {
                let _ = app.emit("base-layer", BaseLayer { path: path.clone(), layer: base });
            }
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
async fn config() -> ConfigInfo {
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
        // First run: start empty, the first save creates delta.toml
        Err(_) => ConfigInfo { config: Some(Config::default()), source, legacy: false, problems: vec![] },
    }
}

/// Validate and save the config edited in the app. deltad (if running) notices the
/// file change and applies it within a second.
#[tauri::command]
async fn save_config(config: Config) -> Result<ConfigInfo, Vec<String>> {
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

/// What the keyboard's LEDs show right now, read back from it.
#[derive(Serialize)]
struct KeyboardLighting {
    effect: u8,
    brightness: u8,
    /// Every key's colour when one colour covers the board (solid, breathing, off).
    all: Option<String>,
    /// `"row,col"` → `#RRGGBB` in Per Key RGB, per-key brightness applied (the
    /// keyboard's own brightness isn't).
    keys: HashMap<String, String>,
    /// An animated effect Delta can't draw.
    animated: bool,
}

/// Keychron's Per Key RGB effect number in this firmware (see deltad's highlighter).
const PER_KEY_EFFECT: u8 = 23;

/// What the keyboard's LEDs show now. Like every command that touches hardware or
/// files, it's `async` so it runs off the window's thread (plain `fn` commands run on
/// it and stall the UI while they wait).
#[tauri::command]
async fn keyboard_lighting(state: State<'_, AppState>, path: String) -> Result<KeyboardLighting, String> {
    use delta_core::lighting::to_hex;
    use delta_via::Hsv;
    let mut boards = state.keyboards.lock().unwrap();
    let open = boards.get_mut(&path).ok_or("keyboard not open")?;
    let err = |e: delta_via::ViaError| e.to_string();
    if open.leds.is_none() {
        let leds = match open.via.keychron_rgb().map_err(err)? {
            Some(n) => Some((n, open.via.led_map(open.rows, open.cols).map_err(err)?)),
            None => None,
        };
        open.leds = Some(leds);
    }
    let l = open.via.lighting().map_err(err)?;
    let mut out =
        KeyboardLighting { effect: l.effect, brightness: l.brightness, all: None, keys: HashMap::new(), animated: false };
    match l.effect {
        0 => out.all = Some("#000000".into()),
        1 | 2 => out.all = Some(to_hex(Hsv { h: l.hue, s: l.sat, v: 255 }.to_rgb())),
        PER_KEY_EFFECT => {
            if let Some(Some((count, map))) = &open.leds {
                let colors = open.via.per_key_colors(*count).map_err(err)?;
                for (r, row) in map.iter().enumerate() {
                    for (c, led) in row.iter().enumerate() {
                        if let Some(hsv) = led.and_then(|i| colors.get(i as usize)) {
                            out.keys.insert(format!("{r},{c}"), to_hex(hsv.to_rgb()));
                        }
                    }
                }
            }
        }
        _ => out.animated = true,
    }
    Ok(out)
}

/// Ask deltad to show a lighting profile being edited (JSON), or stop with "".
#[tauri::command]
async fn preview_lighting(profile: String) -> Result<(), String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    conn.call_method(
        Some("org.seglectic.Delta"),
        "/org/seglectic/Delta",
        Some("org.seglectic.Delta1"),
        "PreviewLighting",
        &(profile,),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ask deltad to loop a key effect being edited (JSON), or stop with "".
#[tauri::command]
async fn preview_effect(effect: String) -> Result<(), String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    conn.call_method(
        Some("org.seglectic.Delta"),
        "/org/seglectic/Delta",
        Some("org.seglectic.Delta1"),
        "PreviewEffect",
        &(effect,),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Colours the keys get while the delta key is held, for the Caps-layer preview.
#[derive(Serialize)]
struct DeltaColors {
    /// Binding ID → `#RRGGBB`
    bindings: HashMap<String, String>,
    background: String,
    delta_key: String,
}

#[tauri::command]
async fn delta_colors(config: Config) -> DeltaColors {
    use delta_core::lighting::{self, to_hex};
    DeltaColors {
        bindings: lighting::lit_bindings(&config)
            .map(|b| (b.id.clone(), to_hex(lighting::binding_color(&config, b))))
            .collect(),
        background: to_hex(lighting::background(&config)),
        delta_key: to_hex(lighting::DELTA_KEY),
    }
}

/// Cheap fingerprint of the plugged-in keyboards and mice, polled so the window can
/// re-read when something is plugged in or out.
#[tauri::command]
async fn devices_signature() -> String {
    let mut ids: Vec<String> = discover().iter().map(|d| d.path.display().to_string()).collect();
    ids.extend(list_mice().iter().map(|m| format!("{:04x}:{:04x}", m.vendor_id, m.product_id)));
    ids.join(",")
}

/// KDE global shortcuts, for the "KDE shortcut" action picker.
#[tauri::command]
async fn shortcuts() -> Result<Vec<delta_core::system::Shortcut>, String> {
    delta_core::system::list_shortcuts().await.map_err(|e| e.to_string())
}

/// Installed apps (from .desktop files), for the "only in this app" picker.
#[tauri::command]
async fn app_suggestions() -> Vec<delta_core::apps::AppChoice> {
    delta_core::apps::installed_apps()
}

/// Apps with open windows, with launch commands, from deltad (which asks KWin).
/// Returned as deltad's JSON text.
#[tauri::command]
async fn running_apps() -> Result<String, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let reply = conn
        .call_method(
            Some("org.seglectic.Delta"),
            "/org/seglectic/Delta",
            Some("org.seglectic.Delta1"),
            "ListRunningApps",
            &(),
        )
        .await
        .map_err(|e| format!("couldn't ask deltad (is it running?): {e}"))?;
    reply.body().deserialize::<String>().map_err(|e| e.to_string())
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

/// Installation and permission state, for the setup banner and status line.
#[derive(Serialize)]
struct SetupState {
    app_version: &'static str,
    /// A `tauri dev` build: never installs anything.
    dev: bool,
    owner: Owner,
    install: Option<install::Sync>,
    install_error: Option<String>,
    permissions: Permissions,
    needs_setup: bool,
    needs_relogin: bool,
}

#[tauri::command]
async fn setup_state(state: State<'_, AppState>) -> Result<SetupState, String> {
    let (installed, install_error) = match state.install.lock().unwrap().clone() {
        Some(Ok(s)) => (Some(s), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    let permissions = Permissions::check();
    Ok(SetupState {
        app_version: env!("CARGO_PKG_VERSION"),
        dev: cfg!(debug_assertions),
        owner: install::owner(&Paths::from_env()),
        install: installed,
        install_error,
        needs_setup: permissions.needs_setup(),
        needs_relogin: permissions.needs_relogin(),
        permissions,
    })
}

/// The one root step (polkit asks for a password): udev rules, uinput, `input` group.
#[tauri::command]
async fn run_setup() -> Result<(), String> {
    let out = install::setup_command().output().map_err(|e| format!("couldn't run pkexec: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(()),
        // pkexec: dialog dismissed / not authorized
        Some(126 | 127) => Err("cancelled".into()),
        _ => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
    }
}

/// Released builds: install the bundled deltad/delta/xremap and the menu entry.
fn install_bundled(app: &AppHandle) {
    let paths = Paths::from_env();
    let result = match install::bundled_dir() {
        Some(dir) => install::sync(&paths, &dir).map_err(|e| e.to_string()),
        None => Err("can't find the app's own directory".into()),
    };
    match &result {
        Ok(s) => eprintln!("install: {s:?}"),
        Err(e) => eprintln!("install failed: {e}"),
    }
    // $APPIMAGE: the AppImage file itself (the binary runs from a temporary mount)
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let icon = include_bytes!("../icons/128x128.png");
        if let Err(e) = install::write_menu_entry(&paths, std::path::Path::new(&appimage), icon) {
            eprintln!("menu entry: {e}");
        }
    }
    *app.state::<AppState>().install.lock().unwrap() = Some(result);
    let _ = app.emit("setup-changed", ());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            if !cfg!(debug_assertions) {
                let handle = app.handle().clone();
                std::thread::spawn(move || install_bundled(&handle));
            }
            Ok(())
        })
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
            running_apps,
            deltad_status,
            delta_colors,
            devices_signature,
            keyboard_lighting,
            preview_lighting,
            preview_effect,
            setup_state,
            run_setup
        ])
        .run(tauri::generate_context!())
        .expect("error while running Delta");
}
