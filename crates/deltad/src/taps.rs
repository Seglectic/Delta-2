//! Tap dance: count quick presses of a key and run the binding for that many taps.
//!
//! The KWin script calls `Tap("delta:a")` on every press. Each press restarts a wait
//! of `engine.tap_ms`; when it runs out, the binding with that tap count runs. Reaching
//! the highest count the key has runs it straight away.

use delta_core::config::{Action, Config};
use delta_core::system;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct Counter {
    config: Arc<Mutex<Option<Config>>>,
    /// Per key: taps so far, and a number bumped on every tap so a wait can tell
    /// whether it's still the latest.
    pending: Arc<Mutex<HashMap<String, (u8, u64)>>>,
}

impl Counter {
    pub fn new(config: Arc<Mutex<Option<Config>>>) -> Self {
        Self { config, pending: Arc::default() }
    }

    pub fn tap(&self, group: String) {
        let Some((max, wait)) = self.with_group(&group, |config, g| (g.max_taps(), config.engine.tap_ms)) else {
            eprintln!("tap on {group}, which has no tap dance");
            return;
        };
        let (count, seq) = {
            let mut pending = self.pending.lock().unwrap();
            let entry = pending.entry(group.clone()).or_insert((0, 0));
            *entry = (entry.0 + 1, entry.1 + 1);
            *entry
        };
        if count >= max {
            self.pending.lock().unwrap().remove(&group);
            self.fire(group, count);
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(wait.into())).await;
            let done = {
                let mut pending = this.pending.lock().unwrap();
                match pending.get(&group) {
                    Some(&(count, s)) if s == seq => pending.remove(&group).map(|_| count),
                    _ => None,
                }
            };
            if let Some(count) = done {
                this.fire(group, count);
            }
        });
    }

    fn with_group<T>(&self, group: &str, f: impl FnOnce(&Config, &delta_core::config::TapGroup) -> T) -> Option<T> {
        let config = self.config.lock().unwrap();
        let config = config.as_ref()?;
        let groups = config.tap_groups();
        groups.iter().find(|g| g.name == group).map(|g| f(config, g))
    }

    /// Run the binding for `count` taps, if the key has one.
    fn fire(&self, group: String, count: u8) {
        let Some(job) = self.with_group(&group, |_, g| g.member(count).map(|b| (b.tap_shortcut(), b.action.clone())))
        else {
            return;
        };
        let Some((shortcut, action)) = job else { return };
        tokio::spawn(async move {
            let result = match &action {
                a if a.is_window_action() => system::invoke_shortcut("kwin", &shortcut).await,
                Action::KwinShortcut { name, component } => system::invoke_shortcut(component, name).await,
                Action::Command { run } => {
                    // As xremap runs commands: in their own scope, so they outlive deltad
                    let run = run.clone();
                    std::thread::spawn(move || {
                        let _ = std::process::Command::new("systemd-run")
                            .args(["--user", "--scope", "--quiet", "sh", "-c", &run])
                            .status();
                    });
                    Ok(())
                }
                // Rejected by validation
                _ => Ok(()),
            };
            if let Err(e) = result {
                eprintln!("tap dance {group} ×{count}: {e}");
            }
        });
    }
}
