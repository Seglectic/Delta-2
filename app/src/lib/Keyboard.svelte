<script lang="ts">
  import type { Binding, Definition, Keycode, PhysicalKey } from "./types";
  import { bindingsFor, isDeltaLayer, keyId, qmkName, shortAction } from "./bindings";

  interface Props {
    definition: Definition;
    /** Keycodes of the layer being shown, [row][col]. */
    layer: Keycode[][];
    /** Shown dimmed where `layer` is transparent. */
    baseLayer: Keycode[][];
    bindings: Binding[];
    modifierKey: string;
    /** Show what keys do while the delta modifier is held. */
    deltaView: boolean;
    pressed: Set<string>;
    /** Knobs turning right now: encoder → direction. */
    turning: Record<number, "left" | "right">;
    selected: string | null;
    onselect: (id: string) => void;
  }

  let { definition, layer, baseLayer, bindings, modifierKey, deltaView, pressed, turning, selected, onselect }: Props =
    $props();

  /** Pixels per key unit, and the gap around each cap. */
  const U = 60;
  const GAP = 3;

  const width = $derived(Math.max(...definition.keys.map((k) => k.x + k.w)) * U);
  const height = $derived(Math.max(...definition.keys.map((k) => k.y + k.h)) * U);
  // Only the first choice of each layout option is drawn for now
  const keys = $derived(definition.keys.filter((k) => !k.option || k.option[1] === 0));

  function keycode(k: PhysicalKey): { kc: Keycode | undefined; inherited: boolean } {
    const kc = layer[k.row]?.[k.col];
    if (kc?.name === "KC_TRANSPARENT") return { kc: baseLayer[k.row]?.[k.col], inherited: true };
    return { kc, inherited: false };
  }

  function cap(k: PhysicalKey) {
    const { kc, inherited } = keycode(k);
    const own = bindingsFor(kc, bindings);
    const isModifier = kc?.name === qmkName(modifierKey);
    if (deltaView) {
      const action = own.find((b) => isDeltaLayer(b) && !b.only_in);
      return {
        main: isModifier ? "Δ" : action ? shortAction(action) : (kc?.label ?? ""),
        sub: action ? kc?.label : undefined,
        active: !!action || isModifier,
        dim: !action && !isModifier,
        isModifier,
        perApp: false,
      };
    }
    const base = own.filter((b) => !isDeltaLayer(b));
    return {
      main: isModifier ? "Δ" : (kc?.label ?? ""),
      sub: isModifier ? kc?.label : base.length ? shortAction(base[0]) : undefined,
      active: base.length > 0,
      dim: inherited,
      isModifier,
      perApp: base.length > 0 && base.every((b) => b.only_in),
    };
  }
</script>

<svg viewBox="0 0 {width} {height}" class="keyboard" role="group" aria-label={definition.name}>
  {#each keys as k (keyId(k) + k.x + k.y)}
    {@const id = keyId(k)}
    {@const c = cap(k)}
    {@const isPressed = pressed.has(`${k.row},${k.col}`)}
    {@const turn = k.encoder !== null ? turning[k.encoder] : undefined}
    <g
      class="key {k.color ?? 'alpha'}"
      class:pressed={isPressed}
      class:turning={!!turn}
      class:selected={selected === id}
      class:active={c.active}
      class:dim={c.dim}
      class:modifier={c.isModifier}
      class:per-app={c.perApp}
      transform={k.r ? `rotate(${k.r} ${k.rx * U} ${k.ry * U})` : undefined}
      role="button"
      tabindex="0"
      aria-label={c.main}
      onclick={() => onselect(id)}
      onkeydown={(e) => e.key === "Enter" && onselect(id)}
    >
      {#if k.encoder !== null}
        <circle
          class="cap"
          cx={(k.x + k.w / 2) * U}
          cy={(k.y + k.h / 2) * U}
          r={(Math.min(k.w, k.h) * U) / 2 - GAP}
        />
        <circle class="knob-ring" cx={(k.x + k.w / 2) * U} cy={(k.y + k.h / 2) * U} r={(Math.min(k.w, k.h) * U) / 2 - 9} />

      {:else}
        <rect class="cap" x={k.x * U + GAP} y={k.y * U + GAP} width={k.w * U - GAP * 2} height={k.h * U - GAP * 2} rx="7" />
      {/if}
      {#if turn}
        <text class="main turn" x={(k.x + k.w / 2) * U} y={(k.y + k.h / 2) * U + 7}>{turn === "left" ? "↺" : "↻"}</text>
      {:else}
        <text class="main" x={(k.x + k.w / 2) * U} y={(k.y + k.h / 2) * U + (c.sub ? -4 : 5)}>{c.main}</text>
      {/if}
      {#if c.sub && !turn}
        <text class="sub" x={(k.x + k.w / 2) * U} y={(k.y + k.h / 2) * U + 14}>{c.sub}</text>
      {/if}
    </g>
  {/each}
</svg>

<style>
  .keyboard {
    width: 100%;
    height: auto;
    display: block;
    user-select: none;
  }
  .key {
    cursor: pointer;
    outline: none;
  }
  .cap {
    fill: var(--key-alpha);
    stroke: var(--crust);
    stroke-width: 1.5;
    transition:
      fill 60ms ease-out,
      filter 60ms ease-out;
  }
  .mod .cap {
    fill: var(--key-mod);
  }
  .accent .cap {
    fill: var(--key-accent);
  }
  .knob-ring {
    fill: none;
    stroke: var(--surface2);
    stroke-dasharray: 3 5;
    pointer-events: none;
  }
  text {
    text-anchor: middle;
    fill: var(--text);
    pointer-events: none;
    font-size: 13px;
  }
  .sub {
    font-size: 10.5px;
    fill: var(--subtext0);
  }
  .dim text {
    opacity: 0.4;
  }
  .active .sub,
  .active .main {
    fill: var(--blue);
  }
  .per-app .sub {
    fill: var(--teal);
  }
  .modifier .cap {
    fill: var(--mauve-deep);
  }
  .modifier .main {
    fill: var(--mauve);
    font-size: 18px;
    font-weight: 700;
  }
  .selected .cap {
    stroke: var(--lavender);
    stroke-width: 2.5;
  }
  .key:focus-visible .cap {
    stroke: var(--lavender);
    stroke-width: 2.5;
  }
  .turning .cap {
    fill: var(--teal);
    filter: drop-shadow(0 0 8px var(--teal));
  }
  .turning .main,
  .turning .sub {
    fill: var(--crust);
  }
  .turning .turn {
    font-size: 22px;
    font-weight: 700;
  }
  .pressed .cap {
    fill: var(--blue);
    filter: drop-shadow(0 0 8px var(--blue));
  }
  .pressed text,
  .pressed .main,
  .pressed .sub {
    fill: var(--crust);
    opacity: 1;
  }
</style>
