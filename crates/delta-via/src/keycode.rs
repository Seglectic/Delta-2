//! Turn QMK keycodes (VIA protocol 12, QMK 0.19+) into names.
//!
//! Ranges follow QMK's `quantum/keycodes.h`. Anything not covered here is shown
//! as hex rather than guessed.

/// Basic HID keycodes 0x00–0xE7 as `(code, QMK name without KC_, short label)`.
const BASIC: &[(u16, &str, &str)] = &[
    (0x00, "NO", ""),
    (0x01, "TRANSPARENT", "▽"),
    (0x28, "ENTER", "Enter"),
    (0x29, "ESCAPE", "Esc"),
    (0x2A, "BACKSPACE", "Bksp"),
    (0x2B, "TAB", "Tab"),
    (0x2C, "SPACE", "Space"),
    (0x2D, "MINUS", "-"),
    (0x2E, "EQUAL", "="),
    (0x2F, "LEFT_BRACKET", "["),
    (0x30, "RIGHT_BRACKET", "]"),
    (0x31, "BACKSLASH", "\\"),
    (0x32, "NONUS_HASH", "#"),
    (0x33, "SEMICOLON", ";"),
    (0x34, "QUOTE", "'"),
    (0x35, "GRAVE", "`"),
    (0x36, "COMMA", ","),
    (0x37, "DOT", "."),
    (0x38, "SLASH", "/"),
    (0x39, "CAPS_LOCK", "Caps"),
    (0x46, "PRINT_SCREEN", "PrtSc"),
    (0x47, "SCROLL_LOCK", "ScrLk"),
    (0x48, "PAUSE", "Pause"),
    (0x49, "INSERT", "Ins"),
    (0x4A, "HOME", "Home"),
    (0x4B, "PAGE_UP", "PgUp"),
    (0x4C, "DELETE", "Del"),
    (0x4D, "END", "End"),
    (0x4E, "PAGE_DOWN", "PgDn"),
    (0x4F, "RIGHT", "→"),
    (0x50, "LEFT", "←"),
    (0x51, "DOWN", "↓"),
    (0x52, "UP", "↑"),
    (0x53, "NUM_LOCK", "NumLk"),
    (0x54, "KP_SLASH", "/"),
    (0x55, "KP_ASTERISK", "*"),
    (0x56, "KP_MINUS", "-"),
    (0x57, "KP_PLUS", "+"),
    (0x58, "KP_ENTER", "Enter"),
    (0x62, "KP_0", "0"),
    (0x63, "KP_DOT", "."),
    (0x64, "NONUS_BACKSLASH", "\\"),
    (0x65, "APPLICATION", "Menu"),
    (0x66, "KB_POWER", "Power"),
    (0x67, "KP_EQUAL", "="),
    (0x7F, "KB_MUTE", "Mute"),
    (0x80, "KB_VOLUME_UP", "Vol+"),
    (0x81, "KB_VOLUME_DOWN", "Vol-"),
    (0x85, "KP_COMMA", ","),
    (0xA5, "SYSTEM_POWER", "Power"),
    (0xA6, "SYSTEM_SLEEP", "Sleep"),
    (0xA7, "SYSTEM_WAKE", "Wake"),
    (0xA8, "AUDIO_MUTE", "Mute"),
    (0xA9, "AUDIO_VOL_UP", "Vol+"),
    (0xAA, "AUDIO_VOL_DOWN", "Vol-"),
    (0xAB, "MEDIA_NEXT_TRACK", "Next"),
    (0xAC, "MEDIA_PREV_TRACK", "Prev"),
    (0xAD, "MEDIA_STOP", "Stop"),
    (0xAE, "MEDIA_PLAY_PAUSE", "Play"),
    (0xAF, "MEDIA_SELECT", "Media"),
    (0xB0, "MEDIA_EJECT", "Eject"),
    (0xB1, "MAIL", "Mail"),
    (0xB2, "CALCULATOR", "Calc"),
    (0xB3, "MY_COMPUTER", "PC"),
    (0xB4, "WWW_SEARCH", "Search"),
    (0xB5, "WWW_HOME", "Home"),
    (0xB6, "WWW_BACK", "Back"),
    (0xB7, "WWW_FORWARD", "Fwd"),
    (0xB8, "WWW_STOP", "Stop"),
    (0xB9, "WWW_REFRESH", "Refresh"),
    (0xBA, "WWW_FAVORITES", "Fav"),
    (0xBB, "MEDIA_FAST_FORWARD", "FFwd"),
    (0xBC, "MEDIA_REWIND", "Rew"),
    (0xBD, "BRIGHTNESS_UP", "Bri+"),
    (0xBE, "BRIGHTNESS_DOWN", "Bri-"),
    (0xCD, "MS_UP", "M↑"),
    (0xCE, "MS_DOWN", "M↓"),
    (0xCF, "MS_LEFT", "M←"),
    (0xD0, "MS_RIGHT", "M→"),
    (0xD1, "MS_BTN1", "Click"),
    (0xD2, "MS_BTN2", "RClick"),
    (0xD3, "MS_BTN3", "MClick"),
    (0xD4, "MS_BTN4", "MB4"),
    (0xD5, "MS_BTN5", "MB5"),
    (0xD6, "MS_BTN6", "MB6"),
    (0xD7, "MS_BTN7", "MB7"),
    (0xD8, "MS_BTN8", "MB8"),
    (0xD9, "MS_WH_UP", "Wh↑"),
    (0xDA, "MS_WH_DOWN", "Wh↓"),
    (0xDB, "MS_WH_LEFT", "Wh←"),
    (0xDC, "MS_WH_RIGHT", "Wh→"),
    (0xDD, "MS_ACCEL0", "Acc0"),
    (0xDE, "MS_ACCEL1", "Acc1"),
    (0xDF, "MS_ACCEL2", "Acc2"),
    (0xE0, "LEFT_CTRL", "Ctrl"),
    (0xE1, "LEFT_SHIFT", "Shift"),
    (0xE2, "LEFT_ALT", "Alt"),
    (0xE3, "LEFT_GUI", "Super"),
    (0xE4, "RIGHT_CTRL", "RCtrl"),
    (0xE5, "RIGHT_SHIFT", "RShift"),
    (0xE6, "RIGHT_ALT", "RAlt"),
    (0xE7, "RIGHT_GUI", "RSuper"),
];

/// A decoded keycode: QMK-style name (`KC_F13`, `MO(1)`) and a short key-cap label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keycode {
    pub code: u16,
    pub name: String,
    pub label: String,
}

impl Keycode {
    fn new(code: u16, name: impl Into<String>, label: impl Into<String>) -> Self {
        Self { code, name: name.into(), label: label.into() }
    }
}

/// `custom` are the keyboard's own keycodes (VIA definition `customKeycodes`),
/// numbered from QMK's `QK_KB_0` (0x7E00). Older Keychron factory firmware (VIA
/// protocol 11, seen on a Q5 in 2026) numbers them from 0x7F00 instead, so that
/// range is read the same way.
pub fn decode(code: u16, custom: &[String]) -> Keycode {
    match code {
        0x0000..=0x00FF => basic(code),
        0x0100..=0x1FFF => {
            let inner = basic(code & 0xFF);
            let mods = mod_names(((code >> 8) & 0x1F) as u8);
            Keycode::new(
                code,
                format!("{}({})", mods.join("|"), inner.name),
                format!("{}+{}", mods.join("+"), inner.label),
            )
        }
        0x2000..=0x3FFF => {
            let inner = basic(code & 0xFF);
            let mods = mod_names(((code >> 8) & 0x1F) as u8);
            Keycode::new(
                code,
                format!("MT({}, {})", mods.join("|"), inner.name),
                format!("{}/{}", inner.label, mods.join("+")),
            )
        }
        0x4000..=0x4FFF => {
            let inner = basic(code & 0xFF);
            let layer = (code >> 8) & 0x0F;
            Keycode::new(code, format!("LT({layer}, {})", inner.name), format!("{}/L{layer}", inner.label))
        }
        0x5200..=0x52FF => layer_key(code),
        0x7820..=0x782A => lighting(code),
        0x7700..=0x777F => Keycode::new(code, format!("QK_MACRO_{}", code - 0x7700), format!("M{}", code - 0x7700)),
        0x7C00 => Keycode::new(code, "QK_BOOT", "Boot"),
        0x7C01 => Keycode::new(code, "QK_REBOOT", "Reboot"),
        0x7E00..=0x7E3F | 0x7F00..=0x7F3F => {
            let n = (code & 0x3F) as usize;
            match custom.get(n) {
                Some(name) => Keycode::new(code, format!("QK_KB_{n}"), name.clone()),
                None => Keycode::new(code, format!("QK_KB_{n}"), format!("KB{n}")),
            }
        }
        0x7E40..=0x7E7F => Keycode::new(code, format!("QK_USER_{}", code - 0x7E40), format!("U{}", code - 0x7E40)),
        _ => Keycode::new(code, format!("{code:#06x}"), format!("{code:#06x}")),
    }
}

fn basic(code: u16) -> Keycode {
    if let Some(&(_, name, label)) = BASIC.iter().find(|(c, ..)| *c == code) {
        return Keycode::new(code, format!("KC_{name}"), label);
    }
    let (name, label) = match code {
        0x04..=0x1D => {
            let c = (b'A' + (code - 0x04) as u8) as char;
            (c.to_string(), c.to_string())
        }
        0x1E..=0x27 => {
            let d = ((code - 0x1E + 1) % 10).to_string();
            (d.clone(), d)
        }
        0x3A..=0x45 => (format!("F{}", code - 0x3A + 1), format!("F{}", code - 0x3A + 1)),
        0x68..=0x73 => (format!("F{}", code - 0x68 + 13), format!("F{}", code - 0x68 + 13)),
        0x59..=0x61 => (format!("KP_{}", code - 0x59 + 1), (code - 0x59 + 1).to_string()),
        _ => return Keycode::new(code, format!("{code:#06x}"), format!("{code:#04x}")),
    };
    Keycode::new(code, format!("KC_{name}"), label)
}

/// Mod bits as QMK packs them: 1 Ctrl, 2 Shift, 4 Alt, 8 GUI, 0x10 = right-hand.
fn mod_names(bits: u8) -> Vec<&'static str> {
    let right = bits & 0x10 != 0;
    [(0x01, "Ctrl"), (0x02, "Shift"), (0x04, "Alt"), (0x08, "Super")]
        .into_iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| if right { right_mod(name) } else { name })
        .collect()
}

fn right_mod(name: &str) -> &'static str {
    match name {
        "Ctrl" => "RCtrl",
        "Shift" => "RShift",
        "Alt" => "RAlt",
        _ => "RSuper",
    }
}

/// RGB keycodes (QMK `QK_UNDERGLOW_*`, which also drive RGB matrix boards).
fn lighting(code: u16) -> Keycode {
    let (name, label) = match code {
        0x7820 => ("RGB_TOG", "RGB"),
        0x7821 => ("RGB_MOD", "RGB→"),
        0x7822 => ("RGB_RMOD", "RGB←"),
        0x7823 => ("RGB_HUI", "Hue+"),
        0x7824 => ("RGB_HUD", "Hue-"),
        0x7825 => ("RGB_SAI", "Sat+"),
        0x7826 => ("RGB_SAD", "Sat-"),
        0x7827 => ("RGB_VAI", "Bri+"),
        0x7828 => ("RGB_VAD", "Bri-"),
        0x7829 => ("RGB_SPI", "Spd+"),
        _ => ("RGB_SPD", "Spd-"),
    };
    Keycode::new(code, name, label)
}

fn layer_key(code: u16) -> Keycode {
    let layer = code & 0x1F;
    let (name, label) = match code & 0xFFE0 {
        0x5200 => ("TO", "TO"),
        0x5220 => ("MO", "MO"),
        0x5240 => ("DF", "DF"),
        0x5260 => ("TG", "TG"),
        0x5280 => ("OSL", "OSL"),
        0x52C0 => ("TT", "TT"),
        0x52E0 => ("PDF", "PDF"),
        _ => return Keycode::new(code, format!("{code:#06x}"), format!("{code:#06x}")),
    };
    Keycode::new(code, format!("{name}({layer})"), format!("{label}{layer}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(code: u16) -> String {
        decode(code, &[]).name
    }

    #[test]
    fn basic_keys() {
        assert_eq!(name(0x04), "KC_A");
        assert_eq!(name(0x1E), "KC_1");
        assert_eq!(name(0x27), "KC_0");
        assert_eq!(name(0x3A), "KC_F1");
        assert_eq!(name(0x45), "KC_F12");
        assert_eq!(name(0x59), "KC_KP_1");
        assert_eq!(name(0x29), "KC_ESCAPE");
    }

    #[test]
    fn q5_extra_keys_from_the_monitor_test() {
        // What libinput showed on 2026-10-27: top-right F13-F15, knob F16/F17/F18,
        // Num Lock F19, Caps Lock F20
        let f: Vec<String> = (0x68..=0x6F).map(name).collect();
        assert_eq!(f, ["KC_F13", "KC_F14", "KC_F15", "KC_F16", "KC_F17", "KC_F18", "KC_F19", "KC_F20"]);
    }

    #[test]
    fn layers_mods_and_custom() {
        assert_eq!(name(0x5221), "MO(1)");
        assert_eq!(name(0x5262), "TG(2)");
        assert_eq!(name(0x0104), "Ctrl(KC_A)");
        assert_eq!(name(0x1204), "RShift(KC_A)");
        assert_eq!(name(0x4129), "LT(1, KC_ESCAPE)");
        assert_eq!(decode(0x7E01, &["Mission Control".into(), "Launch Pad".into()]).label, "Launch Pad");
        // Legacy Keychron numbering, from the Q5's Mac layer bottom row
        let q5 = ["MCtrl", "LPad", "LOpt", "ROpt", "LCmd", "RCmd"].map(String::from);
        assert_eq!(decode(0x7F02, &q5).label, "LOpt");
        assert_eq!(decode(0x7F04, &q5).label, "LCmd");
        assert_eq!(name(0x7C00), "QK_BOOT");
    }
}
