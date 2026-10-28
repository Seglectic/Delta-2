<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Binding, Keycode, KeyboardInfo, PhysicalKey } from "./types";
  import { bindingsFor, deltaKeyName, describeAction, qmkName, whenText } from "./bindings";

  interface Props {
    key: PhysicalKey;
    keyboard: KeyboardInfo;
    layerNames: string[];
    /** The firmware layer in use; what a key sends there is what Delta sees. */
    baseLayer: number;
    bindings: Binding[];
    modifierKey: string;
    onadd: (keyName: string) => void;
    onedit: (binding: Binding) => void;
    onremove: (binding: Binding) => void;
    /** A third column: the key's lighting. */
    lighting?: Snippet;
  }

  let { key, keyboard, layerNames, baseLayer, bindings, modifierKey, onadd, onedit, onremove, lighting }: Props =
    $props();

  const perLayer = $derived(keyboard.layers.map((l) => l[key.row]?.[key.col]));
  const turns = $derived(key.encoder !== null ? keyboard.encoders.map((l) => l[key.encoder!]) : []);

  /** What can be bound here: the key itself, or a knob's press and two turns. */
  const targets = $derived.by(() => {
    const press = perLayer[baseLayer];
    const list: { label: string; kc: Keycode | undefined }[] = [
      { label: key.encoder !== null ? "Press" : "This key", kc: press },
    ];
    if (key.encoder !== null && turns[baseLayer]) {
      list.push({ label: "Turn left", kc: turns[baseLayer][0] }, { label: "Turn right", kc: turns[baseLayer][1] });
    }
    return list.map((t) => ({
      ...t,
      keyName: deltaKeyName(t.kc),
      isModifier: t.kc?.name === qmkName(modifierKey),
      bound: bindingsFor(t.kc, bindings),
    }));
  });
</script>

<section class="panel">
  <header>
    <h2>{key.encoder !== null ? `Knob ${key.encoder}` : `Key ${key.row},${key.col}`}</h2>
    <span class="muted">
      {key.encoder !== null ? `press is matrix ${key.row},${key.col}` : `${key.w}u × ${key.h}u`}
    </span>
  </header>

  <div class="columns" class:three={!!lighting}>
    <div>
      <h3>Firmware</h3>
      <table>
        <tbody>
          {#each perLayer as kc, i}
            <tr class:transparent={kc?.name === "KC_TRANSPARENT"} class:base={i === baseLayer}>
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

    <div class="delta">
      <h3>Delta</h3>
      {#each targets as t}
        <div class="target">
          {#if targets.length > 1}
            <h4>{t.label} <code>{t.kc?.name ?? "—"}</code></h4>
          {/if}
          {#if t.bound.length}
            <ul>
              {#each t.bound as b (b.id)}
                <li>
                  <div>
                    <span class="when">{whenText(b, modifierKey)}</span>
                    <span class="what">{describeAction(b)}</span>
                  </div>
                  <span class="actions">
                    <button class="link" onclick={() => onedit(b)}>edit</button>
                    <button class="link danger" onclick={() => onremove(b)}>remove</button>
                  </span>
                </li>
              {/each}
            </ul>
          {/if}
          {#if t.isModifier}
            <p class="muted">This is the Delta key ({modifierKey.toUpperCase()}). Hold it with other keys.</p>
          {:else if t.keyName}
            <button class="add" onclick={() => onadd(t.keyName!)}>+ Add action</button>
          {:else}
            <p class="muted">{t.kc?.name ?? "This key"} can't be bound yet (only letters, digits, F1–F24, arrows, Space, Tab, Esc, Home, End).</p>
          {/if}
        </div>
      {/each}
    </div>
    {#if lighting}{@render lighting()}{/if}
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
  h4 {
    margin: 0 0 6px;
    font-size: 13px;
    font-weight: 600;
  }
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 24px;
  }
  .columns.three {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) minmax(0, 1fr);
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
  .base th {
    color: var(--blue);
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
  .target + .target {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--surface0);
  }
  ul {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  li > div {
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
  .actions {
    display: flex;
    gap: 10px;
    align-items: start;
  }
  button {
    font: inherit;
    cursor: pointer;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--blue);
  }
  .danger {
    color: var(--red);
  }
  .add {
    font-size: 13px;
    color: var(--text);
    background: var(--surface0);
    border: 1px dashed var(--surface2);
    border-radius: 8px;
    padding: 5px 12px;
  }
  .add:hover {
    border-color: var(--blue);
  }
  .muted {
    margin: 0;
    color: var(--overlay0);
    font-size: 13px;
  }
</style>
