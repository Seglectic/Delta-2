//! Find VIA-capable keyboards and talk to them over `/dev/hidraw*`.
//!
//! A QMK keyboard with VIA exposes a raw-HID interface with usage page 0xFF60 and
//! usage 0x61. sysfs tells us each hidraw node's report descriptor and USB IDs, so
//! no HID library is needed.

use crate::protocol::{
    CHANNEL_RGB_MATRIX, Hsv, KeyboardValue, LightingCommand, MAX_BUFFER_CHUNK, MAX_COLORS_PER_WRITE, REPORT_LEN,
    ReadCommand, UNHANDLED, VALUE_BRIGHTNESS, VALUE_EFFECT, is_ram_only_write, is_read_only,
};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long to wait for the keyboard to answer one command.
const TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum ViaError {
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path}: no answer to {command} within {TIMEOUT:?}")]
    Timeout { path: String, command: String },
    #[error("{path}: keyboard doesn't support {command}")]
    Unsupported { path: String, command: String },
    #[error("{path}: keyboard rejected {command}")]
    Rejected { path: String, command: String },
    #[error("refusing to send {0}: not on the read or RAM-write allowlist")]
    Refused(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaDevice {
    pub path: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
}

/// Every hidraw node with the VIA raw-HID usage (page 0xFF60, usage 0x61).
pub fn discover() -> Vec<ViaDevice> {
    let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") else { return vec![] };
    let mut devices: Vec<ViaDevice> = entries
        .flatten()
        .filter_map(|entry| {
            let sys = entry.path().join("device");
            let descriptor = std::fs::read(sys.join("report_descriptor")).ok()?;
            if !is_via_descriptor(&descriptor) {
                return None;
            }
            let uevent = std::fs::read_to_string(sys.join("uevent")).ok()?;
            let (vendor_id, product_id) = parse_hid_id(&uevent)?;
            let name = uevent.lines().find_map(|l| l.strip_prefix("HID_NAME=")).unwrap_or("unknown").to_string();
            Some(ViaDevice { path: Path::new("/dev").join(entry.file_name()), vendor_id, product_id, name })
        })
        .collect();
    devices.sort_by(|a, b| a.path.cmp(&b.path));
    devices
}

/// The descriptor starts with Usage Page (0x06) 0xFF60 and Usage (0x09) 0x61.
fn is_via_descriptor(d: &[u8]) -> bool {
    d.windows(5).any(|w| w == [0x06, 0x60, 0xFF, 0x09, 0x61])
}

/// `HID_ID=0003:00003434:00000151` → (0x3434, 0x0151)
fn parse_hid_id(uevent: &str) -> Option<(u16, u16)> {
    let id = uevent.lines().find_map(|l| l.strip_prefix("HID_ID="))?;
    let mut parts = id.split(':').skip(1);
    let vid = u32::from_str_radix(parts.next()?, 16).ok()?;
    let pid = u32::from_str_radix(parts.next()?, 16).ok()?;
    Some((vid as u16, pid as u16))
}

/// An open connection to one keyboard's VIA interface.
pub struct Via {
    file: File,
    path: String,
    /// VIA protocol version, asked once (the switch-matrix format depends on it).
    protocol: Option<u16>,
}

impl Via {
    pub fn open(device: &ViaDevice) -> Result<Self, ViaError> {
        let path = device.path.display().to_string();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&device.path)
            .map_err(|source| ViaError::Io { path: path.clone(), source })?;
        Ok(Self { file, path, protocol: None })
    }

    /// Send one read command and return the keyboard's 32-byte answer.
    pub fn request(&mut self, cmd: ReadCommand) -> Result<[u8; REPORT_LEN], ViaError> {
        self.transact(cmd.encode(), cmd.echo_len(), &format!("{cmd:?}"))
    }

    /// Send one RAM-only lighting change. Keychron commands report success in byte 2.
    pub fn send(&mut self, cmd: LightingCommand) -> Result<(), ViaError> {
        let reply = self.transact(cmd.encode(), cmd.echo_len(), &format!("{cmd:?}"))?;
        if reply[0] == 0xA8 && reply[2] != 0 {
            return Err(ViaError::Rejected { path: self.path.clone(), command: format!("{cmd:?}") });
        }
        Ok(())
    }

    /// Write a packet and wait for the reply that echoes its first `echo` bytes. Other
    /// programs' replies (hidraw delivers every report to every reader) are skipped.
    fn transact(&mut self, packet: [u8; REPORT_LEN], echo: usize, label: &str) -> Result<[u8; REPORT_LEN], ViaError> {
        // Backstop for the typed commands: only allowlisted packets ever go out
        if !is_read_only(&packet) && !is_ram_only_write(&packet) {
            return Err(ViaError::Refused(label.into()));
        }
        let io = |source| ViaError::Io { path: self.path.clone(), source };

        // Drop anything left over from an earlier, timed-out request
        let mut stale = [0u8; REPORT_LEN];
        while self.file.read(&mut stale).is_ok_and(|n| n > 0) {}

        // hidraw wants the report ID first; VIA doesn't use report IDs, so it's 0
        let mut out = [0u8; REPORT_LEN + 1];
        out[1..].copy_from_slice(&packet);
        self.file.write_all(&out).map_err(io)?;

        let deadline = Instant::now() + TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !self.wait_readable(left).map_err(io)? {
                return Err(ViaError::Timeout { path: self.path.clone(), command: label.into() });
            }
            let mut reply = [0u8; REPORT_LEN];
            match self.file.read(&mut reply) {
                Ok(n) if n > 0 && reply[..echo] == packet[..echo] => return Ok(reply),
                // Unhandled replies keep the rest of the request, so match on it
                Ok(n) if n > 0 && reply[0] == UNHANDLED && reply[1..echo] == packet[1..echo] => {
                    return Err(ViaError::Unsupported { path: self.path.clone(), command: label.into() });
                }
                // Some other report (another program's reply, a vendor message): keep waiting
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(io(e)),
            }
        }
    }

    fn wait_readable(&self, timeout: Duration) -> std::io::Result<bool> {
        let mut pfd = libc::pollfd { fd: self.file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: pfd is a valid pollfd for the duration of the call
        let n = unsafe { libc::poll(&mut pfd, 1, ms) };
        if n < 0 { Err(std::io::Error::last_os_error()) } else { Ok(n > 0) }
    }

    /// VIA protocol version (12 = QMK 0.19+ keycodes).
    pub fn protocol_version(&mut self) -> Result<u16, ViaError> {
        let r = self.request(ReadCommand::ProtocolVersion)?;
        Ok(u16::from_be_bytes([r[1], r[2]]))
    }

    /// Selected layout options, packed as VIA stores them.
    pub fn layout_options(&mut self) -> Result<u32, ViaError> {
        let r = self.request(ReadCommand::KeyboardValue(KeyboardValue::LayoutOptions))?;
        Ok(u32::from_be_bytes([r[2], r[3], r[4], r[5]]))
    }

    /// Matrix positions `(row, col)` whose switches are held right now.
    pub fn pressed(&mut self, rows: u8, cols: u8) -> Result<Vec<(u8, u8)>, ViaError> {
        let protocol = match self.protocol {
            Some(p) => p,
            None => {
                let p = self.protocol_version()?;
                self.protocol = Some(p);
                p
            }
        };
        if protocol < 12 {
            let r = self.request(ReadCommand::SwitchMatrix { row: 0 })?;
            return Ok(decode_matrix(&r[2..], rows, cols));
        }
        // Protocol 12+: as many rows as fit in one reply, from a starting row
        let per_reply = (28 / (cols as usize).div_ceil(8).clamp(1, 4)) as u8;
        let mut pressed = Vec::new();
        for start in (0..rows).step_by(per_reply.max(1) as usize) {
            let r = self.request(ReadCommand::SwitchMatrix { row: start })?;
            let count = per_reply.min(rows - start);
            pressed.extend(decode_matrix(&r[3..], count, cols).into_iter().map(|(row, col)| (row + start, col)));
        }
        Ok(pressed)
    }

    /// The raw macro buffer: `count` NUL-terminated macros, in QMK's encoding.
    pub fn macros(&mut self) -> Result<(u8, Vec<u8>), ViaError> {
        let count = self.request(ReadCommand::MacroCount)?[1];
        let r = self.request(ReadCommand::MacroBufferSize)?;
        let total = u16::from_be_bytes([r[1], r[2]]) as usize;
        let mut bytes = Vec::with_capacity(total);
        while bytes.len() < total {
            let size = (total - bytes.len()).min(MAX_BUFFER_CHUNK as usize) as u8;
            let r = self.request(ReadCommand::MacroBuffer { offset: bytes.len() as u16, size })?;
            bytes.extend_from_slice(&r[4..4 + size as usize]);
        }
        Ok((count, bytes))
    }

    /// LED count when the keyboard runs Keychron firmware with per-key RGB, else None.
    pub fn keychron_rgb(&mut self) -> Result<Option<u8>, ViaError> {
        match self.request(ReadCommand::KeychronProtocol) {
            Ok(_) => {}
            Err(ViaError::Unsupported { .. }) => return Ok(None),
            Err(e) => return Err(e),
        }
        let r = match self.request(ReadCommand::KeychronLedCount) {
            Ok(r) => r,
            Err(ViaError::Unsupported { .. }) => return Ok(None),
            Err(e) => return Err(e),
        };
        Ok((r[2] == 0 && r[3] > 0).then_some(r[3]))
    }

    /// Keychron firmware version string, e.g. `v1.1.0 2026-10-27-16:54:49`.
    pub fn keychron_firmware_version(&mut self) -> Result<String, ViaError> {
        let r = self.request(ReadCommand::KeychronFirmwareVersion)?;
        let text = r[1..].split(|b| *b == 0).next().unwrap_or_default();
        Ok(String::from_utf8_lossy(text).into_owned())
    }

    /// Keychron firmware: the base layer in use (Mac/Windows switch) and the highest
    /// active layer right now.
    pub fn keychron_layers(&mut self) -> Result<(u8, u8), ViaError> {
        let r = self.request(ReadCommand::KeychronDefaultLayer)?;
        Ok((r[1], r[2]))
    }

    /// LED index of each matrix position (`[row][col]`), None where there's no LED.
    pub fn led_map(&mut self, rows: u8, cols: u8) -> Result<Vec<Vec<Option<u8>>>, ViaError> {
        (0..rows)
            .map(|row| {
                let r = self.request(ReadCommand::KeychronLedIndex { row })?;
                Ok((0..cols as usize).map(|c| r.get(3 + c).copied().filter(|i| *i != 0xFF)).collect())
            })
            .collect()
    }

    /// RGB matrix `(effect, brightness)`; effect 0 means the lights are off.
    pub fn lighting(&mut self) -> Result<(u8, u8), ViaError> {
        let get = |via: &mut Self, value| -> Result<u8, ViaError> {
            Ok(via.request(ReadCommand::LightingValue { channel: CHANNEL_RGB_MATRIX, value })?[3])
        };
        Ok((get(self, VALUE_EFFECT)?, get(self, VALUE_BRIGHTNESS)?))
    }

    /// Replace the per-key colours (RAM only), 9 LEDs per write. `colors` must cover
    /// exactly `led_count` LEDs: Keychron's firmware doesn't bounds-check writes.
    pub fn set_per_key_colors(&mut self, colors: &[Hsv], led_count: u8) -> Result<(), ViaError> {
        if colors.len() != led_count as usize {
            return Err(ViaError::Invalid(format!("{} colours for {led_count} LEDs", colors.len())));
        }
        for (i, chunk) in colors.chunks(MAX_COLORS_PER_WRITE).enumerate() {
            let mut buf = [Hsv::default(); MAX_COLORS_PER_WRITE];
            buf[..chunk.len()].copy_from_slice(chunk);
            let start = (i * MAX_COLORS_PER_WRITE) as u8;
            self.send(LightingCommand::PerKeyColors { start, count: chunk.len() as u8, colors: buf })?;
        }
        Ok(())
    }

    pub fn layer_count(&mut self) -> Result<u8, ViaError> {
        Ok(self.request(ReadCommand::LayerCount)?[1])
    }

    /// The whole dynamic keymap: `[layer][row][col]` keycodes.
    pub fn keymap(&mut self, rows: u8, cols: u8) -> Result<Vec<Vec<Vec<u16>>>, ViaError> {
        let layers = self.layer_count()?;
        let total = layers as usize * rows as usize * cols as usize * 2;
        let mut bytes = Vec::with_capacity(total);
        while bytes.len() < total {
            let size = (total - bytes.len()).min(MAX_BUFFER_CHUNK as usize) as u8;
            let r = self.request(ReadCommand::KeymapBuffer { offset: bytes.len() as u16, size })?;
            bytes.extend_from_slice(&r[4..4 + size as usize]);
        }
        let codes: Vec<u16> = bytes.chunks_exact(2).map(|b| u16::from_be_bytes([b[0], b[1]])).collect();
        Ok(codes
            .chunks(rows as usize * cols as usize)
            .map(|layer| layer.chunks(cols as usize).map(<[u16]>::to_vec).collect())
            .collect())
    }

    /// `(counter-clockwise, clockwise)` keycodes for each encoder on each layer.
    pub fn encoders(&mut self, layers: u8, encoders: u8) -> Result<Vec<Vec<(u16, u16)>>, ViaError> {
        (0..layers)
            .map(|layer| {
                (0..encoders)
                    .map(|encoder| {
                        let mut turn = |clockwise| -> Result<u16, ViaError> {
                            let r = self.request(ReadCommand::Encoder { layer, encoder, clockwise })?;
                            Ok(u16::from_be_bytes([r[4], r[5]]))
                        };
                        Ok((turn(false)?, turn(true)?))
                    })
                    .collect()
            })
            .collect()
    }
}

/// QMK sends each row as a big-endian bit field, 1–4 bytes wide depending on the
/// column count; bit `c` is column `c`.
fn decode_matrix(data: &[u8], rows: u8, cols: u8) -> Vec<(u8, u8)> {
    let width = (cols as usize).div_ceil(8).clamp(1, 4);
    let mut pressed = Vec::new();
    for row in 0..rows {
        let start = row as usize * width;
        let Some(bytes) = data.get(start..start + width) else { break };
        let bits = bytes.iter().fold(0u32, |acc, b| acc << 8 | *b as u32);
        pressed.extend((0..cols).filter(|c| bits & (1 << c) != 0).map(|c| (row, c)));
    }
    pressed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_matrix_rows() {
        // 6 rows × 18 cols = 3 bytes per row. Row 0: col 0 and col 17; row 3: col 12
        let mut data = [0u8; 18];
        data[0..3].copy_from_slice(&[0x02, 0x00, 0x01]);
        data[9..12].copy_from_slice(&[0x00, 0x10, 0x00]);
        assert_eq!(decode_matrix(&data, 6, 18), [(0, 0), (0, 17), (3, 12)]);
    }

    #[test]
    fn parses_hid_id() {
        assert_eq!(parse_hid_id("DRIVER=hid-generic\nHID_ID=0003:00003434:00000151\n"), Some((0x3434, 0x0151)));
    }

    #[test]
    fn recognises_via_descriptor() {
        // Start of the Q5's raw-HID descriptor
        assert!(is_via_descriptor(&[0x06, 0x60, 0xFF, 0x09, 0x61, 0xA1, 0x01]));
        // A normal keyboard descriptor (Generic Desktop / Keyboard)
        assert!(!is_via_descriptor(&[0x05, 0x01, 0x09, 0x06, 0xA1, 0x01]));
    }
}
