<script lang="ts">
  import type { KeyLight, LightingProfile } from "./types";
  import { keyLightColor } from "./bindings";

  interface Props {
    /** The app the sidebar is showing (null = everywhere). */
    app: string | null;
    /** Its saved profile, if any. */
    profile: LightingProfile | undefined;
    /** The profile being painted, or null when not painting. */
    editing: LightingProfile | null;
    /** What a click on a key paints. */
    brush: KeyLight;
    erase: boolean;
    saving: boolean;
    onopen: () => void;
    onsave: () => void;
    ondelete: () => void;
    onclose: () => void;
  }

  let {
    app,
    profile,
    editing = $bindable(),
    brush = $bindable(),
    erase = $bindable(),
    saving,
    onopen,
    onsave,
    ondelete,
    onclose,
  }: Props = $props();

  function toggleBackground(on: boolean) {
    if (!editing) return;
    editing.background = on ? { key: "", color: "#1E1E2E", brightness: 60 } : undefined;
  }
</script>

<section class="profiles">
  <h3>Key colours{editing ? " · paint mode" : ""}</h3>
  {#if !editing}
    <div class="row">
      {#if profile}
        <span class="swatches">
          {#if profile.background}
            <span class="swatch" title="Other keys" style:background={keyLightColor(profile.background)}></span>
          {/if}
          {#each [...new Map((profile.keys ?? []).map((k) => [keyLightColor(k), k])).values()].slice(0, 8) as k}
            <span class="swatch small" style:background={keyLightColor(k)}></span>
          {/each}
        </span>
        <span class="muted">{profile.keys?.length ?? 0} key(s){profile.background ? " + the rest" : ""}</span>
        <button onclick={onopen}>Paint mode</button>
      {:else}
        <button onclick={onopen}>Paint mode</button>
        <span class="muted">no colours yet</span>
      {/if}
    </div>
    <p class="hint">
      Paint mode colours many keys at once. For one key, click it on the board.{#if !profile}
        {app === null
          ? " Used in any app without colours of its own, instead of the normal lighting."
          : ` Used while ${app} has focus.`}{/if}
    </p>
  {:else}
    <span class="muted">{editing.keys?.length ?? 0} key(s) painted · shown on the keyboard while you edit</span>
    <div class="row">
      <span class="label">Brush</span>
      <input type="color" bind:value={brush.color} disabled={erase} title="Colour" />
      <label class="range"
        >Brightness <input type="range" min="0" max="255" bind:value={brush.brightness} disabled={erase} />
        <code>{brush.brightness}</code></label
      >
      <span class="swatch" style:background={erase ? "transparent" : keyLightColor(brush)}></span>
      <label><input type="checkbox" bind:checked={erase} /> Eraser</label>
    </div>
    <div class="row">
      <label
        ><input
          type="checkbox"
          checked={!!editing.background}
          onchange={(e) => toggleBackground(e.currentTarget.checked)}
        /> Light the other keys</label
      >
      {#if editing.background}
        <input type="color" bind:value={editing.background.color} title="Colour" />
        <label class="range"
          >Brightness <input type="range" min="0" max="255" bind:value={editing.background.brightness} /></label
        >
      {/if}
    </div>
    <p class="hint">Click keys on the board to paint them. Clicking a key that already has the brush's colour clears it.</p>
    <div class="row">
      <button class="primary" onclick={onsave} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
      <button onclick={onclose}>Cancel</button>
      {#if profile}<button class="danger" onclick={ondelete}>Delete profile</button>{/if}
    </div>
  {/if}
</section>

<style>
  .profiles {
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
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  label,
  .label {
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
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
  button.primary {
    background: var(--blue);
    border-color: var(--blue);
    color: var(--crust);
  }
  button.danger {
    color: var(--red);
  }
  input[type="color"] {
    width: 40px;
    height: 28px;
    border: none;
    background: none;
    padding: 0;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 1px solid var(--surface2);
  }
  .swatches {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .swatch.small {
    width: 14px;
    height: 14px;
    border-radius: 4px;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    min-width: 2.5em;
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
