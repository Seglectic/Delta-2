//! Read-only access to QMK keyboards over VIA's raw-HID protocol, plus the VIA
//! definitions that say where each key physically sits.

pub mod device;
pub mod keycode;
pub mod layout;
pub mod library;
pub mod protocol;

pub use device::{LightingState, Via, ViaDevice, ViaError, discover};
pub use layout::{Definition, PhysicalKey};
pub use protocol::{Hsv, LightingCommand};
