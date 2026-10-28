//! Per-key lighting on keyboards running Keychron firmware with per-key RGB, RAM only
//! (nothing is saved to the keyboard: a replug restores whatever the user had).
//!
//! - Lighting profiles: the focused app's per-key colours, else the everywhere
//!   profile, else `[lighting.normal]`. The GUI can preview a profile being edited.
//! - Caps held: bound keys in their colours. Without a profile the buffer holds those
//!   colours ahead of time, so holding the delta key only switches the effect.

use crate::effects::{self, Geometry, Running};
use delta_core::Config;
use delta_core::config::{KeyEffect, LightingProfile, NormalLighting, parse_hex_color};
use delta_core::lighting::{self, Rgb};
use delta_core::paths::Paths;
use delta_via::keycode::{code_for_key_name, decode};
use delta_via::{Definition, Hsv, LightingCommand, LightingState, Via, ViaError, discover};
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
/// How long a GUI preview lasts unless it's sent again.
pub const PREVIEW_TTL: Duration = Duration::from_secs(10);

/// What the lighting thread works from: the config deltad last loaded (with a
/// counter bumped on every change), the focused window, and the GUI's preview.
#[derive(Default, Clone)]
pub struct Shared {
    pub config: Arc<Mutex<Option<Config>>>,
    pub generation: Arc<AtomicU64>,
    /// Focused window's (class, desktop file), from the KWin script.
    pub focus: Arc<Mutex<Option<(String, String)>>>,
    /// A profile the GUI is editing, shown instead of the focused app's until it expires.
    pub preview: Arc<Mutex<Option<(LightingProfile, Instant)>>>,
    /// An effect the GUI is editing, played on a loop until it expires.
    pub preview_effect: Arc<Mutex<Option<(KeyEffect, Instant)>>>,
}

impl Shared {
    pub fn set(&self, config: Config) {
        *self.config.lock().unwrap() = Some(config);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    fn effect_preview(&self) -> Option<KeyEffect> {
        let mut preview = self.preview_effect.lock().unwrap();
        if preview.as_ref().is_some_and(|(_, at)| at.elapsed() > PREVIEW_TTL) {
            *preview = None;
        }
        preview.as_ref().map(|(e, _)| e.clone())
    }

    /// The profile to show right now, if any.
    fn profile(&self, config: Option<&Config>) -> Option<LightingProfile> {
        let mut preview = self.preview.lock().unwrap();
        if preview.as_ref().is_some_and(|(_, at)| at.elapsed() > PREVIEW_TTL) {
            *preview = None;
        }
        if let Some((p, _)) = preview.as_ref() {
            return Some(p.clone());
        }
        // An effect being previewed plays over its own app's profile
        let config = config?;
        if let Some(app) = self.effect_preview().and_then(|e| e.app) {
            let profiles = &config.lighting.profiles;
            return profiles
                .iter()
                .find(|p| p.app.as_ref() == Some(&app))
                .or_else(|| profiles.iter().find(|p| p.app.is_none()))
                .cloned();
        }
        let focus = self.focus.lock().unwrap();
        lighting::active_profile(config, focus.as_ref().map(|(c, d)| (c.as_str(), d.as_str()))).cloned()
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

/// An animation in progress: it owns the per-key buffer until the last effect ends.
struct Anim {
    /// What the effects are drawn over: the profile, the normal colour, or black.
    base: Vec<Rgb>,
    /// Lighting to go back to; None when a profile is up (it's re-uploaded instead).
    before: Option<LightingState>,
    /// The last frame sent, so the next only sends what changed.
    sent: Option<Vec<Hsv>>,
}

/// Switch to solid Per Key RGB (the colours must already be uploaded).
fn per_key_on(via: &mut Via) -> Result<(), ViaError> {
    via.send(LightingCommand::PerKeyType(PER_KEY_SOLID))?;
    via.send(LightingCommand::Effect(PER_KEY_EFFECT))
}

fn restore(via: &mut Via, saved: &LightingState) -> Result<(), ViaError> {
    via.send(LightingCommand::Color { h: saved.hue, s: saved.sat })?;
    via.send(LightingCommand::Effect(saved.effect))?;
    via.send(LightingCommand::Brightness(saved.brightness))
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
    // QMK names of every layer's keycodes, which profiles use to name keys
    let names: Vec<Vec<Vec<String>>> = keymap
        .iter()
        .map(|layer| {
            layer.iter().map(|row| row.iter().map(|&c| decode(c, &def.custom_keycodes).name).collect()).collect()
        })
        .collect();
    eprintln!("highlight: {} ready ({led_count} LEDs)", def.name);

    // Caps-layer colours in the buffer: (config generation, base layer)
    let mut uploaded: Option<(u64, u8)> = None;
    // Boards without Keychron's layer query use layer 0
    let mut base_layer = via.keychron_layers()?.map_or(0, |l| l.0);
    let mut layers_checked = Instant::now();
    let mut held: Option<LightingState> = None; // what to go back to after highlighting
    let mut warned_effect = false;
    let mut applied_normal: Option<u64> = None;
    // The profile on the keyboard now, and what was there before any profile
    let mut shown: Option<(LightingProfile, u8)> = None;
    let mut before_profile: Option<LightingState> = None;
    let geo = Geometry::new(&def, &led_map, led_count);
    let mut running: Vec<Running> = vec![];
    let mut anim: Option<Anim> = None;
    let mut was_pressed: Vec<(u8, u8)> = vec![];
    let mut previewing: Option<KeyEffect> = None;
    hide_num_lock_indicator(&mut via)?;

    loop {
        let generation = shared.generation.load(Ordering::Relaxed);
        let config = shared.config.lock().unwrap().clone();
        let enabled = config.as_ref().is_some_and(|c| c.lighting.caps_highlight);
        let layer = (base_layer as usize).min(keymap.len() - 1);
        let layer_codes = &keymap[layer];
        let modifier = config.as_ref().and_then(|c| code_for_key_name(&c.engine.modifier_key));
        let normal = config.as_ref().and_then(|c| c.lighting.normal.clone());
        let profile = shared.profile(config.as_ref());

        if held.is_none() && anim.is_none() {
            match &profile {
                // Profile wanted and not up yet (or changed): upload it and go per-key
                Some(p) if shown.as_ref() != Some(&(p.clone(), base_layer)) => {
                    if shown.is_none() && before_profile.is_none() {
                        before_profile = Some(via.lighting()?);
                    }
                    via.set_per_key_colors(&hsv(profile_colors(p, &names[layer], &led_map, led_count)), led_count)?;
                    per_key_on(&mut via)?;
                    shown = Some((p.clone(), base_layer));
                    uploaded = None;
                }
                Some(_) => {}
                // No profile any more: back to the normal lighting, or what was there
                None => {
                    if shown.take().is_some() {
                        match (&normal, before_profile.take()) {
                            (Some(n), _) => apply_normal(&mut via, n)?,
                            (None, Some(saved)) => restore(&mut via, &saved)?,
                            (None, None) => {}
                        }
                    }
                }
            }

            // Put the user's normal lighting on when the keyboard connects or it changes
            if shown.is_none() && applied_normal != Some(generation) {
                if let Some(n) = &normal {
                    apply_normal(&mut via, n)?;
                }
                applied_normal = Some(generation);
            }

            // Without a profile the buffer is free: keep the Caps colours ready in it
            if shown.is_none()
                && enabled
                && uploaded != Some((generation, base_layer))
                && let Some(config) = &config
            {
                via.set_per_key_colors(&key_colors(config, layer_codes, &led_map, led_count), led_count)?;
                uploaded = Some((generation, base_layer));
            }
        }

        let pressed = via.pressed(def.rows, def.cols)?;
        let modifier_down =
            modifier.is_some_and(|m| pressed.iter().any(|&(r, c)| layer_codes[r as usize][c as usize] == m));

        // Key-triggered effects for keys that just went down
        if held.is_none()
            && let Some(config) = &config
        {
            let focus = shared.focus.lock().unwrap().clone();
            let focus = focus.as_ref().map(|(c, d)| (c.as_str(), d.as_str()));
            for &(r, c) in pressed.iter().filter(|p| !was_pressed.contains(p)) {
                let name = &names[layer][r as usize][c as usize];
                for e in lighting::active_effects(config, focus).filter(|e| e.trigger == *name) {
                    running.extend(Running::start(e, Some((r, c)), &geo, &names[layer]));
                }
            }
        }
        was_pressed.clone_from(&pressed);
        // and the GUI's preview, on a loop
        let wanted = shared.effect_preview();
        if wanted != previewing {
            running.retain(|r| !r.is_looping());
            if let Some(e) = &wanted {
                running.extend(Running::start(e, None, &geo, &names[layer]).map(Running::looping));
            }
            previewing = wanted;
        }

        if held.is_none() && !running.is_empty() {
            if anim.is_none() {
                let before = if shown.is_some() { None } else { Some(via.lighting()?) };
                let base = match (&shown, &before) {
                    (Some((p, _)), _) => profile_colors(p, &names[layer], &led_map, led_count),
                    (None, Some(b)) if matches!(b.effect, 1 | 2) => {
                        vec![Hsv { h: b.hue, s: b.sat, v: 255 }.to_rgb(); led_count as usize]
                    }
                    _ => vec![(0, 0, 0); led_count as usize],
                };
                if shown.is_none() {
                    per_key_on(&mut via)?;
                }
                anim = Some(Anim { base, before, sent: None });
            }
            let a = anim.as_mut().expect("just set");
            let frame = hsv(effects::frame(&a.base, &mut running, &geo, Instant::now()));
            via.set_per_key_colors_since(&frame, a.sent.as_deref(), led_count)?;
            a.sent = Some(frame);
        }
        // Done (or Caps went down): put back what was there
        if (running.is_empty() || modifier_down && enabled)
            && let Some(a) = anim.take()
        {
            match a.before {
                Some(before) => restore(&mut via, &before)?,
                None => shown = None, // the profile is uploaded again above
            }
            uploaded = None;
        }

        match (held, modifier_down && enabled) {
            (None, true) => {
                let saved = via.lighting()?;
                if uploaded != Some((generation, base_layer))
                    && let Some(config) = &config
                {
                    via.set_per_key_colors(&key_colors(config, layer_codes, &led_map, led_count), led_count)?;
                    uploaded = Some((generation, base_layer));
                }
                per_key_on(&mut via)?;
                if saved.brightness < MIN_BRIGHTNESS {
                    via.send(LightingCommand::Brightness(MIN_BRIGHTNESS))?;
                }
                if !warned_effect && via.lighting()?.effect != PER_KEY_EFFECT {
                    eprintln!(
                        "highlight: firmware didn't accept effect {PER_KEY_EFFECT}; is it Keychron's per-key build?"
                    );
                    warned_effect = true;
                }
                held = Some(saved);
            }
            (Some(saved), false) => {
                match &shown {
                    // The profile's colours go back in; it stays per-key
                    Some((p, _)) => {
                        via.set_per_key_colors(&hsv(profile_colors(p, &names[layer], &led_map, led_count)), led_count)?;
                        via.send(LightingCommand::Brightness(saved.brightness))?;
                        uploaded = None;
                    }
                    // Exactly what was there, so knob brightness changes survive
                    None => restore(&mut via, &saved)?,
                }
                held = None;
            }
            _ => {}
        }

        // Now and then: the Mac/Windows switch changes the base layer, and the
        // firmware's own blink indicators (Win lock, OS switch) reload the lighting
        // saved in EEPROM, which overwrites what Delta set in RAM
        if held.is_none() && anim.is_none() && layers_checked.elapsed() > RECHECK_LAYERS {
            base_layer = via.keychron_layers()?.map_or(0, |l| l.0);
            hide_num_lock_indicator(&mut via)?;
            let now = via.lighting()?;
            match &normal {
                _ if shown.is_some() => {
                    if now.effect != PER_KEY_EFFECT {
                        shown = None; // re-applied next time round
                    }
                }
                Some(n) if drifted(&now, n) => apply_normal(&mut via, n)?,
                _ => {}
            }
            layers_checked = Instant::now();
        }
        // Faster while animating: the frame writes already take most of a tick
        std::thread::sleep(if anim.is_some() { POLL / 4 } else { POLL });
    }
}

/// Keychron paints the Num Lock key's LED white while Num Lock is on, over whatever
/// Delta set there. That key is rebound (F19 on the Q5), so turn the indicator off,
/// in RAM only; the EEPROM reloads that undo it are caught by the periodic check.
fn hide_num_lock_indicator(via: &mut Via) -> Result<(), ViaError> {
    const NUM_LOCK: u8 = 0x01;
    let Some((disabled, color)) = via.indicators()? else { return Ok(()) };
    if disabled & NUM_LOCK == 0 {
        via.send(LightingCommand::Indicators { disabled: disabled | NUM_LOCK, color })?;
    }
    Ok(())
}

fn hsv(colors: Vec<Rgb>) -> Vec<Hsv> {
    colors.into_iter().map(rgb_to_hsv).collect()
}

/// A profile's per-key colours: its keys (found by QMK name on the base layer) on
/// the background, brightness applied.
fn profile_colors(p: &LightingProfile, names: &[Vec<String>], led_map: &[Vec<Option<u8>>], led_count: u8) -> Vec<Rgb> {
    let background = p.background.as_ref().map_or((0, 0, 0), lighting::key_light_rgb);
    let mut colors = vec![background; led_count as usize];
    for k in &p.keys {
        let color = lighting::key_light_rgb(k);
        for (r, row) in names.iter().enumerate() {
            for (c, name) in row.iter().enumerate() {
                if *name == k.key
                    && let Some(Some(led)) = led_map.get(r).and_then(|row| row.get(c))
                    && let Some(slot) = colors.get_mut(*led as usize)
                {
                    *slot = color;
                }
            }
        }
    }
    colors
}

/// The per-key colour buffer: each Caps-layer binding's key in its colour, the delta
/// key itself white, everything else the Caps-layer background. Stock firmware
/// ignores per-key brightness, so a black background shows as white there.
fn key_colors(config: &Config, layer: &[Vec<u16>], led_map: &[Vec<Option<u8>>], led_count: u8) -> Vec<Hsv> {
    let mut colors = vec![rgb_to_hsv(lighting::background(config)); led_count as usize];
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
    for b in lighting::lit_bindings(config) {
        if let Some(code) = code_for_key_name(&b.key) {
            paint(code, rgb_to_hsv(lighting::binding_color(config, b)));
        }
    }
    if let Some(code) = code_for_key_name(&config.engine.modifier_key) {
        paint(code, rgb_to_hsv(lighting::DELTA_KEY));
    }
    colors
}

/// Whether the keyboard's lighting no longer matches `[lighting.normal]`. Brightness
/// isn't compared: the knob changes it on purpose.
fn drifted(now: &LightingState, n: &NormalLighting) -> bool {
    if now.effect != n.effect {
        return true;
    }
    let Some(rgb) = parse_hex_color(&n.color) else { return false };
    let want = rgb_to_hsv(rgb);
    // Hue means nothing for white
    n.effect != 0 && (now.sat != want.s || (want.s != 0 && now.hue != want.h))
}

/// Effect, colour and brightness from `[lighting.normal]` (RAM only).
fn apply_normal(via: &mut Via, n: &delta_core::config::NormalLighting) -> Result<(), ViaError> {
    if let Some(rgb) = parse_hex_color(&n.color) {
        let hsv = rgb_to_hsv(rgb);
        via.send(LightingCommand::Color { h: hsv.h, s: hsv.s })?;
    }
    via.send(LightingCommand::Effect(n.effect))?;
    via.send(LightingCommand::Brightness(n.brightness))
}

/// RGB → QMK's 0–255 HSV.
fn rgb_to_hsv((r, g, b): Rgb) -> Hsv {
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
    // Firmware that honours per-key value scales the keyboard's brightness by it
    Hsv { h: (hue / 360.0 * 255.0).round() as u8, s: (sat * 255.0).round() as u8, v: (max * 255.0).round() as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(rgb_to_hsv((255, 0, 0)), Hsv { h: 0, s: 255, v: 255 });
        assert_eq!(rgb_to_hsv((0, 0, 255)).h, 170);
        assert_eq!(rgb_to_hsv((255, 255, 255)).s, 0);
        assert_eq!(rgb_to_hsv((0, 0, 0)).v, 0);
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
        assert_eq!(colors[0], rgb_to_hsv((0x58, 0x65, 0xF2)), "R is Discord blurple");
        assert_eq!(colors[1].v, 0, "base-layer bindings aren't shown");
        assert_eq!(colors[2], Hsv { h: 0, s: 0, v: 255 }, "the delta key is white");
    }

    #[test]
    fn background_fills_unbound_keys() {
        let mut config = Config::from_toml("[engine]\nmodifier_key = \"f20\"").unwrap();
        config.lighting.delta_background = Some("#FF0000".into());
        let colors = key_colors(&config, &[vec![0x6F, 0x15]], &[vec![Some(0), Some(1)]], 2);
        assert_eq!(colors[1], Hsv { h: 0, s: 255, v: 255 });
        assert_eq!(colors[0].s, 0, "the delta key stays white");
    }

    #[test]
    fn drift_ignores_brightness_and_white_hue() {
        let white = NormalLighting { effect: 1, color: "#FFFFFF".into(), brightness: 180 };
        let now = |effect, hue, sat| LightingState { effect, brightness: 79, hue, sat };
        assert!(!drifted(&now(1, 40, 0), &white), "hue doesn't matter for white");
        assert!(drifted(&now(1, 0, 255), &white), "EEPROM's red came back");
        assert!(drifted(&now(5, 0, 0), &white));
        let off = NormalLighting { effect: 0, ..white };
        assert!(!drifted(&now(0, 0, 255), &off));
    }
}
