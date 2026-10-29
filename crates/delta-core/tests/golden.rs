//! Import each fixture's legacy `delta.conf` and check the Rust generators produce
//! what the legacy bash `delta` produced (recorded with `tools/legacy-render.sh`),
//! apart from the intentional changes listed in `fixtures/CHANGES.md`.
//!
//! - `live`: today's real config.
//! - `edge`: every legacy feature.

use delta_core::{Config, generate, import};
use std::path::{Path, PathBuf};

const LEGACY_XREMAP: &str = "/usr/bin/xremap";
const LEGACY_XREMAP_CONFIG: &str = "/home/segger/.config/xremap/config.yml";
const LEGACY_LAUNCHER: &str = "/home/segger/.local/bin/delta/delta-launcher";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn imported(name: &str) -> Config {
    let result = import::import_conf(&read(fixture(name).join("delta.conf")));
    assert!(result.warnings.is_empty(), "unexpected import warnings: {:?}", result.warnings);
    result.config.validate().unwrap_or_else(|p| panic!("imported config invalid:\n{p}"));
    result.config
}

/// Drop the first three comment lines, which differ by design.
fn without_header(js: &str) -> &str {
    js.splitn(4, '\n').nth(3).unwrap()
}

#[track_caller]
fn assert_same(actual: &str, expected: &str, what: &str) {
    if actual != expected {
        let line = actual.lines().zip(expected.lines()).position(|(a, e)| a != e);
        panic!(
            "{what} differs (first differing line: {line:?})\n--- actual ---\n{actual}\n--- expected ---\n{expected}"
        );
    }
}

fn check(name: &str) {
    let config = imported(name);
    let expected = fixture(name).join("expected");

    assert_same(&generate::xremap_config(&config, "delta"), &read(expected.join("xremap.yml")), "xremap.yml");
    assert_same(
        without_header(&generate::kwin_script(&config)),
        without_header(&read(expected.join("main.js"))),
        "main.js",
    );
    assert_same(
        &generate::xremap_service(&config, LEGACY_XREMAP, LEGACY_XREMAP_CONFIG),
        &read(expected.join("xremap.service")),
        "xremap.service",
    );
    assert_same(
        &generate::launcher_unit(LEGACY_LAUNCHER),
        &read(expected.join("delta-launch@.service")),
        "launcher unit",
    );
}

#[test]
fn live_config_matches_legacy_output() {
    check("live");
}

#[test]
fn edge_config_matches_legacy_output() {
    check("edge");
}

#[test]
fn toml_round_trip_keeps_everything() {
    for name in ["live", "edge"] {
        let config = imported(name);
        let back = Config::from_toml(&config.to_toml()).expect("re-parse");
        assert_eq!(back, config, "{name}: TOML round trip changed the config");
    }
}

#[test]
fn launch_commands_survive_import() {
    let config = imported("live");
    let discord = config.app("discord").expect("discord app");
    assert_eq!(discord.launch, "/usr/bin/flatpak run com.discordapp.Discord");
    assert_eq!(config.engine.modifier_key, "f20");
    assert_eq!(config.xremap.ignore_devices, ["Logitech G502"]);
}

#[test]
fn validation_reports_every_problem() {
    let config = Config::from_toml(
        r#"
        [[bindings]]
        id = "Bad Id"
        key = "A"
        action = "app"
        app = "nope"

        [[bindings]]
        id = "Dup"
        key = "B"
        action = "keys"
        send = "Alt+Up"

        [[bindings]]
        id = "Dup"
        key = "C"
        only_in = "firefox"
        action = "fixed"
        width = 10
        height = 10
        "#,
    )
    .unwrap();
    let problems = config.validate().unwrap_err().0;
    assert_eq!(problems.len(), 4, "{problems:#?}");
}

#[test]
fn generated_kwin_script_is_valid_javascript() {
    let Ok(node) = which_node() else {
        eprintln!("node not installed; skipping");
        return;
    };
    for name in ["live", "edge"] {
        let dir = std::env::temp_dir().join(format!("delta-golden-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("main.js");
        std::fs::write(&file, generate::kwin_script(&imported(name))).unwrap();
        let status = std::process::Command::new(&node).arg("--check").arg(&file).status().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(status.success(), "{name}: generated KWin script doesn't parse");
    }
}

fn which_node() -> Result<PathBuf, ()> {
    let path = std::env::var_os("PATH").ok_or(())?;
    std::env::split_paths(&path).map(|d| d.join("node")).find(|p| p.is_file()).ok_or(())
}

#[test]
fn command_and_shortcut_actions_go_through_xremap() {
    let config = Config::from_toml(
        r#"
        [engine]
        modifier_key = "f20"

        [[bindings]]
        id = "Notes"
        key = "N"
        action = "command"
        run = "kate \"$HOME/notes.md\""

        [[bindings]]
        id = "Overview"
        key = "F13"
        layer = "base"
        action = "kwin_shortcut"
        name = "Overview"

        [[bindings]]
        id = "Close"
        key = "F14"
        layer = "base"
        only_in = "firefox"
        action = "kwin_shortcut"
        name = "Window Close"
        "#,
    )
    .unwrap();
    config.validate().unwrap();
    let yaml = generate::xremap_config(&config, "/usr/bin/delta");
    assert!(
        yaml.contains(
            r#"F20-n: {launch: ["systemd-run", "--user", "--scope", "--quiet", "sh", "-c", "kate \"$HOME/notes.md\""]}"#
        ),
        "{yaml}"
    );
    assert!(
        yaml.contains(r#"F13: {launch: ["/usr/bin/delta", "shortcut", "--component", "kwin", "Overview"]}"#),
        "{yaml}"
    );
    assert!(
        yaml.contains(r#"F14: {launch: ["/usr/bin/delta", "shortcut", "--component", "kwin", "Window Close"]}"#),
        "{yaml}"
    );
    // None of them register a KDE shortcut of their own
    assert!(!generate::kwin_script(&config).contains("registerShortcut(\"Notes\""));
}

const TAP_DANCE: &str = r#"
    [engine]
    modifier_key = "f20"

    [[apps]]
    name = "dolphin"
    launch = "dolphin"

    [[bindings]]
    id = "Left"
    key = "A"
    action = "tile"
    layout = "thirds_right"
    side = "left"

    [[bindings]]
    id = "LeftHalf"
    key = "A"
    taps = 2
    action = "tile"
    layout = "halves"
    side = "left"

    [[bindings]]
    id = "Files3"
    key = "F"
    taps = 3
    action = "app"
    app = "dolphin"

    [[bindings]]
    id = "Notes"
    key = "N"
    action = "command"
    run = "kate"

    [[bindings]]
    id = "Notes2"
    key = "N"
    taps = 2
    action = "kwin_shortcut"
    name = "Overview"
"#;

#[test]
fn tap_dance_keys_report_taps_to_deltad() {
    let config = Config::from_toml(TAP_DANCE).unwrap();
    config.validate().unwrap();

    let groups = config.tap_groups();
    let names: Vec<_> = groups.iter().map(|g| (g.name.as_str(), g.shortcut_id.as_str(), g.max_taps())).collect();
    assert_eq!(names, [("delta:a", "Left", 2), ("delta:f", "DeltaTap_delta_f", 3), ("delta:n", "Notes", 2)]);

    // xremap: every tap key just sends the chord, once, even the command one
    let xremap = generate::xremap_config(&config, "/bin/delta");
    for key in ["a", "f", "n"] {
        assert_eq!(xremap.matches(&format!("F20-{key}:")).count(), 1, "{xremap}");
        assert!(xremap.contains(&format!("F20-{key}: C-A-Super-{key}")), "{xremap}");
    }

    // KWin: the key's shortcut reports the tap; window actions get key-less shortcuts
    let js = generate::kwin_script(&config);
    assert!(js.contains(r#"registerShortcut("Left", "Delta: Left", "Ctrl+Alt+Meta+A", function() { callDBus("#));
    assert!(js.contains(r#""Tap", "delta:a""#));
    assert!(js.contains(r#"registerShortcut("Left.tap", "Delta: Left (tap ×1)", "", function() { tileIntoLayout("#));
    assert!(js.contains(r#"registerShortcut("LeftHalf.tap", "#));
    assert!(js.contains(r#"registerShortcut("DeltaTap_delta_f", "Delta: tap dance on F", "Ctrl+Alt+Meta+F""#));
    assert!(js.contains(r#"registerShortcut("Notes", "Delta: Notes", "Ctrl+Alt+Meta+N""#));
    assert!(!js.contains("Notes.tap") && !js.contains("Notes2.tap"), "deltad runs those itself");
    assert_eq!(js.matches(r#""Ctrl+Alt+Meta+A""#).count(), 1);

    if let Ok(node) = which_node() {
        let file = std::env::temp_dir().join(format!("delta-tap-{}.js", std::process::id()));
        std::fs::write(&file, &js).unwrap();
        let status = std::process::Command::new(&node).arg("--check").arg(&file).status().unwrap();
        std::fs::remove_file(&file).ok();
        assert!(status.success(), "generated KWin script doesn't parse");
    }
}

#[test]
fn tap_dance_rules() {
    let bad = TAP_DANCE.to_string()
        + r#"
    [[bindings]]
    id = "LeftAgain"
    key = "a"
    taps = 2
    action = "command"
    run = "true"

    [[bindings]]
    id = "Four"
    key = "Z"
    taps = 4
    action = "command"
    run = "true"

    [[bindings]]
    id = "Send"
    key = "N"
    taps = 3
    action = "keys"
    send = "Alt+Up"

    [[bindings]]
    id = "PerApp"
    key = "K"
    taps = 2
    only_in = "dolphin"
    action = "keys"
    send = "Alt+Up"
    "#;
    let problems = Config::from_toml(&bad).unwrap().validate().unwrap_err().0;
    let has = |s: &str| problems.iter().any(|p| p.contains(s));
    assert!(has(r#""LeftAgain": "a" on the Delta layer (2 taps) is already bound"#), "{problems:#?}");
    assert!(has(r#""Four": taps is 1 to 3"#), "{problems:#?}");
    assert!(has(r#""Send": N has a tap dance, and those can't send keys yet"#), "{problems:#?}");
    assert!(has(r#""PerApp": tap dance doesn't work in per-app bindings yet"#), "{problems:#?}");
    assert_eq!(problems.len(), 4, "{problems:#?}");

    // Round trip: taps survive saving, single taps don't write the field
    let config = Config::from_toml(TAP_DANCE).unwrap();
    let toml = config.to_toml();
    assert_eq!(toml.matches("taps = ").count(), 3, "{toml}");
    assert_eq!(Config::from_toml(&toml).unwrap(), config);
}

#[test]
fn saving_keeps_the_previous_config() {
    let dir = std::env::temp_dir().join(format!("delta-history-{}", std::process::id()));
    let path = dir.join("delta.toml");
    let mut config = Config::from_toml(TAP_DANCE).unwrap();
    config.save(&path).unwrap();
    let history = || std::fs::read_dir(dir.join("history")).map_or(0, |d| d.count());
    assert_eq!(history(), 0, "nothing to keep on the first save");
    let first = std::fs::read_to_string(&path).unwrap();
    config.engine.tap_ms = 300;
    std::thread::sleep(std::time::Duration::from_millis(2));
    config.save(&path).unwrap();
    assert_eq!(history(), 1);
    let kept = std::fs::read_dir(dir.join("history")).unwrap().next().unwrap().unwrap().path();
    assert_eq!(std::fs::read_to_string(kept).unwrap(), first);
    std::fs::remove_dir_all(&dir).unwrap();
}
