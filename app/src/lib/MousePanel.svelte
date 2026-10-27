<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { InputDevice, InputEvent } from "./types";

  interface Props {
    device: InputDevice;
  }

  let { device }: Props = $props();

  /** Buttons every mouse has, with friendly names. */
  const MAIN = [
    ["BTN_LEFT", "Left click"],
    ["BTN_MIDDLE", "Wheel click"],
    ["BTN_RIGHT", "Right click"],
  ];
  const THUMB = [
    ["BTN_SIDE", "Thumb back"],
    ["BTN_EXTRA", "Thumb forward"],
  ];
  const KNOWN = new Set([...MAIN, ...THUMB].map(([name]) => name));
  const WHEEL = ["up", "down", "left", "right"] as const;
  type WheelDir = (typeof WHEEL)[number];
  const WHEEL_LABEL: Record<WheelDir, string> = { up: "Scroll ↑", down: "Scroll ↓", left: "Tilt ←", right: "Tilt →" };

  let held = $state(new Set<string>());
  let wheel = $state<Record<WheelDir, boolean>>({ up: false, down: false, left: false, right: false });
  /** Extra buttons, learned by pressing them; remembered per mouse. */
  let extras = $state<string[]>(loadExtras());
  let log = $state<string[]>([]);

  const storageKey = () => `delta.mouse.${device.name}.buttons`;
  const short = (name: string) => name.replace(/^(KEY|BTN)_/, "");

  function loadExtras(): string[] {
    try {
      return JSON.parse(localStorage.getItem(storageKey()) ?? "[]");
    } catch {
      return [];
    }
  }

  function saveExtras() {
    try {
      localStorage.setItem(storageKey(), JSON.stringify(extras));
    } catch {}
  }

  function forgetExtras() {
    extras = [];
    saveExtras();
  }

  const timers: Partial<Record<WheelDir, ReturnType<typeof setTimeout>>> = {};
  function flash(dir: WheelDir) {
    wheel[dir] = true;
    clearTimeout(timers[dir]);
    timers[dir] = setTimeout(() => (wheel[dir] = false), 160);
  }

  function record(line: string) {
    log = [line, ...log].slice(0, 8);
  }

  function handle(e: InputEvent) {
    if (e.kind === "button") {
      const next = new Set(held);
      if (e.pressed) next.add(e.name);
      else next.delete(e.name);
      held = next;
      if (e.pressed && !KNOWN.has(e.name) && !extras.includes(e.name)) {
        extras = [...extras, e.name];
        saveExtras();
      }
      if (e.pressed) record(`${[...next].map(short).join(" + ")}`);
    } else {
      const dir: WheelDir =
        e.axis === "vertical" ? (e.notches > 0 ? "up" : "down") : e.notches > 0 ? "right" : "left";
      flash(dir);
      if (held.size) record(`${[...held].map(short).join(" + ")} + ${WHEEL_LABEL[dir]}`);
    }
  }

  onMount(() => {
    invoke("watch_mouse", { path: device.path });
    const off = listen<InputEvent>("input", (e) => {
      if (e.payload.path === device.path) handle(e.payload);
    });
    return () => off.then((f) => f());
  });
</script>

<section class="mouse">
  <div class="groups">
    <div class="group">
      <h3>Buttons</h3>
      <div class="row">
        {#each MAIN as [name, label]}
          <div class="tile" class:on={held.has(name)}><b>{label}</b><code>{name}</code></div>
        {/each}
      </div>
    </div>

    <div class="group">
      <h3>Wheel</h3>
      <div class="row">
        {#each WHEEL as dir}
          <div class="tile small" class:on={wheel[dir]}><b>{WHEEL_LABEL[dir]}</b></div>
        {/each}
      </div>
    </div>

    <div class="group">
      <h3>Thumb</h3>
      <div class="row">
        {#each THUMB as [name, label]}
          <div class="tile" class:on={held.has(name)}><b>{label}</b><code>{name}</code></div>
        {/each}
      </div>
    </div>

    <div class="group">
      <h3>
        Extra buttons
        {#if extras.length}<button class="link" onclick={forgetExtras}>forget</button>{/if}
      </h3>
      <div class="row">
        {#each extras as name}
          <div class="tile" class:on={held.has(name)}><b>{short(name)}</b><code>{name}</code></div>
        {/each}
        <div class="tile ghost">Press any other button to add it here</div>
      </div>
    </div>
  </div>

  <aside>
    <h3>Chord preview</h3>
    {#if log.length === 0}
      <p class="muted">Hold buttons together or hold one and scroll. Combinations show up here.</p>
    {:else}
      <ol>
        {#each log as line, i}
          <li class:latest={i === 0}>{line}</li>
        {/each}
      </ol>
    {/if}
    <p class="note">
      Delta only listens to this mouse. It never grabs it, so pointer speed, acceleration and DPI are untouched.
    </p>
  </aside>
</section>

<style>
  .mouse {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    gap: 18px;
    background: var(--mantle);
    border: 1px solid var(--surface0);
    border-radius: 16px;
    padding: 22px;
  }
  .groups {
    display: grid;
    gap: 18px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .tile {
    min-width: 120px;
    min-height: 56px;
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--key-mod);
    border: 1px solid var(--crust);
    display: grid;
    align-content: center;
    gap: 3px;
    transition:
      background 60ms ease-out,
      box-shadow 60ms ease-out;
  }
  .tile.small {
    min-width: 90px;
  }
  .tile b {
    font-weight: 600;
    font-size: 13px;
  }
  .tile code {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--subtext0);
  }
  .tile.on {
    background: var(--blue);
    box-shadow: 0 0 10px var(--blue);
    color: var(--crust);
  }
  .tile.on code {
    color: var(--crust);
  }
  .tile.ghost {
    background: transparent;
    border: 1px dashed var(--surface2);
    color: var(--overlay0);
    font-size: 12px;
    max-width: 180px;
  }
  .link {
    font: inherit;
    font-size: 11px;
    text-transform: none;
    letter-spacing: 0;
    background: none;
    border: none;
    color: var(--blue);
    cursor: pointer;
    padding: 0;
  }
  aside {
    border-left: 1px solid var(--surface0);
    padding-left: 18px;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--subtext0);
  }
  .latest {
    color: var(--blue);
  }
  .muted {
    color: var(--overlay0);
    font-size: 13px;
  }
  .note {
    margin-top: 18px;
    font-size: 12px;
    color: var(--teal);
  }
</style>
