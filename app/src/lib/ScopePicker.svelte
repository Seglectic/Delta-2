<script lang="ts">
  import type { AppChoice, KeyEffect, LightingProfile } from "./types";

  interface Props {
    /** `[[apps]]` names. */
    apps: string[];
    profiles: LightingProfile[];
    effects: KeyEffect[];
    running: AppChoice[];
    /** The app picked (null = everywhere). */
    scope: string | null;
    /** Can't switch while an editor is open. */
    locked: boolean;
    onrefresh: () => void;
    onpick: (target: { app: string | null } | { running: AppChoice }) => void;
  }

  let { apps, profiles, effects, running, scope, locked, onrefresh, onpick }: Props = $props();

  const notSaved = $derived(running.filter((r) => !apps.includes(r.name)));

  /** "profile, 2 effects" for the option label. */
  function summary(app: string | null): string {
    const parts = [];
    if (profiles.some((p) => (p.app ?? null) === app)) parts.push("profile");
    const n = effects.filter((e) => (e.app ?? null) === app).length;
    if (n) parts.push(`${n} effect${n > 1 ? "s" : ""}`);
    return parts.length ? ` · ${parts.join(", ")}` : "";
  }

  function pick(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (v === "*") onpick({ app: null });
    else if (v.startsWith("app:")) onpick({ app: v.slice(4) });
    else if (v.startsWith("run:")) {
      const r = notSaved.find((r) => r.name === v.slice(4));
      if (r) onpick({ running: r });
    }
  }
</script>

<label class="scope" class:app={scope !== null} title="Which profile the lighting panels edit">
  <span class="text">
    <span class="kicker">{scope === null ? "Profile" : "App profile"}</span>
    <span class="name">{scope ?? "Global"}</span>
  </span>
  <svg class="chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" /></svg>
  <select onchange={pick} onfocus={onrefresh} disabled={locked} value={scope === null ? "*" : `app:${scope}`}>
      <option value="*">Global{summary(null)}</option>
      {#each apps as app}<option value="app:{app}">{app}{summary(app)}</option>{/each}
      {#if scope !== null && !apps.includes(scope)}
        <option value="app:{scope}">{scope} (new)</option>
      {/if}
      {#if notSaved.length}
        <optgroup label="Running now">
          {#each notSaved.filter((r) => r.name !== scope) as r}<option value="run:{r.name}">{r.label}</option>{/each}
        </optgroup>
      {/if}
  </select>
</label>

<style>
  /* The header's title: what profile is being edited, big enough to see at a glance.
     The real <select> covers the whole box, invisibly, so a click anywhere opens it. */
  .scope {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 5px 12px 6px 14px;
    border-radius: 10px;
    border: 1px solid var(--surface1);
    background: var(--surface0);
    min-width: 200px;
    cursor: pointer;
    transition: border-color 120ms, background 120ms;
  }
  .scope:hover {
    border-color: var(--surface2);
    background: var(--surface1);
  }
  .scope.app {
    border-color: var(--peach);
    background: color-mix(in srgb, var(--peach) 14%, var(--base));
  }
  .scope.app:hover {
    background: color-mix(in srgb, var(--peach) 22%, var(--base));
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .kicker {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--subtext0);
  }
  .scope.app .kicker {
    color: var(--peach);
  }
  .name {
    font-size: 22px;
    font-weight: 700;
    line-height: 1.15;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chevron {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    fill: none;
    stroke: var(--subtext0);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .scope.app .chevron {
    stroke: var(--peach);
  }
  select {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
    font-size: 14px;
  }
  select:disabled {
    cursor: default;
  }
  .scope:has(select:disabled) {
    opacity: 0.7;
    cursor: default;
  }
  .scope:has(select:focus-visible) {
    outline: 2px solid var(--blue);
    outline-offset: 2px;
  }
  option,
  optgroup {
    background: var(--base);
    color: var(--text);
  }
</style>
