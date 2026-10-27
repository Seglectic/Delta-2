//! Read-only access to QMK keyboards over VIA's raw-HID protocol, plus the VIA
//! definitions that say where each key physically sits.

pub mod device;
pub mod keycode;
pub mod layout;
pub mod protocol;

pub use device::{Via, ViaDevice, ViaError, discover};
pub use layout::{Definition, PhysicalKey};
