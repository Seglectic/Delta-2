//! VIA raw-HID commands, read-only.
//!
//! Every VIA message is a 32-byte report: byte 0 is the command ID, the rest are
//! arguments; the keyboard echoes the command ID back with the answer filled in.
//! Only read commands exist in [`ReadCommand`], so this crate can't change a
//! keyboard's keymap, settings or EEPROM (see plan decision 0004).

/// VIA reports are always 32 bytes.
pub const REPORT_LEN: usize = 32;

/// The keyboard answers this command ID for anything it doesn't support.
pub const UNHANDLED: u8 = 0xFF;

/// Most data a single `KeymapBuffer` read returns (32 bytes minus the 4-byte header).
pub const MAX_BUFFER_CHUNK: u8 = 28;

/// Command IDs from QMK's `quantum/via.h`, read commands only.
mod id {
    pub const GET_PROTOCOL_VERSION: u8 = 0x01;
    pub const GET_KEYBOARD_VALUE: u8 = 0x02;
    pub const DYNAMIC_KEYMAP_GET_KEYCODE: u8 = 0x04;
    pub const DYNAMIC_KEYMAP_MACRO_GET_COUNT: u8 = 0x0C;
    pub const DYNAMIC_KEYMAP_MACRO_GET_BUFFER_SIZE: u8 = 0x0D;
    pub const DYNAMIC_KEYMAP_MACRO_GET_BUFFER: u8 = 0x0E;
    pub const DYNAMIC_KEYMAP_GET_LAYER_COUNT: u8 = 0x11;
    pub const DYNAMIC_KEYMAP_GET_BUFFER: u8 = 0x12;
    pub const DYNAMIC_KEYMAP_GET_ENCODER: u8 = 0x14;
}

/// Every command ID this crate is allowed to send.
pub const READ_ONLY_IDS: &[u8] = &[
    id::GET_PROTOCOL_VERSION,
    id::GET_KEYBOARD_VALUE,
    id::DYNAMIC_KEYMAP_GET_KEYCODE,
    id::DYNAMIC_KEYMAP_MACRO_GET_COUNT,
    id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER_SIZE,
    id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER,
    id::DYNAMIC_KEYMAP_GET_LAYER_COUNT,
    id::DYNAMIC_KEYMAP_GET_BUFFER,
    id::DYNAMIC_KEYMAP_GET_ENCODER,
];

/// Values readable through `GET_KEYBOARD_VALUE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardValue {
    Uptime = 0x01,
    LayoutOptions = 0x02,
    /// Which switches are held right now (VIA's matrix tester).
    SwitchMatrixState = 0x03,
    FirmwareVersion = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadCommand {
    ProtocolVersion,
    KeyboardValue(KeyboardValue),
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
}

impl ReadCommand {
    pub fn id(&self) -> u8 {
        match self {
            ReadCommand::ProtocolVersion => id::GET_PROTOCOL_VERSION,
            ReadCommand::KeyboardValue(_) => id::GET_KEYBOARD_VALUE,
            ReadCommand::Keycode { .. } => id::DYNAMIC_KEYMAP_GET_KEYCODE,
            ReadCommand::MacroCount => id::DYNAMIC_KEYMAP_MACRO_GET_COUNT,
            ReadCommand::MacroBufferSize => id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER_SIZE,
            ReadCommand::MacroBuffer { .. } => id::DYNAMIC_KEYMAP_MACRO_GET_BUFFER,
            ReadCommand::LayerCount => id::DYNAMIC_KEYMAP_GET_LAYER_COUNT,
            ReadCommand::KeymapBuffer { .. } => id::DYNAMIC_KEYMAP_GET_BUFFER,
            ReadCommand::Encoder { .. } => id::DYNAMIC_KEYMAP_GET_ENCODER,
        }
    }

    pub fn encode(&self) -> [u8; REPORT_LEN] {
        let mut r = [0u8; REPORT_LEN];
        r[0] = self.id();
        match *self {
            ReadCommand::KeyboardValue(v) => r[1] = v as u8,
            ReadCommand::Keycode { layer, row, col } => r[1..4].copy_from_slice(&[layer, row, col]),
            ReadCommand::KeymapBuffer { offset, size } | ReadCommand::MacroBuffer { offset, size } => {
                r[1..3].copy_from_slice(&offset.to_be_bytes());
                r[3] = size.min(MAX_BUFFER_CHUNK);
            }
            ReadCommand::Encoder { layer, encoder, clockwise } => {
                r[1..4].copy_from_slice(&[layer, encoder, clockwise as u8]);
            }
            ReadCommand::ProtocolVersion
            | ReadCommand::MacroCount
            | ReadCommand::MacroBufferSize
            | ReadCommand::LayerCount => {}
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every command shape, including edge values.
    fn every_command() -> Vec<ReadCommand> {
        vec![
            ReadCommand::ProtocolVersion,
            ReadCommand::KeyboardValue(KeyboardValue::Uptime),
            ReadCommand::KeyboardValue(KeyboardValue::LayoutOptions),
            ReadCommand::KeyboardValue(KeyboardValue::SwitchMatrixState),
            ReadCommand::KeyboardValue(KeyboardValue::FirmwareVersion),
            ReadCommand::Keycode { layer: 255, row: 255, col: 255 },
            ReadCommand::MacroCount,
            ReadCommand::MacroBufferSize,
            ReadCommand::MacroBuffer { offset: 0, size: 28 },
            ReadCommand::LayerCount,
            ReadCommand::KeymapBuffer { offset: u16::MAX, size: u8::MAX },
            ReadCommand::Encoder { layer: 3, encoder: 1, clockwise: true },
        ]
    }

    #[test]
    fn only_read_commands_can_be_encoded() {
        // QMK's write/reset/bootloader IDs. None may ever leave this crate.
        let writes = [0x03, 0x05, 0x06, 0x07, 0x09, 0x0A, 0x0B, 0x0F, 0x10, 0x13, 0x15];
        for cmd in every_command() {
            let id = cmd.encode()[0];
            assert!(READ_ONLY_IDS.contains(&id), "{cmd:?} encodes non-read id {id:#04x}");
            assert!(!writes.contains(&id), "{cmd:?} encodes write id {id:#04x}");
        }
    }

    #[test]
    fn keyboard_value_never_encodes_a_set() {
        // GET_KEYBOARD_VALUE with these arguments is harmless; SET_KEYBOARD_VALUE is 0x03.
        let r = ReadCommand::KeyboardValue(KeyboardValue::LayoutOptions).encode();
        assert_eq!(&r[..2], &[0x02, 0x02]);
    }

    #[test]
    fn buffer_reads_are_capped_to_one_report() {
        let r = ReadCommand::KeymapBuffer { offset: 0x0102, size: 200 }.encode();
        assert_eq!(&r[..4], &[0x12, 0x01, 0x02, MAX_BUFFER_CHUNK]);
    }
}
