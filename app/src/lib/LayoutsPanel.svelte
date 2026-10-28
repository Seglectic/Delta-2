<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    onclose,
    onchanged,
  }: {
    onclose: () => void;
    /** A layout was added: re-read the keyboards. */
    onchanged: () => void;
  } = $props();

  interface Downloaded {
    saved: number;
    cached: number;
    unreadable: number;
  }

  let busy = $state<string | null>(null);
  let result = $state<string | null>(null);
  let problem = $state<string | null>(null);

  async function download(vendor: number | null, what: string) {
    busy = what;
    result = problem = null;
    try {
      const d = await invoke<Downloaded>("download_layouts", { vendor });
      result = `${what}: ${d.saved} new, ${d.cached} already here.`;
      if (d.saved) onchanged();
    } catch (e) {
      problem = String(e);
    } finally {
      busy = null;
    }
  }

  async function importFile() {
    const path = await open({ multiple: false, filters: [{ name: "VIA layout", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    result = problem = null;
    try {
      result = `${await invoke<string>("import_layout", { path })} now uses this layout.`;
      onchanged();
    } catch (e) {
      problem = String(e);
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label="Keyboard layouts"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === "Escape" && onclose()}
  >
    <h2>Keyboard layouts</h2>
    <p class="muted">
      Delta draws your keyboard from its VIA layout. Each board's layout is fetched the first time it's plugged
      in; download them all to work offline.
    </p>

    <section>
      <h3>Download from VIA</h3>
      <div class="row">
        <button onclick={() => download(0x3434, "Keychron")} disabled={!!busy}>
          {busy === "Keychron" ? "Downloading…" : "Keychron boards"}
        </button>
        <button onclick={() => download(null, "Every board")} disabled={!!busy}>
          {busy === "Every board" ? "Downloading…" : "Every board (~2,000)"}
        </button>
      </div>
    </section>

    <section>
      <h3>Use a layout file</h3>
      <p class="muted">
        For boards VIA doesn't list, like NuPhy's: use the JSON file the maker publishes. It's used instead of a
        downloaded layout for that board.
      </p>
      <div class="row"><button onclick={importFile}>Choose file…</button></div>
    </section>

    {#if result}<p class="ok">{result}</p>{/if}
    {#if problem}<p class="error">{problem}</p>{/if}

    <div class="actions"><button class="primary" onclick={onclose}>Done</button></div>
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
  .panel {
    width: min(480px, calc(100vw - 32px));
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
  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--subtext0);
  }
  p {
    margin: 0 0 8px;
  }
  .muted {
    color: var(--overlay0);
    font-size: 13px;
  }
  .row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .ok {
    color: var(--green);
  }
  .error {
    color: var(--red);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
  button.primary {
    background: var(--blue);
    border-color: var(--blue);
    color: var(--crust);
  }
</style>
