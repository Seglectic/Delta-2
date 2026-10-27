//! Read buttons, keys and wheels from Linux input devices **without grabbing them**.
//!
//! Every other program (KDE, xremap) keeps receiving the same events, and pointer
//! motion is never read at all, so listening can't change how a mouse feels.
//! Reading `/dev/input/event*` needs the `input` group.

use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Hi-res wheel units per notch (kernel convention).
const HI_RES_PER_NOTCH: f64 = 120.0;

#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    pub path: PathBuf,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// Moves a pointer (has relative X/Y).
    pub pointer: bool,
    /// Created by software (xremap's output device, for example), not hardware.
    pub virtual_device: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A key or button went down or up. `name` is the kernel's: `BTN_SIDE`, `KEY_F21`.
    Button { code: u16, name: String, pressed: bool },
    /// The wheel moved; `notches` is fractional on hi-res wheels (G502: 1/8 steps).
    Wheel { axis: Axis, notches: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// Every input device we can open.
pub fn list() -> Vec<InputDevice> {
    let mut devices: Vec<InputDevice> = evdev::enumerate().map(|(path, dev)| describe(path, &dev)).collect();
    devices.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    devices
}

fn describe(path: PathBuf, dev: &Device) -> InputDevice {
    let id = dev.input_id();
    let pointer = dev
        .supported_relative_axes()
        .is_some_and(|axes| axes.contains(RelativeAxisCode::REL_X) && axes.contains(RelativeAxisCode::REL_Y));
    InputDevice {
        path,
        name: dev.name().unwrap_or("unknown").to_string(),
        vendor_id: id.vendor(),
        product_id: id.product(),
        pointer,
        // Hardware has a physical path (usb-…); uinput devices usually don't
        virtual_device: dev.physical_path().is_none_or(str::is_empty),
    }
}

/// Kernel name for a key/button code, e.g. 275 → `BTN_SIDE`, 191 → `KEY_F21`.
pub fn key_name(code: u16) -> String {
    format!("{:?}", KeyCode::new(code))
}

/// Read events from one device until `on_event` returns false or the device goes
/// away. Blocks, so run it on its own thread. Never grabs the device.
pub fn listen(path: &Path, mut on_event: impl FnMut(Event) -> bool) -> std::io::Result<()> {
    let mut dev = Device::open(path)?;
    // With a hi-res wheel the kernel sends both; use the finer one only
    let hi_res = dev.supported_relative_axes().is_some_and(|a| a.contains(RelativeAxisCode::REL_WHEEL_HI_RES));
    loop {
        for ev in dev.fetch_events()? {
            let event = match ev.destructure() {
                // value 2 is auto-repeat; only real presses and releases matter
                EventSummary::Key(_, code, value @ (0 | 1)) => {
                    Event::Button { code: code.code(), name: format!("{code:?}"), pressed: value == 1 }
                }
                EventSummary::RelativeAxis(_, axis, value) => {
                    let (axis, notches) = match axis {
                        RelativeAxisCode::REL_WHEEL_HI_RES => (Axis::Vertical, value as f64 / HI_RES_PER_NOTCH),
                        RelativeAxisCode::REL_HWHEEL_HI_RES => (Axis::Horizontal, value as f64 / HI_RES_PER_NOTCH),
                        RelativeAxisCode::REL_WHEEL if !hi_res => (Axis::Vertical, value as f64),
                        RelativeAxisCode::REL_HWHEEL if !hi_res => (Axis::Horizontal, value as f64),
                        // Pointer motion and everything else is ignored on purpose
                        _ => continue,
                    };
                    Event::Wheel { axis, notches }
                }
                _ => continue,
            };
            if !on_event(event) {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_kernel() {
        assert_eq!(key_name(275), "BTN_SIDE");
        assert_eq!(key_name(191), "KEY_F21");
        assert_eq!(key_name(272), "BTN_LEFT");
    }
}
