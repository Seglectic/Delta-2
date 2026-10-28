//! What colour each key gets: per-app lighting profiles, and the Caps layer while the
//! delta key is held. Shared by deltad (which lights the keyboard) and the app (which
//! previews it).

use crate::config::{Action, Binding, Config, KeyEffect, KeyLight, Layer, LightingProfile, parse_hex_color};

pub type Rgb = (u8, u8, u8);

/// Colour of the delta key itself while held.
pub const DELTA_KEY: Rgb = (0xFF, 0xFF, 0xFF);

/// Bindings that light up: Caps layer, not limited to one app.
pub fn lit_bindings(config: &Config) -> impl Iterator<Item = &Binding> {
    config.bindings.iter().filter(|b| b.layer == Layer::Delta && b.only_in.is_none())
}

/// Colour for keys without a Caps-layer action; black (off) when unset.
pub fn background(config: &Config) -> Rgb {
    config.lighting.delta_background.as_deref().and_then(parse_hex_color).unwrap_or((0, 0, 0))
}

/// The binding's own colour, else its app's, else a default for the kind of action.
pub fn binding_color(config: &Config, b: &Binding) -> Rgb {
    let hex = b.color.clone().or_else(|| match &b.action {
        Action::App { app } => config.app(app).and_then(|a| a.color.clone()).or_else(|| known_app_color(app)),
        _ => None,
    });
    if let Some(rgb) = hex.as_deref().and_then(parse_hex_color) {
        return rgb;
    }
    match &b.action {
        Action::Tile { .. } => (0x89, 0xB4, 0xFA),                      // blue
        Action::Span { .. } | Action::Fixed { .. } => (0x94, 0xE2, 0xD5), // teal
        Action::Keys { .. } => (0xF9, 0xE2, 0xAF),                      // yellow
        Action::Command { .. } => (0xFA, 0xB3, 0x87),                   // peach
        Action::KwinShortcut { .. } => (0xCB, 0xA6, 0xF7),              // mauve
        // Apps without a colour get a stable hue from their name
        Action::App { app } => hue_to_rgb(name_hue(app)),
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

/// A fully saturated colour from a 0–255 hue.
fn hue_to_rgb(hue: u8) -> Rgb {
    let h = hue as f32 / 255.0 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u8 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let byte = |v: f32| (v * 255.0).round() as u8;
    (byte(r), byte(g), byte(b))
}

/// The profile for the focused window `(class, desktop file)`: its app's, else the
/// everywhere one.
pub fn active_profile<'a>(config: &'a Config, focus: Option<(&str, &str)>) -> Option<&'a LightingProfile> {
    let app = focus.and_then(|(class, desktop)| config.app_for_window(class, desktop)).map(|a| a.name.as_str());
    let profiles = &config.lighting.profiles;
    app.and_then(|name| profiles.iter().find(|p| p.app.as_deref() == Some(name)))
        .or_else(|| profiles.iter().find(|p| p.app.is_none()))
}

/// A key light's colour with its brightness applied: what the LED shows at full
/// keyboard brightness.
pub fn key_light_rgb(k: &KeyLight) -> Rgb {
    let (r, g, b) = parse_hex_color(&k.color).unwrap_or((0, 0, 0));
    let scale = |c: u8| ((c as u16 * k.brightness as u16 + 127) / 255) as u8;
    (scale(r), scale(g), scale(b))
}

/// How strongly a burst shows `dist` key widths from its origin when `t` (0–1) of the
/// way through: a soft ring at `t * distance`, dimmed by `fade` and scaled by
/// `strength`. The app's preview (`effects.ts`) draws with the same formula.
pub fn burst_amount(e: &KeyEffect, dist: f32, t: f32) -> f32 {
    if !(0.0..=1.0).contains(&t) {
        return 0.0;
    }
    let x = (dist - t * e.distance) / e.width;
    (-x * x).exp() * (1.0 - t).powf(e.fade) * e.strength as f32 / 255.0
}

/// The burst's colour at `dist`: `color` at the origin to `color_end` at `distance`.
pub fn burst_color(e: &KeyEffect, dist: f32) -> Rgb {
    let start = parse_hex_color(&e.color).unwrap_or((255, 255, 255));
    let end = e.color_end.as_deref().and_then(parse_hex_color).unwrap_or(start);
    mix(start, end, (dist / e.distance).clamp(0.0, 1.0))
}

/// `a` (0–1) of the way from `from` to `to`.
pub fn mix(from: Rgb, to: Rgb, a: f32) -> Rgb {
    let a = a.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * a).round() as u8;
    (ch(from.0, to.0), ch(from.1, to.1), ch(from.2, to.2))
}

/// Effects that apply for the focused window: its app's and the everywhere ones.
pub fn active_effects<'a>(config: &'a Config, focus: Option<(&str, &str)>) -> impl Iterator<Item = &'a KeyEffect> {
    let app = focus.and_then(|(class, desktop)| config.app_for_window(class, desktop)).map(|a| a.name.clone());
    config.lighting.effects.iter().filter(move |e| e.app.is_none() || e.app == app)
}

pub fn to_hex((r, g, b): Rgb) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_profile_wins_over_everywhere() {
        let config = Config::from_toml(
            r##"
            [engine]
            modifier_key = "f20"
            [[apps]]
            name = "overwatch"
            launch = "steam steam://rungameid/2357570"
            match = ["steam_app_2357570"]
            [[lighting.profiles]]
            background = { color = "#112233" }
            [[lighting.profiles]]
            app = "overwatch"
            keys = [{ key = "KC_Q", color = "#0000FF", brightness = 128 }]
            "##,
        )
        .unwrap();
        config.validate().unwrap();
        let ow = active_profile(&config, Some(("steam_app_2357570", ""))).unwrap();
        assert_eq!(ow.app.as_deref(), Some("overwatch"));
        assert_eq!(key_light_rgb(&ow.keys[0]), (0, 0, 128));
        assert!(active_profile(&config, Some(("firefox", "firefox"))).unwrap().app.is_none());
        assert!(active_profile(&config, None).unwrap().app.is_none());
    }

    #[test]
    fn burst_ring_travels_and_fades() {
        let e: KeyEffect = toml::from_str(
            r##"
            id = "send"
            trigger = "KC_ENTER"
            color = "#FF0000"
            color_end = "#0000FF"
            "##,
        )
        .unwrap();
        // At the start the ring sits on the origin; halfway it's 4 keys out
        assert!(burst_amount(&e, 0.0, 0.0) > 0.99);
        assert!(burst_amount(&e, 4.0, 0.5) > burst_amount(&e, 0.0, 0.5));
        assert!(burst_amount(&e, 4.0, 0.5) < 0.51, "faded by half");
        assert_eq!(burst_amount(&e, 0.0, 1.5), 0.0, "over");
        assert_eq!(burst_color(&e, 0.0), (255, 0, 0));
        assert_eq!(burst_color(&e, 8.0), (0, 0, 255));
    }
}
