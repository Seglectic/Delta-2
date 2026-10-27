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
  error: string | null;
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
  | { action: "keys"; send: string };

export type Binding = Action & {
  id: string;
  key: string;
  /** Missing means "delta" (Caps held). */
  layer?: "delta" | "base";
  only_in?: string;
  label?: string;
};

export interface App {
  name: string;
  launch: string;
  match?: string[];
}

export interface Config {
  engine: { backend: string; modifier_key: string };
  apps?: App[];
  bindings?: Binding[];
}

export interface ConfigInfo {
  config: Config | null;
  source: string;
  legacy: boolean;
  problems: string[];
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
