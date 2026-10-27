<script lang="ts">
  import { untrack } from "svelte";
  import type { Config } from "./types";

  interface Props {
    config: Config;
    saving: boolean;
    onsave: (lighting: Config["lighting"]) => void;
  }

  let { config, saving, onsave }: Props = $props();

  const EFFECTS: [number, string][] = [
    [1, "Solid colour"],
    [2, "Breathing"],
    [0, "Off"],
  ];

  // Seeded once; the panel is re-created when the config reloads
  const seed = untrack(() => config.lighting ?? {});
  let custom = $state(!!seed.normal);
  let effect = $state(seed.normal?.effect ?? 1);
  let color = $state(seed.normal?.color ?? "#89B4FA");
  let brightness = $state(seed.normal?.brightness ?? 180);
  let highlight = $state(seed.caps_highlight ?? true);

  function save() {
    onsave({
      caps_highlight: highlight,
      ...(custom ? { normal: { effect, color: color.toUpperCase(), brightness } } : {}),
    });
  }
</script>

<section class="lighting">
  <h3>Lighting</h3>
  <label><input type="checkbox" bind:checked={highlight} /> Light up bound keys while Caps is held</label>
  <label><input type="checkbox" bind:checked={custom} /> Set the normal lighting</label>
  {#if custom}
    <div class="row">
      <select bind:value={effect}>
        {#each EFFECTS as [value, label]}<option {value}>{label}</option>{/each}
      </select>
      {#if effect !== 0}
        <input type="color" bind:value={color} title="Colour" />
        <label class="range">Brightness <input type="range" min="0" max="255" bind:value={brightness} /></label>
      {/if}
    </div>
  {:else}
    <p class="hint">The keyboard keeps whatever lighting it has.</p>
  {/if}
  <button onclick={save} disabled={saving}>{saving ? "Saving…" : "Apply"}</button>
</section>

<style>
  .lighting {
    background: var(--mantle);
    border: 1px solid var(--surface0);
    border-radius: 12px;
    padding: 14px 18px;
    display: grid;
    gap: 8px;
    justify-items: start;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
  }
  label {
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
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
  input[type="color"] {
    width: 40px;
    height: 28px;
    border: none;
    background: none;
    padding: 0;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--overlay0);
  }
</style>
