//! Keyboard definitions (VIA JSON) and their physical layouts.
//!
//! Firmware knows keycodes per matrix position, not where keys sit. VIA definition
//! files fill that in with a KLE (keyboard-layout-editor.com) layout whose key
//! labels carry the matrix position: label 0 is `row,col`, label 8 is a layout
//! option `group,choice`, label 9 marks a knob as `e<id>`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Definitions shipped with Delta (from github.com/the-via/keyboards, GPL-3.0).
const BUILTIN: &[&str] = &[include_str!("../../../layouts/keychron/q5_ansi_encoder.json")];

#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub rows: u8,
    pub cols: u8,
    /// Names for the keyboard's own keycodes, in `QK_KB_0…` order.
    pub custom_keycodes: Vec<String>,
    pub keys: Vec<PhysicalKey>,
}

/// One key cap (or knob) and where it is, in key units (1u = one standard key).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalKey {
    pub row: u8,
    pub col: u8,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// This is knob `n` (its press is `row,col`).
    pub encoder: Option<u8>,
    /// Only shown when layout option `group` is set to `choice`.
    pub option: Option<(u8, u8)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViaJson {
    name: String,
    vendor_id: String,
    product_id: String,
    matrix: Matrix,
    #[serde(default)]
    custom_keycodes: Vec<CustomKeycode>,
    layouts: Layouts,
}

#[derive(Deserialize)]
struct Matrix {
    rows: u8,
    cols: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CustomKeycode {
    name: String,
    short_name: Option<String>,
}

#[derive(Deserialize)]
struct Layouts {
    keymap: Vec<Vec<Value>>,
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("not a VIA definition: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bad {field} {value:?}")]
    Field { field: &'static str, value: String },
}

impl Definition {
    pub fn parse(json: &str) -> Result<Self, LayoutError> {
        let via: ViaJson = serde_json::from_str(json)?;
        let hex = |field: &'static str, value: &str| {
            u16::from_str_radix(value.trim_start_matches("0x"), 16)
                .map_err(|_| LayoutError::Field { field, value: value.into() })
        };
        Ok(Self {
            vendor_id: hex("vendorId", &via.vendor_id)?,
            product_id: hex("productId", &via.product_id)?,
            name: via.name,
            rows: via.matrix.rows,
            cols: via.matrix.cols,
            custom_keycodes: via.custom_keycodes.into_iter().map(|k| k.short_name.unwrap_or(k.name)).collect(),
            keys: parse_kle(&via.layouts.keymap)?,
        })
    }

    /// The built-in definition for a USB ID, or one from `extra_dir` (`*.json`).
    pub fn find(vendor_id: u16, product_id: u16, extra_dir: Option<&Path>) -> Option<Self> {
        let extra: Vec<String> = extra_dir
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .collect();
        extra
            .iter()
            .map(String::as_str)
            .chain(BUILTIN.iter().copied())
            .filter_map(|json| Self::parse(json).ok())
            .find(|d| d.vendor_id == vendor_id && d.product_id == product_id)
    }

    /// How many knobs the layout has.
    pub fn encoder_count(&self) -> u8 {
        self.keys.iter().filter_map(|k| k.encoder).map(|e| e + 1).max().unwrap_or(0)
    }
}

/// Walk KLE rows. Property objects change the next key (`x`, `y` offsets, `w`, `h`);
/// after each key the cursor moves right by its width; each row starts one unit down.
fn parse_kle(rows: &[Vec<Value>]) -> Result<Vec<PhysicalKey>, LayoutError> {
    let mut keys = Vec::new();
    let mut y = 0.0;
    for row in rows {
        let mut x = 0.0;
        let (mut w, mut h) = (1.0, 1.0);
        for item in row {
            match item {
                Value::Object(props) => {
                    let num = |k: &str| props.get(k).and_then(Value::as_f64);
                    x += num("x").unwrap_or(0.0);
                    y += num("y").unwrap_or(0.0);
                    w = num("w").unwrap_or(w);
                    h = num("h").unwrap_or(h);
                }
                Value::String(label) => {
                    let labels: Vec<&str> = label.split('\n').collect();
                    let (row, col) = pair(labels[0])
                        .ok_or_else(|| LayoutError::Field { field: "matrix label", value: label.clone() })?;
                    let encoder = labels.get(9).and_then(|l| l.strip_prefix('e')).and_then(|n| n.parse().ok());
                    let option = labels.get(8).and_then(|l| pair(l));
                    keys.push(PhysicalKey { row, col, x, y, w, h, encoder, option });
                    x += w;
                    (w, h) = (1.0, 1.0);
                }
                _ => {}
            }
        }
        y += 1.0;
    }
    Ok(keys)
}

/// `"3,12"` → (3, 12)
fn pair(s: &str) -> Option<(u8, u8)> {
    let (a, b) = s.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q5() -> Definition {
        Definition::find(0x3434, 0x0151, None).expect("Q5 definition is built in")
    }

    #[test]
    fn q5_definition_loads() {
        let d = q5();
        assert_eq!(d.name, "Keychron Q5 ANSI Knob");
        assert_eq!((d.rows, d.cols), (6, 18));
        assert_eq!(d.encoder_count(), 1);
        assert_eq!(d.custom_keycodes[0], "MCtrl");
    }

    #[test]
    fn q5_positions() {
        let d = q5();
        let at = |r, c| d.keys.iter().find(|k| (k.row, k.col) == (r, c) && k.encoder.is_none()).unwrap();
        // Escape top-left; F1 after a 1u gap
        assert_eq!((at(0, 0).x, at(0, 0).y), (0.0, 0.0));
        assert_eq!(at(0, 2).x, 2.0);
        // Number row is pushed down 0.25u; Backspace is 2u wide
        assert_eq!(at(1, 0).y, 1.25);
        assert_eq!(at(1, 13).w, 2.0);
        // Knob sits at the end of the top row
        let knob = d.keys.iter().find(|k| k.encoder == Some(0)).unwrap();
        assert_eq!((knob.row, knob.col, knob.y), (3, 12, 0.0));
        // Space bar
        assert_eq!(at(5, 6).w, 6.25);
    }

    #[test]
    fn every_matrix_position_is_unique() {
        let d = q5();
        let mut seen = std::collections::HashSet::new();
        for k in d.keys.iter().filter(|k| k.encoder.is_none()) {
            assert!(seen.insert((k.row, k.col)), "duplicate {:?}", (k.row, k.col));
            assert!(k.row < d.rows && k.col < d.cols);
        }
    }
}
