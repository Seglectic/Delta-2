//! `delta keyboard`: read QMK keyboards over VIA and draw their keymaps.

use anyhow::{Context, Result, bail};
use delta_core::paths::Paths;
use delta_via::keycode::decode;
use delta_via::{Definition, Via, discover};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Terminal characters per key unit, horizontally and vertically.
const CELL_W: f64 = 6.0;
const CELL_H: f64 = 2.0;

pub fn list(paths: &Paths) -> Result<()> {
    let devices = discover();
    if devices.is_empty() {
        println!("No keyboards with a VIA interface found.");
    }
    for d in devices {
        let def = Definition::load(d.vendor_id, d.product_id, &paths.layouts, &paths.layout_cache, true);
        let layout = def.map_or_else(|e| e.to_string(), |d| d.name);
        println!("{}  {:04x}:{:04x}  {}  ({layout})", d.path.display(), d.vendor_id, d.product_id, d.name);
    }
    Ok(())
}

pub fn dump(paths: &Paths, only_layer: Option<u8>) -> Result<()> {
    let devices = discover();
    let Some(device) = devices.first() else { bail!("No keyboards with a VIA interface found.") };
    let def = Definition::load(device.vendor_id, device.product_id, &paths.layouts, &paths.layout_cache, true)
        .with_context(|| format!("you can also put its VIA definition JSON in {}", paths.layouts.display()))?;

    let mut via = Via::open(device)?;
    let protocol = via.protocol_version()?;
    let keymap = via.keymap(def.rows, def.cols)?;
    let encoders = via.encoders(keymap.len() as u8, def.encoder_count())?;
    println!("{} — VIA protocol {protocol}, {} layers, {}", def.name, keymap.len(), device.path.display());

    for (layer, matrix) in keymap.iter().enumerate() {
        if only_layer.is_some_and(|l| l as usize != layer) {
            continue;
        }
        println!("\nLayer {layer}");
        let label = |row: u8, col: u8| decode(matrix[row as usize][col as usize], &def.custom_keycodes).label;
        print!("{}", draw(&def, label));
        for (i, (ccw, cw)) in encoders[layer].iter().enumerate() {
            let name = |c: u16| decode(c, &def.custom_keycodes).name;
            println!("Knob {i}: left {}  right {}", name(*ccw), name(*cw));
        }
    }
    Ok(())
}

/// What the first keyboard's firmware can do: VIA and Keychron protocol, layers,
/// lighting, and which LED sits under each key.
pub fn info(paths: &Paths) -> Result<()> {
    let devices = discover();
    let Some(device) = devices.first() else { bail!("No keyboards with a VIA interface found.") };
    let def = Definition::load(device.vendor_id, device.product_id, &paths.layouts, &paths.layout_cache, true)?;
    let mut via = Via::open(device)?;
    println!("{} ({})", def.name, device.path.display());
    println!("VIA protocol:     {}", via.protocol_version()?);
    let Some(leds) = via.keychron_rgb()? else {
        println!("Keychron per-key RGB: not supported by this firmware");
        return Ok(());
    };
    println!("Keychron firmware: {}", via.keychron_firmware_version()?);
    match via.keychron_layers()? {
        Some((base, active)) => println!("Layers:           base {base}, active {active}"),
        None => println!("Layers:           the firmware doesn't report them"),
    }
    let l = via.lighting()?;
    println!("Lighting:         effect {}, brightness {}, hue {}, sat {}", l.effect, l.brightness, l.hue, l.sat);
    let map = via.led_map(def.rows, def.cols)?;
    let mapped = map.iter().flatten().flatten().count();
    println!("Per-key RGB:      {leds} LEDs, {mapped} mapped to matrix positions");
    for (r, row) in map.iter().enumerate() {
        let leds: Vec<String> = row.iter().map(|l| l.map_or("-".into(), |i| i.to_string())).collect();
        println!("  row {r} LEDs:     {}", leds.join(" "));
    }
    Ok(())
}

/// Save everything VIA can read about the first keyboard, so it can be restored
/// exactly after a firmware update resets it. Raw keycodes are the source of
/// truth; names are only there for people reading the file.
pub fn backup(paths: &Paths, out: Option<PathBuf>) -> Result<()> {
    let devices = discover();
    let Some(device) = devices.first() else { bail!("No keyboards with a VIA interface found.") };
    let def = Definition::load(device.vendor_id, device.product_id, &paths.layouts, &paths.layout_cache, true)?;
    let mut via = Via::open(device)?;

    let protocol = via.protocol_version()?;
    let layout_options = via.layout_options()?;
    let keymap = via.keymap(def.rows, def.cols)?;
    let encoders = via.encoders(keymap.len() as u8, def.encoder_count())?;
    let (macro_count, macro_buffer) = via.macros()?;

    let name = |c: u16| decode(c, &def.custom_keycodes).name;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let json = serde_json::json!({
        "format": "delta-keymap-backup/1",
        "taken_unix": now,
        "keyboard": {
            "name": def.name,
            "device_name": device.name,
            "vendor_id": format!("{:04x}", device.vendor_id),
            "product_id": format!("{:04x}", device.product_id),
            "via_protocol": protocol,
        },
        "matrix": { "rows": def.rows, "cols": def.cols },
        "layout_options": layout_options,
        "layers": keymap,
        "encoders": encoders,
        "macros": { "count": macro_count, "buffer_hex": macro_buffer.iter().map(|b| format!("{b:02x}")).collect::<String>() },
        "names": {
            "layers": keymap.iter().map(|l| l.iter().map(|r| r.iter().map(|c| name(*c)).collect::<Vec<_>>()).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "encoders": encoders.iter().map(|l| l.iter().map(|(a, b)| [name(*a), name(*b)]).collect::<Vec<_>>()).collect::<Vec<_>>(),
        },
    });

    let out = out.unwrap_or_else(|| {
        let date = civil_date(now);
        paths.backups.join(format!("{:04x}_{:04x}-{date}.json", device.vendor_id, device.product_id))
    });
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&json)? + "\n")?;
    println!(
        "Saved {} ({} layers, {} knob(s), {macro_count} macro slots) to {}",
        def.name,
        keymap.len(),
        def.encoder_count(),
        out.display()
    );
    Ok(())
}

/// Unix seconds → `YYYY-MM-DD` (UTC), without a date library.
fn civil_date(unix: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm
    let z = (unix / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Lay the keys out on a character grid at their physical positions.
fn draw(def: &Definition, label: impl Fn(u8, u8) -> String) -> String {
    let height = def.keys.iter().map(|k| ((k.y + k.h) * CELL_H).ceil() as usize).max().unwrap_or(0);
    let width = def.keys.iter().map(|k| ((k.x + k.w) * CELL_W).ceil() as usize).max().unwrap_or(0);
    let mut grid = vec![vec![' '; width]; height];
    for k in &def.keys {
        let text = if k.encoder.is_some() { format!("({})", label(k.row, k.col)) } else { label(k.row, k.col) };
        let (row, col) = ((k.y * CELL_H).round() as usize, (k.x * CELL_W).round() as usize);
        let room = (k.w * CELL_W) as usize - 1;
        for (i, ch) in text.chars().take(room).enumerate() {
            grid[row][col + i] = ch;
        }
    }
    grid.into_iter().map(|line| line.into_iter().collect::<String>().trim_end().to_string() + "\n").collect()
}
