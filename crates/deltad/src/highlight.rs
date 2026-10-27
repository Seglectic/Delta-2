//! Caps-held key highlighting on keyboards running Keychron firmware with per-key RGB.
//!
//! The per-key colour buffer is filled with "what's bound" colours ahead of time (RAM
//! only, re-sent when the config or base layer changes). Holding the delta key just
//! switches the effect to Per Key RGB; letting go switches back. Nothing is saved to
//! the keyboard: a replug restores whatever the user had.

use delta_core::Config;
use delta_core::config::{Action, Layer, parse_hex_color};
use delta_core::paths::Paths;
use delta_via::keycode::code_for_key_name;
use delta_via::{Definition, Hsv, LightingCommand, Via, ViaError, discover};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Keychron's "Per Key RGB" effect number (keyboards/keychron/q5/via_json).
const PER_KEY_EFFECT: u8 = 23;
const PER_KEY_SOLID: u8 = 0;
/// Brightness to use while highlighting if the lights are dimmer than this (or off).
const MIN_BRIGHTNESS: u8 = 160;
const POLL: Duration = Duration::from_millis(10);
const RECHECK_LAYERS: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(3);

/// The config deltad last loaded, plus a counter bumped on every change.
#[derive(Default, Clone)]
pub struct Shared {
    pub config: Arc<Mutex<Option<Config>>>,
    pub generation: Arc<AtomicU64>,
}

impl Shared {
    pub fn set(&self, config: Config) {
        *self.config.lock().unwrap() = Some(config);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn spawn(shared: Shared) {
    std::thread::Builder::new()
        .name("highlight".into())
        .spawn(move || {
            let mut reported = String::new();
            loop {
                // Errors are expected (keyboard unplugged, other firmware); say each once
                if let Err(e) = run(&shared) {
                    let msg = e.to_string();
                    if msg != reported {
                        eprintln!("highlight: {msg}");
                        reported = msg;
                    }
                }
                std::thread::sleep(RETRY);
            }
        })
        .expect("spawn highlight thread");
}

/// One keyboard session: returns when the keyboard goes away or errors.
fn run(shared: &Shared) -> Result<(), ViaError> {
    let paths = Paths::from_env();
    let found = discover().into_iter().find_map(|d| {
        let mut via = Via::open(&d).ok()?;
        let leds = via.keychron_rgb().ok()??;
        Some((d, via, leds))
    });
    let Some((device, mut via, led_count)) = found else {
        return Err(ViaError::Invalid("no keyboard with Keychron per-key RGB".into()));
    };
    let def = Definition::load(device.vendor_id, device.product_id, &paths.layouts, &paths.layout_cache, true)
        .map_err(|e| ViaError::Invalid(e.to_string()))?;
    let led_map = via.led_map(def.rows, def.cols)?;
    let keymap = via.keymap(def.rows, def.cols)?;
    eprintln!("highlight: {} ready ({led_count} LEDs)", def.name);

    let mut uploaded: Option<(u64, u8)> = None; // (config generation, base layer)
    let mut base_layer = via.keychron_layers()?.0;
    let mut layers_checked = Instant::now();
    let mut held: Option<(u8, u8)> = None; // saved (effect, brightness) while highlighting
    let mut warned_effect = false;
    let mut applied_normal: Option<u64> = None;

    loop {
        let generation = shared.generation.load(Ordering::Relaxed);
        let config = shared.config.lock().unwrap().clone();
        let enabled = config.as_ref().is_some_and(|c| c.lighting.caps_highlight);
        let layer_codes = &keymap[(base_layer as usize).min(keymap.len() - 1)];
        let modifier = config.as_ref().and_then(|c| code_for_key_name(&c.engine.modifier_key));

        // Put the user's normal lighting on when the keyboard connects or it changes
        let normal = config.as_ref().and_then(|c| c.lighting.normal.clone());
        if held.is_none() && applied_normal != Some(generation) {
            if let Some(n) = &normal {
                apply_normal(&mut via, n)?;
            }
            applied_normal = Some(generation);
        }

        // Refresh the colour buffer when idle and something changed
        if held.is_none()
            && enabled
            && uploaded != Some((generation, base_layer))
            && let Some(config) = &config
        {
            let colors = key_colors(config, layer_codes, &led_map, led_count);
            via.set_per_key_colors(&colors, led_count)?;
            uploaded = Some((generation, base_layer));
        }

        let pressed = via.pressed(def.rows, def.cols)?;
        let modifier_down =
            modifier.is_some_and(|m| pressed.iter().any(|&(r, c)| layer_codes[r as usize][c as usize] == m));

        match (held, modifier_down && enabled) {
            (None, true) => {
                let saved = via.lighting()?;
                via.send(LightingCommand::PerKeyType(PER_KEY_SOLID))?;
                via.send(LightingCommand::Effect(PER_KEY_EFFECT))?;
                if saved.1 < MIN_BRIGHTNESS {
                    via.send(LightingCommand::Brightness(MIN_BRIGHTNESS))?;
                }
                if !warned_effect && via.lighting()?.0 != PER_KEY_EFFECT {
                    eprintln!(
                        "highlight: firmware didn't accept effect {PER_KEY_EFFECT}; is it Keychron's per-key build?"
                    );
                    warned_effect = true;
                }
                held = Some(saved);
            }
            (Some((effect, brightness)), false) => {
                match &normal {
                    Some(n) => apply_normal(&mut via, n)?,
                    None => {
                        via.send(LightingCommand::Effect(effect))?;
                        via.send(LightingCommand::Brightness(brightness))?;
                    }
                }
                held = None;
            }
            _ => {}
        }

        // The Mac/Windows switch changes the base layer; check now and then
        if held.is_none() && layers_checked.elapsed() > RECHECK_LAYERS {
            base_layer = via.keychron_layers()?.0;
            layers_checked = Instant::now();
        }
        std::thread::sleep(POLL);
    }
}

/// The per-key colour buffer: each Caps-layer binding's key in its colour, the delta
/// key itself white, everything else brightness 0 (dark on firmware that honours
/// per-key brightness, the effect's base colour otherwise).
fn key_colors(config: &Config, layer: &[Vec<u16>], led_map: &[Vec<Option<u8>>], led_count: u8) -> Vec<Hsv> {
    let mut colors = vec![Hsv { h: 0, s: 0, v: 0 }; led_count as usize];
    let mut paint = |code: u16, color: Hsv| {
        for (r, row) in layer.iter().enumerate() {
            for (c, &k) in row.iter().enumerate() {
                if k == code
                    && let Some(Some(led)) = led_map.get(r).and_then(|row| row.get(c))
                    && let Some(slot) = colors.get_mut(*led as usize)
                {
                    *slot = color;
                }
            }
        }
    };
    for b in config.bindings.iter().filter(|b| b.layer == Layer::Delta && b.only_in.is_none()) {
        if let Some(code) = code_for_key_name(&b.key) {
            paint(code, binding_color(config, b));
        }
    }
    if let Some(code) = code_for_key_name(&config.engine.modifier_key) {
        paint(code, Hsv { h: 0, s: 0, v: 255 });
    }
    colors
}

fn binding_color(config: &Config, b: &delta_core::config::Binding) -> Hsv {
    let hex = b.color.clone().or_else(|| match &b.action {
        Action::App { app } => config.app(app).and_then(|a| a.color.clone()).or_else(|| known_app_color(app)),
        _ => None,
    });
    if let Some((r, g, bl)) = hex.as_deref().and_then(parse_hex_color) {
        return rgb_to_hsv(r, g, bl);
    }
    match &b.action {
        Action::Tile { .. } => rgb_to_hsv(0x89, 0xB4, 0xFA), // blue
        Action::Span { .. } | Action::Fixed { .. } => rgb_to_hsv(0x94, 0xE2, 0xD5), // teal
        Action::Keys { .. } => rgb_to_hsv(0xF9, 0xE2, 0xAF), // yellow
        Action::Command { .. } => rgb_to_hsv(0xFA, 0xB3, 0x87), // peach
        Action::KwinShortcut { .. } => rgb_to_hsv(0xCB, 0xA6, 0xF7), // mauve
        // Apps without a colour get a stable hue from their name
        Action::App { app } => Hsv { h: name_hue(app), s: 255, v: 255 },
    }
}

/// Brand-ish colours for common apps, used when the config doesn't set one.
fn known_app_color(app: &str) -> Option<String> {
    let hex = match app {
        "discord" => "#5865F2",
        "dolphin" => "#1D99F3",
        "konsole" => "#2ECC71",
        "obsidian" => "#7C3AED",
        "firefox" => "#FF7139",
        "steam" => "#66C0F4",
        _ => return None,
    };
    Some(hex.into())
}

fn name_hue(name: &str) -> u8 {
    name.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as u8
}

/// Effect, colour and brightness from `[lighting.normal]` (RAM only).
fn apply_normal(via: &mut Via, n: &delta_core::config::NormalLighting) -> Result<(), ViaError> {
    if let Some((r, g, b)) = parse_hex_color(&n.color) {
        let hsv = rgb_to_hsv(r, g, b);
        via.send(LightingCommand::Color { h: hsv.h, s: hsv.s })?;
    }
    via.send(LightingCommand::Effect(n.effect))?;
    via.send(LightingCommand::Brightness(n.brightness))
}

/// RGB → QMK's 0–255 HSV.
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> Hsv {
    let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = rf.max(gf).max(bf);
    let delta = max - rf.min(gf).min(bf);
    let hue = if delta == 0.0 {
        0.0
    } else if max == rf {
        60.0 * (((gf - bf) / delta).rem_euclid(6.0))
    } else if max == gf {
        60.0 * ((bf - rf) / delta + 2.0)
    } else {
        60.0 * ((rf - gf) / delta + 4.0)
    };
    let sat = if max == 0.0 { 0.0 } else { delta / max };
    // Full value: the keyboard's brightness setting decides how bright
    Hsv { h: (hue / 360.0 * 255.0).round() as u8, s: (sat * 255.0).round() as u8, v: 255 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(rgb_to_hsv(255, 0, 0), Hsv { h: 0, s: 255, v: 255 });
        assert_eq!(rgb_to_hsv(0, 0, 255).h, 170);
        assert_eq!(rgb_to_hsv(255, 255, 255).s, 0);
    }

    #[test]
    fn paints_bound_keys_and_the_delta_key() {
        let config = Config::from_toml(
            r#"
            [engine]
            modifier_key = "f20"
            [[apps]]
            name = "discord"
            launch = "discord"
            [[bindings]]
            id = "R"
            key = "R"
            action = "app"
            app = "discord"
            [[bindings]]
            id = "Plain"
            key = "F19"
            layer = "base"
            action = "app"
            app = "discord"
            "#,
        )
        .unwrap();
        // 1×3 matrix: F20, R, F19 on LEDs 2, 0, 1
        let layer = vec![vec![0x6F, 0x15, 0x6E]];
        let leds = vec![vec![Some(2), Some(0), Some(1)]];
        let colors = key_colors(&config, &layer, &leds, 3);
        assert_eq!(colors[0], rgb_to_hsv(0x58, 0x65, 0xF2), "R is Discord blurple");
        assert_eq!(colors[1].v, 0, "base-layer bindings aren't shown");
        assert_eq!(colors[2], Hsv { h: 0, s: 0, v: 255 }, "the delta key is white");
    }
}
