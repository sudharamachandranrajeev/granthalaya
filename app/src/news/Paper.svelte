<script lang="ts">
  import { onMount } from 'svelte'
  import { cubicOut, cubicIn } from 'svelte/easing'
  import { api, errText, type Edition, type EditionStory } from '../lib/api'
  import { nav } from '../lib/nav.svelte'
  import { reducedMotion, swipe, transition } from '../lib/motion'
  import StoryBlock from './StoryBlock.svelte'
  import ArticleView from './ArticleView.svelte'
  import { news } from './news.svelte'

  /** Open a past edition (and one story in it) instead of today's. */
  let { date, story }: { date?: string; story?: string } = $props()

  let edition = $state<Edition | null>(null)
  let error = $state<string | null>(null)
  let printing = $state(false)
  let page = $state(0)
  let dir = $state<1 | -1>(1)
  let reading = $state<EditionStory | null>(null)
  let dates = $state<string[]>([])
  let chosen = $state<string[]>([])
  let step = $state(0)

  const steps = ['Reading the newsrooms…', 'Grouping the same events across outlets…', 'Checking who owns and funds each outlet…', 'Weighing the wording and the sources…', 'Laying out the pages…']
  const total = $derived(edition ? edition.pages.length + 1 : 0)

  async function print(force = false) {
    printing = true
    error = null
    const timer = setInterval(() => (step = Math.min(step + 1, steps.length - 1)), 2600)
    try {
      edition = await api.editionToday(force)
      page = 0
      dates = await api.editionDates()
    } catch (e) {
      error = errText(e)
    } finally {
      clearInterval(timer)
      step = 0
      printing = false
    }
  }

  async function pick(date: string) {
    const e = await api.editionGet(date)
    if (e) transition(() => { edition = e; page = 0 })
  }

  async function openPast(d: string, storyId?: string) {
    try {
      const e = await api.editionGet(d)
      if (!e) { error = `No paper was kept for ${d}.`; return }
      edition = e
      dates = await api.editionDates()
      const found = storyId && e.pages.flatMap((p) => p.stories).find((s) => s.story.id === storyId)
      if (found) reading = found
    } catch (e) {
      error = errText(e)
    }
  }

  onMount(() => {
    if (date) news.load().then(() => openPast(date, story))
    else news.load().then(() => { if (news.prefs.onboarded) print() })
    const key = (e: KeyboardEvent) => {
      if (reading) return
      if (e.key === 'ArrowRight' || e.key === 'PageDown') turn(1)
      else if (e.key === 'ArrowLeft' || e.key === 'PageUp') turn(-1)
      else if (e.key === 'Escape') nav.back()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  })

  function turn(d: 1 | -1) {
    const next = page + d
    if (!edition || next < 0 || next >= total) return
    dir = d
    page = next
    document.querySelector('.press')?.scrollTo({ top: 0, behavior: 'instant' as ScrollBehavior })
  }

  function openStory(s: EditionStory) {
    transition(() => (reading = s))
  }

  // Page turn: the leaving sheet swings on the spine; the new one settles in.
  function flipOut(_: Element) {
    if (reducedMotion()) return { duration: 0 }
    const d = dir
    return { duration: 520, easing: cubicIn, css: (t: number) => `transform-origin:${d > 0 ? 'left' : 'right'} center; transform: perspective(2200px) rotateY(${(1 - t) * -85 * d}deg); opacity:${0.3 + t * 0.7}; box-shadow: ${(1 - t) * 40 * d}px 0 60px rgb(0 0 0 / ${(1 - t) * 0.5}); z-index:2` }
  }
  function flipIn(_: Element) {
    if (reducedMotion()) return { duration: 0 }
    return { duration: 620, easing: cubicOut, css: (t: number) => `transform: scale(${0.985 + t * 0.015}); filter: brightness(${0.8 + t * 0.2})` }
  }

  async function startPaper() {
    await news.finishOnboarding(chosen)
    print()
  }
</script>

<div class="shell">
  <nav class="backbar">
    <button onclick={() => nav.back()}>← Desk</button>
    <span class="spacer"></span>
    {#if dates.length > 1}
      <label class="sr-only" for="ed">Past editions</label>
      <select id="ed" onchange={(e) => pick((e.target as HTMLSelectElement).value)}>
        {#each dates as d}<option value={d} selected={d === edition?.date}>{d}</option>{/each}
      </select>
    {/if}
    <button onclick={() => nav.go({ name: 'archive' })}>Archive</button>
    {#if edition && !date}<button onclick={() => print(true)} disabled={printing}>Reprint</button>{/if}
  </nav>

  <div class="press" use:swipe={(d) => turn(d === 'left' ? 1 : -1)}>
    {#if news.loaded && !news.prefs.onboarded}
      <section class="sheet paper onboard" style="view-transition-name: paper-sheet">
        <div class="mast">The Granthālaya Daily</div>
        <h2>What should your paper follow?</h2>
        <p>Pick a few. The paper still shows the day's biggest stories whatever you choose, so it never narrows you into one view. Your ▲ ▼ and star ratings teach it the rest.</p>
        <div class="chips">
          {#each news.topics as t}
            <button class="chip" aria-pressed={chosen.includes(t.id)} onclick={() => (chosen = chosen.includes(t.id) ? chosen.filter((x) => x !== t.id) : [...chosen, t.id])}>{t.label}</button>
          {/each}
        </div>
        <p class="small">Sensitive subjects (politics, conflict, health, religion, migration, justice) only lead a page when at least three independent outlets carry them.</p>
        <button class="btn" onclick={startPaper}>Print my paper</button>
      </section>
    {:else if printing && !edition}
      <section class="sheet paper loading" style="view-transition-name: paper-sheet">
        <div class="mast">The Granthālaya Daily</div>
        <div class="rollers" aria-hidden="true">{#each Array(9) as _, i}<span style="--d:{i * 90}ms"></span>{/each}</div>
        <p class="step" aria-live="polite">{steps[step]}</p>
      </section>
    {:else if error && !edition}
      <section class="sheet paper loading">
        <div class="mast">The Granthālaya Daily</div>
        <p class="step">{error}</p>
        <button class="btn" onclick={() => print()}>Try again</button>
      </section>
    {:else if edition}
      <div class="stack">
        {#key page}
          <section class="sheet paper" in:flipIn out:flipOut style={page === 0 ? 'view-transition-name: paper-sheet' : ''}>
            {#if page < edition.pages.length}
              {@const p = edition.pages[page]}
              {#if page === 0}
                <header class="front">
                  <div class="ears"><span>“All the news that is free to read”</span><span>Price: nothing. Ads: none.</span></div>
                  <div class="mast big">The Granthālaya Daily</div>
                  <div class="dateline">
                    <span>No. {edition.number}</span>
                    <span>{new Date(edition.date + 'T12:00').toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })}</span>
                    <span>{edition.sources_ok.length} newsrooms · {edition.pages.length} pages</span>
                  </div>
                </header>
              {:else}
                <header class="inner">
                  <span>The Granthālaya Daily</span><h2>{p.title}</h2><span>Page {page + 1}</span>
                </header>
              {/if}
              {#if page === 0 && p.stories[0]}
                <div class="leadrow"><StoryBlock item={p.stories[0]} lead onopen={openStory} morph={reading?.story.id !== p.stories[0].story.id} /></div>
                <div class="columns">{#each p.stories.slice(1) as s (s.story.id)}<StoryBlock item={s} onopen={openStory} morph={reading?.story.id !== s.story.id} />{/each}</div>
              {:else}
                <div class="columns">{#each p.stories as s (s.story.id)}<StoryBlock item={s} onopen={openStory} morph={reading?.story.id !== s.story.id} />{/each}</div>
              {/if}
            {:else}
              <div class="end">
                <div class="mast">That’s today’s paper.</div>
                <p class="lede">You’ve reached the last page. Tomorrow’s edition prints in the morning.</p>
                <div class="candor">
                  <h3>How this edition was made</h3>
                  <p>Read {edition.stories_read} stories from {edition.sources_ok.length} newsrooms: {edition.sources_ok.join(', ')}.</p>
                  {#if edition.sources_failed.length}<p>Could not reach: {edition.sources_failed.join(', ')}. Their stories are missing today.</p>{/if}
                  <p>Stories that several outlets report are merged into one, led by the telling with the most neutral wording and best-attributed claims. The meter under each story shows the evidence: how many independent outlets carry it, how loaded the wording is, how claims are attributed, who owns and funds the outlet, and whether anyone in the story does.</p>
                  <p>Your ▲ ▼ and stars stay on this device and only change which stories you see first. Two front-page places always go to the day's most-covered events.</p>
                </div>
                <button class="btn ghost" onclick={() => { dir = -1; page = 0 }}>Back to the front page</button>
              </div>
            {/if}
          </section>
        {/key}
      </div>
      <div class="pager">
        <button onclick={() => turn(-1)} disabled={page === 0} aria-label="Previous page">‹</button>
        <div class="dots" role="tablist" aria-label="Pages">
          {#each Array(total) as _, i}
            <button role="tab" aria-selected={i === page} aria-label={i < edition.pages.length ? edition.pages[i].title : 'End'} onclick={() => { dir = i > page ? 1 : -1; page = i }}></button>
          {/each}
        </div>
        <button onclick={() => turn(1)} disabled={page >= total - 1} aria-label="Next page">›</button>
      </div>
    {/if}
  </div>
</div>

{#if reading}
  <ArticleView item={reading} onclose={() => transition(() => (reading = null), 'back')} />
{/if}

<style>
  .shell { min-height: 100dvh; display: flex; flex-direction: column; background: radial-gradient(ellipse at 50% 0%, var(--desk-glow), var(--desk) 70%); }
  .backbar select { background: transparent; color: var(--paper); border: 1px solid color-mix(in srgb, var(--paper) 40%, transparent); border-radius: 6px; padding: 0.3em; font-family: var(--font-type); }
  .backbar select option { color: black; }
  .press { flex: 1; padding: 0.5rem clamp(0.5rem, 2vw, 2rem) 5rem; overflow-y: auto; touch-action: pan-y; }
  .stack { display: grid; max-width: 1240px; margin: 0 auto; }
  .stack > :global(*) { grid-area: 1 / 1; }
  .sheet {
    max-width: 1240px; width: 100%; margin: 0 auto;
    padding: clamp(1rem, 3vw, 2.4rem);
    box-shadow: var(--shadow);
    border-radius: 2px;
    position: relative;
    backface-visibility: hidden;
  }
  .sheet::after { content: ''; position: absolute; inset: 0; pointer-events: none; background: linear-gradient(90deg, rgb(0 0 0 / 0.07), transparent 4%, transparent 96%, rgb(0 0 0 / 0.05)); }
  .mast { font-family: var(--font-mast); text-align: center; font-size: clamp(2rem, 5vw, 3rem); line-height: 1.05; }
  .mast.big { font-size: clamp(2.6rem, 8.5vw, 6rem); margin: 0.2rem 0; }
  .ears { display: flex; justify-content: space-between; font-family: var(--font-type); font-size: 0.7rem; color: var(--ink-faint); gap: 1rem; }
  .dateline { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 0.5rem; border-block: 3px double var(--ink); padding: 0.3rem 0; margin-bottom: 1.2rem; font-family: var(--font-type); font-size: 0.8rem; }
  .inner { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; border-bottom: 3px double var(--ink); margin-bottom: 1.2rem; font-family: var(--font-type); font-size: 0.75rem; color: var(--ink-faint); }
  .inner h2 { font-family: var(--font-head); font-weight: 900; color: var(--ink); font-size: clamp(1.5rem, 3vw, 2.2rem); margin: 0; text-transform: uppercase; letter-spacing: 0.04em; }
  .leadrow { border-bottom: 2px solid var(--ink); margin-bottom: 1rem; }
  .columns { columns: 3 17rem; column-gap: 1.6rem; column-rule: 1px solid var(--rule); }
  .onboard, .loading { max-width: 720px; text-align: center; display: grid; gap: 0.9rem; justify-items: center; }
  .onboard h2 { font-family: var(--font-head); font-size: 1.6rem; margin: 0.5rem 0 0; }
  .onboard p { max-width: 52ch; line-height: 1.5; margin: 0; }
  .chips { display: flex; flex-wrap: wrap; justify-content: center; gap: 0.45rem; }
  .small { font-family: var(--font-type); font-size: 0.78rem; color: var(--ink-soft); }
  .rollers { display: grid; gap: 0.55rem; width: min(100%, 520px); margin: 1.2rem 0; }
  .rollers span { height: 0.55rem; background: color-mix(in srgb, var(--ink) 30%, transparent); transform-origin: left; animation: type 1.6s var(--d) infinite var(--ease-out); }
  .rollers span:nth-child(3n) { width: 70%; }
  .step { font-family: var(--font-type); }
  .end { text-align: center; display: grid; gap: 1rem; justify-items: center; padding: 2rem 0; }
  .lede { font-size: 1.15rem; font-style: italic; }
  .candor { max-width: 62ch; text-align: left; border: 1px solid var(--rule); padding: 1rem 1.4rem; line-height: 1.5; }
  .candor h3 { font-family: var(--font-head); margin: 0 0 0.4rem; }
  .candor p { margin: 0.4rem 0; font-size: 0.95rem; }
  .pager { position: fixed; bottom: calc(env(safe-area-inset-bottom) + 0.8rem); left: 50%; translate: -50% 0; display: flex; align-items: center; gap: 0.6rem; padding: 0.35rem 0.6rem; border-radius: 999px; background: color-mix(in srgb, var(--desk) 80%, transparent); backdrop-filter: blur(8px); box-shadow: 0 6px 20px rgb(0 0 0 / 0.4); z-index: 10; }
  .pager > button { border: 0; background: var(--paper); color: var(--ink); width: 2.2rem; height: 2.2rem; border-radius: 50%; font-size: 1.3rem; line-height: 1; transition: transform 200ms var(--ease-spring); }
  .pager > button:hover:not(:disabled) { transform: scale(1.1); }
  .pager > button:disabled { opacity: 0.35; }
  .dots { display: flex; gap: 0.35rem; }
  .dots button { width: 0.55rem; height: 0.55rem; padding: 0; border-radius: 50%; border: 0; background: color-mix(in srgb, var(--paper) 35%, transparent); transition: all 250ms var(--ease-spring); }
  .dots button[aria-selected='true'] { background: var(--paper); width: 1.4rem; border-radius: 999px; }
  @keyframes type { 0% { transform: scaleX(0); opacity: 0.2; } 50% { transform: scaleX(1); opacity: 1; } 100% { transform: scaleX(1); opacity: 0.1; } }
</style>
