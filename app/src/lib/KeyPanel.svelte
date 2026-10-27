<script lang="ts">
  import type { Binding, Keycode, KeyboardInfo, PhysicalKey } from "./types";
  import { bindingsFor, describeAction, whenText } from "./bindings";

  interface Props {
    key: PhysicalKey;
    keyboard: KeyboardInfo;
    layerNames: string[];
    bindings: Binding[];
    modifierKey: string;
  }

  let { key, keyboard, layerNames, bindings, modifierKey }: Props = $props();

  const perLayer = $derived(keyboard.layers.map((l) => l[key.row]?.[key.col]));
  const turns = $derived(key.encoder !== null ? keyboard.encoders.map((l) => l[key.encoder!]) : []);

  /** Bindings on anything this key or knob can send, on any layer, once each. */
  const related = $derived.by(() => {
    const codes: Keycode[] = [...perLayer, ...turns.flat()].filter(Boolean);
    const seen = new Set<string>();
    return codes
      .flatMap((kc) => bindingsFor(kc, bindings))
      .filter((b) => !seen.has(b.id) && seen.add(b.id));
  });
</script>

<section class="panel">
  <header>
    <h2>{key.encoder !== null ? `Knob ${key.encoder}` : `Key ${key.row},${key.col}`}</h2>
    <span class="muted">
      {key.encoder !== null ? `press is matrix ${key.row},${key.col}` : `${key.w}u × ${key.h}u`}
    </span>
  </header>

  <div class="columns">
    <div>
      <h3>Firmware</h3>
      <table>
        <tbody>
          {#each perLayer as kc, i}
            <tr class:transparent={kc?.name === "KC_TRANSPARENT"}>
              <th>{layerNames[i]}</th>
              <td>
                {#if key.encoder !== null}press {/if}<code>{kc?.name ?? "—"}</code>
                {#if turns[i]}
                  <div class="turns">turn <code>{turns[i][0].name}</code> / <code>{turns[i][1].name}</code></div>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div>
      <h3>Delta</h3>
      {#if related.length === 0}
        <p class="muted">Nothing bound to this key yet.</p>
      {:else}
        <ul>
          {#each related as b (b.id)}
            <li>
              <span class="when">{whenText(b, modifierKey)}</span>
              <span class="what">{describeAction(b)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</section>

<style>
  .panel {
    background: var(--mantle);
    border: 1px solid var(--surface0);
    border-radius: 12px;
    padding: 14px 18px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  h3 {
    margin: 12px 0 6px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
  }
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 24px;
  }
  table {
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    text-align: left;
    font-weight: 500;
    color: var(--subtext0);
    padding: 3px 14px 3px 0;
    white-space: nowrap;
    vertical-align: top;
  }
  td {
    padding: 3px 0;
  }
  .transparent td {
    opacity: 0.45;
  }
  .turns {
    color: var(--subtext0);
    font-size: 12px;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  li {
    display: grid;
    gap: 2px;
  }
  .when {
    font-size: 12px;
    color: var(--blue);
  }
  .what {
    font-size: 14px;
  }
  .muted {
    color: var(--overlay0);
    font-size: 13px;
  }
</style>
