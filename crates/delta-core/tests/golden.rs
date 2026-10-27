//! Import each fixture's legacy `delta.conf` and check the Rust generators produce
//! what the legacy bash `delta` produced (recorded with `tools/legacy-render.sh`).
//!
//! - `live`: today's real config; output must match byte for byte.
//! - `edge`: every legacy feature; expected output has two intentional fixes (see FIXES.md).

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

    assert_same(&generate::xremap_config(&config), &read(expected.join("xremap.yml")), "xremap.yml");
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
