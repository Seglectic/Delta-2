// Key-effect preview for the on-screen board. Same formula as deltad's
// (delta-core `lighting::burst_amount` / `burst_color`, deltad `effects.rs`).

import type { Definition, KeyEffect, PhysicalKey } from "./types";

/** Pause between repeats of the looping preview, like deltad's. */
export const LOOP_GAP_MS = 500;

type Rgb = [number, number, number];

const parse = (hex: string): Rgb => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};
const hex = (c: Rgb) => "#" + c.map((v) => Math.round(v).toString(16).padStart(2, "0")).join("").toUpperCase();
const mix = (a: Rgb, b: Rgb, t: number): Rgb => {
  const k = Math.min(1, Math.max(0, t));
  return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
};

export function burstAmount(e: KeyEffect, dist: number, t: number): number {
  if (t < 0 || t > 1) return 0;
  const x = (dist - t * e.distance) / e.width;
  return Math.exp(-x * x) * Math.pow(1 - t, e.fade) * (e.strength / 255);
}

export function burstColor(e: KeyEffect, dist: number): Rgb {
  const start = parse(e.color);
  return mix(start, e.color_end ? parse(e.color_end) : start, dist / e.distance);
}

/** Keys as drawn: the first choice of each layout option. */
const drawn = (def: Definition) => def.keys.filter((k) => !k.option || k.option[1] === 0);
const centre = (k: PhysicalKey): [number, number] => [k.x + k.w / 2, k.y + k.h / 2];

/** Where the burst starts: a named key, the middle of the board, or the trigger. */
export function effectOrigin(
  e: KeyEffect,
  def: Definition,
  nameAt: (row: number, col: number) => string | undefined,
): [number, number] | null {
  if (e.origin === "center") {
    const keys = drawn(def);
    return [Math.max(...keys.map((k) => k.x + k.w)) / 2, Math.max(...keys.map((k) => k.y + k.h)) / 2];
  }
  const want = e.origin ?? e.trigger;
  const k = drawn(def).find((k) => nameAt(k.row, k.col) === want);
  return k ? centre(k) : null;
}

/** "row,col" → colour for one frame `ms` into the loop, drawn over `base`. */
export function effectFrame(
  e: KeyEffect,
  def: Definition,
  origin: [number, number],
  base: Record<string, string> | null,
  ms: number,
): Record<string, string> {
  const t = (ms % (e.duration_ms + LOOP_GAP_MS)) / e.duration_ms;
  const out: Record<string, string> = {};
  for (const k of drawn(def)) {
    const id = `${k.row},${k.col}`;
    const [x, y] = centre(k);
    const dist = Math.hypot(x - origin[0], y - origin[1]);
    const under = parse(base?.[id] ?? "#000000");
    out[id] = hex(mix(under, burstColor(e, dist), burstAmount(e, dist, t)));
  }
  return out;
}
