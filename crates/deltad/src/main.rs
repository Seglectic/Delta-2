//! deltad: Delta's background service.
//!
//! - Watches `~/.config/delta/delta.toml` and applies it whenever it changes.
//! - Serves `org.seglectic.Delta` on the session bus for the GUI and scripts.
//! - Loads the KWin script at login (replacing the old autostart entry).

use anyhow::Result;
use delta_core::paths::Paths;
use delta_core::{Config, apps, system};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tokio::sync::oneshot;
use zbus::object_server::SignalEmitter;

mod effects;
mod highlight;
use zbus::{fdo, interface};

const BUS_NAME: &str = "org.seglectic.Delta";
const OBJECT_PATH: &str = "/org/seglectic/Delta";
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const KWIN_WAIT: Duration = Duration::from_secs(60);
/// Name of the one-shot KWin script that lists open windows.
const LIST_WINDOWS: &str = "delta-list-windows";
/// One-shot KWin script that reports the focused window, for when deltad starts after
/// the main script (which reports every focus change).
const REPORT_FOCUS: &str = "delta-report-focus";
const REPORT_FOCUS_SCRIPT: &str = r#"var w = workspace.activeWindow;
callDBus("org.seglectic.Delta", "/org/seglectic/Delta", "org.seglectic.Delta1", "FocusChanged",
         w ? "" + (w.resourceClass || "") : "", w ? "" + (w.desktopFileName || "") : "");
"#;

struct Delta {
    paths: Paths,
    /// The `delta` CLI next to this binary; `delta-launch@.service` runs it.
    launcher: PathBuf,
    /// Last config that loaded and validated.
    config: Option<Config>,
    /// Modification time of the config file when last read (`Some(None)`: it was missing).
    /// `None` until the first read.
    seen: Option<Option<SystemTime>>,
    last_error: Option<String>,
    /// Waiting `ListRunningApps` call, answered by the KWin script's `ReportWindows`.
    window_reply: Mutex<Option<oneshot::Sender<String>>>,
    /// The current config, for the key-highlight thread.
    shared: highlight::Shared,
}

impl Delta {
    fn new(paths: Paths, shared: highlight::Shared) -> Self {
        let launcher = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.canonicalize().ok())
            .and_then(|exe| exe.parent().map(|dir| dir.join("delta")))
            .unwrap_or_else(|| "delta".into());
        Self { paths, launcher, config: None, seen: None, last_error: None, window_reply: Mutex::new(None), shared }
    }

    fn file_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(&self.paths.config).and_then(|m| m.modified()).ok()
    }

    /// Re-read and apply the config if the file changed since last time.
    /// Returns true when it was re-read (whether or not that succeeded).
    async fn refresh(&mut self) -> bool {
        let mtime = self.file_mtime();
        if self.seen == Some(mtime) {
            return false;
        }
        self.seen = Some(mtime);
        if mtime.is_none() {
            self.fail(format!("{} doesn't exist; run `delta import` to create it", self.paths.config.display()));
            return true;
        }
        match Config::load(&self.paths.config) {
            Ok(config) => {
                self.shared.set(config.clone());
                self.config = Some(config);
                self.apply_current().await;
            }
            // Keep running with the last good config
            Err(e) => self.fail(format!("config not applied: {e}")),
        }
        true
    }

    async fn apply_current(&mut self) -> Vec<String> {
        let Some(config) = &self.config else { return vec![] };
        match system::apply(config, &self.paths, &self.launcher).await {
            Ok(report) => {
                self.last_error = None;
                let written: Vec<String> = report.written.iter().map(|p| p.display().to_string()).collect();
                eprintln!("applied config ({} file(s) changed)", written.len());
                written
            }
            Err(e) => {
                self.fail(format!("apply failed: {e}"));
                vec![]
            }
        }
    }

    fn fail(&mut self, message: String) {
        eprintln!("{message}");
        self.last_error = Some(message);
    }
}

#[interface(name = "org.seglectic.Delta1")]
impl Delta {
    /// The config file's text, comments included.
    fn get_config(&self) -> fdo::Result<String> {
        std::fs::read_to_string(&self.paths.config).map_err(|e| fdo::Error::FileNotFound(e.to_string()))
    }

    /// Problems with a config, without saving it. Empty means it's valid.
    fn validate(&self, toml: String) -> Vec<String> {
        match Config::from_toml(&toml) {
            Ok(config) => config.validate().err().map(|p| p.0).unwrap_or_default(),
            Err(e) => vec![e.to_string()],
        }
    }

    /// Validate, save and apply a new config.
    async fn set_config(
        &mut self,
        toml: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let config = Config::from_toml(&toml).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        config.validate().map_err(|p| fdo::Error::InvalidArgs(p.to_string()))?;

        // Write via a temp file so a half-written config is never picked up
        let tmp = self.paths.config.with_extension("toml.tmp");
        std::fs::write(&tmp, &toml)
            .and_then(|()| std::fs::rename(&tmp, &self.paths.config))
            .map_err(|e| fdo::Error::IOError(e.to_string()))?;

        self.seen = Some(self.file_mtime());
        self.shared.set(config.clone());
        self.config = Some(config);
        self.apply_current().await;
        Self::config_changed(&emitter).await?;
        match &self.last_error {
            Some(e) => Err(fdo::Error::Failed(e.clone())),
            None => Ok(()),
        }
    }

    /// Regenerate and reload everything from the current config. Returns the files written.
    async fn apply(&mut self) -> fdo::Result<Vec<String>> {
        let written = self.apply_current().await;
        match &self.last_error {
            Some(e) => Err(fdo::Error::Failed(e.clone())),
            None => Ok(written),
        }
    }

    /// Run a binding's action by ID, as if its hotkey were pressed.
    async fn trigger(&self, id: String) -> fdo::Result<()> {
        system::invoke_shortcut("kwin", &id).await.map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// Apps with open windows, as JSON (`delta_core::apps::AppChoice` list): name,
    /// window class, launch command and match strings, ready for `[[apps]]`.
    async fn list_running_apps(&self) -> fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        *self.window_reply.lock().unwrap() = Some(tx);

        let dir =
            std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("delta");
        let script = dir.join("list-windows.js");
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&script, apps::LIST_WINDOWS_SCRIPT))
            .map_err(|e| fdo::Error::IOError(e.to_string()))?;
        let failed = |e: String| fdo::Error::Failed(format!("listing windows: {e}"));
        system::run_kwin_script(&script, LIST_WINDOWS).await.map_err(|e| failed(e.to_string()))?;

        let reply = tokio::time::timeout(Duration::from_secs(2), rx).await;
        let _ = system::unload_kwin_script(LIST_WINDOWS).await;
        let json = reply.map_err(|_| failed("KWin didn't answer".into()))?.map_err(|e| failed(e.to_string()))?;
        let windows: Vec<apps::Window> = serde_json::from_str(&json).map_err(|e| failed(e.to_string()))?;
        serde_json::to_string(&apps::running_apps(&windows)).map_err(|e| failed(e.to_string()))
    }

    /// Called by the one-shot KWin script with the open windows as JSON.
    fn report_windows(&self, json: String) {
        if let Some(tx) = self.window_reply.lock().unwrap().take() {
            let _ = tx.send(json);
        }
    }

    /// Called by the KWin script whenever another window gets focus.
    fn focus_changed(&self, class: String, desktop: String) {
        *self.shared.focus.lock().unwrap() = Some((class, desktop));
    }

    /// Show a lighting profile (JSON, as in `[[lighting.profiles]]`) on the keyboard
    /// instead of the focused app's, for a few seconds; send it again to keep it up.
    /// An empty string ends the preview.
    fn preview_lighting(&self, json: String) -> fdo::Result<()> {
        let mut preview = self.shared.preview.lock().unwrap();
        if json.is_empty() {
            *preview = None;
            return Ok(());
        }
        let profile = serde_json::from_str(&json).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        *preview = Some((profile, std::time::Instant::now()));
        Ok(())
    }

    /// Play a lighting effect (JSON, as in `[[lighting.effects]]`) on a loop for a few
    /// seconds; send it again to keep it going. An empty string stops it.
    fn preview_effect(&self, json: String) -> fdo::Result<()> {
        let mut preview = self.shared.preview_effect.lock().unwrap();
        if json.is_empty() {
            *preview = None;
            return Ok(());
        }
        let effect = serde_json::from_str(&json).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        *preview = Some((effect, std::time::Instant::now()));
        Ok(())
    }

    /// Component states, for status displays.
    async fn status(&self) -> HashMap<String, String> {
        let kwin = match system::kwin_script_loaded().await {
            Ok(true) => "loaded".into(),
            Ok(false) => "not loaded".into(),
            Err(e) => format!("unknown ({e})"),
        };
        HashMap::from([
            ("config_path".into(), self.paths.config.display().to_string()),
            ("bindings".into(), self.config.as_ref().map_or(0, |c| c.bindings.len()).to_string()),
            ("xremap".into(), system::xremap_state()),
            ("kwin_script".into(), kwin),
            ("last_error".into(), self.last_error.clone().unwrap_or_default()),
            (
                "focus".into(),
                self.shared.focus.lock().unwrap().as_ref().map(|(c, d)| format!("{c} ({d})")).unwrap_or_default(),
            ),
        ])
    }

    /// The config was changed (by the GUI or by editing the file).
    #[zbus(signal)]
    async fn config_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Have KWin call `FocusChanged` once with the window focused now.
async fn report_focus() -> Result<()> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("delta");
    let script = dir.join("report-focus.js");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&script, REPORT_FOCUS_SCRIPT)?;
    system::run_kwin_script(&script, REPORT_FOCUS).await?;
    // The script answers right away; the bus call above is handled while we wait
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = system::unload_kwin_script(REPORT_FOCUS).await;
    Ok(())
}

/// The KWin script can only be loaded once KWin is on the bus.
async fn wait_for_kwin(conn: &zbus::Connection) {
    let Ok(dbus) = fdo::DBusProxy::new(conn).await else { return };
    let name = zbus::names::BusName::try_from("org.kde.KWin").expect("valid bus name");
    let start = std::time::Instant::now();
    while start.elapsed() < KWIN_WAIT {
        if dbus.name_has_owner(name.clone()).await.unwrap_or(false) {
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    eprintln!("KWin didn't appear on the bus within {KWIN_WAIT:?}; continuing anyway");
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let shared = highlight::Shared::default();
    highlight::spawn(shared.clone());
    let conn = zbus::connection::Builder::session()
        .name(BUS_NAME)
        .serve_at(OBJECT_PATH, Delta::new(Paths::from_env(), shared.clone()))
        .build()
        .await?;
    let iface = conn.object_server().interface::<_, Delta>(OBJECT_PATH).await?;
    eprintln!("deltad serving {BUS_NAME}");

    wait_for_kwin(&conn).await;
    if let Err(e) = report_focus().await {
        eprintln!("couldn't ask KWin for the focused window: {e}");
    }

    // Poll the config's mtime. Simple, and survives editors that replace the file.
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let changed = iface.get_mut().await.refresh().await;
                if changed {
                    DeltaSignals::config_changed(&iface).await?;
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}
