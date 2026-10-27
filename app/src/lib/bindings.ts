// Connect firmware keycodes to Delta bindings, and describe actions briefly.

import type { Binding, Keycode, PhysicalKey } from "./types";

/** Stable ID for a key in the UI: `row,col`, or `e<n>` for knob n. */
export function keyId(k: PhysicalKey): string {
  return k.encoder !== null ? `e${k.encoder}` : `${k.row},${k.col}`;
}

/** Delta/xremap key names that differ from QMK's (`KC_` + name). */
const ALIASES: Record<string, string> = {
  ESC: "ESCAPE",
  BKSP: "BACKSPACE",
  DEL: "DELETE",
  INS: "INSERT",
  PGUP: "PAGE_UP",
  PAGEUP: "PAGE_UP",
  PGDN: "PAGE_DOWN",
  PAGEDOWN: "PAGE_DOWN",
  CAPSLOCK: "CAPS_LOCK",
  CAPS: "CAPS_LOCK",
  NUMLOCK: "NUM_LOCK",
  SCROLLLOCK: "SCROLL_LOCK",
  PRINT: "PRINT_SCREEN",
  SYSRQ: "PRINT_SCREEN",
  APOSTROPHE: "QUOTE",
  LEFTBRACE: "LEFT_BRACKET",
  RIGHTBRACE: "RIGHT_BRACKET",
};

/** QMK name for a Delta key name: `Left` → `KC_LEFT`, `grave` → `KC_GRAVE`. */
export function qmkName(deltaKey: string): string {
  const k = deltaKey.toUpperCase();
  return "KC_" + (ALIASES[k] ?? k);
}

export function isDeltaLayer(b: Binding): boolean {
  return (b.layer ?? "delta") === "delta";
}

/** Every binding on the key that sends `kc`. */
export function bindingsFor(kc: Keycode | undefined, bindings: Binding[]): Binding[] {
  if (!kc) return [];
  return bindings.filter((b) => qmkName(b.key) === kc.name);
}

const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** One- or two-word description for a key cap. */
export function shortAction(b: Binding): string {
  switch (b.action) {
    case "tile": {
      const left = { halves: "½", thirds_left: "⅓", thirds_right: "⅔" }[b.layout];
      const right = { halves: "½", thirds_left: "⅔", thirds_right: "⅓" }[b.layout];
      return b.side === "left" ? `◧ ${left}` : `◨ ${right}`;
    }
    case "span":
      return b.x === 0 && b.width === 1 ? "▣ Full" : `▭ ${Math.round(b.width * 100)}%`;
    case "fixed":
      return `▣ ${b.width}×${b.height}`;
    case "app":
      return capitalize(b.app);
    case "keys":
      return b.send.replace(/\+/g, " ");
  }
}

/** A full sentence for the details panel. */
export function describeAction(b: Binding): string {
  switch (b.action) {
    case "tile": {
      const share = {
        halves: { left: "left half", right: "right half" },
        thirds_left: { left: "left third", right: "right two-thirds" },
        thirds_right: { left: "left two-thirds", right: "right third" },
      }[b.layout][b.side];
      return `Tile the window into the ${share}`;
    }
    case "span":
      return b.x === 0 && b.width === 1
        ? "Stretch the window across the screen"
        : `Resize the window to ${Math.round(b.width * 100)}% wide, starting at ${Math.round(b.x * 100)}%`;
    case "fixed":
      return `Center the window at ${b.width}×${b.height}`;
    case "app":
      return `Focus ${capitalize(b.app)}, or launch it`;
    case "keys":
      return `Send ${b.send}`;
  }
}

/** When a binding applies, in words. */
export function whenText(b: Binding, modifierKey: string): string {
  const held = isDeltaLayer(b) ? `Hold ${modifierLabel(modifierKey)} + ${b.key}` : b.key;
  return b.only_in ? `${held}, only in ${b.only_in}` : held;
}

export function modifierLabel(modifierKey: string): string {
  return modifierKey.toLowerCase() === "capslock" ? "Caps Lock" : `Caps (${modifierKey.toUpperCase()})`;
}
