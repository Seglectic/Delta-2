<script lang="ts">
  import { untrack } from "svelte";
  import type { KeyEffect, KeyLight } from "./types";
  import { keyLightColor } from "./bindings";

  interface Props {
    /** QMK name of the key, e.g. KC_W. */
    name: string;
    /** The profile being edited, for the heading: an app, or null for Global. */
    app: string | null;
    /** The key's colour in that profile, if set. */
    light: KeyLight | undefined;
    /** Press effects this key starts in that profile. */
    effects: KeyEffect[];
    saving: boolean;
    /** Show a colour on the keyboard while it's being picked (null = unset). */
    onpreview: (light: KeyLight | null) => void;
    onsave: (light: KeyLight | null) => void;
    oneffect: (effect: KeyEffect | null) => void;
  }

  let { name, app, light, effects, saving, onpreview, onsave, oneffect }: Props = $props();

  // Edited here, saved when the picker closes or the slider is let go (the parent
  // re-creates this when the saved colour changes)
  const seed = untrack(() => light);
  let color = $state(seed?.color ?? "#FFFFFF");
  let brightness = $state(seed?.brightness ?? 255);
  const current = (): KeyLight => ({ key: name, color: color.toUpperCase(), brightness });
</script>

<div class="lighting">
  <h3>Lighting · {app ?? "Global"}</h3>
  {#if light}
    <div class="row">
      <input
        type="color"
        bind:value={color}
        oninput={() => onpreview(current())}
        onchange={() => onsave(current())}
        title="Colour"
      />
      <span class="swatch" style:background={keyLightColor(current())}></span>
      <button class="link danger" onclick={() => onsave(null)} disabled={saving}>clear</button>
    </div>
    <label class="range">
      Brightness
      <input
        type="range"
        min="0"
        max="255"
        bind:value={brightness}
        oninput={() => onpreview(current())}
        onchange={() => onsave(current())}
      />
      <code>{brightness}</code>
    </label>
  {:else}
    <p class="muted">No colour of its own.</p>
    <button onclick={() => onsave(current())} disabled={saving}>Set a colour</button>
  {/if}

  <h4>Press effects</h4>
  {#each effects as e (e.id)}
    <div class="row">
      <span
        class="dot"
        style:background={e.color_end ? `linear-gradient(135deg, ${e.color} 40%, ${e.color_end})` : e.color}
      ></span>
      <span class="muted">burst {e.origin === "center" ? "from the centre" : e.origin ? "from another key" : ""}</span>
      <button class="link" onclick={() => oneffect(e)}>edit</button>
    </div>
  {/each}
  <button onclick={() => oneffect(null)}>+ Press effect</button>
</div>

<style>
  .lighting {
    display: grid;
    gap: 8px;
    justify-items: start;
    align-content: start;
  }
  h3 {
    margin: 12px 0 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
  }
  h4 {
    margin: 8px 0 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--subtext0);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .range {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    justify-self: stretch;
  }
  .range input {
    flex: 1;
    min-width: 60px;
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
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 4px;
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
  .link {
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--blue);
  }
  .link.danger {
    color: var(--red);
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    min-width: 2.5em;
  }
  .muted {
    margin: 0;
    color: var(--overlay0);
    font-size: 13px;
  }
</style>
