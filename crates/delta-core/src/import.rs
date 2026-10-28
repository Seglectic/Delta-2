//! Import the legacy INI-style `delta.conf` into a [`Config`].
//!
//! Follows the bash parser's rules: `#` lines and blank lines are skipped, a
//! `[section]` header must start the line, and values split on the first `=`.
//! Two legacy bugs are fixed rather than copied: `app.KEY` names split on the last
//! dot (so `org.kde.konsole.Q` keeps its full class), and per-app keys are later
//! grouped by app.

use crate::config::{Action, App, Backend, Binding, Config, Engine, Layer, Override, Side, TileLayout, TileTarget};

/// Parse result plus anything that was skipped, for the user to review.
pub struct Imported {
    pub config: Config,
    pub warnings: Vec<String>,
}

pub fn import_conf(text: &str) -> Imported {
    let sections = parse_sections(text);
    let mut warnings = Vec::new();
    let mut config = Config::default();

    let get = |section: &str, key: &str| -> Option<String> {
        sections.iter().filter(|(name, _)| name == section).flat_map(|(_, lines)| lines).find_map(|line| {
            let (k, v) = line.split_once('=')?;
            // Legacy get_value strips every space from the value
            (k.trim() == key).then(|| v.replace(' ', ""))
        })
    };
    let lines = |section: &str| -> Vec<&str> {
        sections
            .iter()
            .filter(|(name, _)| name == section)
            .flat_map(|(_, lines)| lines.iter().map(String::as_str))
            .collect()
    };

    for (name, _) in &sections {
        let known = [
            "device",
            "bindings",
            "launchers",
            "app_bindings",
            "xremap_ignore_devices",
            "aliases",
            "guards",
            "overrides",
        ];
        if !known.contains(&name.as_str()) {
            warnings.push(format!("unknown section [{name}] skipped"));
        }
    }

    // [device]
    config.engine = Engine {
        backend: Backend::Xremap,
        modifier_key: get("device", "modifier_key").filter(|v| !v.is_empty()).unwrap_or_else(|| "capslock".into()),
        ..Engine::default()
    };
    if let Some(backend) = get("device", "backend")
        && !backend.is_empty()
        && backend != "xremap"
    {
        warnings.push(format!("backend {backend:?} is no longer supported; using xremap"));
    }

    // [launchers]
    for line in lines("launchers") {
        let Some((name, cmd)) = line.split_once('=') else {
            warnings.push(format!("[launchers] line without '=' skipped: {line}"));
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        config.apps.push(legacy_app(name, cmd.trim()));
    }

    // [bindings]
    for line in lines("bindings") {
        let Some((id, rest)) = line.split_once('=') else {
            warnings.push(format!("[bindings] line without '=' skipped: {line}"));
            continue;
        };
        let id = id.replace(' ', "");
        let fields: Vec<&str> = rest.split_whitespace().collect();
        match parse_binding(&id, &fields) {
            Ok(b) => config.bindings.push(b),
            Err(e) => warnings.push(format!("[bindings] {id}: {e}")),
        }
    }

    // [aliases]: a bare key that fires the same action as an existing binding.
    // Becomes a base-layer binding with the legacy ID/label so KDE keeps its entry.
    for line in lines("aliases") {
        let Some((id, key)) = line.split_once('=') else { continue };
        let (id, key) = (id.replace(' ', ""), key.replace(' ', ""));
        if id.is_empty() || key.is_empty() {
            continue;
        }
        let Some(orig) = config.bindings.iter().find(|b| b.id == id && b.layer == Layer::Delta).cloned() else {
            warnings.push(format!("[aliases] {id} = {key}: no binding called {id}"));
            continue;
        };
        let action = match orig.action {
            // Overrides for a span match on the original key, not the alias key
            Action::Span { x, width, name: None } => Action::Span { x, width, name: Some(orig.key.to_lowercase()) },
            other => other,
        };
        config.bindings.push(Binding {
            id: format!("{id}_{key}"),
            label: Some(format!("Delta: {id} ({key})")),
            color: None,
            key,
            layer: Layer::Base,
            only_in: None,
            taps: 1,
            action,
        });
    }

    // [app_bindings]: app.KEY = Mod+Key
    for line in lines("app_bindings") {
        let Some((appkey, target)) = line.split_once('=') else { continue };
        let (appkey, target) = (appkey.replace(' ', ""), target.replace(' ', ""));
        if appkey.is_empty() || target.is_empty() {
            continue;
        }
        let Some((app, key)) = appkey.rsplit_once('.') else {
            warnings.push(format!("[app_bindings] {appkey}: expected app.KEY"));
            continue;
        };
        config.bindings.push(Binding {
            id: format!("{}_{}", app.replace(['.', '-'], "_"), key),
            key: key.into(),
            layer: Layer::Base,
            only_in: Some(app.into()),
            label: None,
            color: None,
            taps: 1,
            action: Action::Keys { send: target },
        });
    }

    // [xremap_ignore_devices]: one device name per line
    config.xremap.ignore_devices =
        lines("xremap_ignore_devices").into_iter().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();

    // [guards]
    if let Some(v) = get("guards", "ignore_fullscreen").filter(|v| !v.is_empty()) {
        config.guards.ignore_fullscreen = v.eq_ignore_ascii_case("true");
    }
    let list = |v: Option<String>| -> Vec<String> {
        v.map(|v| v.split(',').filter(|s| !s.is_empty()).map(String::from).collect()).unwrap_or_default()
    };
    config.guards.ignored_desktop_files = list(get("guards", "ignored_desktop_files"));
    config.guards.ignored_classes = list(get("guards", "ignored_classes"));

    // [overrides]: app_class.KEY = x width
    for line in lines("overrides") {
        let Some((appkey, rest)) = line.split_once('=') else { continue };
        let appkey = appkey.replace(' ', "");
        let Some((app, key)) = appkey.rsplit_once('.') else {
            warnings.push(format!("[overrides] {appkey}: expected app_class.KEY"));
            continue;
        };
        let nums: Vec<f64> = rest.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        let [x, width] = nums[..] else {
            warnings.push(format!("[overrides] {appkey}: expected two numbers"));
            continue;
        };
        config.overrides.push(Override { app: app.into(), key: key.to_lowercase(), x, width });
    }

    Imported { config, warnings }
}

/// The legacy KWin script hard-coded how to recognise these apps' windows, and
/// tiled Discord into the left third whenever it was focused.
fn legacy_app(name: &str, launch: &str) -> App {
    let (match_, focus_tile): (&[&str], _) = match name {
        "discord" => (
            &["com.discordapp.discord", "discord"],
            Some(TileTarget { layout: TileLayout::ThirdsLeft, side: Side::Left }),
        ),
        "obsidian" => (&["md.obsidian.obsidian", "obsidian"], None),
        "konsole" => (&["org.kde.konsole", "konsole"], None),
        "code" => (&["code", "vscode"], None),
        _ => (&[], None),
    };
    App {
        name: name.into(),
        launch: launch.into(),
        match_: match_.iter().map(|m| m.to_string()).collect(),
        focus_tile,
        color: None,
    }
}

fn parse_binding(id: &str, f: &[&str]) -> Result<Binding, String> {
    let key = f.first().ok_or("missing key")?.to_string();
    let action = match f.get(1).copied() {
        Some("tile") => {
            let layout = match f.get(2).copied() {
                Some("halves") => TileLayout::Halves,
                Some("thirds_left") => TileLayout::ThirdsLeft,
                Some("thirds_right") => TileLayout::ThirdsRight,
                other => return Err(format!("unknown tile layout {other:?}")),
            };
            let side = match f.get(3).copied() {
                Some("left") => Side::Left,
                Some("right") => Side::Right,
                other => return Err(format!("unknown tile side {other:?}")),
            };
            Action::Tile { layout, side }
        }
        Some("fixed") => {
            let n = |i: usize| f.get(i).and_then(|v| v.parse().ok()).ok_or("fixed needs width and height");
            Action::Fixed { width: n(2)?, height: n(3)? }
        }
        Some("app") => Action::App { app: f.get(2).ok_or("app needs a profile name")?.to_string() },
        Some(x) => {
            let num = |v: Option<&&str>| v.and_then(|v| v.parse::<f64>().ok());
            match (num(Some(&x)), num(f.get(2))) {
                (Some(x), Some(width)) => Action::Span { x, width, name: None },
                _ => return Err(format!("can't read action from {:?}", f[1..].join(" "))),
            }
        }
        None => return Err("missing action".into()),
    };
    Ok(Binding { id: id.into(), key, layer: Layer::Delta, only_in: None, label: None, color: None, taps: 1, action })
}

/// `[section]` → its non-comment, non-blank lines, in file order. Sections may repeat.
fn parse_sections(text: &str) -> Vec<(String, Vec<String>)> {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push((name.into(), Vec::new()));
            continue;
        }
        if let Some((_, lines)) = sections.last_mut() {
            lines.push(line.into());
        }
    }
    sections
}
