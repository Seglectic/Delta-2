<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Keyboard from "./lib/Keyboard.svelte";
  import KeyPanel from "./lib/KeyPanel.svelte";
  import MousePanel from "./lib/MousePanel.svelte";
  import { keyId, modifierLabel, qmkName } from "./lib/bindings";
  import type { ConfigInfo, InputDevice, KeyboardInfo, KnobTurn, Pressed } from "./lib/types";

  let keyboards = $state<KeyboardInfo[]>([]);
  let mice = $state<InputDevice[]>([]);
  let current = $state(0);
  /** Which device tab is open: a keyboard index, or a mouse's input path. */
  let tab = $state<{ kind: "keyboard"; index: number } | { kind: "mouse"; path: string }>({
    kind: "keyboard",
    index: 0,
  });
  let turning = $state<Record<number, "left" | "right">>({});
  let configInfo = $state<ConfigInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  /** Firmware layer picked in the tabs (remembered between runs). */
  let chosenLayer = $state(readNumber("delta.layer", 0));
  /** Manual "show the Caps layer" toggle. */
  let deltaToggle = $state(false);
  let selected = $state<string | null>(null);
  let pressed = $state(new Set<string>());

  const keyboard = $derived(keyboards[current]);
  const definition = $derived(keyboard?.definition ?? null);
  const bindings = $derived(configInfo?.config?.bindings ?? []);
  const modifierKey = $derived(configInfo?.config?.engine.modifier_key ?? "capslock");
  const baseLayer = $derived(Math.min(chosenLayer, Math.max(0, (keyboard?.layers.length ?? 1) - 1)));

  /** Keycode names of everything held right now, read on the chosen layer. */
  const heldNames = $derived(
    [...pressed].map((id) => {
      const [r, c] = id.split(",").map(Number);
      return keyboard?.layers[baseLayer]?.[r]?.[c];
    }),
  );
  /** Holding a firmware MO(n) key shows layer n, like the keyboard does. */
  const shownLayer = $derived.by(() => {
    const mo = heldNames.map((kc) => kc?.name.match(/^MO\((\d+)\)$/)).find(Boolean);
    return mo ? Number(mo[1]) : baseLayer;
  });
  /** Holding the delta modifier shows the Caps layer. */
  const deltaHeld = $derived(heldNames.some((kc) => kc?.name === qmkName(modifierKey)));
  const deltaView = $derived(deltaToggle || deltaHeld);

  /** "Layer 1 · Fn" for layers some MO() key reaches. */
  const layerNames = $derived.by(() => {
    const reached = new Set<number>();
    for (const layer of keyboard?.layers ?? [])
      for (const row of layer)
        for (const kc of row) {
          const m = kc.name.match(/^MO\((\d+)\)$/);
          if (m) reached.add(Number(m[1]));
        }
    return (keyboard?.layers ?? []).map((_, i) => (reached.has(i) ? `Layer ${i} · Fn` : `Layer ${i}`));
  });

  const selectedKey = $derived(definition?.keys.find((k) => keyId(k) === selected) ?? null);
  const mouse = $derived(tab.kind === "mouse" ? mice.find((m) => m.path === (tab as { path: string }).path) : undefined);

  async function openKeyboard(i: number) {
    tab = { kind: "keyboard", index: i };
    if (i !== current) {
      current = i;
      pressed = new Set();
      if (keyboards[i]?.definition) await invoke("watch_keys", { path: keyboards[i].path });
    }
  }

  const knobTimers: Record<number, ReturnType<typeof setTimeout>> = {};
  function knobTurned(t: KnobTurn) {
    if (t.path !== keyboard?.path) return;
    turning = { ...turning, [t.encoder]: t.direction };
    clearTimeout(knobTimers[t.encoder]);
    knobTimers[t.encoder] = setTimeout(() => {
      const { [t.encoder]: _, ...rest } = turning;
      turning = rest;
    }, 180);
  }

  function readNumber(key: string, fallback: number): number {
    try {
      const v = localStorage.getItem(key);
      return v === null ? fallback : Number(v);
    } catch {
      return fallback;
    }
  }

  function chooseLayer(i: number) {
    chosenLayer = i;
    try {
      localStorage.setItem("delta.layer", String(i));
    } catch {}
  }

  async function load() {
    loading = true;
    error = null;
    try {
      [configInfo, keyboards, mice] = await Promise.all([
        invoke<ConfigInfo>("config"),
        invoke<KeyboardInfo[]>("keyboards"),
        invoke<InputDevice[]>("mice"),
      ]);
      current = Math.max(0, keyboards.findIndex((k) => k.definition && !k.error));
      if (tab.kind === "keyboard") tab = { kind: "keyboard", index: current };
      if (keyboards[current]?.definition) await invoke("watch_keys", { path: keyboards[current].path });
      await invoke("watch_knobs");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    const unlisten = [
      listen<Pressed>("pressed", (e) => {
        if (e.payload.path === keyboard?.path) pressed = new Set(e.payload.keys.map(([r, c]) => `${r},${c}`));
      }),
      listen<KnobTurn>("knob", (e) => knobTurned(e.payload)),
      listen<string>("keyboard-error", (e) => {
        error = `Lost the keyboard: ${e.payload}`;
        pressed = new Set();
      }),
    ];
    load();
    return () => unlisten.forEach((p) => p.then((f) => f()));
  });
</script>

<main>
  <header class="top">
    <div class="title">
      <span class="logo">Δ</span>
      <div>
        <h1>{mouse ? mouse.name : (definition?.name ?? "Delta")}</h1>
        {#if mouse}
          <span class="muted"
            >{mouse.vendor_id.toString(16).padStart(4, "0")}:{mouse.product_id.toString(16).padStart(4, "0")} · {mouse.path}</span
          >
        {:else if keyboard}
          <span class="muted"
            >{keyboard.device_name} · {keyboard.vendor_id.toString(16).padStart(4, "0")}:{keyboard.product_id
              .toString(16)
              .padStart(4, "0")}{keyboard.protocol !== null ? ` · VIA ${keyboard.protocol}` : ""}</span
          >
        {/if}
      </div>
    </div>
    <button class="ghost" onclick={load} disabled={loading}>{loading ? "Reading…" : "Re-read"}</button>
  </header>

  {#if error}
    <p class="banner error">{error}</p>
  {/if}
  {#if keyboard?.error}
    <p class="banner error">{keyboard.error}</p>
  {/if}

  {#if keyboards.length || mice.length}
    <nav class="devices">
      {#each keyboards as k, i}
        <button class:on={tab.kind === "keyboard" && i === current} onclick={() => openKeyboard(i)}>
          ⌨ {k.definition?.name ?? k.device_name}
        </button>
      {/each}
      {#each mice as m (m.path)}
        <button class:on={mouse?.path === m.path} onclick={() => (tab = { kind: "mouse", path: m.path })}>
          🖱 {m.name}
        </button>
      {/each}
    </nav>
  {/if}

  {#if mouse}
    {#key mouse.path}
      <MousePanel device={mouse} />
    {/key}
  {:else if definition && keyboard}
    <nav class="layers">
      {#each layerNames as name, i}
        <button class:on={i === baseLayer} class:shown={i === shownLayer && i !== baseLayer} onclick={() => chooseLayer(i)}>
          {name}
        </button>
      {/each}
      <span class="spacer"></span>
      <button class="delta-toggle" class:on={deltaView} onclick={() => (deltaToggle = !deltaToggle)}>
        Δ {modifierLabel(modifierKey)} layer{deltaHeld ? " (held)" : ""}
      </button>
    </nav>

    <section class="board" class:delta={deltaView}>
      <Keyboard
        {definition}
        layer={keyboard.layers[shownLayer]}
        baseLayer={keyboard.layers[baseLayer]}
        {bindings}
        {modifierKey}
        {deltaView}
        {pressed}
        {turning}
        {selected}
        onselect={(id) => (selected = selected === id ? null : id)}
      />
    </section>

    <section class="held">
      <span class="label">Held</span>
      {#if pressed.size === 0}
        <span class="muted">press a key on the keyboard</span>
      {:else}
        {#each heldNames as kc}
          <code>{kc?.name ?? "?"}</code>
        {/each}
      {/if}
    </section>

    {#if selectedKey}
      <KeyPanel key={selectedKey} {keyboard} {layerNames} {bindings} {modifierKey} />
    {:else}
      <p class="muted hint">Click a key to see what it sends on each layer and what Delta does with it.</p>
    {/if}
  {:else if !loading && keyboards.length === 0}
    <p class="banner">No keyboard with a VIA interface found. Plug one in over USB and press Re-read.</p>
  {/if}

  <footer>
    {#if configInfo}
      <span>Bindings from <code>{configInfo.source}</code></span>
      {#if configInfo.legacy}
        <span class="note">preview of the old delta.conf; run <code>delta import</code> to switch</span>
      {/if}
      {#each configInfo.problems as p}
        <span class="note">{p}</span>
      {/each}
    {/if}
  </footer>
</main>

<style>
  main {
    max-width: 1500px;
    margin: 0 auto;
    padding: 18px 24px 28px;
    display: grid;
    gap: 14px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }
  .logo {
    font-size: 30px;
    font-weight: 700;
    color: var(--blue);
    line-height: 1;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  .devices {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--surface0);
    padding-bottom: 12px;
  }
  .layers {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  button {
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--surface0);
    border: 1px solid var(--surface1);
    border-radius: 8px;
    padding: 6px 12px;
    cursor: pointer;
  }
  button:hover {
    background: var(--surface1);
  }
  button.on {
    background: var(--blue);
    border-color: var(--blue);
    color: var(--crust);
  }
  button.shown {
    border-color: var(--blue);
    color: var(--blue);
  }
  .delta-toggle.on {
    background: var(--mauve);
    border-color: var(--mauve);
  }
  button.ghost {
    background: transparent;
  }
  .board {
    background: var(--mantle);
    border: 1px solid var(--surface0);
    border-radius: 16px;
    padding: 22px;
    transition: border-color 120ms;
  }
  .board.delta {
    border-color: var(--mauve);
  }
  .held {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
    flex-wrap: wrap;
  }
  .held .label {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
  }
  .held code {
    background: var(--blue);
    color: var(--crust);
    border-radius: 6px;
    padding: 2px 8px;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
  }
  .banner {
    margin: 0;
    padding: 10px 14px;
    border-radius: 10px;
    background: var(--surface0);
  }
  .banner.error {
    background: color-mix(in srgb, var(--red) 18%, var(--base));
    color: var(--red);
  }
  .muted {
    color: var(--overlay0);
    font-size: 13px;
  }
  .hint {
    margin: 0;
  }
  footer {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--overlay0);
  }
  .note {
    color: var(--peach);
  }
</style>
