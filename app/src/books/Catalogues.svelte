<script lang="ts">
  import { onMount } from 'svelte'
  import { api, errText, type Catalogue, type CataloguePage, type NavLink, type Preset, type Pub } from '../lib/api'
  import { nav } from '../lib/nav.svelte'

  let catalogues = $state<Catalogue[]>([])
  let presets = $state<Preset[]>([])
  let shelf = $state(new Set<string>())
  let error = $state<string | null>(null)
  let busy = $state<string | null>(null)

  type Form = { url: string; title: string; user: string; password: string; login: boolean; note: string; editing: boolean }
  let form = $state<Form | null>(null)

  /** The open catalogue and the pages walked through in it. */
  let open = $state<{ cat: Catalogue; trail: CataloguePage[] } | null>(null)
  let query = $state('')
  const page = $derived(open ? open.trail[open.trail.length - 1] : null)

  onMount(async () => {
    try {
      ;[catalogues, presets] = await Promise.all([api.opdsList(), api.opdsPresets()])
      shelf = new Set((await api.library()).map((s) => s.book.id))
    } catch (e) { error = errText(e) }
  })

  const added = (url: string) => catalogues.some((c) => c.url === url)

  function startAdd(p?: Preset) {
    error = null
    form = { url: p?.url ?? '', title: p?.title ?? '', user: '', password: '', login: p?.login ?? false, note: p?.note ?? '', editing: false }
  }
  function startEdit(c: Catalogue) {
    error = null
    form = { url: c.url, title: c.title, user: c.user, password: '', login: !!c.user, note: '', editing: true }
  }

  async function save(e: Event) {
    e.preventDefault()
    if (!form) return
    busy = 'save'
    error = null
    try {
      const user = form.login ? form.user : ''
      // Editing with the password left blank keeps the saved one.
      const password = form.login ? (form.editing && !form.password ? null : form.password) : null
      await api.opdsSave(form.url, form.title, user, password)
      catalogues = await api.opdsList()
      form = null
    } catch (err) { error = errText(err) } finally { busy = null }
  }

  async function remove(c: Catalogue) {
    await api.opdsRemove(c.id)
    catalogues = await api.opdsList()
  }

  async function load(cat: Catalogue, url: string | null, how: 'push' | 'replace' | 'root', search?: string) {
    busy = 'page'
    error = null
    try {
      const p = await api.opdsBrowse(cat.id, url, search ? query : null, search ?? null)
      if (how === 'root' || !open) open = { cat, trail: [p] }
      else if (how === 'push') open = { cat, trail: [...open.trail, p] }
      else open = { cat, trail: [...open.trail.slice(0, -1), p] }
      window.scrollTo({ top: 0 })
    } catch (e) { error = errText(e) } finally { busy = null }
  }

  function back(to: number) {
    if (open) open = { ...open, trail: open.trail.slice(0, to + 1) }
  }

  function search(e: Event) {
    e.preventDefault()
    if (open && page?.search && query.trim()) load(open.cat, null, 'push', page.search)
  }

  async function get(b: Pub, format: string, thenRead = false) {
    if (!open) return
    busy = b.id
    error = null
    try {
      const id = await api.opdsGet(open.cat.id, $state.snapshot(b), format)
      shelf = new Set([...shelf, id])
      if (thenRead) nav.go({ name: 'reader', id })
    } catch (e) { error = errText(e) } finally { busy = null }
  }

  const host = (u: string) => { try { return new URL(u).host } catch { return u } }
</script>

{#snippet links(list: NavLink[])}
  <ul class="links">
    {#each list as l (l.url)}
      <li>
        <button class="nl" disabled={busy === 'page'} onclick={() => open && load(open.cat, l.url, 'push')}>
          {#if l.cover?.startsWith('https:')}<img src={l.cover} alt="" loading="lazy" referrerpolicy="no-referrer" />{/if}
          <span><strong>{l.title || 'Untitled'}</strong>{#if l.summary}<small>{l.summary}</small>{/if}</span>
          <span class="arrow" aria-hidden="true">›</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet books(list: Pub[])}
  <div class="books">
    {#each list as b (b.id)}
      <article class="book paper">
        {#if b.cover?.startsWith('https:')}<img src={b.cover} alt="" loading="lazy" referrerpolicy="no-referrer" />{:else}<span class="blank">{b.title}</span>{/if}
        <div>
          <h3>{b.title || 'Untitled'}</h3>
          {#if b.authors.length}<p class="by">{b.authors.join(', ')}</p>{/if}
          {#if b.summary}<p class="sum">{b.summary}</p>{/if}
          <div class="row">
            {#if shelf.has(b.id)}
              <button class="btn" onclick={() => nav.go({ name: 'reader', id: b.id })}>Read</button>
              <span class="dim">On your shelf</span>
            {:else}
              <button class="btn" disabled={busy === b.id} onclick={() => get(b, b.downloads[0].format, true)}>
                {busy === b.id ? 'Fetching…' : `Get ${b.downloads[0].format.toUpperCase()} and read`}
              </button>
              {#each b.downloads.slice(1) as d}
                <button class="btn ghost" disabled={busy === b.id} onclick={() => get(b, d.format)}>Get {d.format.toUpperCase()}</button>
              {/each}
            {/if}
          </div>
        </div>
      </article>
    {/each}
  </div>
{/snippet}

<div class="room">
  <nav class="backbar">
    <button onclick={() => (open ? (open.trail.length > 1 ? back(open.trail.length - 2) : (open = null)) : nav.back())}>
      ← {open ? (open.trail.length > 1 ? 'Back' : 'Catalogues') : 'Library'}
    </button>
    <span class="spacer"></span>
    {#if !open}<button onclick={() => startAdd()}>Add a catalogue</button>{/if}
  </nav>

  {#if error}<p class="error" role="alert">{error} <button onclick={() => (error = null)}>Dismiss</button></p>{/if}

  {#if !open}
    <header class="plaque">
      <h1>Catalogues</h1>
      <p>Book catalogues in the open OPDS format: public libraries, book sites, your own book server</p>
    </header>

    <section aria-label="Your catalogues">
      {#each catalogues as c (c.id)}
        <div class="cat paper">
          <button class="open" disabled={busy === 'page'} onclick={() => load(c, null, 'root')}>
            <strong>{c.title}</strong>
            <small>{host(c.url)}{c.user ? ' · signed in' : ''}</small>
          </button>
          <button class="btn ghost" onclick={() => startEdit(c)}>Settings</button>
          <button class="btn ghost" onclick={() => remove(c)}>Remove</button>
        </div>
      {:else}
        <p class="empty">No catalogues yet. Add one below, or paste the address of any OPDS catalogue.</p>
      {/each}
      {#if busy === 'page'}<p class="dim">Opening…</p>{/if}
    </section>

    <section aria-label="Suggested catalogues">
      <h2 class="shelf-label">Suggested</h2>
      {#each presets as p (p.url)}
        <div class="cat paper">
          <div class="open static"><strong>{p.title}</strong><small>{p.note}</small></div>
          {#if added(p.url)}<span class="dim">Added</span>{:else}<button class="btn" onclick={() => startAdd(p)}>Add</button>{/if}
        </div>
      {/each}
    </section>
  {:else if page}
    <header class="crumbs">
      <button class="link" onclick={() => back(0)}>{open.cat.title}</button>
      {#each open.trail.slice(1) as p, i}
        <span aria-hidden="true">›</span>
        {#if i + 1 < open.trail.length - 1}<button class="link" onclick={() => back(i + 1)}>{p.title || 'Results'}</button>{:else}<span>{p.title || 'Results'}</span>{/if}
      {/each}
    </header>

    {#if page.search}
      <section>
        <form class="search" onsubmit={search}>
          <input bind:value={query} placeholder="Search {open.cat.title}" aria-label="Search this catalogue" />
          <button class="btn" disabled={busy === 'page'}>{busy === 'page' ? 'Searching…' : 'Search'}</button>
        </form>
      </section>
    {/if}

    <section aria-label={page.title || 'Catalogue page'} aria-busy={busy === 'page'}>
      {#if page.links.length}{@render links(page.links)}{/if}
      {#if page.books.length}{@render books(page.books)}{/if}
      {#each page.groups as g}
        <h2 class="shelf-label">{g.title}</h2>
        {#if g.links.length}{@render links(g.links)}{/if}
        {#if g.books.length}{@render books(g.books)}{/if}
      {/each}
      {#if !page.links.length && !page.books.length && !page.groups.length}
        <p class="empty">Nothing here that this library can open. Books sold, lent with copy protection, or in audio form are left out.</p>
      {/if}
      {#if page.prev || page.next}
        <div class="row pager">
          {#if page.prev}<button class="btn ghost" disabled={busy === 'page'} onclick={() => open && load(open.cat, page.prev, 'replace')}>← Earlier</button>{/if}
          <span class="spacer"></span>
          {#if page.next}<button class="btn" disabled={busy === 'page'} onclick={() => open && load(open.cat, page.next, 'replace')}>More →</button>{/if}
        </div>
      {/if}
    </section>
  {/if}
</div>

{#if form}
  <div class="scrim" onclick={() => (form = null)} aria-hidden="true"></div>
  <div class="dialog paper" role="dialog" aria-modal="true" aria-label="Catalogue"><form onsubmit={save}>
    <h2>{form.editing ? 'Catalogue settings' : 'Add a catalogue'}</h2>
    {#if form.note}<p class="note">{form.note}</p>{/if}
    <label>Address <input bind:value={form.url} required placeholder="https://example.org/opds" inputmode="url" autocomplete="off" readonly={form.editing} /></label>
    <label>Name <input bind:value={form.title} placeholder="Taken from the catalogue if left empty" /></label>
    <label class="check"><input type="checkbox" bind:checked={form.login} /> This catalogue needs a login</label>
    {#if form.login}
      <label>User name <input bind:value={form.user} autocomplete="username" /></label>
      <label>Password <input type="password" bind:value={form.password} autocomplete="current-password" placeholder={form.editing ? 'Leave empty to keep the saved one' : ''} /></label>
      <p class="note">The password is kept in your system's keyring (on phones, in this app's private storage) and is only ever sent to this catalogue's own site.</p>
    {/if}
    <div class="row">
      <button class="btn" disabled={busy === 'save'}>{busy === 'save' ? 'Checking…' : form.editing ? 'Save' : 'Add'}</button>
      <button type="button" class="btn ghost" onclick={() => (form = null)}>Cancel</button>
    </div>
  </form></div>
{/if}

<style>
  .room {
    min-height: 100dvh; padding-bottom: 3rem; color: var(--paper);
    background:
      repeating-linear-gradient(90deg, rgb(0 0 0 / 0.12) 0 2px, transparent 2px 140px),
      radial-gradient(ellipse at 50% -10%, color-mix(in srgb, var(--gold) 25%, transparent), transparent 60%),
      linear-gradient(var(--desk-2), var(--desk));
  }
  .plaque { text-align: center; margin: 1rem auto 1.5rem; width: fit-content; max-width: 92vw; padding: 0.6rem 2.2rem; border: 2px solid var(--gold); outline: 1px solid var(--gold); outline-offset: 4px; color: var(--gold); background: linear-gradient(#2a1d13, #1d140d); border-radius: 4px; box-shadow: 0 10px 30px rgb(0 0 0 / 0.5); }
  .plaque h1 { font-family: var(--font-fell); font-weight: 400; margin: 0; font-size: clamp(1.8rem, 5vw, 2.6rem); letter-spacing: 0.06em; }
  .plaque p { margin: 0.2rem 0 0; font-family: var(--font-type); font-size: 0.8rem; opacity: 0.8; }
  section { max-width: 1000px; margin: 0 auto 1.6rem; padding: 0 1rem; display: grid; gap: 0.6rem; }
  .shelf-label { font-family: var(--font-fell); font-weight: 400; font-size: 1.3rem; margin: 0.6rem 0 0; color: color-mix(in srgb, var(--paper) 90%, transparent); }
  .dim { opacity: 0.65; font-family: var(--font-type); font-size: 0.8rem; }
  .empty { font-family: var(--font-type); opacity: 0.75; text-align: center; padding: 1.5rem; margin: 0; }
  .cat { display: flex; align-items: center; gap: 0.6rem; padding: 0.7rem 0.9rem; border-radius: 6px; flex-wrap: wrap; }
  .cat .open { all: unset; cursor: pointer; flex: 1; min-width: 12rem; display: grid; gap: 0.15rem; }
  .cat .open.static { cursor: default; }
  .cat .open:focus-visible { outline: 2px solid var(--ink); outline-offset: 3px; }
  .cat strong { font-family: var(--font-fell); font-weight: 400; font-size: 1.25rem; }
  .cat small { font-family: var(--font-type); font-size: 0.75rem; color: var(--ink-soft); line-height: 1.4; }
  .cat .dim { color: var(--ink-soft); }
  .crumbs { max-width: 1000px; margin: 1rem auto; padding: 0 1rem; display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: baseline; font-family: var(--font-fell); font-size: 1.4rem; color: var(--gold); }
  .link { all: unset; cursor: pointer; text-decoration: underline; text-decoration-thickness: 1px; text-underline-offset: 3px; }
  .search { display: flex; gap: 0.5rem; }
  .search input { flex: 1; min-width: 0; padding: 0.6rem 0.8rem; font-family: var(--font-book); font-size: 1rem; border-radius: 6px; border: 1px solid color-mix(in srgb, var(--paper) 30%, transparent); background: rgb(0 0 0 / 0.25); color: var(--paper); }
  .search .btn { background: var(--paper); color: var(--ink); border-color: var(--paper); }
  .links { list-style: none; margin: 0; padding: 0; display: grid; gap: 1px; border-radius: 6px; overflow: hidden; }
  .nl { all: unset; box-sizing: border-box; width: 100%; cursor: pointer; display: flex; align-items: center; gap: 0.8rem; padding: 0.7rem 0.9rem; background: rgb(0 0 0 / 0.22); }
  .nl:hover, .nl:focus-visible { background: rgb(0 0 0 / 0.35); }
  .nl img { width: 38px; height: 54px; object-fit: cover; border-radius: 2px; flex: none; }
  .nl span:not(.arrow) { flex: 1; display: grid; gap: 0.15rem; }
  .nl strong { font-family: var(--font-book); font-weight: 400; font-size: 1.05rem; }
  .nl small { font-family: var(--font-type); font-size: 0.72rem; opacity: 0.7; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .arrow { font-size: 1.4rem; opacity: 0.6; }
  .books { display: grid; gap: 0.8rem; }
  .book { display: grid; grid-template-columns: 90px 1fr; gap: 1rem; padding: 0.9rem; border-radius: 6px; }
  .book img, .blank { width: 90px; aspect-ratio: 2 / 3; object-fit: cover; border-radius: 2px 5px 5px 2px; box-shadow: var(--shadow); }
  .blank { display: grid; place-items: center; padding: 0.4rem; text-align: center; font-family: var(--font-fell); font-size: 0.75rem; color: var(--gold); background: #3b4a6b; overflow: hidden; }
  .book h3 { margin: 0; font-family: var(--font-fell); font-weight: 400; font-size: 1.3rem; line-height: 1.15; }
  .by { margin: 0.2rem 0 0; font-family: var(--font-type); font-size: 0.78rem; color: var(--ink-soft); }
  .sum { margin: 0.4rem 0 0; font-family: var(--font-book); font-size: 0.9rem; line-height: 1.5; display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; }
  .book .dim { color: var(--ink-soft); }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-top: 0.6rem; }
  .pager .spacer { flex: 1; }
  .pager .btn.ghost { color: var(--paper); border-color: color-mix(in srgb, var(--paper) 50%, transparent); }
  .error { max-width: 1000px; margin: 1rem auto; padding: 0.6rem 1rem; background: var(--accent); color: var(--paper); font-family: var(--font-type); border-radius: 6px; }
  .error button { background: none; border: 1px solid var(--paper); color: var(--paper); border-radius: 4px; }
  .scrim { position: fixed; inset: 0; background: rgb(0 0 0 / 0.55); z-index: 30; }
  .dialog { position: fixed; z-index: 31; left: 50%; top: 50%; translate: -50% -50%; width: min(94vw, 520px); max-height: 90dvh; overflow: auto; display: grid; gap: 0.7rem; padding: 1.3rem; border-radius: 6px; box-shadow: var(--shadow); }
  .dialog form { display: contents; }
  .dialog h2 { margin: 0; font-family: var(--font-fell); font-weight: 400; font-size: 1.6rem; }
  .dialog label { display: grid; gap: 0.25rem; font-family: var(--font-type); font-size: 0.8rem; color: var(--ink-soft); }
  .dialog label.check { display: flex; align-items: center; gap: 0.5rem; }
  .dialog input:not([type='checkbox']) { font-family: var(--font-book); font-size: 1rem; padding: 0.5rem 0.6rem; border-radius: 4px; border: 1px solid color-mix(in srgb, var(--ink) 30%, transparent); background: rgb(255 255 255 / 0.35); color: var(--ink); }
  .dialog input[readonly] { opacity: 0.7; }
  .note { margin: 0; font-family: var(--font-type); font-size: 0.75rem; line-height: 1.45; color: var(--ink-soft); }
</style>
