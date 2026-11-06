<script lang="ts">
  import { onMount } from 'svelte'
  import { ACCEPT, readMeta } from './foliate'
  import { api, errText, type Book, type Shelved, type Taste } from '../lib/api'
  import { nav } from '../lib/nav.svelte'
  import { tilt, transition } from '../lib/motion'
  import Stars from '../news/Stars.svelte'
  import TasteCard from './Taste.svelte'

  let shelf = $state<Shelved[]>([])
  let suggested = $state<Book[]>([])
  let results = $state<Book[] | null>(null)
  let taste = $state<Taste | null>(null)
  let editingTaste = $state(false)
  let query = $state('')
  let busy = $state<string | null>(null)
  let error = $state<string | null>(null)
  let detail = $state<{ book: Book; shelved: Shelved | null } | null>(null)
  let loadingSuggest = $state(false)
  let shelfFilter = $state('')

  const fold = (t: string) => t.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()
  /** Books on the shelf whose title or author has every typed word. */
  const shown = $derived.by(() => {
    const words = fold(shelfFilter).split(/\s+/).filter(Boolean)
    if (!words.length) return shelf
    return shelf.filter((s) => { const hay = fold(`${s.book.title} ${s.book.authors.join(' ')}`); return words.every((w) => hay.includes(w)) })
  })

  const CLOTH = ['#7a2e22', '#2f5d50', '#3b4a6b', '#6b4a2b', '#5a2f4d', '#2c4a3a', '#8a6a2a', '#4a2c22', '#24435a', '#5e5a2a']
  function hash(s: string) { let h = 2166136261; for (const c of s) h = Math.imul(h ^ c.charCodeAt(0), 16777619); return h >>> 0 }
  // A view-transition name must be unique on the page: the open detail cover owns it,
  // then the shelf spine, then a suggestion face.
  const onShelf = $derived(new Set(shelf.map((x) => x.book.id)))
  function vt(id: string, where: 'spine' | 'face') {
    if (detail?.book.id === id || (where === 'face' && onShelf.has(id))) return ''
    return `view-transition-name: book-${hash(id)}`
  }
  const spine = (b: Book) => { const h = hash(b.title); return { color: CLOTH[h % CLOTH.length], height: 78 + (h % 22), width: 34 + ((h >> 5) % 18) } }

  async function refresh() { shelf = await api.library() }
  async function loadSuggestions() {
    loadingSuggest = true
    try { suggested = await api.suggest() } catch (e) { error = errText(e) } finally { loadingSuggest = false }
  }

  onMount(async () => {
    taste = await api.tasteGet()
    await refresh()
    if (taste.done) loadSuggestions()
  })

  async function search(e: Event) {
    e.preventDefault()
    if (!query.trim()) { results = null; return }
    busy = 'search'
    try { results = await api.search(query) } catch (err) { error = errText(err) } finally { busy = null }
  }

  async function add(b: Book, thenRead = false) {
    busy = b.id
    error = null
    try {
      await api.bookAdd(b)
      await refresh()
      suggested = suggested.filter((x) => x.id !== b.id)
      if (thenRead) read(b.id)
      else detail = { book: b, shelved: shelf.find((s) => s.book.id === b.id) ?? null }
    } catch (e) { error = errText(e) } finally { busy = null }
  }

  async function importFile(e: Event) {
    const input = e.target as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    busy = 'import'
    error = null
    try {
      const { title, author } = await readMeta(file)
      await api.bookImport(new Uint8Array(await file.arrayBuffer()), file.name, title, author)
      await refresh()
    } catch (err) { error = errText(err) } finally { busy = null }
  }

  async function exportNotes() {
    error = null
    try { await api.highlightsExport(null) } catch (e) { error = errText(e) }
  }

  function read(id: string) { detail = null; nav.go({ name: 'reader', id }) }
  async function rate(id: string, r: number | null) {
    await api.bookRate(id, r)
    await refresh()
    if (detail) detail = { ...detail, shelved: shelf.find((s) => s.book.id === id) ?? null }
  }
  async function remove(id: string) {
    await api.bookRemove(id)
    detail = null
    await refresh()
  }
  const show = (book: Book) => transition(() => (detail = { book, shelved: shelf.find((s) => s.book.id === book.id) ?? null }))
</script>

<div class="room">
  <nav class="backbar">
    <button onclick={() => nav.back()}>← Desk</button>
    <span class="spacer"></span>
    <button onclick={() => (editingTaste = true)}>My taste</button>
    <button onclick={() => nav.go({ name: 'catalogues' })}>Catalogues</button>
    <button onclick={exportNotes} title="Save every highlight and note as Markdown">My notes</button>
    <label class="importbtn">
      {busy === 'import' ? 'Importing…' : 'Add my own book'}
      <input type="file" accept={ACCEPT} onchange={importFile} hidden />
    </label>
  </nav>

  <header class="plaque" style="view-transition-name: books-cover">
    <h1>The Library</h1>
    <p>{shelf.length ? `${shelf.length} ${shelf.length === 1 ? 'book' : 'books'} on your shelf` : 'Your shelf is waiting'}</p>
  </header>

  {#if error}<p class="error" role="alert">{error} <button onclick={() => (error = null)}>Dismiss</button></p>{/if}

  <section aria-label="Your shelf">
    <div class="shelfhead">
      <h2 class="shelf-label">On your shelf</h2>
      {#if shelf.length > 1}
        <input class="filter" bind:value={shelfFilter} placeholder="Find on your shelf" aria-label="Find a book on your shelf by title or author" />
      {/if}
    </div>
    <div class="shelf">
      {#each shown as s (s.book.id)}
        {@const sp = spine(s.book)}
        <button class="spine" style="--c:{sp.color}; height:{sp.height}%; width:{sp.width}px; {vt(s.book.id, 'spine')}" onclick={() => show(s.book)} title={s.book.title}>
          <span class="st">{s.book.title}</span>
          {#if s.progress > 0}<span class="mark" style="bottom:{Math.round(s.progress * 80) + 8}%"></span>{/if}
        </button>
      {:else}
        <p class="empty">{shelf.length ? 'No book on your shelf matches that.' : 'Pick a book below, search the catalogues, or add your own book file.'}</p>
      {/each}
    </div>
  </section>

  <section aria-label="Suggested for you">
    <h2 class="shelf-label">Suggested for you {#if loadingSuggest}<span class="dim">· browsing the stacks…</span>{/if}</h2>
    <div class="display">
      {#each suggested.slice(0, 18) as b (b.id)}
        <button class="face" use:tilt={{ max: 6 }} onclick={() => show(b)} style={vt(b.id, 'face')}>
          {#if b.cover}<img src={b.cover} alt="" loading="lazy" referrerpolicy="no-referrer" />{:else}<span class="blank" style="--c:{spine(b).color}">{b.title}</span>{/if}
          <span class="ft">{b.title}</span>
          <span class="fa">{b.authors.join(', ')}</span>
        </button>
      {/each}
    </div>
  </section>

  <section aria-label="Search">
    <form class="search" onsubmit={search}>
      <input bind:value={query} placeholder="Search the open catalogues by title or author" aria-label="Search books" />
      <button class="btn" disabled={busy === 'search'}>{busy === 'search' ? 'Searching…' : 'Search'}</button>
    </form>
    {#if results}
      <ul class="results">
        {#each results as b (b.id)}
          <li><button class="link" onclick={() => show(b)}><strong>{b.title}</strong> <span class="dim">{b.authors.join(', ')}</span></button></li>
        {:else}<li class="dim">Nothing found.</li>{/each}
      </ul>
    {/if}
  </section>
</div>

{#if detail}
  {@const d = detail}
  <div class="scrim" onclick={() => (detail = null)} aria-hidden="true"></div>
  <div class="detail paper" role="dialog" aria-modal="true" aria-label={d.book.title}>
    <div class="dcover" style="view-transition-name: book-{hash(d.book.id)}">
      {#if d.book.cover}<img src={d.book.cover} alt="" referrerpolicy="no-referrer" />{:else}<span class="blank" style="--c:{spine(d.book).color}">{d.book.title}</span>{/if}
    </div>
    <div class="dinfo">
      <h2>{d.book.title}</h2>
      <p class="by">{d.book.authors.join(', ') || 'Unknown author'} · {d.book.source}</p>
      {#if d.book.summary}<p class="sum">{d.book.summary}</p>{/if}
      {#if d.book.subjects.length}<p class="subj">{d.book.subjects.slice(0, 5).join(' · ')}</p>{/if}
      {#if d.shelved}
        <p class="dim">{Math.round(d.shelved.progress * 100)}% read</p>
        <div class="row"><span class="dim">Your rating</span><Stars label="Rate this book from 1 to 10" value={d.shelved.rating} onrate={(v) => rate(d.book.id, v)} /></div>
        <div class="row">
          <button class="btn" onclick={() => read(d.book.id)}>{d.shelved.progress > 0 ? 'Continue reading' : 'Start reading'}</button>
          <button class="btn ghost" onclick={() => remove(d.book.id)}>Remove</button>
        </div>
      {:else}
        <div class="row">
          <button class="btn" disabled={busy === d.book.id} onclick={() => add(d.book, true)}>{busy === d.book.id ? 'Fetching…' : 'Add and read'}</button>
          <button class="btn ghost" disabled={busy === d.book.id} onclick={() => add(d.book)}>Add to shelf</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if taste && (!taste.done || editingTaste)}
  <TasteCard {taste} ondone={(t) => { taste = t; editingTaste = false; loadSuggestions() }} />
{/if}

<style>
  .room {
    min-height: 100dvh; padding-bottom: 3rem; color: var(--paper);
    background:
      repeating-linear-gradient(90deg, rgb(0 0 0 / 0.12) 0 2px, transparent 2px 140px),
      radial-gradient(ellipse at 50% -10%, color-mix(in srgb, var(--gold) 25%, transparent), transparent 60%),
      linear-gradient(var(--desk-2), var(--desk));
  }
  .importbtn { border: 1px solid color-mix(in srgb, var(--paper) 40%, transparent); border-radius: 6px; padding: 0.35em 0.8em; cursor: pointer; }
  .plaque { text-align: center; margin: 1rem auto 1.5rem; width: fit-content; padding: 0.6rem 2.2rem; border: 2px solid var(--gold); outline: 1px solid var(--gold); outline-offset: 4px; color: var(--gold); background: linear-gradient(#2a1d13, #1d140d); border-radius: 4px; box-shadow: 0 10px 30px rgb(0 0 0 / 0.5); }
  .plaque h1 { font-family: var(--font-fell); font-weight: 400; margin: 0; font-size: clamp(1.8rem, 5vw, 2.6rem); letter-spacing: 0.06em; }
  .plaque p { margin: 0.2rem 0 0; font-family: var(--font-type); font-size: 0.8rem; opacity: 0.8; }
  section { max-width: 1200px; margin: 0 auto 2rem; padding: 0 1rem; }
  .shelfhead { display: flex; align-items: baseline; gap: 1rem; flex-wrap: wrap; justify-content: space-between; }
  .filter { padding: 0.35rem 0.6rem; font-family: var(--font-book); font-size: 0.9rem; border-radius: 6px; border: 1px solid color-mix(in srgb, var(--paper) 30%, transparent); background: rgb(0 0 0 / 0.25); color: var(--paper); min-width: 0; width: min(100%, 260px); margin-bottom: 0.6rem; }
  .shelf-label { font-family: var(--font-fell); font-weight: 400; font-size: 1.3rem; margin: 0 0 0.6rem; color: color-mix(in srgb, var(--paper) 90%, transparent); }
  .dim { opacity: 0.65; font-family: var(--font-type); font-size: 0.8rem; }
  .shelf {
    height: 240px; display: flex; align-items: flex-end; gap: 3px; padding: 0 1rem; overflow-x: auto; overflow-y: hidden;
    border-bottom: 18px solid #6b4a2b;
    box-shadow: 0 14px 0 -4px #4a321c, 0 24px 30px rgb(0 0 0 / 0.5), inset 0 -30px 40px -20px rgb(0 0 0 / 0.6);
    background: linear-gradient(#1a120b, #24180f);
    border-radius: 4px 4px 0 0;
  }
  .spine {
    flex: none; position: relative; border: 0; padding: 0.6rem 0; border-radius: 3px 3px 0 0;
    background:
      linear-gradient(90deg, rgb(0 0 0 / 0.35), transparent 18%, rgb(255 255 255 / 0.12) 40%, transparent 60%, rgb(0 0 0 / 0.35)),
      repeating-linear-gradient(0deg, rgb(255 255 255 / 0.03) 0 1px, transparent 1px 3px),
      var(--c);
    color: var(--gold);
    display: flex; justify-content: center;
    transition: transform 380ms var(--ease-spring), box-shadow 300ms;
    box-shadow: 2px 0 3px rgb(0 0 0 / 0.4);
  }
  .spine::before, .spine::after { content: ''; position: absolute; left: 4px; right: 4px; height: 2px; background: var(--gold); opacity: 0.6; }
  .spine::before { top: 10%; } .spine::after { bottom: 8%; }
  .spine:hover { transform: translateY(-14px) rotate(-2deg); box-shadow: 6px 8px 14px rgb(0 0 0 / 0.5); }
  .st { writing-mode: vertical-rl; transform: rotate(180deg); font-family: var(--font-fell); font-size: 0.9rem; max-height: 82%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-shadow: 0 1px 0 rgb(0 0 0 / 0.6); margin-top: 14%; }
  .mark { position: absolute; right: -3px; width: 6px; height: 3px; background: var(--accent); }
  .empty { margin: auto; font-family: var(--font-type); opacity: 0.7; padding: 2rem; text-align: center; }
  .display { display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: 1.4rem 1rem; perspective: 1000px; }
  .face { --rx: 0deg; --ry: 0deg; border: 0; background: none; padding: 0; text-align: left; color: var(--paper); display: grid; gap: 0.3rem; transform: rotateX(var(--rx)) rotateY(var(--ry)); animation: up 600ms var(--ease-out) both; }
  .face img, .blank { width: 100%; aspect-ratio: 2 / 3; object-fit: cover; border-radius: 2px 6px 6px 2px; box-shadow: var(--shadow), inset 6px 0 8px -4px rgb(0 0 0 / 0.5); display: block; }
  .blank { display: grid; place-items: center; padding: 0.8rem; text-align: center; font-family: var(--font-fell); color: var(--gold); background: var(--c); border: 1px solid rgb(0 0 0 / 0.3); }
  .ft { font-family: var(--font-head); font-size: 0.9rem; line-height: 1.2; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .fa { font-family: var(--font-type); font-size: 0.72rem; opacity: 0.65; }
  .search { display: flex; gap: 0.5rem; }
  .search input { flex: 1; min-width: 0; padding: 0.6rem 0.8rem; font-family: var(--font-book); font-size: 1rem; border-radius: 6px; border: 1px solid color-mix(in srgb, var(--paper) 30%, transparent); background: rgb(0 0 0 / 0.25); color: var(--paper); }
  .search .btn { background: var(--paper); color: var(--ink); border-color: var(--paper); }
  .results { list-style: none; padding: 0; margin: 1rem 0 0; display: grid; gap: 0.5rem; }
  .link { all: unset; cursor: pointer; font-family: var(--font-book); }
  .link:hover strong { text-decoration: underline; }
  .error { max-width: 1200px; margin: 0 auto 1rem; padding: 0.6rem 1rem; background: var(--accent); color: var(--paper); font-family: var(--font-type); border-radius: 6px; }
  .error button { background: none; border: 1px solid var(--paper); color: var(--paper); border-radius: 4px; }
  .scrim { position: fixed; inset: 0; background: rgb(0 0 0 / 0.55); z-index: 30; }
  .detail { position: fixed; z-index: 31; left: 50%; top: 50%; translate: -50% -50%; width: min(94vw, 760px); max-height: 90dvh; overflow: auto; display: grid; grid-template-columns: 200px 1fr; gap: 1.4rem; padding: 1.4rem; border-radius: 6px; box-shadow: var(--shadow); }
  @media (max-width: 560px) { .detail { grid-template-columns: 1fr; } .dcover { max-width: 160px; margin: 0 auto; } }
  .dcover img, .dcover .blank { width: 100%; aspect-ratio: 2 / 3; object-fit: cover; border-radius: 2px 6px 6px 2px; box-shadow: var(--shadow); }
  .dinfo h2 { font-family: var(--font-fell); font-weight: 400; font-size: 1.8rem; margin: 0; line-height: 1.1; }
  .by { font-family: var(--font-type); font-size: 0.82rem; color: var(--ink-soft); }
  .sum { font-family: var(--font-book); line-height: 1.55; font-size: 0.95rem; }
  .subj { font-family: var(--font-type); font-size: 0.72rem; color: var(--ink-faint); }
  .detail .dim { color: var(--ink-soft); opacity: 1; }
  .row { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; margin-top: 0.6rem; }
  @keyframes up { from { opacity: 0; transform: translateY(16px); } }
</style>
