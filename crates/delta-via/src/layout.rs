//! Keyboard definitions (VIA JSON) and their physical layouts.
//!
//! Firmware knows keycodes per matrix position, not where keys sit. VIA definitions
//! fill that in. They come in two shapes:
//!
//! - **compiled**: what usevia.app serves by USB ID (`layouts.keys`, positions already
//!   worked out). Delta fetches these automatically; see [`Definition::load`].
//! - **source**: the files in github.com/the-via/keyboards (`layouts.keymap`, a KLE
//!   layout whose key labels carry the matrix position: label 0 `row,col`, label 8 a
//!   layout option `group,choice`, label 9 a knob `e<id>`). Accepted for files you
//!   drop in yourself.

use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const VIA_DEFINITIONS: &str = "https://usevia.app/definitions/v3";

#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub rows: u8,
    pub cols: u8,
    /// Short names for the keyboard's own keycodes, in `QK_KB_0…` order.
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
    /// Rotation in degrees around (`rx`, `ry`).
    pub r: f64,
    pub rx: f64,
    pub ry: f64,
    /// This is knob `n` (its press is `row,col`).
    pub encoder: Option<u8>,
    /// Only exists when layout option `group` is set to `choice`.
    pub option: Option<(u8, u8)>,
    /// Key-cap colour role from the definition: `alpha`, `mod` or `accent`.
    pub color: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("not a VIA definition: {0}")]
    Json(#[from] serde_json::Error),
    #[error("VIA definition is missing {0}")]
    Missing(&'static str),
    #[error("bad {field} {value:?}")]
    Field { field: &'static str, value: String },
    #[error("no layout for {vendor_id:04x}:{product_id:04x} ({reason})")]
    NotFound { vendor_id: u16, product_id: u16, reason: String },
}

/// VIA identifies boards by `vendor_id << 16 | product_id`.
pub fn vendor_product_id(vendor_id: u16, product_id: u16) -> u32 {
    (vendor_id as u32) << 16 | product_id as u32
}

impl Definition {
    pub fn parse(json: &str) -> Result<Self, LayoutError> {
        let v: Value = serde_json::from_str(json)?;
        let name = v["name"].as_str().ok_or(LayoutError::Missing("name"))?.to_string();

        let (vendor_id, product_id) = match v.get("vendorProductId").and_then(Value::as_u64) {
            Some(id) => ((id >> 16) as u16, id as u16),
            None => (hex(&v, "vendorId")?, hex(&v, "productId")?),
        };
        let dim = |k: &'static str| v["matrix"][k].as_u64().map(|n| n as u8).ok_or(LayoutError::Missing("matrix"));

        let custom_keycodes = v["customKeycodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| k["shortName"].as_str().or(k["name"].as_str()).map(String::from))
            .collect();

        let layouts = &v["layouts"];
        let keys = if layouts.get("keys").is_some() {
            compiled_keys(layouts)?
        } else if let Some(rows) = layouts.get("keymap").and_then(Value::as_array) {
            kle_keys(rows)?
        } else {
            return Err(LayoutError::Missing("layouts.keys or layouts.keymap"));
        };

        Ok(Self { name, vendor_id, product_id, rows: dim("rows")?, cols: dim("cols")?, custom_keycodes, keys })
    }

    /// Find a keyboard's definition, in order:
    /// 1. `user_dir/*.json` (your own files win),
    /// 2. `cache_dir/<vid>_<pid>.json`,
    /// 3. usevia.app, saved into `cache_dir` (only when `fetch` is true).
    pub fn load(
        vendor_id: u16,
        product_id: u16,
        user_dir: &Path,
        cache_dir: &Path,
        fetch: bool,
    ) -> Result<Self, LayoutError> {
        let matches = |d: &Definition| d.vendor_id == vendor_id && d.product_id == product_id;

        let user = std::fs::read_dir(user_dir).into_iter().flatten().flatten();
        for entry in user.filter(|e| e.path().extension().is_some_and(|x| x == "json")) {
            if let Some(d) = std::fs::read_to_string(entry.path()).ok().and_then(|j| Self::parse(&j).ok())
                && matches(&d)
            {
                return Ok(d);
            }
        }

        let cached = cache_dir.join(format!("{vendor_id:04x}_{product_id:04x}.json"));
        if let Some(d) = std::fs::read_to_string(&cached).ok().and_then(|j| Self::parse(&j).ok()) {
            return Ok(d);
        }

        let not_found = |reason: String| LayoutError::NotFound { vendor_id, product_id, reason };
        if !fetch {
            return Err(not_found("not cached".into()));
        }
        let url = format!("{VIA_DEFINITIONS}/{}.json", vendor_product_id(vendor_id, product_id));
        let json = ureq::get(&url)
            .call()
            .and_then(|mut r| r.body_mut().read_to_string())
            .map_err(|e| not_found(format!("{url}: {e}")))?;
        let def = Self::parse(&json)?;
        if !matches(&def) {
            return Err(not_found(format!("{url} describes {:04x}:{:04x}", def.vendor_id, def.product_id)));
        }
        // Caching is best-effort; a read-only cache dir just means fetching again
        let _ = std::fs::create_dir_all(cache_dir).and_then(|()| std::fs::write(&cached, &json));
        Ok(def)
    }

    /// How many knobs the layout has.
    pub fn encoder_count(&self) -> u8 {
        self.keys.iter().filter_map(|k| k.encoder).map(|e| e + 1).max().unwrap_or(0)
    }
}

fn hex(v: &Value, field: &'static str) -> Result<u16, LayoutError> {
    let s = v[field].as_str().ok_or(LayoutError::Missing(field))?;
    u16::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|_| LayoutError::Field { field, value: s.into() })
}

/// Compiled layout: `keys` are always present; `optionKeys` holds extra keys per
/// `group → choice`.
fn compiled_keys(layouts: &Value) -> Result<Vec<PhysicalKey>, LayoutError> {
    let parse = |k: &Value, option| -> Result<PhysicalKey, LayoutError> {
        let num = |f: &str| k[f].as_f64();
        let pos = |f: &'static str| num(f).map(|n| n as u8).ok_or(LayoutError::Missing("key row/col"));
        Ok(PhysicalKey {
            row: pos("row")?,
            col: pos("col")?,
            x: num("x").unwrap_or(0.0),
            y: num("y").unwrap_or(0.0),
            w: num("w").unwrap_or(1.0),
            h: num("h").unwrap_or(1.0),
            r: num("r").unwrap_or(0.0),
            rx: num("rx").unwrap_or(0.0),
            ry: num("ry").unwrap_or(0.0),
            encoder: num("ei").map(|n| n as u8),
            option,
            color: k["color"].as_str().map(String::from),
        })
    };

    let mut keys = Vec::new();
    for k in layouts["keys"].as_array().ok_or(LayoutError::Missing("layouts.keys"))? {
        // Decals are labels drawn on the board, not keys
        if k["d"].as_bool() != Some(true) {
            keys.push(parse(k, None)?);
        }
    }
    for (group, choices) in layouts["optionKeys"].as_object().into_iter().flatten() {
        for (choice, list) in choices.as_object().into_iter().flatten() {
            let option = group.parse().ok().zip(choice.parse().ok());
            for k in list.as_array().into_iter().flatten() {
                keys.push(parse(k, option)?);
            }
        }
    }
    Ok(keys)
}

/// KLE source layout. Property objects change the next key (`x`, `y` offsets, `w`,
/// `h`, rotation `r`/`rx`/`ry`); after each key the cursor moves right by its width;
/// each row starts one unit down (back at `rx` when rotated).
fn kle_keys(rows: &[Value]) -> Result<Vec<PhysicalKey>, LayoutError> {
    let mut keys = Vec::new();
    let (mut y, mut r, mut rx, mut ry) = (0.0, 0.0, 0.0, 0.0);
    for row in rows {
        let Some(row) = row.as_array() else { continue };
        let mut x = rx;
        let (mut w, mut h) = (1.0, 1.0);
        for item in row {
            match item {
                Value::Object(props) => {
                    let num = |k: &str| props.get(k).and_then(Value::as_f64);
                    if let Some(v) = num("r") {
                        r = v;
                    }
                    if num("rx").is_some() || num("ry").is_some() {
                        rx = num("rx").unwrap_or(rx);
                        ry = num("ry").unwrap_or(ry);
                        (x, y) = (rx, ry);
                    }
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
                    keys.push(PhysicalKey { row, col, x, y, w, h, r, rx, ry, encoder, option, color: None });
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

    /// A small made-up board in source (KLE) form: 1.25u gap row, a 2u key, a
    /// tall key, a knob, and a two-choice layout option.
    const KLE: &str = r#"{
        "name": "Test Board", "vendorId": "0x1234", "productId": "0x0001",
        "matrix": {"rows": 3, "cols": 4},
        "customKeycodes": [{"name": "Mission Control", "shortName": "MCtrl"}, {"name": "Launch"}],
        "layouts": {"keymap": [
            ["0,0", {"x": 1}, "0,1", "0,2\n\n\n\n\n\n\n\n\ne0"],
            [{"y": 0.25, "w": 2}, "1,0", {"h": 2}, "1,2"],
            ["2,0\n\n\n\n\n\n\n\n0,0", {"w": 2}, "2,0\n\n\n\n\n\n\n\n0,1"]
        ]}
    }"#;

    /// The same board as usevia.app would serve it, compiled.
    const COMPILED: &str = r#"{
        "name": "Test Board", "vendorProductId": 305397761,
        "matrix": {"rows": 3, "cols": 4},
        "customKeycodes": [{"name": "Mission Control", "shortName": "MCtrl"}, {"name": "Launch"}],
        "layouts": {"width": 4, "height": 3.25, "keys": [
            {"row": 0, "col": 0, "x": 0, "y": 0, "w": 1, "h": 1, "r": 0, "rx": 0, "ry": 0, "color": "accent"},
            {"row": 0, "col": 1, "x": 2, "y": 0, "w": 1, "h": 1, "r": 0, "rx": 0, "ry": 0, "color": "alpha"},
            {"row": 0, "col": 2, "x": 3, "y": 0, "w": 1, "h": 1, "r": 0, "rx": 0, "ry": 0, "ei": 0, "color": "mod"},
            {"row": 1, "col": 0, "x": 0, "y": 1.25, "w": 2, "h": 1, "r": 0, "rx": 0, "ry": 0, "color": "mod"},
            {"row": 1, "col": 2, "x": 2, "y": 1.25, "w": 1, "h": 2, "r": 0, "rx": 0, "ry": 0, "color": "mod"},
            {"row": 9, "col": 9, "x": 0, "y": 3, "w": 1, "h": 1, "d": true}
        ], "optionKeys": {"0": {
            "0": [{"row": 2, "col": 0, "x": 0, "y": 2.25, "w": 1, "h": 1}],
            "1": [{"row": 2, "col": 0, "x": 1, "y": 2.25, "w": 2, "h": 1}]
        }}}
    }"#;

    /// row, col, x, y, w, h, knob, layout option
    type Geometry = (u8, u8, f64, f64, f64, f64, Option<u8>, Option<(u8, u8)>);

    fn geometry(d: &Definition) -> Vec<Geometry> {
        let mut g: Vec<_> = d.keys.iter().map(|k| (k.row, k.col, k.x, k.y, k.w, k.h, k.encoder, k.option)).collect();
        g.sort_by(|a, b| a.partial_cmp(b).unwrap());
        g
    }

    #[test]
    fn both_formats_give_the_same_board() {
        let (kle, compiled) = (Definition::parse(KLE).unwrap(), Definition::parse(COMPILED).unwrap());
        assert_eq!((kle.vendor_id, kle.product_id), (0x1234, 0x0001));
        assert_eq!((compiled.vendor_id, compiled.product_id), (0x1234, 0x0001));
        assert_eq!(geometry(&kle), geometry(&compiled));
        assert_eq!(kle.custom_keycodes, ["MCtrl", "Launch"]);
        assert_eq!(kle.encoder_count(), 1);
    }

    #[test]
    fn compiled_keeps_colours_and_skips_decals() {
        let d = Definition::parse(COMPILED).unwrap();
        assert_eq!(d.keys.len(), 7);
        assert_eq!(d.keys[0].color.as_deref(), Some("accent"));
    }

    #[test]
    fn user_files_override_and_nothing_is_fetched_when_offline() {
        let dir = std::env::temp_dir().join(format!("delta-layout-test-{}", std::process::id()));
        let (user, cache) = (dir.join("user"), dir.join("cache"));
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join("mine.json"), KLE).unwrap();

        let found = Definition::load(0x1234, 0x0001, &user, &cache, false).unwrap();
        assert_eq!(found.name, "Test Board");
        assert!(matches!(Definition::load(0x1234, 0x0002, &user, &cache, false), Err(LayoutError::NotFound { .. })));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Checks the KLE parser against VIA's own compiler on a real board. Needs two
    /// local files (they're GPL-3.0, so not in this repo):
    /// `DELTA_KLE=q5_source.json DELTA_COMPILED=q5_compiled.json cargo test -- --ignored`
    #[test]
    #[ignore]
    fn kle_parser_matches_via_compiler() {
        let read = |var| std::fs::read_to_string(std::env::var(var).expect(var)).unwrap();
        let (kle, compiled) =
            (Definition::parse(&read("DELTA_KLE")).unwrap(), Definition::parse(&read("DELTA_COMPILED")).unwrap());
        assert_eq!(geometry(&kle), geometry(&compiled));
    }
}
