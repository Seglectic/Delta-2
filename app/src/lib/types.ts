// Shapes sent by the Rust side (app/src-tauri/src/lib.rs)

export interface Keycode {
  code: number;
  name: string;
  label: string;
}

export interface PhysicalKey {
  row: number;
  col: number;
  x: number;
  y: number;
  w: number;
  h: number;
  r: number;
  rx: number;
  ry: number;
  encoder: number | null;
  option: [number, number] | null;
  color: string | null;
}

export interface Definition {
  name: string;
  vendor_id: number;
  product_id: number;
  rows: number;
  cols: number;
  custom_keycodes: string[];
  keys: PhysicalKey[];
}

export interface KeyboardInfo {
  path: string;
  device_name: string;
  vendor_id: number;
  product_id: number;
  protocol: number | null;
  definition: Definition | null;
  /** [layer][row][col] */
  layers: Keycode[][][];
  /** [layer][knob] → [left, right] */
  encoders: [Keycode, Keycode][][];
  /** The layer the board is on (Keychron's Mac/Windows switch); null when it doesn't say. */
  base_layer: number | null;
  error: string | null;
}

export interface BaseLayer {
  path: string;
  layer: number;
}

export interface Pressed {
  path: string;
  keys: [number, number][];
}

export type Action =
  | { action: "tile"; layout: "halves" | "thirds_left" | "thirds_right"; side: "left" | "right" }
  | { action: "span"; x: number; width: number; name?: string }
  | { action: "fixed"; width: number; height: number }
  | { action: "app"; app: string }
  | { action: "keys"; send: string }
  | { action: "command"; run: string }
  | { action: "kwin_shortcut"; name: string; component?: string };

export type ActionKind = Action["action"];

export type Binding = Action & {
  id: string;
  key: string;
  /** Missing means "delta" (Caps held). */
  layer?: "delta" | "base";
  only_in?: string;
  label?: string;
  /** #RRGGBB key colour while Caps is held. */
  color?: string;
};

export interface App {
  name: string;
  launch: string;
  match?: string[];
  focus_tile?: { layout: string; side: string };
}

export interface Shortcut {
  component: string;
  component_name: string;
  name: string;
  label: string;
}

/** An app offered by the pickers (running or installed), ready for `[[apps]]`. */
export interface AppChoice {
  label: string;
  name: string;
  /** Window class; what "only in" matches. */
  class: string;
  launch: string;
  match: string[];
  caption: string | null;
}

export interface Config {
  engine: { backend: string; modifier_key: string };
  apps?: App[];
  bindings?: Binding[];
  lighting?: {
    caps_highlight?: boolean;
    normal?: { effect: number; color: string; brightness: number };
    /** #RRGGBB for keys without a Caps-layer action; unset = off. */
    delta_background?: string;
    profiles?: LightingProfile[];
    effects?: KeyEffect[];
  };
  // Other sections are passed through untouched
  [section: string]: unknown;
}

/** One key's light in a profile. */
export interface KeyLight {
  /** QMK name of what the key sends on the base layer, e.g. KC_W. Empty for a background. */
  key: string;
  color: string;
  /** 0–255 */
  brightness: number;
}

/** Per-key colours, for one app (`app`) or everywhere else. */
export interface LightingProfile {
  app?: string;
  /** Keys not listed; unset = off. */
  background?: KeyLight;
  keys?: KeyLight[];
}

/** A burst that spreads out when `trigger` is pressed (`[[lighting.effects]]`). */
export interface KeyEffect {
  id: string;
  /** Only while this app has focus; unset = everywhere. */
  app?: string;
  /** QMK name on the base layer, e.g. KC_ENTER. */
  trigger: string;
  /** A key name or "center"; unset = the trigger key. */
  origin?: string;
  color: string;
  color_end?: string;
  /** Peak brightness 0–255. */
  strength: number;
  /** How far it travels, in key widths. */
  distance: number;
  /** Ring thickness, in key widths. */
  width: number;
  duration_ms: number;
  /** 0 = no dimming while travelling, 1 = linear, more = sooner. */
  fade: number;
}

/** What the keyboard's LEDs show right now (read back from it). */
export interface KeyboardLighting {
  effect: number;
  brightness: number;
  /** One colour for every key (solid, breathing, off). */
  all: string | null;
  /** "row,col" → colour, in Per Key RGB. */
  keys: Record<string, string>;
  /** An animation Delta can't draw. */
  animated: boolean;
}

/** What each key lights up as while the delta key is held (#RRGGBB). */
export interface DeltaColors {
  /** Binding ID → colour */
  bindings: Record<string, string>;
  background: string;
  delta_key: string;
}

export interface ConfigInfo {
  config: Config | null;
  source: string;
  legacy: boolean;
  problems: string[];
}

/** Who runs deltad: nobody yet, this app (from ~/.local/share/delta/bin), or something
    else (a dev install from packaging/install.sh). */
export type Owner = { kind: "nobody" } | { kind: "app" } | { kind: "other"; exec: string };

export interface SetupState {
  app_version: string;
  /** `tauri dev` build: installs nothing. */
  dev: boolean;
  owner: Owner;
  install: { kind: "skipped"; exec: string } | { kind: "done"; updated: string[]; restarted: boolean } | null;
  install_error: string | null;
  permissions: { input_session: boolean; input_configured: boolean; uinput: boolean; hidraw: boolean; rules: boolean };
  needs_setup: boolean;
  needs_relogin: boolean;
}

/** One physical mouse; `paths` are its input nodes (some devices have several). */
export interface Mouse {
  name: string;
  vendor_id: number;
  product_id: number;
  paths: string[];
}

export type InputEvent = { path: string } & (
  | { kind: "button"; code: number; name: string; pressed: boolean }
  | { kind: "wheel"; axis: "vertical" | "horizontal"; notches: number }
);

export interface KnobTurn {
  path: string;
  encoder: number;
  direction: "left" | "right";
}
