<script lang="ts">
  import { onMount } from "svelte";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Keyboard from "./lib/Keyboard.svelte";
  import KeyPanel from "./lib/KeyPanel.svelte";
  import MousePanel from "./lib/MousePanel.svelte";
  import LayoutsPanel from "./lib/LayoutsPanel.svelte";
  import BindingEditor from "./lib/BindingEditor.svelte";
  import LightingPanel from "./lib/LightingPanel.svelte";
  import ProfilePanel from "./lib/ProfilePanel.svelte";
  import ScopePicker from "./lib/ScopePicker.svelte";
  import KeyLighting from "./lib/KeyLighting.svelte";
  import EffectPanel from "./lib/EffectPanel.svelte";
  import { effectFrame, effectOrigin } from "./lib/effects";
  import { keyId, keyLightColor, modifierLabel, qmkName } from "./lib/bindings";
  import type {
    BaseLayer,
    App as DeltaApp,
    AppChoice,
    Binding,
    Config,
    ConfigInfo,
    DeltaColors,
    KeyboardInfo,
    KeyboardLighting,
    KeyEffect,
    KeyLight,
    LightingProfile,
    KnobTurn,
    Mouse,
    Pressed,
    SetupState,
    Shortcut,
  } from "./lib/types";

  let keyboards = $state<KeyboardInfo[]>([]);
  let mice = $state<Mouse[]>([]);
  let current = $state(0);
  /** Which device tab is open: a keyboard index, or a mouse by USB ID. */
  let tab = $state<{ kind: "keyboard"; index: number } | { kind: "mouse"; id: string }>({
    kind: "keyboard",
    index: 0,
  });
  let turning = $state<Record<number, "left" | "right">>({});

  // Binding editor
  let editing = $state<{ keyName: string; binding: Binding | null } | null>(null);
  let editorProblems = $state<string[]>([]);
  let saving = $state(false);
  let shortcuts = $state<Shortcut[]>([]);
  let suggestions = $state<AppChoice[]>([]);
  let running = $state<AppChoice[]>([]);
  /** deltad's status map, or null when it isn't running. */
  let deltad = $state<Record<string, string> | null>(null);
  let setup = $state<SetupState | null>(null);
  let settingUp = $state(false);
  let showLayouts = $state(false);
  /** A newer release on GitHub (released builds only). */
  let update = $state<Update | null>(null);
  let updating = $state<string | null>(null);
  let updateOpen = $state(false);
  let configInfo = $state<ConfigInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  /** Firmware layer picked in the tabs, for boards that don't say which they're on
      (remembered between runs). */
  let chosenLayer = $state(readNumber("delta.layer", 0));
  /** Another layer opened in the tabs just to look at; the board's own stays in use. */
  let viewLayer = $state<number | null>(null);
  /** Clicking the delta key keeps the Caps layer shown. */
  let deltaToggle = $state(false);
  let deltaColors = $state<DeltaColors | null>(null);
  /** The keyboard's LEDs as last read back. */
  let ledState = $state<KeyboardLighting | null>(null);

  /** The app the lighting sidebar is for (null = the global profile). */
  let scope = $state<string | null>(null);
  /** Its `[[apps]]` entry when it's a running app not saved yet: added with the first save. */
  let scopeNewApp = $state<DeltaApp | null>(null);

  /** The picked profile with a key's colour being dragged in the key panel. */
  let keyPreview = $state<LightingProfile | null>(null);

  // Per-key profile painting
  let painting = $state<LightingProfile | null>(null);
  let brush = $state<KeyLight>({ key: "", color: "#FFFFFF", brightness: 255 });
  let erase = $state(false);

  // Key effect editing
  let editingEffect = $state<KeyEffect | null>(null);
  let picking = $state<"trigger" | "origin" | null>(null);
  /** Animation clock for the on-screen preview (ms). */
  let effectClock = $state(0);
  let selected = $state<string | null>(null);
  let pressed = $state(new Set<string>());

  const keyboard = $derived(keyboards[current]);
  const scopeProfile = $derived(
    (configInfo?.config?.lighting?.profiles ?? []).find((p) => (p.app ?? null) === scope),
  );
  const definition = $derived(keyboard?.definition ?? null);
  const bindings = $derived(configInfo?.config?.bindings ?? []);
  const modifierKey = $derived(configInfo?.config?.engine.modifier_key ?? "capslock");
  /** The layer in use: the board's, else the one picked in the tabs. */
  const baseLayer = $derived(
    Math.min(keyboard?.base_layer ?? chosenLayer, Math.max(0, (keyboard?.layers.length ?? 1) - 1)),
  );

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
    return mo ? Number(mo[1]) : (viewLayer ?? baseLayer);
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

  function selectKey(id: string) {
    const k = definition?.keys.find((k) => keyId(k) === id);
    if (painting) {
      if (k) paintKey(k.row, k.col);
      return;
    }
    if (editingEffect) {
      const name = k && keyboard?.layers[baseLayer]?.[k.row]?.[k.col]?.name;
      if (name && picking === "trigger") editingEffect.trigger = name;
      if (name && picking === "origin") editingEffect.origin = name;
      picking = null;
      return;
    }
    const isModifier = k && keyboard?.layers[baseLayer]?.[k.row]?.[k.col]?.name === qmkName(modifierKey);
    if (isModifier) {
      deltaToggle = !deltaToggle;
      selected = deltaToggle ? id : null;
    } else {
      selected = selected === id ? null : id;
    }
  }

  function paintKey(row: number, col: number) {
    const name = keyboard?.layers[baseLayer]?.[row]?.[col]?.name;
    if (!painting || !name) return;
    const keys = (painting.keys ?? []).filter((k) => k.key !== name);
    const had = painting.keys?.find((k) => k.key === name);
    const same = had && had.color.toUpperCase() === brush.color.toUpperCase() && had.brightness === brush.brightness;
    if (!erase && !same) keys.push({ key: name, color: brush.color.toUpperCase(), brightness: brush.brightness });
    painting.keys = keys;
  }

  /** "Enter" for KC_ENTER, from the keymap's base layer. */
  function keyLabel(name: string): string {
    for (const row of keyboard?.layers[baseLayer] ?? []) for (const kc of row) if (kc.name === name) return kc.label || name;
    return name;
  }

  function pickScope(target: { app: string | null } | { running: AppChoice }) {
    if ("running" in target) {
      const r = target.running;
      scopeNewApp = { name: r.name, launch: r.launch, ...(r.match.length ? { match: r.match } : {}) };
      scope = r.name;
    } else {
      scopeNewApp = null;
      scope = target.app;
    }
  }

  /** Save `config`, adding the picked app to `[[apps]]` if it's new. */
  async function saveWithScope(config: Config): Promise<boolean> {
    if (scopeNewApp) config.apps = [...(config.apps ?? []), scopeNewApp];
    const ok = await save(config);
    if (ok) scopeNewApp = null;
    else error = editorProblems.join("; ");
    return ok;
  }

  /** Edit `effect`, or a new one for the picked app (started by `trigger`, else picked). */
  function openEffect(effect: KeyEffect | null, trigger?: string) {
    closeProfile();
    picking = null;
    if (effect) {
      editingEffect = structuredClone($state.snapshot(effect));
      return;
    }
    editingEffect = {
      id: "",
      ...(scope ? { app: scope } : {}),
      trigger: trigger ?? "KC_ENTER",
      color: "#89B4FA",
      strength: 255,
      distance: 8,
      width: 1.5,
      duration_ms: 600,
      fade: 1,
    };
    picking = trigger ? null : "trigger";
  }

  function closeEffect() {
    editingEffect = null;
    picking = null;
    invoke("preview_effect", { effect: "" }).catch(() => {});
  }

  /** The effect as saved: an ID if it's new, colours upper-case, unset parts left out. */
  function cleanEffect(e: KeyEffect, taken: string[]): KeyEffect {
    let id = e.id;
    if (!id) {
      const stem = `${e.app ?? "all"}-${e.trigger.replace(/^KC_/, "")}`.toLowerCase().replace(/[^a-z0-9_-]/g, "");
      id = stem;
      for (let n = 2; taken.includes(id); n++) id = `${stem}-${n}`;
    }
    const out: KeyEffect = { ...e, id, color: e.color.toUpperCase() };
    if (e.color_end) out.color_end = e.color_end.toUpperCase();
    else delete out.color_end;
    if (!e.origin) delete out.origin;
    return out;
  }

  async function saveEffect() {
    if (!editingEffect) return;
    const config = editableConfig();
    const list = config.lighting?.effects ?? [];
    const effect = cleanEffect($state.snapshot(editingEffect), list.map((e) => e.id));
    const at = list.findIndex((e) => e.id === effect.id);
    if (at >= 0) list[at] = effect;
    else list.push(effect);
    config.lighting = { ...config.lighting, effects: list };
    if (await saveWithScope(config)) closeEffect();
  }

  async function deleteEffect() {
    if (!editingEffect || !confirm(`Delete this ${keyLabel(editingEffect.trigger)} effect?`)) return;
    const config = editableConfig();
    const id = editingEffect.id;
    config.lighting = { ...config.lighting, effects: (config.lighting?.effects ?? []).filter((e) => e.id !== id) };
    if (await save(config)) closeEffect();
    else error = editorProblems.join("; ");
  }

  // Loop the effect being edited on the keyboard (deltad stops after 10 s without a
  // refresh) and on the board here
  $effect(() => {
    if (!editingEffect) return;
    const json = JSON.stringify(cleanEffect($state.snapshot(editingEffect), []));
    const send = () => invoke("preview_effect", { effect: json }).catch(() => {});
    send();
    const timer = setInterval(send, 3000);
    return () => clearInterval(timer);
  });
  $effect(() => {
    if (!editingEffect) return;
    const start = performance.now();
    let frame = requestAnimationFrame(function tick(now) {
      effectClock = now - start;
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  });

  /** Paint the picked app's profile (a new one if it has none). */
  function openProfile() {
    closeEffect();
    const found = scopeProfile;
    painting = found ? structuredClone($state.snapshot(found)) : { ...(scope ? { app: scope } : {}), keys: [] };
    deltaToggle = false;
    selected = null;
  }

  function closeProfile() {
    painting = null;
    invoke("preview_lighting", { profile: "" }).catch(() => {});
  }

  /** The picked profile with one key's colour changed (null = unset). */
  function profileWithKey(name: string, light: KeyLight | null): LightingProfile {
    const p: LightingProfile = scopeProfile
      ? structuredClone($state.snapshot(scopeProfile))
      : { ...(scope ? { app: scope } : {}), keys: [] };
    p.keys = [...(p.keys ?? []).filter((k) => k.key !== name), ...(light ? [light] : [])];
    return cleanProfile(p);
  }

  /** Show one key's colour on the keyboard while it's being picked (saving ends it). */
  function previewKeyLight(name: string, light: KeyLight | null) {
    keyPreview = profileWithKey(name, light);
  }

  /** Set or clear one key's colour in the picked profile, straight from the key panel. */
  async function saveKeyLight(name: string, light: KeyLight | null) {
    const config = editableConfig();
    const profile = profileWithKey(name, light);
    const others = (config.lighting?.profiles ?? []).filter((p) => (p.app ?? null) !== (profile.app ?? null));
    const empty = !profile.keys?.length && !profile.background;
    config.lighting = { ...config.lighting, profiles: empty ? others : [...others, profile] };
    await saveWithScope(config);
    keyPreview = null;
  }

  /** The profile as saved: colours upper-case, empty parts left out. */
  function cleanProfile(p: LightingProfile): LightingProfile {
    return {
      ...(p.app ? { app: p.app } : {}),
      ...(p.background ? { background: { ...p.background, color: p.background.color.toUpperCase() } } : {}),
      ...(p.keys?.length ? { keys: p.keys } : {}),
    };
  }

  async function saveProfile() {
    if (!painting) return;
    const config = editableConfig();
    const profile = cleanProfile($state.snapshot(painting));
    const others = (config.lighting?.profiles ?? []).filter((p) => (p.app ?? null) !== (profile.app ?? null));
    config.lighting = { ...config.lighting, profiles: [...others, profile] };
    if (await saveWithScope(config)) closeProfile();
  }

  async function deleteProfile() {
    if (!painting || !confirm(`Delete the ${painting.app ?? "Global"} lighting profile?`)) return;
    const config = editableConfig();
    const app = painting.app ?? null;
    config.lighting = { ...config.lighting, profiles: (config.lighting?.profiles ?? []).filter((p) => (p.app ?? null) !== app) };
    if (await save(config)) closeProfile();
    else error = editorProblems.join("; ");
  }

  // Show the profile being painted on the keyboard (deltad drops it after 10 s
  // without a refresh, so a crashed window can't leave it stuck)
  $effect(() => {
    if (!painting) return;
    const json = JSON.stringify(cleanProfile($state.snapshot(painting)));
    const send = () => invoke("preview_lighting", { profile: json }).catch(() => {});
    send();
    const timer = setInterval(send, 3000);
    return () => clearInterval(timer);
  });

  // Show the picked app's colours on the keyboard, whichever window has focus (Global
  // needs no preview: this window has focus, so the keyboard shows Global already)
  $effect(() => {
    if (painting || editingEffect) return;
    const p = keyPreview ?? (scope !== null ? (scopeProfile ?? null) : null);
    if (!p) {
      invoke("preview_lighting", { profile: "" }).catch(() => {});
      return;
    }
    const json = JSON.stringify(cleanProfile($state.snapshot(p)));
    const send = () => invoke("preview_lighting", { profile: json }).catch(() => {});
    send();
    const timer = setInterval(send, 3000);
    return () => clearInterval(timer);
  });

  /** "row,col" → colour of every key under a profile. */
  function profileLeds(p: LightingProfile): Record<string, string> {
    const out: Record<string, string> = {};
    for (const k of definition?.keys ?? []) {
      const name = keyboard?.layers[baseLayer]?.[k.row]?.[k.col]?.name;
      const light = p.keys?.find((l) => l.key === name) ?? p.background;
      out[`${k.row},${k.col}`] = light ? keyLightColor(light) : "#000000";
    }
    return out;
  }

  /** "row,col" → colour for the board: what's being edited, else the keyboard's LEDs. */
  const boardLeds = $derived.by((): Record<string, string> | null => {
    if (!definition || !keyboard) return null;
    const out: Record<string, string> = {};
    if (painting) return profileLeds(painting);
    if (editingEffect) {
      const e = editingEffect;
      const nameAt = (r: number, c: number) => keyboard.layers[baseLayer]?.[r]?.[c]?.name;
      const origin = effectOrigin(e, definition, nameAt);
      // Over the profile the effect would play on
      const profiles = configInfo?.config?.lighting?.profiles ?? [];
      const p = (e.app && profiles.find((p) => p.app === e.app)) || profiles.find((p) => !p.app);
      return origin ? effectFrame(e, definition, origin, p ? profileLeds(p) : null, effectClock) : null;
    }
    if (!ledState || ledState.animated) return null;
    if (ledState.all) {
      for (const k of definition.keys) out[`${k.row},${k.col}`] = ledState.all;
      return out;
    }
    return ledState.keys;
  });

  // What the keys light up as while the delta key is held
  $effect(() => {
    const config = configInfo?.config;
    if (!config) return;
    invoke<DeltaColors>("delta_colors", { config: $state.snapshot(config) })
      .then((c) => (deltaColors = c))
      .catch(() => (deltaColors = null));
  });
  const mouseId = (m: Mouse) => `${m.vendor_id}:${m.product_id}`;
  const mouse = $derived(tab.kind === "mouse" ? mice.find((m) => mouseId(m) === (tab as { id: string }).id) : undefined);

  const editableConfig = (): Config =>
    structuredClone($state.snapshot(configInfo?.config)) ?? { engine: { backend: "xremap", modifier_key: "capslock" } };

  async function openEditor(keyName: string, binding: Binding | null) {
    editorProblems = [];
    editing = { keyName, binding };
    if (!shortcuts.length) invoke<Shortcut[]>("shortcuts").then((s) => (shortcuts = s)).catch(() => {});
    if (!suggestions.length) invoke<AppChoice[]>("app_suggestions").then((s) => (suggestions = s));
    running = [];
    refreshRunning();
  }

  /** Fresh every time: what's open changes. */
  function refreshRunning() {
    invoke<string>("running_apps")
      .then((json) => (running = JSON.parse(json)))
      .catch(() => {});
  }

  async function save(config: Config): Promise<boolean> {
    // Never save over a config that didn't load: the fallback would wipe the file
    if (!configInfo?.config) {
      editorProblems = ["delta.toml didn't load, so nothing was saved; fix it first"];
      return false;
    }
    saving = true;
    try {
      configInfo = await invoke<ConfigInfo>("save_config", { config });
      // deltad polls the file once a second; check what it made of it
      setTimeout(refreshDeltad, 1500);
      return true;
    } catch (problems) {
      editorProblems = Array.isArray(problems) ? problems : [String(problems)];
      return false;
    } finally {
      saving = false;
    }
  }

  async function saveBinding(binding: Binding, newApp: DeltaApp | null) {
    const config = editableConfig();
    const list = config.bindings ?? [];
    const at = list.findIndex((b) => b.id === binding.id);
    if (at >= 0) list[at] = binding;
    else list.push(binding);
    config.bindings = list;
    if (newApp) config.apps = [...(config.apps ?? []), newApp];
    if (await save(config)) editing = null;
  }

  async function removeBinding(binding: Binding) {
    if (!confirm(`Remove "${binding.id}"?`)) return;
    const config = editableConfig();
    config.bindings = (config.bindings ?? []).filter((b) => b.id !== binding.id);
    if (!(await save(config))) error = editorProblems.join("; ");
  }

  async function refreshDeltad() {
    deltad = await invoke<Record<string, string> | null>("deltad_status");
  }

  async function refreshSetup() {
    setup = await invoke<SetupState>("setup_state").catch(() => null);
  }

  async function checkForUpdate() {
    if (!setup || setup.dev) return;
    update = await check().catch((e) => {
      console.warn("update check failed", e);
      return null;
    });
  }

  /** Swap in the new AppImage and restart; at start the new build replaces deltad. */
  async function installUpdate() {
    if (!update) return;
    let total = 0;
    let done = 0;
    updating = "Downloading…";
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        if (e.event === "Progress") {
          done += e.data.chunkLength;
          if (total) updating = `Downloading… ${Math.round((done / total) * 100)}%`;
        }
        if (e.event === "Finished") updating = "Restarting…";
      });
      await relaunch();
    } catch (e) {
      error = `Update failed: ${e}`;
      updating = null;
    }
  }

  async function runSetup() {
    settingUp = true;
    try {
      await invoke("run_setup");
    } catch (e) {
      if (e !== "cancelled") error = `Setup failed: ${e}`;
    } finally {
      settingUp = false;
      await refreshSetup();
    }
  }

  async function openKeyboard(i: number) {
    tab = { kind: "keyboard", index: i };
    if (i !== current) {
      current = i;
      viewLayer = null;
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
    // The board says which layer it's on: other tabs are only for looking
    if (keyboard?.base_layer != null) {
      viewLayer = i === baseLayer ? null : i;
      return;
    }
    chosenLayer = i;
    try {
      localStorage.setItem("delta.layer", String(i));
    } catch {}
  }

  async function reloadConfig() {
    try {
      const fresh = await invoke<ConfigInfo>("config");
      // Only replace when it changed, so open panels keep their edits
      if (JSON.stringify(fresh) !== JSON.stringify(configInfo)) configInfo = fresh;
    } catch {}
  }

  async function load() {
    loading = true;
    error = null;
    try {
      [configInfo, keyboards, mice] = await Promise.all([
        invoke<ConfigInfo>("config"),
        invoke<KeyboardInfo[]>("keyboards"),
        invoke<Mouse[]>("mice"),
      ]);
      current = Math.max(0, keyboards.findIndex((k) => k.definition && !k.error));
      if (tab.kind === "keyboard") tab = { kind: "keyboard", index: current };
      if (keyboards[current]?.definition) await invoke("watch_keys", { path: keyboards[current].path });
      await invoke("watch_knobs");
      refreshDeltad();
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
      // The Mac/Windows switch moved
      listen<BaseLayer>("base-layer", (e) => {
        const k = keyboards.find((k) => k.path === e.payload.path);
        if (k) k.base_layer = e.payload.layer;
        viewLayer = null;
      }),
      // The released app finished installing deltad
      listen("setup-changed", () => {
        refreshSetup();
        refreshDeltad();
      }),
      listen<string>("keyboard-error", (e) => {
        error = `Lost the keyboard: ${e.payload}`;
        pressed = new Set();
      }),
    ];
    // Re-read by itself when a keyboard or mouse is plugged in or out
    let signature: string | null = null;
    const devicePoll = setInterval(async () => {
      const now = await invoke<string>("devices_signature").catch(() => signature);
      if (signature !== null && now !== signature && !loading) load();
      signature = now;
    }, 2000);
    // and pick up delta.toml edits made outside the app
    window.addEventListener("focus", reloadConfig);
    // Mirror the keyboard's LEDs
    const ledPoll = setInterval(async () => {
      if (loading || !keyboard?.definition) return;
      ledState = await invoke<KeyboardLighting>("keyboard_lighting", { path: keyboard.path }).catch(() => null);
    }, 500);
    load();
    refreshSetup().then(checkForUpdate);
    return () => {
      clearInterval(devicePoll);
      clearInterval(ledPoll);
      invoke("preview_lighting", { profile: "" }).catch(() => {});
      invoke("preview_effect", { effect: "" }).catch(() => {});
      window.removeEventListener("focus", reloadConfig);
      unlisten.forEach((p) => p.then((f) => f()));
    };
  });
</script>

<main>
  <header class="top">
    <span class="logo">Δ</span>
    {#if definition && keyboard && !mouse}
      <ScopePicker
        apps={(configInfo?.config?.apps ?? []).map((a) => a.name)}
        profiles={configInfo?.config?.lighting?.profiles ?? []}
        effects={configInfo?.config?.lighting?.effects ?? []}
        {running}
        {scope}
        locked={!!painting || !!editingEffect}
        onrefresh={refreshRunning}
        onpick={pickScope}
      />
    {/if}
    <div class="title">
      <div>
        <h1>{mouse ? mouse.name : (definition?.name ?? "Delta")}</h1>
        {#if mouse}
          <span class="muted"
            >{mouse.vendor_id.toString(16).padStart(4, "0")}:{mouse.product_id.toString(16).padStart(4, "0")} · {mouse.paths.join(", ")}</span
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
    {#if loading}<span class="muted">Reading…</span>{/if}
    {#if keyboards.length + mice.length > 1}
      <nav class="devices">
        {#each keyboards as k, i}
          <button class:on={tab.kind === "keyboard" && i === current} onclick={() => openKeyboard(i)}>
            ⌨ {k.definition?.name ?? k.device_name}
          </button>
        {/each}
        {#each mice as m (mouseId(m))}
          <button class:on={mouse === m} onclick={() => (tab = { kind: "mouse", id: mouseId(m) })}>
            🖱 {m.name}
          </button>
        {/each}
      </nav>
    {/if}
  </header>

  {#if setup?.needs_setup}
    <div class="banner setup">
      <span>
        Delta needs permission to read your keyboards and send keys: a udev rule for
        <code>/dev/uinput</code> and <code>/dev/hidraw*</code>, and your user in the <code>input</code> group.
        This asks for your password once.
      </span>
      <button onclick={runSetup} disabled={settingUp}>{settingUp ? "Waiting…" : "Set up"}</button>
    </div>
  {:else if setup?.needs_relogin}
    <p class="banner setup">Permissions are set up. Log out and back in so they apply to this session.</p>
  {/if}
  {#if setup?.install_error}
    <p class="banner error">Couldn't install deltad: {setup.install_error}</p>
  {/if}

  {#if error}
    <p class="banner error">{error}</p>
  {/if}
  {#if keyboard?.error}
    <div class="banner error setup">
      <span>{keyboard.error}</span>
      <button onclick={() => (showLayouts = true)}>Layouts…</button>
    </div>
  {/if}

  {#if mouse}
    {#key mouseId(mouse)}
      <MousePanel device={mouse} />
    {/key}
  {:else if definition && keyboard}
    <nav class="layers">
      {#each layerNames as name, i}
        <button
          class:on={i === baseLayer}
          class:shown={i === shownLayer && i !== baseLayer}
          title={keyboard.base_layer == null
            ? "Use this layer"
            : i === baseLayer
              ? "The board is on this layer (its Mac/Windows switch)"
              : "Look at this layer"}
          onclick={() => chooseLayer(i)}
        >
          {name}{#if keyboard.base_layer != null && i === baseLayer}<span class="on-board">&nbsp;· on board</span>{/if}
        </button>
      {/each}
      {#if viewLayer !== null}
        <span class="muted">only looking: the board is on layer {baseLayer}</span>
      {/if}
      <span class="spacer"></span>
      {#if deltaView}
        <span class="delta-chip">
          Δ {modifierLabel(modifierKey)} layer{deltaHeld ? " (held)" : " · click Δ to go back"}
        </span>
      {:else}
        <span class="muted">Click Δ to see the {modifierLabel(modifierKey)} layer</span>
      {/if}
    </nav>

    <div class="work">
      <div class="center">
        <section
          class="board"
          class:delta={deltaView && !painting && !editingEffect}
          class:painting={!!painting || !!editingEffect}
        >
          <Keyboard
            {definition}
            layer={keyboard.layers[shownLayer]}
            baseLayer={keyboard.layers[baseLayer]}
            {bindings}
            {modifierKey}
            deltaView={deltaView && !painting && !editingEffect}
            {deltaColors}
            leds={boardLeds}
            {pressed}
            {turning}
            {selected}
            onselect={selectKey}
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
          <KeyPanel
            key={selectedKey}
            {keyboard}
            {layerNames}
            {baseLayer}
            {bindings}
            {modifierKey}
            onadd={(keyName) => openEditor(keyName, null)}
            onedit={(b) => openEditor(b.key, b)}
            onremove={removeBinding}
          >
            {#snippet lighting()}
              {@const name = keyboard.layers[baseLayer]?.[selectedKey.row]?.[selectedKey.col]?.name}
              {#if name && selectedKey.encoder === null}
                {#key `${scope}|${name}|${JSON.stringify(scopeProfile?.keys?.find((k) => k.key === name) ?? null)}`}
                  <KeyLighting
                    {name}
                    app={scope}
                    light={scopeProfile?.keys?.find((k) => k.key === name)}
                    effects={(configInfo?.config?.lighting?.effects ?? []).filter(
                      (e) => (e.app ?? null) === scope && e.trigger === name,
                    )}
                    {saving}
                    onpreview={(light) => previewKeyLight(name, light)}
                    onsave={(light) => saveKeyLight(name, light)}
                    oneffect={(effect) => openEffect(effect, name)}
                  />
                {/key}
              {/if}
            {/snippet}
          </KeyPanel>
        {:else}
          <p class="muted hint">Click a key to see what it sends on each layer and what Delta does with it.</p>
        {/if}
      </div>

      <aside class="side">
        <div class="scoped" class:app={scope !== null}>
          {#if !editingEffect}
            <ProfilePanel
              app={scope}
              profile={scopeProfile}
              bind:editing={painting}
              bind:brush
              bind:erase
              {saving}
              onopen={openProfile}
              onsave={saveProfile}
              ondelete={deleteProfile}
              onclose={closeProfile}
            />
          {/if}

          {#if !painting}
            <EffectPanel
              app={scope}
              effects={configInfo?.config?.lighting?.effects ?? []}
              bind:editing={editingEffect}
              bind:picking
              {keyLabel}
              {saving}
              onopen={openEffect}
              onsave={saveEffect}
              ondelete={deleteEffect}
              onclose={closeEffect}
            />
          {/if}
        </div>

        {#if !painting && !editingEffect}
          {#key configInfo?.source + JSON.stringify(configInfo?.config?.lighting ?? {})}
            <LightingPanel
              config={editableConfig()}
              {saving}
              onsave={async (lighting) => {
                const config = editableConfig();
                // The panel only owns these fields; profiles and effects must survive
                config.lighting = {
                  ...config.lighting,
                  normal: undefined,
                  delta_background: undefined,
                  ...lighting,
                };
                if (!(await save(config))) error = editorProblems.join("; ");
              }}
            />
          {/key}
        {/if}
      </aside>
    </div>

  {:else if !loading && keyboards.length === 0}
    <p class="banner">No keyboard with a VIA interface found. Plug one in over USB; Delta picks it up by itself.</p>
  {/if}

  {#if showLayouts}
    <LayoutsPanel onclose={() => (showLayouts = false)} onchanged={load} />
  {/if}

  {#if editing}
    <BindingEditor
      keyName={editing.keyName}
      binding={editing.binding}
      config={editableConfig()}
      {modifierKey}
      {shortcuts}
      {suggestions}
      {running}
      problems={editorProblems}
      {saving}
      onsave={saveBinding}
      oncancel={() => (editing = null)}
    />
  {/if}

  <footer>
    {#if update}
      <div class="update">
        {#if updateOpen}
          <div class="update-pop" role="dialog" aria-label="Update">
            <span><code>{update.currentVersion}</code> → <code>{update.version}</code></span>
            <button class="primary" onclick={installUpdate} disabled={!!updating}>{updating ?? "Update"}</button>
            <button class="link" onclick={() => (updateOpen = false)} disabled={!!updating}>later</button>
          </div>
        {/if}
        <button class="update-dot" title="Update available" onclick={() => (updateOpen = !updateOpen)}>!</button>
      </div>
    {/if}
    <button class="link" onclick={() => (showLayouts = true)}>Layouts…</button>
    {#if setup}
      <span title={setup.owner.kind === "other" ? `deltad.service runs ${setup.owner.exec}` : ""}>
        Delta {setup.app_version}{setup.dev ? " (dev)" : ""}{deltad?.version && deltad.version !== setup.app_version
          ? ` · deltad ${deltad.version}`
          : ""}{setup.owner.kind === "other" ? " · deltad from a dev install" : ""}
      </span>
    {/if}
    {#if deltad}
      <span class:note={!!deltad.last_error}>
        deltad: {deltad.last_error || `applied · xremap ${deltad.xremap} · KWin script ${deltad.kwin_script}`}
      </span>
    {:else}
      <span class="note">deltad isn't running: saved changes take effect after the cutover</span>
    {/if}
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
    height: 100vh;
    box-sizing: border-box;
    padding: 12px 20px 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* Board and key details on the left, lighting on the right: everything in view
     without scrolling the window. Each column scrolls on its own if it must. */
  .work {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 390px;
    gap: 14px;
  }
  .center,
  .side {
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .side {
    padding-right: 2px;
  }
  /* The app picker and what it picks for, apart from the keyboard-wide lighting */
  .scoped {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-left: 10px;
    border-left: 2px solid var(--blue);
  }
  .scoped.app {
    border-left-color: var(--peach);
  }
  @media (max-width: 1100px) {
    main {
      height: auto;
    }
    .work {
      grid-template-columns: minmax(0, 1fr);
    }
    .center,
    .side {
      overflow: visible;
    }
  }
  .top {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  /* The keyboard's name, in the middle: the profile is the header's title */
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    justify-content: center;
    text-align: center;
  }
  .logo {
    font-size: 30px;
    font-weight: 700;
    color: var(--blue);
    line-height: 1;
  }
  h1 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .devices {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
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
  .on-board {
    opacity: 0.75;
    font-size: 12px;
  }
  button.shown {
    border-color: var(--blue);
    color: var(--blue);
  }
  .delta-chip {
    font-size: 13px;
    background: var(--mauve);
    color: var(--crust);
    border-radius: 8px;
    padding: 6px 12px;
  }
  .board {
    flex-shrink: 0;
    background: var(--mantle);
    border: 1px solid var(--surface0);
    border-radius: 16px;
    padding: 16px;
    transition: border-color 120ms;
  }
  /* Leave room under the board for the key details */
  .board :global(.keyboard) {
    max-height: max(240px, 100vh - 400px);
  }
  .board.delta {
    border-color: var(--mauve);
  }
  .board.painting {
    border-color: var(--peach);
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
  .banner.setup {
    display: flex;
    align-items: center;
    gap: 14px;
    background: color-mix(in srgb, var(--peach) 16%, var(--base));
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
  .update {
    position: relative;
    display: flex;
  }
  .update-dot {
    width: 16px;
    height: 16px;
    padding: 0;
    border-radius: 50%;
    border: none;
    background: var(--peach);
    color: var(--crust);
    font-weight: 700;
    font-size: 11px;
    line-height: 16px;
    cursor: pointer;
  }
  .update-pop {
    position: absolute;
    left: 0;
    bottom: calc(100% + 8px);
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--mantle);
    border: 1px solid var(--surface1);
    font-size: 13px;
    white-space: nowrap;
  }
  .update-pop .primary {
    background: var(--blue);
    border-color: var(--blue);
    color: var(--crust);
  }
  footer .update-pop .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--overlay1);
    cursor: pointer;
    font: inherit;
  }
  footer .link {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--blue);
    cursor: pointer;
  }
  footer {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    flex-shrink: 0;
    font-size: 12px;
    color: var(--overlay0);
  }
  .note {
    color: var(--peach);
  }
</style>
