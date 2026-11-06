<script lang="ts">
  import type { EditionStory } from '../lib/api'
  import BiasMeter from './BiasMeter.svelte'
  import Stars from './Stars.svelte'
  import { news } from './news.svelte'

  // morph: carry the shared-element name; off while this story is open, since its article headline holds the name then.
  let { item, lead = false, onopen, morph = true }: { item: EditionStory; lead?: boolean; onopen: (s: EditionStory) => void; morph?: boolean } = $props()
  const s = $derived(item.story)
  const fb = $derived(news.feedback[s.id] ?? [null, null])
  const time = $derived(s.published ? new Date(s.published).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }) : '')
  let imgOk = $state(true)
</script>

<article class="story" class:lead>
  <button class="open" onclick={() => onopen(item)} style={morph ? `view-transition-name: story-${s.id}` : ''}>
    {#if lead && s.image && imgOk}
      <figure><img src={s.image} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={() => (imgOk = false)} /></figure>
    {/if}
    <h3>{s.title}</h3>
  </button>
  <div class="byline">
    <span class="outlet">{s.outlet_name}</span>
    {#if time}<span>{time}</span>{/if}
    {#if item.others.length}<span>+{item.others.length} other {item.others.length === 1 ? 'telling' : 'tellings'}</span>{/if}
    {#if s.paywall}<span class="pay">paywall</span>{/if}
    {#if item.report.opinion}<span class="pay">opinion</span>{/if}
  </div>
  {#if s.summary}<p class="summary">{s.summary}</p>{/if}
  <BiasMeter report={item.report} />
  <div class="actions">
    <button class="vote" aria-label="More like this" aria-pressed={fb[0] === 1} onclick={() => news.vote(s.id, 1)}>▲</button>
    <button class="vote" aria-label="Less like this" aria-pressed={fb[0] === -1} onclick={() => news.vote(s.id, -1)}>▼</button>
    <Stars value={fb[1]} onrate={(v) => news.rate(s.id, v)} />
  </div>
  {#if item.why.length}<p class="why">{item.why[0]}</p>{/if}
</article>

<style>
  .story { break-inside: avoid; padding-bottom: 0.9rem; margin-bottom: 0.9rem; border-bottom: 1px solid var(--rule); animation: ink 700ms var(--ease-out) both; }
  .open { all: unset; cursor: pointer; display: block; }
  .open:focus-visible { outline: 2px solid var(--gold); }
  h3 { font-family: var(--font-head); font-weight: 700; font-size: 1.12rem; line-height: 1.18; margin: 0 0 0.3rem; text-wrap: balance; }
  .open:hover h3 { text-decoration: underline; text-decoration-thickness: 1px; text-underline-offset: 3px; }
  .lead h3 { font-weight: 900; font-size: clamp(1.6rem, 3.2vw, 2.5rem); line-height: 1.05; }
  figure { margin: 0 0 0.6rem; }
  img { width: 100%; max-height: 340px; object-fit: cover; display: block; filter: grayscale(1) contrast(1.1) sepia(0.25); mix-blend-mode: multiply; border: 1px solid var(--rule); }
  .byline { display: flex; flex-wrap: wrap; gap: 0.6rem; font-family: var(--font-type); font-size: 0.72rem; color: var(--ink-faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .outlet { color: var(--ink-soft); }
  .pay { color: var(--warn); }
  .summary { margin: 0.4rem 0 0; line-height: 1.45; text-align: justify; hyphens: auto; font-size: 0.98rem; }
  .lead .summary { font-size: 1.1rem; }
  .lead .summary::first-letter { float: left; font-family: var(--font-head); font-weight: 900; font-size: 3.2em; line-height: 0.85; padding: 0.05em 0.08em 0 0; }
  .actions { display: flex; align-items: center; gap: 0.3rem; margin-top: 0.5rem; flex-wrap: wrap; }
  .vote { border: 1px solid var(--rule); background: transparent; border-radius: 4px; width: 1.9rem; height: 1.6rem; font-size: 0.75rem; color: var(--ink-faint); transition: all 160ms var(--ease-spring); }
  .vote:hover { transform: translateY(-1px); color: var(--ink); }
  .vote[aria-pressed='true'] { background: var(--ink); color: var(--paper); border-color: var(--ink); transform: scale(1.06); }
  .why { margin: 0.35rem 0 0; font-family: var(--font-type); font-size: 0.7rem; color: var(--ink-faint); font-style: italic; }
  @keyframes ink { from { opacity: 0; filter: blur(3px); transform: translateY(6px); } }
</style>
