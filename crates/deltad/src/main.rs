//! deltad: Delta's background service.
//!
//! - Watches `~/.config/delta/delta.toml` and applies it whenever it changes.
//! - Serves `org.seglectic.Delta` on the session bus for the GUI and scripts.
//! - Loads the KWin script at login (replacing the old autostart entry).

use anyhow::Result;
use delta_core::paths::Paths;
use delta_core::{Config, system};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use zbus::object_server::SignalEmitter;
use zbus::{fdo, interface};

const BUS_NAME: &str = "org.seglectic.Delta";
const OBJECT_PATH: &str = "/org/seglectic/Delta";
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const KWIN_WAIT: Duration = Duration::from_secs(60);

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
}

impl Delta {
    fn new(paths: Paths) -> Self {
        let launcher = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.canonicalize().ok())
            .and_then(|exe| exe.parent().map(|dir| dir.join("delta")))
            .unwrap_or_else(|| "delta".into());
        Self { paths, launcher, config: None, seen: None, last_error: None }
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
        system::invoke_shortcut(&id).await.map_err(|e| fdo::Error::Failed(e.to_string()))
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
        ])
    }

    /// The config was changed (by the GUI or by editing the file).
    #[zbus(signal)]
    async fn config_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
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
    let conn = zbus::connection::Builder::session()
        .name(BUS_NAME)
        .serve_at(OBJECT_PATH, Delta::new(Paths::from_env()))
        .build()
        .await?;
    let iface = conn.object_server().interface::<_, Delta>(OBJECT_PATH).await?;
    eprintln!("deltad serving {BUS_NAME}");

    wait_for_kwin(&conn).await;

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
