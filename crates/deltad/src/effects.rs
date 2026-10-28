//! Key-triggered animations (bursts), drawn frame by frame into the per-key buffer.

use delta_core::config::KeyEffect;
use delta_core::lighting::{self, Rgb};
use delta_via::Definition;
use std::time::{Duration, Instant};

/// Pause between repeats when the GUI previews an effect on a loop.
const LOOP_GAP: Duration = Duration::from_millis(500);

/// Where each LED and matrix position sits, in key widths (centre of the key).
pub struct Geometry {
    leds: Vec<Option<(f32, f32)>>,
    matrix: Vec<Vec<Option<(f32, f32)>>>,
    center: (f32, f32),
}

impl Geometry {
    pub fn new(def: &Definition, led_map: &[Vec<Option<u8>>], led_count: u8) -> Self {
        let mut leds = vec![None; led_count as usize];
        let mut matrix = vec![vec![None; def.cols as usize]; def.rows as usize];
        let (mut max_x, mut max_y) = (0.0f32, 0.0f32);
        // Only the first choice of each layout option, like the app draws
        for k in def.keys.iter().filter(|k| k.option.is_none_or(|o| o.1 == 0)) {
            let (x, y, w, h) = (k.x as f32, k.y as f32, k.w as f32, k.h as f32);
            let at = (x + w / 2.0, y + h / 2.0);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
            if let Some(slot) = matrix.get_mut(k.row as usize).and_then(|r| r.get_mut(k.col as usize)) {
                *slot = Some(at);
            }
            if let Some(Some(led)) = led_map.get(k.row as usize).and_then(|r| r.get(k.col as usize))
                && let Some(slot) = leds.get_mut(*led as usize)
            {
                *slot = Some(at);
            }
        }
        Self { leds, matrix, center: (max_x / 2.0, max_y / 2.0) }
    }

    /// Centre of the first key whose base-layer name is `name`.
    fn key(&self, names: &[Vec<String>], name: &str) -> Option<(f32, f32)> {
        names.iter().enumerate().find_map(|(r, row)| {
            row.iter().enumerate().find_map(|(c, n)| (n == name).then(|| self.matrix[r][c]).flatten())
        })
    }

    fn at(&self, row: u8, col: u8) -> Option<(f32, f32)> {
        *self.matrix.get(row as usize)?.get(col as usize)?
    }
}

pub struct Running {
    effect: KeyEffect,
    origin: (f32, f32),
    start: Instant,
    /// GUI preview: starts again after a short pause instead of ending.
    looping: bool,
}

impl Running {
    /// The effect, started by the key at `pressed` (used when it has no own origin).
    pub fn start(effect: &KeyEffect, pressed: Option<(u8, u8)>, geo: &Geometry, names: &[Vec<String>]) -> Option<Self> {
        let origin = match effect.origin.as_deref() {
            Some("center") => Some(geo.center),
            Some(key) => geo.key(names, key),
            None => pressed.and_then(|(r, c)| geo.at(r, c)).or_else(|| geo.key(names, &effect.trigger)),
        }?;
        Some(Self { effect: effect.clone(), origin, start: Instant::now(), looping: false })
    }

    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    pub fn is_looping(&self) -> bool {
        self.looping
    }

    fn progress(&mut self, now: Instant) -> Option<f32> {
        let duration = Duration::from_millis(self.effect.duration_ms as u64);
        let mut elapsed = now.duration_since(self.start);
        if self.looping && elapsed > duration + LOOP_GAP {
            self.start = now;
            elapsed = Duration::ZERO;
        }
        if elapsed > duration {
            return if self.looping { Some(1.0) } else { None };
        }
        Some(elapsed.as_secs_f32() / duration.as_secs_f32())
    }
}

/// One frame: every running effect drawn over `base`. Finished effects are dropped.
pub fn frame(base: &[Rgb], running: &mut Vec<Running>, geo: &Geometry, now: Instant) -> Vec<Rgb> {
    let mut out = base.to_vec();
    running.retain_mut(|run| {
        let Some(t) = run.progress(now) else { return false };
        for (led, slot) in out.iter_mut().enumerate() {
            let Some((x, y)) = geo.leds.get(led).copied().flatten() else { continue };
            let dist = ((x - run.origin.0).powi(2) + (y - run.origin.1).powi(2)).sqrt();
            let amount = lighting::burst_amount(&run.effect, dist, t);
            if amount > 0.004 {
                *slot = lighting::mix(*slot, lighting::burst_color(&run.effect, dist), amount);
            }
        }
        true
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> Geometry {
        // One row of five keys on LEDs 0–4, centres at x = 0.5 … 4.5
        let at = |x: f32| Some((x + 0.5, 0.5));
        Geometry {
            leds: (0..5).map(|i| at(i as f32)).collect(),
            matrix: vec![(0..5).map(|i| at(i as f32)).collect()],
            center: (2.5, 0.5),
        }
    }

    fn effect(origin: Option<&str>) -> KeyEffect {
        let mut e: KeyEffect = toml::from_str(
            r##"
            id = "send"
            trigger = "KC_ENTER"
            color = "#FF0000"
            distance = 4.0
            width = 0.6
            duration_ms = 1000
            "##,
        )
        .unwrap();
        e.origin = origin.map(String::from);
        e
    }

    #[test]
    fn burst_starts_on_its_origin_and_ends() {
        let geo = geometry();
        let names = vec![["KC_A", "KC_B", "KC_C", "KC_D", "KC_ENTER"].map(String::from).to_vec()];
        let base = vec![(0, 0, 0); 5];

        let mut running: Vec<Running> = Running::start(&effect(None), Some((0, 4)), &geo, &names).into_iter().collect();
        let t0 = running[0].start;
        let first = frame(&base, &mut running, &geo, t0);
        assert!(first[4].0 > 250 && first[0] == (0, 0, 0), "lit at the pressed key only: {first:?}");

        let mut centred: Vec<Running> =
            Running::start(&effect(Some("center")), Some((0, 4)), &geo, &names).into_iter().collect();
        let t1 = centred[0].start;
        let first = frame(&base, &mut centred, &geo, t1);
        assert!(first[2].0 > 250 && first[4].0 < 10, "from the middle: {first:?}");

        let later = t0 + Duration::from_millis(1100);
        frame(&base, &mut running, &geo, later);
        assert!(running.is_empty(), "finished effects are dropped");
    }
}
