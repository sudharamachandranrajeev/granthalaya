<script lang="ts">
  import type { Report } from '../lib/api'
  let { report, compact = true }: { report: Report; compact?: boolean } = $props()

  const tone = $derived(report.score >= 80 ? 'good' : report.score >= 60 ? 'ok' : report.score >= 40 ? 'care' : 'weak')
  const owner = $derived(report.outlet.owners[0]?.label ?? report.outlet.funders[0]?.label)
  const flagged = $derived(report.influences.length)
</script>

<div class="meter {tone}" class:compact>
  <div class="row">
    <span class="score" aria-label="Genuineness {report.score} out of 100">{report.score}</span>
    <div class="gauge" aria-hidden="true">
      {#each report.components as c}
        <span class="seg" style="flex:{c.max}" title="{c.label}: {c.points}/{c.max}">
          <span class="fill" style="width:{(c.points / c.max) * 100}%"></span>
        </span>
      {/each}
    </div>
    <span class="verdict">{report.verdict}</span>
  </div>
  <div class="facts">
    <span>{report.corroboration.independent} independent {report.corroboration.independent === 1 ? 'outlet' : 'outlets'}</span>
    <span>·</span>
    <span>{report.outlet.funding}{owner ? `, ${owner}` : ''}</span>
    {#if flagged}
      <span>·</span>
      <span class="flag">⚑ {flagged} possible {flagged === 1 ? 'influence' : 'influences'}</span>
    {/if}
    {#if report.loaded.length}
      <span>·</span>
      <span>{report.loaded.length} loaded {report.loaded.length === 1 ? 'word' : 'words'}</span>
    {/if}
  </div>
</div>

<style>
  .meter { --c: var(--good); font-family: var(--font-type); font-size: 0.72rem; color: var(--ink-soft); display: grid; gap: 0.25rem; margin-top: 0.45rem; }
  .ok { --c: var(--accent-2); }
  .care { --c: var(--warn); }
  .weak { --c: var(--bad); }
  .row { display: flex; align-items: center; gap: 0.45rem; }
  .score { font-weight: 700; color: var(--c); font-size: 0.95rem; min-width: 1.6em; }
  .gauge { flex: 1; display: flex; gap: 2px; height: 6px; }
  .seg { background: color-mix(in srgb, var(--ink) 12%, transparent); border-radius: 1px; overflow: hidden; }
  .fill { display: block; height: 100%; background: var(--c); transform-origin: left; animation: grow 900ms var(--ease-out) both; }
  .verdict { color: var(--c); white-space: nowrap; }
  .facts { display: flex; flex-wrap: wrap; gap: 0.3rem; color: var(--ink-faint); }
  .flag { color: var(--warn); }
  @keyframes grow { from { transform: scaleX(0); } }
</style>
