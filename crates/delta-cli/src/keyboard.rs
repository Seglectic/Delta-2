//! `delta keyboard`: read QMK keyboards over VIA and draw their keymaps.

use anyhow::{Context, Result, bail};
use delta_core::paths::Paths;
use delta_via::keycode::decode;
use delta_via::{Definition, Via, discover};

/// Terminal characters per key unit, horizontally and vertically.
const CELL_W: f64 = 6.0;
const CELL_H: f64 = 2.0;

pub fn list(paths: &Paths) -> Result<()> {
    let devices = discover();
    if devices.is_empty() {
        println!("No keyboards with a VIA interface found.");
    }
    for d in devices {
        let def = Definition::find(d.vendor_id, d.product_id, Some(&paths.layouts));
        let layout = def.map_or_else(|| "no layout file".into(), |d| d.name);
        println!("{}  {:04x}:{:04x}  {}  ({layout})", d.path.display(), d.vendor_id, d.product_id, d.name);
    }
    Ok(())
}

pub fn dump(paths: &Paths, only_layer: Option<u8>) -> Result<()> {
    let devices = discover();
    let Some(device) = devices.first() else { bail!("No keyboards with a VIA interface found.") };
    let def = Definition::find(device.vendor_id, device.product_id, Some(&paths.layouts)).with_context(|| {
        format!(
            "no layout for {:04x}:{:04x}; put its VIA definition JSON in {}",
            device.vendor_id,
            device.product_id,
            paths.layouts.display()
        )
    })?;

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
