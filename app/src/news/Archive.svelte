<script lang="ts">
  import { onMount } from 'svelte'
  import { api, errText, type EditionInfo, type Hit } from '../lib/api'
  import { nav } from '../lib/nav.svelte'

  let editions = $state<EditionInfo[]>([])
  let hits = $state<Hit[] | null>(null)
  let query = $state('')
  let searching = $state(false)
  let error = $state<string | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  let run = 0

  onMount(async () => {
    try { editions = await api.archiveList() } catch (e) { error = errText(e) }
  })

  /** Search as the reader types, a moment after they pause. */
  function typed() {
    clearTimeout(timer)
    timer = setTimeout(search, 250)
  }

  async function search(e?: Event) {
    e?.preventDefault()
    clearTimeout(timer)
    const q = query.trim()
    const mine = ++run
    if (!q) { hits = null; return }
    searching = true
    try {
      const found = await api.archiveSearch(q)
      if (mine === run) hits = found
    } catch (err) { error = errText(err) } finally { if (mine === run) searching = false }
  }

  /** Split a snippet on the match markers so matches can be shown without HTML. */
  function parts(snippet: string) {
    return snippet.split('\u0002').flatMap((chunk, i) => {
      if (i === 0) return [{ text: chunk, hit: false }]
      const [m, rest = ''] = chunk.split('\u0003')
      return [{ text: m, hit: true }, { text: rest, hit: false }]
    })
  }

  const long = (d: string) => new Date(`${d}T12:00:00`).toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })
</script>

<div class="shell">
  <nav class="backbar">
    <button onclick={() => nav.back()}>← Back</button>
    <span class="spacer"></span>
  </nav>

  <main class="morgue paper">
    <header>
      <h1>The Archive</h1>
      <p>Every paper you have read, kept on this device. Search every story in them.</p>
    </header>

    <form class="search" onsubmit={search} role="search">
      <input bind:value={query} oninput={typed} placeholder="Search past editions: a name, a place, a word" aria-label="Search past editions" />
      <button class="btn" disabled={searching}>{searching ? 'Searching…' : 'Search'}</button>
    </form>

    {#if error}<p class="err" role="alert">{error}</p>{/if}

    {#if hits}
      <section aria-label="Stories found" aria-live="polite">
        <h2>{hits.length ? `${hits.length}${hits.length === 60 ? '+' : ''} ${hits.length === 1 ? 'story' : 'stories'} found` : 'Nothing found'}</h2>
        <ol class="hits">
          {#each hits as h (h.story_id + h.date)}
            <li>
              <button onclick={() => nav.go({ name: 'paper', date: h.date, story: h.story_id })}>
                <small>{long(h.date)} · {h.outlet}</small>
                <strong>{h.title}</strong>
                <span class="snip">{#each parts(h.snippet) as p}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>
              </button>
            </li>
          {/each}
        </ol>
      </section>
    {:else}
      <section aria-label="Past editions">
        <h2>{editions.length} {editions.length === 1 ? 'edition' : 'editions'} kept</h2>
        <ol class="eds">
          {#each editions as e (e.date)}
            <li>
              <button onclick={() => nav.go({ name: 'paper', date: e.date })}>
                <span class="date">{long(e.date)}</span>
                <span class="no">No. {e.number} · {e.stories} stories</span>
                {#if e.lead}<span class="lead">{e.lead}</span>{/if}
              </button>
            </li>
          {:else}
            <li class="empty">No papers yet. Each day's edition is kept here once it is printed.</li>
          {/each}
        </ol>
      </section>
    {/if}
  </main>
</div>

<style>
  .shell { min-height: 100dvh; display: flex; flex-direction: column; background: radial-gradient(ellipse at 50% 0%, var(--desk-glow), var(--desk) 70%); }
  .morgue { width: min(100% - 1rem, 900px); margin: 0.5rem auto 3rem; padding: clamp(1rem, 3vw, 2rem); border-radius: 3px; box-shadow: var(--shadow); }
  header { text-align: center; border-bottom: 3px double var(--ink); padding-bottom: 0.8rem; margin-bottom: 1rem; }
  h1 { font-family: var(--font-mast); font-weight: 400; font-size: clamp(2.2rem, 7vw, 3.6rem); margin: 0; line-height: 1; }
  header p { margin: 0.4rem 0 0; font-family: var(--font-news); font-style: italic; color: var(--ink-soft); }
  .search { display: flex; gap: 0.5rem; margin-bottom: 1.2rem; }
  .search input { flex: 1; min-width: 0; font-family: var(--font-news); font-size: 1.05rem; padding: 0.55rem 0.7rem; border: 1px solid var(--rule); border-radius: 4px; background: rgb(255 255 255 / 0.35); color: var(--ink); }
  h2 { font-family: var(--font-head); font-weight: 700; font-size: 0.85rem; letter-spacing: 0.12em; text-transform: uppercase; border-bottom: 1px solid var(--ink); padding-bottom: 0.2rem; margin: 0 0 0.6rem; }
  ol { list-style: none; margin: 0; padding: 0; }
  li button { all: unset; box-sizing: border-box; display: grid; gap: 0.15rem; width: 100%; cursor: pointer; padding: 0.7rem 0.2rem; border-bottom: 1px solid var(--rule); }
  li button:hover strong, li button:hover .lead { text-decoration: underline; text-decoration-thickness: 1px; }
  li button:focus-visible { outline: 2px solid var(--ink); outline-offset: 2px; }
  .date { font-family: var(--font-head); font-weight: 700; font-size: 1.15rem; }
  .no, small { font-family: var(--font-type); font-size: 0.72rem; color: var(--ink-faint); }
  .lead { font-family: var(--font-news); font-style: italic; color: var(--ink-soft); }
  strong { font-family: var(--font-head); font-size: 1.1rem; line-height: 1.2; }
  .snip { font-family: var(--font-news); font-size: 0.92rem; color: var(--ink-soft); line-height: 1.45; }
  mark { background: color-mix(in srgb, var(--gold) 45%, transparent); color: inherit; padding: 0 0.1em; }
  .empty { font-family: var(--font-type); color: var(--ink-faint); padding: 1rem 0; text-align: center; }
  .err { font-family: var(--font-type); color: var(--bad); }
</style>
