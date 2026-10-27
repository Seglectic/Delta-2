//! VIA and Keychron raw-HID commands: reads, plus a handful of RAM-only lighting writes.
//!
//! Every message is a 32-byte report: byte 0 is the command ID, the rest are
//! arguments; the keyboard echoes the command back with the answer filled in.
//!
//! What can be sent is limited by type (see plan decision 0004):
//! - [`ReadCommand`]: reads only.
//! - [`LightingCommand`]: lighting changes that live in RAM (QMK's `*_noeeprom` and
//!   Keychron's per-key buffer). They're gone after a replug, and nothing here can
//!   send a save, a keymap write or a bootloader jump.
//!
//! [`is_read_only`] and [`is_ram_only_write`] are the allowlists the tests check every
//! encodable packet against.

/// VIA reports are always 32 bytes.
pub const REPORT_LEN: usize = 32;

/// The keyboard answers this command ID for anything it doesn't support.
pub const UNHANDLED: u8 = 0xFF;

/// Most data a single `KeymapBuffer` read returns (32 bytes minus the 4-byte header).
pub const MAX_BUFFER_CHUNK: u8 = 28;

/// Most per-key colours one `PerKeyColors` write carries (Keychron's limit).
pub const MAX_COLORS_PER_WRITE: usize = 9;

/// Command IDs from QMK's `quantum/via.h` and Keychron's `keychron_raw_hid.h`.
mod id {
    pub const GET_PROTOCOL_VERSION: u8 = 0x01;
    pub const GET_KEYBOARD_VALUE: u8 = 0x02;
    pub const DYNAMIC_KEYMAP_GET_KEYCODE: u8 = 0x04;
    pub const CUSTOM_SET_VALUE: u8 = 0x07;
    pub const CUSTOM_GET_VALUE: u8 = 0x08;
    pub const DYNAMIC_KEYMAP_MACRO_GET_COUNT: u8 = 0x0C;
    pub const DYNAMIC_KEYMAP_MACRO_GET_BUFFER_SIZE: u8 = 0x0D;
    pub const DYNAMIC_KEYMAP_MACRO_GET_BUFFER: u8 = 0x0E;
    pub const DYNAMIC_KEYMAP_GET_LAYER_COUNT: u8 = 0x11;
    pub const DYNAMIC_KEYMAP_GET_BUFFER: u8 = 0x12;
    pub const DYNAMIC_KEYMAP_GET_ENCODER: u8 = 0x14;
    pub const KC_GET_PROTOCOL_VERSION: u8 = 0xA0;
    pub const KC_GET_FIRMWARE_VERSION: u8 = 0xA1;
    pub const KC_GET_DEFAULT_LAYER: u8 = 0xA3;
    pub const KC_KEYCHRON_RGB: u8 = 0xA8;
}

/// Keychron RGB subcommands (`keyboards/keychron/common/rgb/keychron_rgb.c`).
mod rgb {
    pub const GET_PROTOCOL_VER: u8 = 0x01;
    pub const GET_LED_COUNT: u8 = 0x05;
    pub const GET_LED_IDX: u8 = 0x06;
    pub const PER_KEY_GET_TYPE: u8 = 0x07;
    pub const PER_KEY_SET_TYPE: u8 = 0x08;
    pub const PER_KEY_SET_COLOR: u8 = 0x0A;
}

/// VIA lighting channel for RGB matrix, and its values.
pub const CHANNEL_RGB_MATRIX: u8 = 0x03;
pub const VALUE_BRIGHTNESS: u8 = 0x01;
pub const VALUE_EFFECT: u8 = 0x02;
pub const VALUE_COLOR: u8 = 0x04;

/// Whether a packet is a read this crate may send.
pub fn is_read_only(p: &[u8]) -> bool {
    match p[0] {
        id::GET_PROTOCOL_VERSION
        | id::DYNAMIC_KEYMAP_GET_KEYCODE
        | id::CUSTOM_GET_VALUE
        | id::DYNAMIC_KEYMAP_MACRO_GET_COUNT
        | id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER_SIZE
        | id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER
        | id::DYNAMIC_KEYMAP_GET_LAYER_COUNT
        | id::DYNAMIC_KEYMAP_GET_BUFFER
        | id::DYNAMIC_KEYMAP_GET_ENCODER
        | id::KC_GET_PROTOCOL_VERSION
        | id::KC_GET_FIRMWARE_VERSION
        | id::KC_GET_DEFAULT_LAYER => true,
        id::GET_KEYBOARD_VALUE => matches!(p[1], 0x01..=0x04),
        id::KC_KEYCHRON_RGB => {
            matches!(p[1], rgb::GET_PROTOCOL_VER | rgb::GET_LED_COUNT | rgb::GET_LED_IDX | rgb::PER_KEY_GET_TYPE)
        }
        _ => false,
    }
}

/// Whether a packet is one of the RAM-only lighting writes this crate may send.
pub fn is_ram_only_write(p: &[u8]) -> bool {
    match p[0] {
        // QMK applies VIA lighting sets with *_noeeprom; only CUSTOM_SAVE (0x09) persists
        id::CUSTOM_SET_VALUE => {
            p[1] == CHANNEL_RGB_MATRIX && matches!(p[2], VALUE_BRIGHTNESS | VALUE_EFFECT | VALUE_COLOR)
        }
        // Keychron keeps per-key type and colours in RAM until RGB_SAVE (0x02)
        id::KC_KEYCHRON_RGB => matches!(p[1], rgb::PER_KEY_SET_TYPE | rgb::PER_KEY_SET_COLOR),
        _ => false,
    }
}

/// Values readable through `GET_KEYBOARD_VALUE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardValue {
    Uptime = 0x01,
    LayoutOptions = 0x02,
    /// Which switches are held right now (VIA's matrix tester). VIA protocol 12+
    /// takes a starting row in byte 2 and answers from byte 3; older firmware answers
    /// from byte 2 and ignores the extra byte.
    SwitchMatrixState = 0x03,
    FirmwareVersion = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadCommand {
    ProtocolVersion,
    KeyboardValue(KeyboardValue),
    /// Switch matrix state from `row` on (VIA protocol 12+; row 0 for older firmware).
    SwitchMatrix {
        row: u8,
    },
    /// One key's keycode.
    Keycode {
        layer: u8,
        row: u8,
        col: u8,
    },
    MacroCount,
    MacroBufferSize,
    /// A slice of the macro buffer (NUL-separated macro strings).
    MacroBuffer {
        offset: u16,
        size: u8,
    },
    LayerCount,
    /// A slice of the whole keymap: layers × rows × cols big-endian u16 keycodes.
    KeymapBuffer {
        offset: u16,
        size: u8,
    },
    /// What a knob sends when turned on a layer.
    Encoder {
        layer: u8,
        encoder: u8,
        clockwise: bool,
    },
    /// A VIA lighting value, e.g. RGB matrix brightness or effect.
    LightingValue {
        channel: u8,
        value: u8,
    },
    /// Keychron firmware: their protocol version (old firmware answers 0xFF).
    KeychronProtocol,
    /// Keychron firmware: version string, e.g. `v1.1.0 2026-10-27-16:54:49`.
    KeychronFirmwareVersion,
    /// Keychron firmware: default (base) layer and highest active layer.
    KeychronDefaultLayer,
    /// Keychron per-key RGB: protocol version.
    KeychronRgbProtocol,
    /// Keychron per-key RGB: number of LEDs.
    KeychronLedCount,
    /// Keychron per-key RGB: LED index for each column of a matrix row.
    KeychronLedIndex {
        row: u8,
    },
    /// Keychron per-key RGB: current per-key mode (solid, breathing, …).
    KeychronPerKeyType,
}

impl ReadCommand {
    pub fn encode(&self) -> [u8; REPORT_LEN] {
        let mut r = [0u8; REPORT_LEN];
        let mut put = |bytes: &[u8]| r[..bytes.len()].copy_from_slice(bytes);
        match *self {
            ReadCommand::ProtocolVersion => put(&[id::GET_PROTOCOL_VERSION]),
            ReadCommand::KeyboardValue(v) => put(&[id::GET_KEYBOARD_VALUE, v as u8]),
            ReadCommand::SwitchMatrix { row } => {
                put(&[id::GET_KEYBOARD_VALUE, KeyboardValue::SwitchMatrixState as u8, row])
            }
            ReadCommand::Keycode { layer, row, col } => put(&[id::DYNAMIC_KEYMAP_GET_KEYCODE, layer, row, col]),
            ReadCommand::MacroCount => put(&[id::DYNAMIC_KEYMAP_MACRO_GET_COUNT]),
            ReadCommand::MacroBufferSize => put(&[id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER_SIZE]),
            ReadCommand::MacroBuffer { offset, size } => {
                let [hi, lo] = offset.to_be_bytes();
                put(&[id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER, hi, lo, size.min(MAX_BUFFER_CHUNK)]);
            }
            ReadCommand::LayerCount => put(&[id::DYNAMIC_KEYMAP_GET_LAYER_COUNT]),
            ReadCommand::KeymapBuffer { offset, size } => {
                let [hi, lo] = offset.to_be_bytes();
                put(&[id::DYNAMIC_KEYMAP_GET_BUFFER, hi, lo, size.min(MAX_BUFFER_CHUNK)]);
            }
            ReadCommand::Encoder { layer, encoder, clockwise } => {
                put(&[id::DYNAMIC_KEYMAP_GET_ENCODER, layer, encoder, clockwise as u8]);
            }
            ReadCommand::LightingValue { channel, value } => put(&[id::CUSTOM_GET_VALUE, channel, value]),
            ReadCommand::KeychronProtocol => put(&[id::KC_GET_PROTOCOL_VERSION]),
            ReadCommand::KeychronFirmwareVersion => put(&[id::KC_GET_FIRMWARE_VERSION]),
            ReadCommand::KeychronDefaultLayer => put(&[id::KC_GET_DEFAULT_LAYER]),
            ReadCommand::KeychronRgbProtocol => put(&[id::KC_KEYCHRON_RGB, rgb::GET_PROTOCOL_VER]),
            ReadCommand::KeychronLedCount => put(&[id::KC_KEYCHRON_RGB, rgb::GET_LED_COUNT]),
            // Ask about every column: the 3-byte little-endian column mask is all ones
            ReadCommand::KeychronLedIndex { row } => {
                put(&[id::KC_KEYCHRON_RGB, rgb::GET_LED_IDX, row, 0xFF, 0xFF, 0xFF])
            }
            ReadCommand::KeychronPerKeyType => put(&[id::KC_KEYCHRON_RGB, rgb::PER_KEY_GET_TYPE]),
        }
        r
    }

    /// How many leading bytes of the reply echo the request (to tell replies apart).
    pub fn echo_len(&self) -> usize {
        echo_len(&self.encode())
    }
}

fn echo_len(p: &[u8]) -> usize {
    match p[0] {
        id::CUSTOM_GET_VALUE | id::CUSTOM_SET_VALUE => 3,
        id::KC_KEYCHRON_RGB | id::GET_KEYBOARD_VALUE => 2,
        _ => 1,
    }
}

/// An HSV colour as QMK stores it, each part 0–255.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hsv {
    pub h: u8,
    pub s: u8,
    pub v: u8,
}

/// Lighting changes that only live in the keyboard's RAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingCommand {
    /// RGB matrix brightness (QMK `rgb_matrix_sethsv_noeeprom`).
    Brightness(u8),
    /// RGB matrix effect; 0 turns the matrix off (QMK `rgb_matrix_mode_noeeprom`).
    Effect(u8),
    /// RGB matrix hue and saturation (QMK `rgb_matrix_sethsv_noeeprom`).
    Color { h: u8, s: u8 },
    /// Keychron per-key mode: 0 solid, 1 breathing, 2–4 reactive.
    PerKeyType(u8),
    /// Keychron per-key colours for `count` LEDs from `start` (at most 9 per write).
    PerKeyColors { start: u8, count: u8, colors: [Hsv; MAX_COLORS_PER_WRITE] },
}

impl LightingCommand {
    pub fn encode(&self) -> [u8; REPORT_LEN] {
        let mut r = [0u8; REPORT_LEN];
        match *self {
            LightingCommand::Brightness(v) => {
                r[..4].copy_from_slice(&[id::CUSTOM_SET_VALUE, CHANNEL_RGB_MATRIX, VALUE_BRIGHTNESS, v])
            }
            LightingCommand::Effect(e) => {
                r[..4].copy_from_slice(&[id::CUSTOM_SET_VALUE, CHANNEL_RGB_MATRIX, VALUE_EFFECT, e])
            }
            LightingCommand::Color { h, s } => {
                r[..5].copy_from_slice(&[id::CUSTOM_SET_VALUE, CHANNEL_RGB_MATRIX, VALUE_COLOR, h, s])
            }
            LightingCommand::PerKeyType(t) => r[..3].copy_from_slice(&[id::KC_KEYCHRON_RGB, rgb::PER_KEY_SET_TYPE, t]),
            LightingCommand::PerKeyColors { start, count, colors } => {
                let count = count.min(MAX_COLORS_PER_WRITE as u8);
                r[..4].copy_from_slice(&[id::KC_KEYCHRON_RGB, rgb::PER_KEY_SET_COLOR, start, count]);
                for (i, c) in colors[..count as usize].iter().enumerate() {
                    r[4 + i * 3..7 + i * 3].copy_from_slice(&[c.h, c.s, c.v]);
                }
            }
        }
        r
    }

    pub fn echo_len(&self) -> usize {
        echo_len(&self.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every read shape, including edge values.
    fn every_read() -> Vec<ReadCommand> {
        vec![
            ReadCommand::ProtocolVersion,
            ReadCommand::KeyboardValue(KeyboardValue::Uptime),
            ReadCommand::KeyboardValue(KeyboardValue::LayoutOptions),
            ReadCommand::KeyboardValue(KeyboardValue::SwitchMatrixState),
            ReadCommand::KeyboardValue(KeyboardValue::FirmwareVersion),
            ReadCommand::SwitchMatrix { row: 9 },
            ReadCommand::Keycode { layer: 255, row: 255, col: 255 },
            ReadCommand::MacroCount,
            ReadCommand::MacroBufferSize,
            ReadCommand::MacroBuffer { offset: 0, size: 28 },
            ReadCommand::LayerCount,
            ReadCommand::KeymapBuffer { offset: u16::MAX, size: u8::MAX },
            ReadCommand::Encoder { layer: 3, encoder: 1, clockwise: true },
            ReadCommand::LightingValue { channel: CHANNEL_RGB_MATRIX, value: VALUE_EFFECT },
            ReadCommand::KeychronProtocol,
            ReadCommand::KeychronFirmwareVersion,
            ReadCommand::KeychronDefaultLayer,
            ReadCommand::KeychronRgbProtocol,
            ReadCommand::KeychronLedCount,
            ReadCommand::KeychronLedIndex { row: 5 },
            ReadCommand::KeychronPerKeyType,
        ]
    }

    fn every_write() -> Vec<LightingCommand> {
        vec![
            LightingCommand::Brightness(255),
            LightingCommand::Effect(0),
            LightingCommand::Color { h: 170, s: 255 },
            LightingCommand::PerKeyType(0),
            LightingCommand::PerKeyColors { start: 88, count: 9, colors: [Hsv { h: 1, s: 2, v: 3 }; 9] },
        ]
    }

    /// QMK/Keychron commands that persist settings, change the keymap or reboot.
    /// None may ever leave this crate.
    const DANGEROUS: &[&[u8]] = &[
        &[0x03],       // VIA set keyboard value
        &[0x05],       // set keycode
        &[0x06],       // keymap reset
        &[0x09],       // VIA custom save (lighting to EEPROM)
        &[0x0A],       // EEPROM reset
        &[0x0B],       // bootloader jump
        &[0x0F],       // macro set buffer
        &[0x10],       // macro reset
        &[0x13],       // keymap set buffer
        &[0x15],       // set encoder
        &[0xA8, 0x02], // Keychron RGB save
        &[0xA8, 0x04], // Keychron set indicator config
        &[0xA8, 0x0D], // Keychron set mixed regions
        &[0xA8, 0x0F], // Keychron set mixed effect list
        &[0xAA],       // Keychron wireless DFU
        &[0xAB],       // Keychron factory test
    ];

    fn is_dangerous(p: &[u8]) -> bool {
        DANGEROUS.iter().any(|d| p.starts_with(d))
    }

    #[test]
    fn reads_are_reads() {
        for cmd in every_read() {
            let p = cmd.encode();
            assert!(is_read_only(&p), "{cmd:?} isn't on the read allowlist: {:02x?}", &p[..4]);
            assert!(!is_ram_only_write(&p) && !is_dangerous(&p), "{cmd:?} encodes a write");
        }
    }

    #[test]
    fn lighting_writes_are_ram_only() {
        for cmd in every_write() {
            let p = cmd.encode();
            assert!(is_ram_only_write(&p), "{cmd:?} isn't on the RAM-write allowlist: {:02x?}", &p[..4]);
            assert!(!is_read_only(&p) && !is_dangerous(&p), "{cmd:?} encodes something else");
        }
    }

    #[test]
    fn allowlists_reject_every_dangerous_command() {
        for d in DANGEROUS {
            let mut p = [0u8; REPORT_LEN];
            p[..d.len()].copy_from_slice(d);
            assert!(!is_read_only(&p) && !is_ram_only_write(&p), "{d:02x?} passes an allowlist");
        }
        // VIA lighting set on another channel or value (e.g. speed) isn't allowed either
        assert!(!is_ram_only_write(&[0x07, 0x03, 0x03, 0, 0]));
        assert!(!is_ram_only_write(&[0x07, 0x02, 0x01, 0, 0]));
    }

    #[test]
    fn buffer_reads_are_capped_to_one_report() {
        let r = ReadCommand::KeymapBuffer { offset: 0x0102, size: 200 }.encode();
        assert_eq!(&r[..4], &[0x12, 0x01, 0x02, MAX_BUFFER_CHUNK]);
    }

    #[test]
    fn per_key_colors_layout() {
        let mut colors = [Hsv::default(); 9];
        colors[0] = Hsv { h: 10, s: 20, v: 30 };
        colors[1] = Hsv { h: 40, s: 50, v: 60 };
        let p = LightingCommand::PerKeyColors { start: 7, count: 2, colors }.encode();
        assert_eq!(&p[..10], &[0xA8, 0x0A, 7, 2, 10, 20, 30, 40, 50, 60]);
    }
}
