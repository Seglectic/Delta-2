<script lang="ts">
  import { untrack } from "svelte";
  import type { ActionKind, App, AppChoice, Binding, Config, Shortcut } from "./types";
  import { isWindowAction, modifierLabel, typesText } from "./bindings";

  interface Props {
    /** Delta key name being bound, e.g. `F13`. */
    keyName: string;
    /** Existing binding to edit, or null to add one. */
    binding: Binding | null;
    config: Config;
    modifierKey: string;
    shortcuts: Shortcut[];
    /** Installed apps. */
    suggestions: AppChoice[];
    /** Apps with open windows right now. */
    running: AppChoice[];
    /** Problems from the last save attempt. */
    problems: string[];
    saving: boolean;
    onsave: (binding: Binding, newApp: App | null) => void;
    oncancel: () => void;
  }

  let { keyName, binding, config, modifierKey, shortcuts, suggestions, running, problems, saving, onsave, oncancel }: Props =
    $props();

  const ACTIONS: [ActionKind, string][] = [
    ["tile", "Tile window"],
    ["span", "Resize window"],
    ["fixed", "Center window"],
    ["app", "Focus or launch app"],
    ["keys", "Send keys"],
    ["command", "Run command"],
    ["kwin_shortcut", "KDE shortcut"],
  ];
  /** The six tile placements, as layout + side. */
  const TILES: [string, string, string][] = [
    ["Left ½", "halves", "left"],
    ["Right ½", "halves", "right"],
    ["Left ⅓", "thirds_left", "left"],
    ["Right ⅔", "thirds_left", "right"],
    ["Left ⅔", "thirds_right", "left"],
    ["Right ⅓", "thirds_right", "right"],
  ];

  // Form state is seeded once from the props; the editor is re-created for each edit
  const seed = untrack(() => ({ binding, config, shortcuts }));
  const b = seed.binding as Record<string, any> | null;
  let layer = $state<"delta" | "base">(seed.binding?.layer ?? "delta");
  let scoped = $state(!!seed.binding?.only_in);
  let taps = $state<number>(seed.binding?.taps ?? 1);
  let onlyIn = $state(seed.binding?.only_in ?? "");
  let kind = $state<ActionKind>(seed.binding?.action ?? "tile");
  let tileLayout = $state<string>(b?.layout ?? "halves");
  let tileSide = $state<string>(b?.side ?? "left");
  let spanX = $state<number>(Math.round((b?.x ?? 0) * 100));
  let spanW = $state<number>(Math.round((b?.width ?? 1) * 100));
  let fixedW = $state<number>(seed.binding?.action === "fixed" ? seed.binding.width : 1280);
  let fixedH = $state<number>(seed.binding?.action === "fixed" ? seed.binding.height : 800);
  let appName = $state<string>(b?.app ?? seed.config.apps?.[0]?.name ?? "");
  let newApp = $state(false);
  let newAppName = $state("");
  let newAppLaunch = $state("");
  let newAppMatch = $state("");
  let send = $state<string>(b?.send ?? "");
  let run = $state<string>(b?.run ?? "");
  let customColor = $state<boolean>(!!b?.color);
  let color = $state<string>(b?.color ?? "#89B4FA");
  let shortcutQuery = $state<string>(
    seed.binding?.action === "kwin_shortcut"
      ? (seed.shortcuts.find((s) => s.name === (b?.name as string))?.label ?? (b?.name as string))
      : "",
  );

  const windowAction = $derived(isWindowAction(kind));
  /** This key (on this layer) counts taps: this binding or another one on it wants more than one. */
  const tapDance = $derived(
    taps > 1 ||
      (seed.config.bindings ?? []).some(
        (o) =>
          o.id !== binding?.id &&
          o.key.toLowerCase() === keyName.toLowerCase() &&
          (o.layer ?? "delta") === layer &&
          !o.only_in &&
          (o.taps ?? 1) > 1,
      ),
  );
  const pickedShortcut = $derived(
    shortcuts.find((s) => s.label === shortcutQuery) ?? shortcuts.find((s) => s.name === shortcutQuery),
  );
  const shortcutMatches = $derived(
    shortcutQuery.length < 2
      ? []
      : shortcuts.filter((s) => (s.label + " " + s.name).toLowerCase().includes(shortcutQuery.toLowerCase())).slice(0, 8),
  );
  const warnTyping = $derived(layer === "base" && !scoped && typesText(keyName));

  /** The configured app a running/installed app already is, if any. */
  function configuredAs(c: AppChoice): App | undefined {
    return (seed.config.apps ?? []).find(
      (a) => a.name === c.name || (a.match ?? [a.name]).some((m) => c.match.includes(m.toLowerCase())),
    );
  }

  /** Pick a running app: select it if it's configured, otherwise fill in a new one. */
  function pickRunning(c: AppChoice) {
    const existing = configuredAs(c);
    if (existing) {
      newApp = false;
      appName = existing.name;
      return;
    }
    const taken = new Set((seed.config.apps ?? []).map((a) => a.name));
    let name = c.name;
    for (let n = 2; taken.has(name); n++) name = `${c.name}-${n}`;
    newApp = true;
    newAppName = name;
    newAppLaunch = c.launch;
    newAppMatch = c.match.join(", ");
  }

  /** Unique, stable ID for a new binding (edits keep theirs: KDE keys shortcuts by ID). */
  function newId(): string {
    const base =
      (layer === "delta" ? "Caps" : "Key") + keyName + (scoped && onlyIn ? "_" + onlyIn.replace(/[^A-Za-z0-9]/g, "_") : "");
    const taken = new Set((config.bindings ?? []).map((x) => x.id));
    let id = base;
    for (let n = 2; taken.has(id); n++) id = `${base}_${n}`;
    return id;
  }

  function submit(e: Event) {
    e.preventDefault();
    let action: Record<string, unknown>;
    let createdApp: App | null = null;
    switch (kind) {
      case "tile":
        action = { action: "tile", layout: tileLayout, side: tileSide };
        break;
      case "span":
        action = { action: "span", x: spanX / 100, width: spanW / 100 };
        break;
      case "fixed":
        action = { action: "fixed", width: fixedW, height: fixedH };
        break;
      case "app":
        if (newApp) {
          const match = newAppMatch.split(",").map((m) => m.trim().toLowerCase()).filter(Boolean);
          createdApp = { name: newAppName.trim().toLowerCase(), launch: newAppLaunch.trim(), ...(match.length ? { match } : {}) };
          action = { action: "app", app: createdApp.name };
        } else {
          action = { action: "app", app: appName };
        }
        break;
      case "keys":
        action = { action: "keys", send: send.trim().replace(/\s*\+\s*/g, "+") };
        break;
      case "command":
        action = { action: "command", run: run.trim() };
        break;
      case "kwin_shortcut":
        action = pickedShortcut
          ? { action: "kwin_shortcut", name: pickedShortcut.name, component: pickedShortcut.component }
          : { action: "kwin_shortcut", name: shortcutQuery.trim() };
        break;
    }
    const out = {
      id: binding?.id ?? newId(),
      key: keyName,
      ...(layer === "base" ? { layer: "base" } : {}),
      ...(scoped && onlyIn.trim() ? { only_in: onlyIn.trim() } : {}),
      ...(customColor ? { color: color.toUpperCase() } : {}),
      ...(taps > 1 ? { taps } : {}),
      ...action,
    } as Binding;
    onsave(out, createdApp);
  }
</script>

<div class="backdrop" role="presentation" onclick={oncancel}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Edit binding"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === "Escape" && oncancel()}
  >
    <form class="editor" onsubmit={submit}>
      <h2>{binding ? "Edit" : "Add"} action for <code>{keyName}</code></h2>

      <fieldset>
        <legend>When</legend>
        <label><input type="radio" bind:group={layer} value="delta" /> Hold {modifierLabel(modifierKey)} + {keyName}</label>
        <label><input type="radio" bind:group={layer} value="base" /> {keyName} on its own</label>
        {#if warnTyping}
          <p class="warn">{keyName} on its own types; this replaces that everywhere.</p>
        {/if}
        <div class="row taps">
          <span>Tap</span>
          <label><input type="radio" bind:group={taps} value={1} /> once</label>
          <label><input type="radio" bind:group={taps} value={2} /> twice</label>
          <label><input type="radio" bind:group={taps} value={3} /> 3 times</label>
        </div>
        {#if tapDance}
          <p class="hint">
            {keyName} has a tap dance: Delta waits {seed.config.engine?.tap_ms ?? 250} ms after a tap to see if
            another follows, then runs the action for that many taps. Works everywhere (not per app), and can't
            send keys yet.
          </p>
        {/if}
      </fieldset>

      <fieldset>
        <legend>Where</legend>
        <label><input type="radio" bind:group={scoped} value={false} /> Everywhere</label>
        <label class="inline">
          <input type="radio" bind:group={scoped} value={true} disabled={windowAction} /> Only in
          <input
            list="app-classes"
            bind:value={onlyIn}
            placeholder="window class, e.g. firefox"
            disabled={!scoped || windowAction}
            required={scoped}
          />
        </label>
        {#if scoped && !windowAction && running.length}
          <div class="chips">
            {#each running as r}
              <button type="button" class:on={onlyIn === r.class} onclick={() => (onlyIn = r.class)} title={r.caption ?? ""}
                >{r.label}</button
              >
            {/each}
          </div>
        {/if}
        <datalist id="app-classes">
          {#each [...running, ...suggestions] as s}<option value={s.class}>{s.label}</option>{/each}
        </datalist>
        {#if windowAction}
          <p class="hint">Window actions apply everywhere for now.</p>
        {/if}
      </fieldset>

      <fieldset>
        <legend>Do</legend>
        <select bind:value={kind} onchange={() => windowAction && (scoped = false)}>
          {#each ACTIONS as [value, label]}<option {value}>{label}</option>{/each}
        </select>

        {#if kind === "tile"}
          <div class="tiles">
            {#each TILES as [label, l, side]}
              <button
                type="button"
                class:on={tileLayout === l && tileSide === side}
                onclick={() => ((tileLayout = l), (tileSide = side))}>{label}</button
              >
            {/each}
          </div>
        {:else if kind === "span"}
          <div class="row">
            <label>Start <input type="number" min="0" max="100" bind:value={spanX} />%</label>
            <label>Width <input type="number" min="1" max="100" bind:value={spanW} />%</label>
            <button type="button" class="link" onclick={() => ((spanX = 0), (spanW = 100))}>full width</button>
          </div>
        {:else if kind === "fixed"}
          <div class="row">
            <input type="number" min="100" bind:value={fixedW} /> × <input type="number" min="100" bind:value={fixedH} /> px
          </div>
        {:else if kind === "app"}
          {#if running.length}
            <p class="hint">Running now</p>
            <div class="chips">
              {#each running as r}
                {@const existing = configuredAs(r)}
                <button
                  type="button"
                  class:on={existing ? !newApp && appName === existing.name : newApp && newAppLaunch === r.launch}
                  onclick={() => pickRunning(r)}
                  title={r.caption ?? r.class}
                >
                  {r.label}{#if existing}<span class="tag">in Delta</span>{/if}
                </button>
              {/each}
            </div>
          {/if}
          {#if !newApp}
            <div class="row">
              <select bind:value={appName}>
                {#each config.apps ?? [] as a}<option value={a.name}>{a.name}</option>{/each}
              </select>
              <button type="button" class="link" onclick={() => (newApp = true)}>new app…</button>
            </div>
          {:else}
            <div class="grid">
              <label>Name <input bind:value={newAppName} placeholder="firefox" required /></label>
              <label>Launch command <input bind:value={newAppLaunch} placeholder="/usr/bin/firefox" required /></label>
              <label
                >Window class contains <input
                  bind:value={newAppMatch}
                  list="app-classes"
                  placeholder="firefox (comma-separate several)"
                /></label
              >
              <button type="button" class="link" onclick={() => (newApp = false)}>use an existing app</button>
            </div>
          {/if}
        {:else if kind === "keys"}
          <input bind:value={send} placeholder="Ctrl+Shift+T, Alt+Up, PlayPause, VolumeUp" required />
        {:else if kind === "command"}
          <input class="mono" bind:value={run} placeholder="kate ~/notes.md" required />
          <p class="hint">Runs with <code>sh -c</code> in its own systemd scope.</p>
        {:else if kind === "kwin_shortcut"}
          <input bind:value={shortcutQuery} placeholder="Search KDE shortcuts: overview, screenshot, desktop 2…" required />
          {#if !pickedShortcut && shortcutMatches.length}
            <ul class="matches">
              {#each shortcutMatches as s}
                <li>
                  <button type="button" onclick={() => (shortcutQuery = s.label)}>
                    {s.label} <span class="hint">{s.component_name}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {:else if pickedShortcut}
            <p class="hint">{pickedShortcut.component_name} · <code>{pickedShortcut.name}</code></p>
          {/if}
        {/if}
      </fieldset>

      <fieldset>
      <legend>Key colour</legend>
      <label
        ><input type="checkbox" bind:checked={customColor} /> Custom colour while Caps is held
        {#if customColor}<input type="color" bind:value={color} />{/if}</label
      >
    </fieldset>

    {#if problems.length}
        <ul class="problems">
          {#each problems as p}<li>{p}</li>{/each}
        </ul>
      {/if}

      <footer>
        <button type="button" class="ghost" onclick={oncancel}>Cancel</button>
        <button type="submit" class="primary" disabled={saving}>{saving ? "Saving…" : "Save"}</button>
      </footer>
    </form>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(17 17 27 / 0.7);
    display: grid;
    place-items: center;
    z-index: 10;
  }
  .editor {
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    overflow: auto;
    background: var(--mantle);
    border: 1px solid var(--surface1);
    border-radius: 14px;
    padding: 18px 20px;
    display: grid;
    gap: 14px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  fieldset {
    border: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 6px;
  }
  legend {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
    margin-bottom: 6px;
  }
  label {
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .grid {
    display: grid;
    gap: 8px;
  }
  .grid label {
    display: grid;
    gap: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  input:not([type="radio"]),
  select {
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--base);
    border: 1px solid var(--surface1);
    border-radius: 8px;
    padding: 6px 10px;
  }
  input[type="number"] {
    width: 80px;
  }
  .inline input:not([type="radio"]) {
    flex: 1;
  }
  input:disabled {
    opacity: 0.5;
  }
  .mono,
  code {
    font-family: var(--mono);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
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
  button.on,
  button.primary {
    background: var(--blue);
    border-color: var(--blue);
    color: var(--crust);
  }
  button.ghost {
    background: transparent;
  }
  button.link {
    background: none;
    border: none;
    color: var(--blue);
    padding: 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chips button {
    padding: 4px 10px;
  }
  .tag {
    margin-left: 6px;
    font-size: 10px;
    color: var(--teal);
  }
  .on .tag {
    color: var(--crust);
  }
  .matches {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .matches button {
    width: 100%;
    text-align: left;
    background: var(--base);
    border-color: var(--surface0);
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--overlay0);
  }
  .warn {
    margin: 0;
    font-size: 12px;
    color: var(--peach);
  }
  .problems {
    margin: 0;
    padding: 10px 14px 10px 28px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--red) 15%, var(--base));
    color: var(--red);
    font-size: 13px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
