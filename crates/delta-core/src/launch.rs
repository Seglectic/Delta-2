//! `delta launch <app>`: start an app's launch command in its own systemd scope,
//! so it doesn't live inside the oneshot launcher unit (or KWin).

use crate::config::Config;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Replaces the current process; only returns on error.
pub fn exec_app(config: &Config, app: &str) -> std::io::Error {
    let Some(entry) = config.app(app) else {
        return std::io::Error::other(format!("unknown delta app: {app}"));
    };
    let argv = match shell_words::split(&entry.launch) {
        Ok(argv) if !argv.is_empty() => argv,
        _ => return std::io::Error::other(format!("bad launch command for {app}: {}", entry.launch)),
    };
    Command::new("systemd-run").args(["--user", "--scope", "--quiet", "--"]).args(argv).exec()
}
