<script lang="ts">
  import type { KeyEffect } from "./types";

  interface Props {
    /** The app the sidebar is showing (null = everywhere). */
    app: string | null;
    /** Every saved effect; only the app's are listed. */
    effects: KeyEffect[];
    /** The effect being edited, or null. */
    editing: KeyEffect | null;
    /** Which key the next board click picks. */
    picking: "trigger" | "origin" | null;
    /** "Enter" for KC_ENTER, from the keymap. */
    keyLabel: (name: string) => string;
    saving: boolean;
    /** A new effect for the app (null), or an existing one. */
    onopen: (effect: KeyEffect | null) => void;
    onsave: () => void;
    ondelete: () => void;
    onclose: () => void;
  }

  let {
    app,
    effects,
    editing = $bindable(),
    picking = $bindable(),
    keyLabel,
    saving,
    onopen,
    onsave,
    ondelete,
    onclose,
  }: Props = $props();

  const mine = $derived(effects.filter((e) => (e.app ?? null) === app));
  const exists = $derived(editing ? effects.some((e) => e.id === editing!.id) : false);

  const originMode = $derived(!editing?.origin ? "same" : editing.origin === "center" ? "center" : "key");
  function setOrigin(mode: string) {
    if (!editing) return;
    if (mode === "same") editing.origin = undefined;
    else if (mode === "center") editing.origin = "center";
    else picking = "origin";
  }
</script>

<section class="effects">
  <h3>Press effects</h3>
  {#if !editing}
    {#if mine.length}
      <ul>
        {#each mine as e (e.id)}
          <li>
            <span class="swatch" style:background={e.color_end
              ? `linear-gradient(135deg, ${e.color} 40%, ${e.color_end})`
              : e.color}></span>
            <span>
              <strong>{keyLabel(e.trigger)}</strong>: burst from {e.origin === "center"
                ? "the centre"
                : keyLabel(e.origin ?? e.trigger)}
            </span>
            <button class="link" onclick={() => onopen(e)}>edit</button>
          </li>
        {/each}
      </ul>
    {/if}
    <button onclick={() => onopen(null)}>+ New press effect</button>
    {#if !mine.length}
      <p class="hint">
        A burst of light that spreads out across the board when you press a key{app === null
          ? ", in any app"
          : ` while ${app} has focus`}.
      </p>
    {/if}
  {:else}
    <span class="muted">Plays on a loop on the keyboard and here while you edit</span>
    <div class="grid">
      <span class="label">Key</span>
      <span class="row">
        <code>{keyLabel(editing.trigger)}</code>
        <button class:on={picking === "trigger"} onclick={() => (picking = picking === "trigger" ? null : "trigger")}
          >{picking === "trigger" ? "Click a key on the board…" : "Pick"}</button
        >
      </span>

      <span class="label">Comes from</span>
      <span class="row">
        <select value={originMode} onchange={(e) => setOrigin(e.currentTarget.value)}>
          <option value="same">The key pressed</option>
          <option value="center">Centre of the board</option>
          <option value="key">Another key…</option>
        </select>
        {#if originMode === "key" && editing.origin}<code>{keyLabel(editing.origin)}</code>{/if}
        {#if originMode === "key" || picking === "origin"}
          <button class:on={picking === "origin"} onclick={() => (picking = picking === "origin" ? null : "origin")}
            >{picking === "origin" ? "Click a key on the board…" : "Pick"}</button
          >
        {/if}
      </span>

      <span class="label">Colour</span>
      <span class="row">
        <input type="color" bind:value={editing.color} />
        <label
          ><input
            type="checkbox"
            checked={!!editing.color_end}
            onchange={(e) => (editing!.color_end = e.currentTarget.checked ? "#FFFFFF" : undefined)}
          /> fades to</label
        >
        {#if editing.color_end}<input type="color" bind:value={editing.color_end} />{/if}
      </span>

      <span class="label">Strength</span>
      <span class="row"
        ><input type="range" min="10" max="255" bind:value={editing.strength} /><code>{editing.strength}</code></span
      >

      <span class="label">Distance</span>
      <span class="row"
        ><input type="range" min="1" max="20" step="0.5" bind:value={editing.distance} /><code
          >{editing.distance} keys</code
        ></span
      >

      <span class="label">Ring width</span>
      <span class="row"
        ><input type="range" min="0.3" max="6" step="0.1" bind:value={editing.width} /><code
          >{editing.width} keys</code
        ></span
      >

      <span class="label">Duration</span>
      <span class="row"
        ><input type="range" min="100" max="3000" step="50" bind:value={editing.duration_ms} /><code
          >{editing.duration_ms} ms</code
        ></span
      >

      <span class="label">Fade</span>
      <span class="row"
        ><input type="range" min="0" max="4" step="0.1" bind:value={editing.fade} /><code
          >{editing.fade === 0 ? "none" : editing.fade}</code
        ></span
      >
    </div>
    <div class="row">
      <button class="primary" onclick={onsave} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
      <button onclick={onclose}>Cancel</button>
      {#if exists}<button class="danger" onclick={ondelete}>Delete</button>{/if}
    </div>
  {/if}
</section>

<style>
  .effects {
    background: var(--mantle);
    border: 1px solid var(--surface0);
    border-radius: 12px;
    padding: 14px 18px;
    display: grid;
    gap: 10px;
    justify-items: start;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
    font-size: 13px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .grid {
    justify-self: stretch;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px 14px;
    align-items: center;
  }
  .grid .row {
    flex-wrap: nowrap;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  label,
  .label {
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .label {
    color: var(--subtext0);
  }
  select,
  button {
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--surface0);
    border: 1px solid var(--surface1);
    border-radius: 8px;
    padding: 5px 10px;
    cursor: pointer;
  }
  button.on {
    border-color: var(--peach);
    color: var(--peach);
  }
  button.primary {
    background: var(--blue);
    border-color: var(--blue);
    color: var(--crust);
  }
  button.danger {
    color: var(--red);
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--blue);
  }
  input[type="color"] {
    width: 40px;
    height: 28px;
    border: none;
    background: none;
    padding: 0;
  }
  input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .grid code {
    min-width: 4.5em;
    text-align: right;
  }
  .swatch {
    width: 14px;
    height: 14px;
    border-radius: 4px;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
  }
  .muted {
    color: var(--overlay0);
    font-size: 13px;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--overlay0);
  }
</style>
