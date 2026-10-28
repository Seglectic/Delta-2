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
    let digits = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(digits, 16).map_err(|_| LayoutError::Field { field, value: s.into() })
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

    // Decals are labels drawn on the board, and row -1 marks keys without a switch
    // (VIA keeps both, in `keys` and in `optionKeys`)
    let wired = |k: &Value| k["d"].as_bool() != Some(true) && k["row"].as_f64().is_none_or(|r| r >= 0.0);
    let mut keys = Vec::new();
    for k in layouts["keys"].as_array().ok_or(LayoutError::Missing("layouts.keys"))?.iter().filter(|k| wired(k)) {
        keys.push(parse(k, None)?);
    }
    for (group, choices) in layouts["optionKeys"].as_object().into_iter().flatten() {
        for (choice, list) in choices.as_object().into_iter().flatten() {
            let option = group.parse().ok().zip(choice.parse().ok());
            for k in list.as_array().into_iter().flatten().filter(|k| wired(k)) {
                keys.push(parse(k, option)?);
            }
        }
    }
    Ok(keys)
}

/// KLE source layout. Property objects change the next key (`x`, `y` offsets, `w`,
/// `h`, rotation `r`/`rx`/`ry`); after each key the cursor moves right by its width;
/// each row starts one unit down (back at `rx` when rotated).
/// KLE stores a key's legends in an order that depends on its alignment (`a`); this
/// maps (alignment, position in the string) to the legend slot. From kle-serial, which
/// VIA uses. VIA reads the matrix position from slot 0, the encoder from 4 and the
/// layout option from 8.
const KLE_LABEL_MAP: [[i8; 12]; 8] = [
    [0, 6, 2, 8, 9, 11, 3, 5, 1, 4, 7, 10],
    [1, 7, -1, -1, 9, 11, 4, -1, -1, -1, -1, 10],
    [3, -1, 5, -1, 9, 11, -1, -1, 4, -1, -1, 10],
    [4, -1, -1, -1, 9, 11, -1, -1, -1, -1, -1, 10],
    [0, 6, 2, 8, 10, -1, 3, 5, 1, 4, 7, -1],
    [1, 7, -1, -1, 10, -1, 4, -1, -1, -1, -1, -1],
    [3, -1, 5, -1, 10, -1, -1, -1, 4, -1, -1, -1],
    [4, -1, -1, -1, 10, -1, -1, -1, -1, -1, -1, -1],
];

/// A KLE legend string's legends by slot.
fn kle_labels(label: &str, align: usize) -> [&str; 12] {
    let mut out = [""; 12];
    for (i, text) in label.split('\n').enumerate().take(12) {
        if let Ok(slot) = usize::try_from(KLE_LABEL_MAP[align.min(7)][i]) {
            out[slot] = text;
        }
    }
    out
}

/// A key as KLE draws it, before VIA's clean-up.
struct KleKey {
    key: PhysicalKey,
    /// False for decals and knobs without a switch: they're not keys, but VIA still
    /// counts them when lining the board up.
    wired: bool,
    /// Second rectangle of ISO / big-ass Enter shapes.
    x2: f64,
    y2: f64,
    w2: f64,
    h2: f64,
}

fn kle_keys(rows: &[Value]) -> Result<Vec<PhysicalKey>, LayoutError> {
    let mut keys = Vec::new();
    let (mut y, mut r, mut rx, mut ry) = (0.0, 0.0, 0.0, 0.0);
    // Alignment carries over to later keys (VIA starts at 0); the rest apply to the next key only
    let mut align = 0;
    let mut decal = false;
    let mut second = None;
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
                    align = num("a").map_or(align, |a| a as usize);
                    decal = props.get("d").and_then(Value::as_bool).unwrap_or(decal);
                    if ["x2", "y2", "w2", "h2"].iter().any(|k| props.contains_key(*k)) {
                        second = Some((num("x2").unwrap_or(0.0), num("y2").unwrap_or(0.0), num("w2"), num("h2")));
                    }
                }
                Value::String(label) => {
                    let labels = kle_labels(label, align);
                    let encoder = labels[4].strip_prefix(['e', 'E']).and_then(|n| n.trim().parse().ok());
                    let option = pair(labels[8]);
                    let mut key = PhysicalKey { row: 0, col: 0, x, y, w, h, r, rx, ry, encoder, option, color: None };
                    let wired = !(decal || labels[0].is_empty() && encoder.is_some());
                    if wired {
                        (key.row, key.col) = pair(labels[0])
                            .ok_or_else(|| LayoutError::Field { field: "matrix label", value: label.clone() })?;
                    }
                    let (x2, y2, w2, h2) = second.take().unwrap_or((0.0, 0.0, None, None));
                    keys.push(KleKey { key, wired, x2, y2, w2: w2.unwrap_or(w), h2: h2.unwrap_or(h) });
                    x += w;
                    (w, h) = (1.0, 1.0);
                    decal = false;
                }
                _ => {}
            }
        }
        y += 1.0;
    }
    align_options(&mut keys);
    Ok(keys.into_iter().filter(|k| k.wired).map(|k| k.key).collect())
}

/// What VIA's compiler (the-via/reader, kle-parser.ts) does to KLE layouts: put the
/// top-left of the default layout (choice 0 of every option, decals included) at 0,0,
/// and move each option's other choices, drawn off to the side in the source, so their
/// top-left key sits on choice 0's.
fn align_options(keys: &mut [KleKey]) {
    let default = |k: &&KleKey| k.key.option.is_none_or(|(_, choice)| choice == 0);
    let boxes = keys.iter().filter(default).map(bounding_box);
    let (min_x, min_y) = boxes.fold((f64::INFINITY, f64::INFINITY), |(x, y), b| (x.min(b.0), y.min(b.1)));
    if !min_x.is_finite() {
        return;
    }
    // The top row's leftmost key, and of an ISO Enter its upper rectangle
    let pivot = |keys: &[KleKey], option: (u8, u8)| {
        let k = keys
            .iter()
            .filter(|k| k.key.option == Some(option))
            .min_by(|a, b| a.key.y.total_cmp(&b.key.y).then(a.key.x.total_cmp(&b.key.x)))?;
        let second_on_top = if k.y2 == 0.0 { k.x2 < 0.0 } else { k.y2 < 0.0 };
        Some(if second_on_top { (k.key.x + k.x2, k.key.y + k.y2) } else { (k.key.x, k.key.y) })
    };
    let deltas: Vec<_> = keys
        .iter()
        .filter_map(|k| k.key.option)
        .filter(|&(_, choice)| choice > 0)
        .map(|option| {
            let zero = pivot(keys, (option.0, 0));
            let this = pivot(keys, option);
            let delta = zero.zip(this).map_or((0.0, 0.0), |(z, t)| (t.0 - z.0, t.1 - z.1));
            (option, delta)
        })
        .collect();
    for KleKey { key: k, .. } in keys.iter_mut() {
        let (dx, dy) = k.option.and_then(|o| deltas.iter().find(|(d, _)| *d == o)).map_or((0.0, 0.0), |d| d.1);
        // rx/ry only move with the board, as in VIA
        (k.x, k.y, k.rx, k.ry) = (k.x - dx - min_x, k.y - dy - min_y, k.rx - min_x, k.ry - min_y);
    }
}

/// Top-left corner of a key's box (both rectangles) after rotation.
fn bounding_box(k: &KleKey) -> (f64, f64) {
    let KleKey { key, x2, y2, w2, h2, .. } = k;
    let (x0, y0) = (key.x.min(key.x + x2), key.y.min(key.y + y2));
    let (x1, y1) = ((key.x + key.w).max(key.x + x2 + w2), (key.y + key.h).max(key.y + y2 + h2));
    let rad = key.r.to_radians();
    [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
        .iter()
        .map(|&(x, y)| {
            let (nx, ny) = (x - key.rx, y - key.ry);
            (key.rx + nx * rad.cos() - ny * rad.sin(), key.ry + nx * rad.sin() + ny * rad.cos())
        })
        .fold((f64::INFINITY, f64::INFINITY), |(x, y), p| (x.min(p.0), y.min(p.1)))
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
            ["2,0\n\n\n0,0", {"w": 2}, "2,0\n\n\n0,1", {"d": true}, "\n\n\n0,1"]
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
            "1": [{"row": 2, "col": 0, "x": 0, "y": 2.25, "w": 2, "h": 1}]
        }}}
    }"#;

    /// row, col, x, y, w, h, knob, layout option
    type Geometry = (u8, u8, f64, f64, f64, f64, Option<u8>, Option<(u8, u8)>);

    fn geometry(d: &Definition) -> Vec<Geometry> {
        // Rotated keys pick up float noise in the last digits
        let round = |v: f64| (v * 1e6).round() / 1e6;
        let mut g: Vec<_> =
            d.keys.iter().map(|k| (k.row, k.col, round(k.x), round(k.y), k.w, k.h, k.encoder, k.option)).collect();
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
