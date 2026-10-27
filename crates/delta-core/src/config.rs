//! The `delta.toml` config model.
//!
//! Everything Delta does is a [`Binding`]: a key, pressed on a layer, optionally only
//! while a given app is focused, runs an [`Action`]. The generators turn this into
//! xremap config, a KWin script and systemd units.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub engine: Engine,
    #[serde(default)]
    pub xremap: Xremap,
    #[serde(default)]
    pub guards: Guards,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<App>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<Binding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// Which input engine turns key presses into actions.
    #[serde(default)]
    pub backend: Backend,
    /// The evdev key held for the delta layer: `capslock`, or `f20` on QMK boards
    /// whose firmware maps Caps Lock to F20.
    #[serde(default = "default_modifier_key")]
    pub modifier_key: String,
}

impl Default for Engine {
    fn default() -> Self {
        Self { backend: Backend::default(), modifier_key: default_modifier_key() }
    }
}

fn default_modifier_key() -> String {
    "capslock".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Xremap,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Xremap {
    /// Device names xremap must not grab (e.g. mice whose extra buttons send keys).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guards {
    /// Skip window actions on fullscreen windows (games).
    #[serde(default = "yes")]
    pub ignore_fullscreen: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_desktop_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_classes: Vec<String>,
}

impl Default for Guards {
    fn default() -> Self {
        Self { ignore_fullscreen: true, ignored_desktop_files: vec![], ignored_classes: vec![] }
    }
}

fn yes() -> bool {
    true
}

/// An app that `app` actions can focus, or launch when it isn't open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    /// Command line used when no window is open. Split like a shell would.
    pub launch: String,
}

// No deny_unknown_fields here: serde doesn't support it together with `flatten`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    /// Stable ID. Window actions register a KDE shortcut under this ID, and KDE stores
    /// the key assignment by ID, so it must never change once in use.
    pub id: String,
    /// Key name as xremap/KDE know it: `A`, `Left`, `F19`, `grave`.
    pub key: String,
    #[serde(default, skip_serializing_if = "Layer::is_default")]
    pub layer: Layer,
    /// Only active while this app class is focused (xremap's `only:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only_in: Option<String>,
    /// Name shown in KDE's shortcut settings. Defaults to `Delta: <id>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub action: Action,
}

impl Binding {
    pub fn label(&self) -> String {
        self.label.clone().unwrap_or_else(|| format!("Delta: {}", self.id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// While the delta modifier (Caps Lock) is held.
    #[default]
    Delta,
    /// The key on its own.
    Base,
}

impl Layer {
    fn is_default(&self) -> bool {
        *self == Layer::Delta
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Put the active window into one column of a two-column KWin tile layout.
    Tile { layout: TileLayout, side: Side },
    /// Resize the active window to a horizontal slice of the screen (fractions 0–1).
    /// `name` is the key [`Override`]s match on; defaults to the binding's key.
    Span {
        x: f64,
        width: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Center the active window at a fixed pixel size.
    Fixed { width: u32, height: u32 },
    /// Focus the app's window (minimize it if already focused), or launch it.
    App { app: String },
    /// Send a key chord instead, e.g. `Alt+Up`, `Ctrl+Shift+T`, `PlayPause`.
    Keys { send: String },
}

impl Action {
    /// Window actions run inside the KWin script; the rest are pure key remaps.
    pub fn is_window_action(&self) -> bool {
        !matches!(self, Action::Keys { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileLayout {
    /// 1/2 | 1/2
    Halves,
    /// 1/3 | 2/3
    ThirdsLeft,
    /// 2/3 | 1/3
    ThirdsRight,
}

impl TileLayout {
    pub fn as_str(&self) -> &'static str {
        match self {
            TileLayout::Halves => "halves",
            TileLayout::ThirdsLeft => "thirds_left",
            TileLayout::ThirdsRight => "thirds_right",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn as_str(&self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// Per-app replacement for a `span` action's fractions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Window class (KWin `resourceClass`), e.g. `org.kde.konsole`.
    pub app: String,
    /// The span's name (its binding key unless set).
    pub key: String,
    pub x: f64,
    pub width: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {path}: {source}")]
    Read { path: String, source: std::io::Error },
    #[error("parsing {path}: {source}")]
    Parse { path: String, source: toml::de::Error },
    #[error("invalid config:\n{0}")]
    Invalid(Problems),
}

/// Every validation problem found, so the GUI/CLI can show them all at once.
#[derive(Debug)]
pub struct Problems(pub Vec<String>);

impl fmt::Display for Problems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.0 {
            writeln!(f, "  - {p}")?;
        }
        Ok(())
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|source| ConfigError::Read { path: path.display().to_string(), source })?;
        let config =
            Self::from_toml(&text).map_err(|source| ConfigError::Parse { path: path.display().to_string(), source })?;
        config.validate().map_err(ConfigError::Invalid)?;
        Ok(config)
    }

    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config always serializes")
    }

    pub fn app(&self, name: &str) -> Option<&App> {
        self.apps.iter().find(|a| a.name == name)
    }

    /// Checks everything the generators rely on. Names end up inside generated
    /// JavaScript, YAML and unit files, so they're limited to safe characters
    /// rather than escaped.
    pub fn validate(&self) -> Result<(), Problems> {
        let mut problems = Vec::new();
        let mut ids = HashSet::new();

        if !is_key_name(&self.engine.modifier_key) {
            problems.push(format!("engine.modifier_key {:?} is not a key name", self.engine.modifier_key));
        }

        let mut app_names = HashSet::new();
        for app in &self.apps {
            if !is_app_name(&app.name) {
                problems.push(format!("app name {:?} may only use letters, digits, '.', '_', '-'", app.name));
            }
            if !app_names.insert(app.name.as_str()) {
                problems.push(format!("app {:?} is defined twice", app.name));
            }
            if shell_words::split(&app.launch).map_or(true, |argv| argv.is_empty()) {
                problems.push(format!("app {:?}: launch command {:?} is empty or badly quoted", app.name, app.launch));
            }
        }

        for b in &self.bindings {
            let at = format!("binding {:?}", b.id);
            if !is_ident(&b.id) {
                problems.push(format!("{at}: id may only use letters, digits and '_'"));
            }
            if !ids.insert(b.id.as_str()) {
                problems.push(format!("{at}: id is used twice"));
            }
            if !is_key_name(&b.key) {
                problems.push(format!("{at}: {:?} is not a key name", b.key));
            }
            if let Some(app) = &b.only_in {
                if !is_app_name(app) {
                    problems.push(format!("{at}: only_in {app:?} may only use letters, digits, '.', '_', '-'"));
                }
                if b.action.is_window_action() {
                    problems.push(format!("{at}: per-app bindings can only send keys for now"));
                }
            }
            if let Some(label) = &b.label
                && label.contains(['"', '\\', '\n'])
            {
                problems.push(format!("{at}: label can't contain quotes, backslashes or newlines"));
            }
            match &b.action {
                Action::App { app } if self.app(app).is_none() => {
                    problems.push(format!("{at}: app {app:?} has no [[apps]] entry"));
                }
                Action::Keys { send } if !is_chord(send) => {
                    problems.push(format!("{at}: {send:?} is not a key chord like Alt+Up"));
                }
                Action::Span { x, width, name } => {
                    if !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(width) {
                        problems.push(format!("{at}: x and width are screen fractions between 0 and 1"));
                    }
                    if name.as_deref().is_some_and(|n| !is_key_name(n)) {
                        problems.push(format!("{at}: name {name:?} is not a key name"));
                    }
                }
                _ => {}
            }
        }

        for o in &self.overrides {
            if !is_app_name(&o.app) || !is_key_name(&o.key) {
                problems.push(format!("override {}.{}: app/key use unsupported characters", o.app, o.key));
            }
        }

        for list in [&self.guards.ignored_desktop_files, &self.guards.ignored_classes] {
            for name in list {
                if !is_app_name(name) {
                    problems.push(format!("guard entry {name:?} may only use letters, digits, '.', '_', '-'"));
                }
            }
        }

        for dev in &self.xremap.ignore_devices {
            if dev.contains('\n') {
                problems.push(format!("xremap.ignore_devices entry {dev:?} contains a newline"));
            }
        }

        if problems.is_empty() { Ok(()) } else { Err(Problems(problems)) }
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_key_name(s: &str) -> bool {
    is_ident(s)
}

fn is_app_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn is_chord(s: &str) -> bool {
    !s.is_empty() && s.split('+').all(is_key_name)
}
