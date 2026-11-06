<script lang="ts">
  let { value = null, onrate, label = 'Rate this story from 1 to 10' }: { value?: number | null; onrate: (v: number | null) => void; label?: string } = $props()
  let hover = $state<number | null>(null)
  const shown = $derived(hover ?? value ?? 0)
</script>

<div class="stars" role="group" aria-label={label} onpointerleave={() => (hover = null)}>
  {#each Array(10) as _, i}
    {@const n = i + 1}
    <button
      type="button"
      aria-pressed={value === n}
      aria-label="{n} of 10"
      class:on={n <= shown}
      onpointerenter={() => (hover = n)}
      onclick={() => onrate(value === n ? null : n)}>★</button>
  {/each}
  <span class="num" aria-hidden="true">{shown ? `${shown}/10` : ''}</span>
</div>

<style>
  .stars { display: inline-flex; align-items: center; gap: 1px; }
  button {
    border: 0; background: none; padding: 0 1px; font-size: 0.95rem; line-height: 1;
    color: color-mix(in srgb, var(--ink) 22%, transparent);
    transition: color 120ms, transform 220ms var(--ease-spring);
  }
  button.on { color: var(--gold); text-shadow: 0 0 0.5px var(--ink); }
  button:hover { transform: scale(1.3) translateY(-1px); }
  .num { font-family: var(--font-type); font-size: 0.72rem; color: var(--ink-faint); min-width: 2.6em; margin-left: 0.3em; }
</style>
