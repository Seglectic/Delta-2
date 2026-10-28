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
    #[serde(default)]
    pub lighting: Lighting,
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
    /// Tap dance: how long to wait for another tap before acting, in milliseconds.
    #[serde(default = "default_tap_ms", skip_serializing_if = "is_default_tap_ms")]
    pub tap_ms: u32,
}

fn default_tap_ms() -> u32 {
    250
}

fn is_default_tap_ms(ms: &u32) -> bool {
    *ms == default_tap_ms()
}

impl Default for Engine {
    fn default() -> Self {
        Self { backend: Backend::default(), modifier_key: default_modifier_key(), tap_ms: default_tap_ms() }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    /// While the delta key is held, light the keys that have a Caps-layer action, in
    /// their colours (keyboards with Keychron per-key RGB firmware).
    #[serde(default = "yes")]
    pub caps_highlight: bool,
    /// The lighting Delta sets when the keyboard connects and returns to after
    /// highlighting. Unset: leave whatever the keyboard has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<NormalLighting>,
    /// `#RRGGBB` for keys with no Caps-layer action while highlighting. Unset: off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_background: Option<String>,
    /// Per-key colours: one profile for everywhere (no `app`) and one per app, used
    /// while that app has focus.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<LightingProfile>,
    /// Animations played when a key is pressed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<KeyEffect>,
}

impl Default for Lighting {
    fn default() -> Self {
        Self { caps_highlight: true, normal: None, delta_background: None, profiles: vec![], effects: vec![] }
    }
}

/// A burst of light that spreads out from a key when another (or the same) key is
/// pressed: a ring travelling outwards, fading as it goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyEffect {
    pub id: String,
    /// An `[[apps]]` name: only while that app has focus. Unset: everywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// The key that starts it, as QMK names what it sends on the base layer (`KC_ENTER`).
    pub trigger: String,
    /// Where the burst comes from: a key name, or `"center"` for the middle of the
    /// board. Unset: the trigger key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// `#RRGGBB` at the origin.
    pub color: String,
    /// `#RRGGBB` at the edge of the burst. Unset: `color` all the way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_end: Option<String>,
    /// Peak brightness, 0–255.
    #[serde(default = "full")]
    pub strength: u8,
    /// How far the ring travels, in key widths.
    #[serde(default = "default_distance")]
    pub distance: f32,
    /// Ring thickness, in key widths.
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default = "default_duration")]
    pub duration_ms: u32,
    /// How quickly it dims while travelling: 0 none, 1 linear, higher sooner.
    #[serde(default = "default_fade")]
    pub fade: f32,
}

fn default_distance() -> f32 {
    8.0
}
fn default_width() -> f32 {
    1.5
}
fn default_duration() -> u32 {
    600
}
fn default_fade() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightingProfile {
    /// An `[[apps]]` name. Unset: everywhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Keys not listed below. Unset: off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<KeyLight>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<KeyLight>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyLight {
    /// What the key sends on the base layer, as QMK names it (`KC_W`, `KC_LSFT`).
    /// Unused for a profile's background.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    /// `#RRGGBB`
    pub color: String,
    /// 0–255, scaled by the keyboard's own brightness (needs Delta's firmware hook).
    #[serde(default = "full")]
    pub brightness: u8,
}

fn full() -> u8 {
    255
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalLighting {
    /// RGB matrix effect number: 0 off, 1 solid colour, 2 breathing.
    pub effect: u8,
    /// `#RRGGBB`
    pub color: String,
    pub brightness: u8,
}

/// An app that `app` actions can focus, or launch when it isn't open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    /// Command line used when no window is open. Split like a shell would.
    pub launch: String,
    /// A window is this app's when its desktop file or window class contains one of
    /// these (case-insensitive). Defaults to the app's name.
    #[serde(default, rename = "match", skip_serializing_if = "Vec::is_empty")]
    pub match_: Vec<String>,
    /// Tile the window here whenever it's focused or launched through Delta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_tile: Option<TileTarget>,
    /// Key colour for this app's bindings, e.g. `#5865F2`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl App {
    /// Lower-cased match strings, falling back to the app name.
    pub fn match_strings(&self) -> Vec<String> {
        let list = if self.match_.is_empty() { std::slice::from_ref(&self.name) } else { &self.match_[..] };
        list.iter().map(|m| m.to_lowercase()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileTarget {
    pub layout: TileLayout,
    pub side: Side,
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
    /// Key colour while the delta key is held, e.g. `#89B4FA`. Defaults by action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Tap dance: fire on this many quick presses (1–3). Bindings on the same key with
    /// different `taps` share it; a single tap then waits `engine.tap_ms` for more.
    #[serde(default = "one_tap", skip_serializing_if = "is_one_tap")]
    pub taps: u8,
    #[serde(flatten)]
    pub action: Action,
}

fn one_tap() -> u8 {
    1
}

fn is_one_tap(taps: &u8) -> bool {
    *taps == 1
}

/// Most taps a binding can ask for.
pub const MAX_TAPS: u8 = 3;

impl Binding {
    pub fn label(&self) -> String {
        self.label.clone().unwrap_or_else(|| format!("Delta: {}", self.id))
    }

    /// Name of the KDE shortcut (no key of its own) that runs this binding's window
    /// action when it's part of a tap dance; deltad fires it after counting taps.
    pub fn tap_shortcut(&self) -> String {
        format!("{}.tap", self.id)
    }
}

/// Bindings sharing one key (and layer) by tap count. The key's KDE shortcut only
/// reports the tap to deltad, which counts and runs the right binding.
#[derive(Debug)]
pub struct TapGroup<'a> {
    /// `delta:a` / `base:f19`: what the KWin script reports to deltad.
    pub name: String,
    /// ID the key's shortcut is registered under: the single-tap binding's, so KDE's
    /// stored key for it keeps working.
    pub shortcut_id: String,
    pub label: String,
    pub layer: Layer,
    pub key: &'a str,
    pub members: Vec<&'a Binding>,
}

impl TapGroup<'_> {
    pub fn max_taps(&self) -> u8 {
        self.members.iter().map(|b| b.taps).max().unwrap_or(1)
    }

    pub fn member(&self, taps: u8) -> Option<&Binding> {
        self.members.iter().copied().find(|b| b.taps == taps)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
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
    /// Run a shell command (`sh -c`), in its own systemd scope.
    Command { run: String },
    /// Fire a KDE global shortcut by its internal name, e.g. `Overview`, `Window Close`.
    KwinShortcut {
        name: String,
        /// The kglobalaccel component that owns it.
        #[serde(default = "default_component", skip_serializing_if = "is_default_component")]
        component: String,
    },
}

fn default_component() -> String {
    "kwin".into()
}

fn is_default_component(c: &String) -> bool {
    c == "kwin"
}

impl Action {
    /// Window actions run inside the KWin script; the rest are handled by xremap.
    pub fn is_window_action(&self) -> bool {
        matches!(self, Action::Tile { .. } | Action::Span { .. } | Action::Fixed { .. } | Action::App { .. })
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

const SAVED_HEADER: &str = "\
# Delta config. Edit it here or in the Delta app; deltad applies changes on save.
# Comments aren't kept when the app saves this file.

";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("writing {path}: {source}")]
    Write { path: String, source: std::io::Error },
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

    /// Validate, then write to `path` via a temp file so a half-written config is
    /// never picked up. Comments in the old file aren't kept.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        self.validate().map_err(ConfigError::Invalid)?;
        let err = |source| ConfigError::Write { path: path.display().to_string(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let text = format!("{SAVED_HEADER}{}", self.to_toml());
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path)).map_err(err)
    }

    /// Keys with a tap dance: every key (outside per-app bindings) where some binding
    /// asks for more than one tap.
    pub fn tap_groups(&self) -> Vec<TapGroup<'_>> {
        let mut groups: Vec<TapGroup> = Vec::new();
        for b in self.bindings.iter().filter(|b| b.only_in.is_none()) {
            let key = b.key.to_lowercase();
            let layer = match b.layer {
                Layer::Delta => "delta",
                Layer::Base => "base",
            };
            let name = format!("{layer}:{key}");
            match groups.iter_mut().find(|g| g.name == name) {
                Some(g) => g.members.push(b),
                None => groups.push(TapGroup {
                    name,
                    shortcut_id: String::new(),
                    label: String::new(),
                    layer: b.layer,
                    key: &b.key,
                    members: vec![b],
                }),
            }
        }
        groups.retain(|g| g.max_taps() > 1);
        for g in &mut groups {
            (g.shortcut_id, g.label) = match g.member(1) {
                Some(single) => (single.id.clone(), single.label()),
                None => {
                    let id = format!("DeltaTap_{}", g.name.replace(':', "_"));
                    (id, format!("Delta: tap dance on {}", g.key))
                }
            };
        }
        groups
    }

    pub fn app(&self, name: &str) -> Option<&App> {
        self.apps.iter().find(|a| a.name == name)
    }

    /// The app a window belongs to, by the same rule as the KWin script: a match
    /// string inside the window's class or desktop file name.
    pub fn app_for_window(&self, class: &str, desktop: &str) -> Option<&App> {
        let (class, desktop) = (class.to_lowercase(), desktop.to_lowercase());
        self.apps.iter().find(|a| a.match_strings().iter().any(|m| class.contains(m) || desktop.contains(m)))
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
            if app.color.as_deref().is_some_and(|c| parse_hex_color(c).is_none()) {
                problems.push(format!("app {:?}: color isn't a #RRGGBB colour", app.name));
            }
            if app.match_.iter().any(|m| !is_app_name(m)) {
                problems.push(format!("app {:?}: match entries may only use letters, digits, '.', '_', '-'", app.name));
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
            if b.color.as_deref().is_some_and(|c| parse_hex_color(c).is_none()) {
                problems.push(format!("{at}: color isn't a #RRGGBB colour"));
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
                Action::Command { run } if run.trim().is_empty() => {
                    problems.push(format!("{at}: the command is empty"));
                }
                Action::KwinShortcut { name, component } => {
                    if name.trim().is_empty() || name.chars().any(char::is_control) {
                        problems.push(format!("{at}: shortcut name {name:?} is empty or has control characters"));
                    }
                    if !is_app_name(component) {
                        problems.push(format!("{at}: shortcut component {component:?} is not a valid name"));
                    }
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

        // One action per trigger: the same key, layer, app and tap count can't do two things
        let mut triggers = HashSet::new();
        for b in &self.bindings {
            let trigger = (b.key.to_lowercase(), b.layer, b.only_in.as_deref(), b.taps);
            if !triggers.insert(trigger) {
                let when = b.only_in.as_deref().map(|a| format!(" in {a}")).unwrap_or_default();
                let taps = if b.taps > 1 { format!(" ({} taps)", b.taps) } else { String::new() };
                problems.push(format!(
                    "binding {:?}: {:?} on the {:?} layer{when}{taps} is already bound",
                    b.id, b.key, b.layer
                ));
            }
            if !(1..=MAX_TAPS).contains(&b.taps) {
                problems.push(format!("binding {:?}: taps is 1 to {MAX_TAPS}", b.id));
            }
            if b.taps > 1 && b.only_in.is_some() {
                problems.push(format!("binding {:?}: tap dance doesn't work in per-app bindings yet", b.id));
            }
        }
        for group in self.tap_groups() {
            for b in &group.members {
                if matches!(b.action, Action::Keys { .. }) {
                    problems.push(format!(
                        "binding {:?}: {} has a tap dance, and those can't send keys yet",
                        b.id, group.key
                    ));
                }
            }
        }

        if let Some(n) = &self.lighting.normal
            && parse_hex_color(&n.color).is_none()
        {
            problems.push("lighting.normal.color isn't a #RRGGBB colour".into());
        }
        if self.lighting.delta_background.as_deref().is_some_and(|c| parse_hex_color(c).is_none()) {
            problems.push("lighting.delta_background isn't a #RRGGBB colour".into());
        }
        let mut effect_ids = std::collections::HashSet::new();
        let qmk_name = |k: &str| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || "_()|".contains(c));
        for e in &self.lighting.effects {
            let at = format!("lighting effect {:?}", e.id);
            if e.id.is_empty() || !e.id.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)) {
                problems.push(format!("{at}: id may only use letters, digits, '_' and '-'"));
            }
            if !effect_ids.insert(&e.id) {
                problems.push(format!("{at} is defined twice"));
            }
            if let Some(app) = &e.app
                && self.app(app).is_none()
            {
                problems.push(format!("{at}: no [[apps]] entry named {app:?}"));
            }
            if !qmk_name(&e.trigger) || e.origin.as_deref().is_some_and(|o| o != "center" && !qmk_name(o)) {
                problems.push(format!("{at}: keys are QMK keycodes like KC_ENTER (origin may be \"center\")"));
            }
            if parse_hex_color(&e.color).is_none() || e.color_end.as_deref().is_some_and(|c| parse_hex_color(c).is_none())
            {
                problems.push(format!("{at}: colours are #RRGGBB"));
            }
            if !(e.distance > 0.0 && e.width > 0.0 && e.fade >= 0.0 && e.duration_ms >= 50) {
                problems.push(format!("{at}: distance and width must be > 0, fade >= 0, duration at least 50 ms"));
            }
        }
        let mut profile_apps = std::collections::HashSet::new();
        for p in &self.lighting.profiles {
            let at = match &p.app {
                Some(app) => format!("lighting profile {app:?}"),
                None => "lighting profile (everywhere)".into(),
            };
            if !profile_apps.insert(p.app.clone()) {
                problems.push(format!("{at} is defined twice"));
            }
            if let Some(app) = &p.app
                && self.app(app).is_none()
            {
                problems.push(format!("{at}: no [[apps]] entry named {app:?}"));
            }
            for k in p.background.iter().chain(&p.keys) {
                if parse_hex_color(&k.color).is_none() {
                    problems.push(format!("{at}: {:?} colour isn't #RRGGBB", k.key));
                }
            }
            if p.keys.iter().any(|k| k.key.is_empty() || !k.key.chars().all(|c| c.is_ascii_alphanumeric() || "_()|".contains(c))) {
                problems.push(format!("{at}: key names are QMK keycodes like KC_W"));
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

/// `#RRGGBB` → (r, g, b).
pub fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
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
