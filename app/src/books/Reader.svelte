<script lang="ts">
  import { onMount } from 'svelte'
  import { api, daypart, device, errText, HIGHLIGHT_COLORS, type Change, type Highlight, type HighlightColor, type Question, type Settings, type Shelved } from '../lib/api'
  import { nav } from '../lib/nav.svelte'
  import { reducedMotion } from '../lib/motion'
  import { FONTS, fontFaces, palette } from './look'
  import { fileOf, foliate, overlayerModule, type FoliateBook, type Relocate } from './foliate'

  // The <foliate-view> element from the vendored foliate-js.
  type Renderer = HTMLElement & { setStyles?: (css: string) => void; render?: () => void; heads?: unknown[] | null }
  type View = HTMLElement & {
    book: FoliateBook
    renderer: Renderer
    isFixedLayout: boolean
    open(file: File): Promise<void>
    init(o: { lastLocation?: string | null; showTextStart?: boolean }): Promise<void>
    goLeft(): Promise<void>
    goRight(): Promise<void>
    next(): Promise<void>
    prev(): Promise<void>
    close(): void
    goTo(target: string): Promise<unknown>
    getCFI(index: number, range: Range): string
    resolveCFI(cfi: string): { index: number }
    getSectionFractions(): number[]
    getProgressOf(index: number, range: Range): { tocItem?: { label?: string } }
    addAnnotation(a: { value: string; color?: string }): Promise<unknown>
    deleteAnnotation(a: { value: string }): Promise<unknown>
    search(o: { query: string; drawOptions?: { color?: string; width?: number } }): AsyncGenerator<SearchStep>
    clearSearch(): void
  }
  type Excerpt = { pre: string; match: string; post: string }
  type SearchStep = 'done' | { progress: number } | { label: string; subitems: { cfi: string; excerpt: Excerpt }[] }
  type Draw = (f: unknown, o: { color: string }) => void

  let { id }: { id: string } = $props()

  const dev = device()
  const part = daypart()
  let host: HTMLDivElement
  let view: View | null = null
  let meta = $state<Shelved | null>(null)
  let settings = $state<Settings | null>(null)
  let question = $state<Question | null>(null)
  let history = $state<Change[]>([])
  let asking = $state(false)
  let panel = $state(false)
  let chrome = $state(true)
  let error = $state<string | null>(null)
  let percent = $state(0)
  let chapter = $state('')
  let pageInfo = $state('')
  let sweep = $state(0)
  let spread = $state(false)
  // Fixed-layout books (PDF, comics, pre-paginated EPUB) are pictures of pages.
  let fixed = $state(false)
  let zoom = $state<string>('fit-page')
  let startedAt = Date.now()
  let turns = 0
  let idle: ReturnType<typeof setTimeout> | undefined
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  // Highlights and notes (reflowable books only: fixed pages have no text overlay).
  let marks = $state<Highlight[]>([])
  let notes = $state(false)
  /** The passage being selected right now, and where to show the toolbar. */
  let sel = $state<{ index: number; range: Range; x: number; y: number; below: boolean } | null>(null)
  /** The highlight whose note or colour is being changed. */
  let editing = $state<{ mark: Highlight; draft: string } | null>(null)
  let notice = $state('')

  // Find in this book.
  let finding = $state(false)
  let findQuery = $state('')
  let found = $state<{ label: string; items: { cfi: string; excerpt: Excerpt }[] }[]>([])
  let findProgress = $state<number | null>(null)
  let findRun = 0
  /** The query whose search finished, for the no-matches line. */
  let searched = $state('')
  const MAX_FOUND = 500
  const foundCount = $derived(found.reduce((n, g) => n + g.items.length, 0))
  const SWATCH: Record<HighlightColor, string> = { yellow: '#e2b93b', green: '#7fb069', blue: '#6c9bd2', pink: '#de7fa2' }
  const docs = new Map<Document, number>()
  let highlightFn: unknown = null

  const look = $derived(settings ? palette(settings) : null)

  /** Book CSS: our paper, ink and type win over the publisher's. */
  function bookCss(s: Settings, l: NonNullable<typeof look>) {
    const family = FONTS[s.font] ?? FONTS.Literata
    return `@namespace epub "http://www.idpf.org/2007/ops";
      ${fontFaces}
      html { color: ${l.ink} !important; background: ${l.paper} !important; font-size: ${s.font_size}px !important; }
      body { color: inherit !important; background: transparent !important; font-family: ${family} !important;
        font-size: ${s.font_size}px !important; line-height: ${s.line_height} !important; }
      p, li, blockquote, dd { font-family: inherit !important; line-height: ${s.line_height} !important;
        text-align: justify; hyphens: auto; -webkit-hyphens: auto; widows: 2; orphans: 2; }
      [align="left"] { text-align: left; } [align="right"] { text-align: right; } [align="center"] { text-align: center; }
      img, svg { max-width: 100%; ${l.dark ? '' : 'mix-blend-mode: multiply;'} filter: grayscale(0.3); }
      /* Paper all the way through: no white boxes or blue links from the publisher's CSS. */
      body *:not(img):not(svg):not(image) { background-color: transparent !important; }
      a:link, a:visited { color: inherit !important; text-decoration-color: ${l.faint} !important; }
      pre { white-space: pre-wrap !important; }
      ::selection { background: rgb(196 154 69 / 0.35); }
      aside[epub|type~="footnote"], aside[epub|type~="endnote"], aside[epub|type~="rearnote"] { display: none; }`
  }

  function apply() {
    if (!view || !settings || !look) return
    const r = view.renderer
    if (fixed) {
      r.setAttribute('zoom', zoom)
      return
    }
    // Lines near 66 characters at the chosen size. foliate-js rounds the column count
    // up, so allow a two-page spread only when each page still holds about 52 characters.
    const minColumn = settings.font_size * 26
    r.setAttribute('max-inline-size', `${Math.round(settings.font_size * 34)}px`)
    r.setAttribute('max-column-count', dev !== 'phone' && host.clientWidth * (1 - 2 * settings.margin) >= 2 * minColumn ? '2' : '1')
    r.setAttribute('gap', `${(settings.margin * 100).toFixed(1)}%`)
    r.setAttribute('margin', '28px')
    r.setStyles?.(bookCss(settings, look))
    r.render?.()
  }

  function wake() {
    chrome = true
    clearTimeout(idle)
    idle = setTimeout(() => { if (!panel && !asking) chrome = false }, 3500)
  }

  /** Physical direction: -1 is the left page, 1 the right. */
  function turn(d: 1 | -1, logical = false) {
    if (!view) return
    if (!reducedMotion()) sweep = d * (sweep && Math.sign(sweep) === d ? Math.abs(sweep) + 1 : 1)
    if (logical) d > 0 ? view.next() : view.prev()
    else d > 0 ? view.goRight() : view.goLeft()
    turns++
    maybeAsk()
  }

  // Ask one comfort question per session, after the reader has settled in.
  function maybeAsk() {
    if (asking || !question) return
    let asked = false
    try { asked = sessionStorage.getItem('granthalaya.asked') === '1' } catch { /* private mode */ }
    if (asked) return
    if (Date.now() - startedAt > 4 * 60_000 || turns >= 20) {
      asking = true
      chrome = true
      try { sessionStorage.setItem('granthalaya.asked', '1') } catch { /* ignore */ }
    }
  }

  async function answer(i: number) {
    if (!question) return
    settings = await api.comfortAnswer(dev, part, question.id, i)
    apply()
    const v = await api.comfortGet(dev, part)
    question = v.question
    history = v.history
    asking = false
  }

  async function setManual(patch: Partial<Settings>) {
    if (!settings) return
    settings = { ...settings, ...patch }
    apply()
    await api.comfortSet(dev, part, $state.snapshot(settings))
  }

  function setZoom(z: string) {
    zoom = z
    apply()
  }
  function zoomBy(f: number) {
    const now = Number(zoom) || 1
    setZoom(String(Math.min(4, Math.max(0.5, Math.round(now * f * 100) / 100))))
  }

  function relocated(d: Relocate) {
    percent = d.fraction ?? 0
    if (d.tocItem?.label) chapter = d.tocItem.label.trim()
    pageInfo = fixed && d.section
      ? `Page ${d.section.current + 1} of ${d.section.total}`
      : d.pageItem?.label ? `Page ${d.pageItem.label}`
      : d.location ? `${d.location.current + 1} of ${d.location.total}` : ''
    spread = !fixed && (view?.renderer.heads?.length ?? 1) > 1
    sel = null
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => api.bookProgress(id, percent, d.cfi).catch(() => {}), 600)
  }

  function loaded(doc: Document, index: number) {
    docs.set(doc, index)
    doc.addEventListener('keydown', key)
    // A plain tap (no text being selected) brings the bars back.
    doc.addEventListener('click', () => { if (!doc.getSelection()?.toString()) wake() })
    if (fixed) return
    let t: ReturnType<typeof setTimeout> | undefined
    doc.addEventListener('selectionchange', () => {
      clearTimeout(t)
      t = setTimeout(() => selected(doc), 250)
    })
  }

  /** Show the highlight toolbar over a fresh selection. */
  function selected(doc: Document) {
    const s = doc.getSelection()
    if (!s || s.isCollapsed || !s.rangeCount || !s.toString().trim()) { sel = null; return }
    const range = s.getRangeAt(0)
    const frame = doc.defaultView?.frameElement?.getBoundingClientRect()
    const r = range.getBoundingClientRect()
    if (!frame) return
    const top = frame.top + r.top
    const below = top < 90
    sel = {
      index: docs.get(doc) ?? 0,
      range,
      x: Math.min(window.innerWidth - 150, Math.max(150, frame.left + r.left + r.width / 2)),
      y: below ? frame.top + r.bottom + 10 : top - 10,
      below,
    }
  }

  /** Where a passage sits in the whole book, for reading order. */
  function placeOf(index: number, range: Range) {
    const f = view?.getSectionFractions() ?? []
    const start = f[index] ?? percent
    const end = f[index + 1] ?? start
    const doc = range.startContainer.ownerDocument
    let within = 0
    if (doc?.body) {
      const before = doc.createRange()
      before.setStart(doc.body, 0)
      before.setEnd(range.startContainer, range.startOffset)
      within = before.toString().length / Math.max(1, doc.body.textContent?.length ?? 1)
    }
    return Math.min(1, start + (end - start) * within)
  }

  async function mark(color: HighlightColor, withNote = false) {
    if (!view || !sel || !meta) return
    const { index, range } = sel
    try {
      const saved = await api.highlightSave({
        book_id: id,
        book_title: meta.book.title,
        book_author: meta.book.authors[0] ?? '',
        cfi: view.getCFI(index, range),
        text: range.toString().trim(),
        color,
        chapter: view.getProgressOf(index, range).tocItem?.label?.trim() ?? chapter,
        progress: placeOf(index, range),
        note: marks.find((m) => m.cfi === view?.getCFI(index, range))?.note ?? '',
      })
      marks = [...marks.filter((m) => m.id !== saved.id), saved].sort((a, b) => a.progress - b.progress)
      await view.addAnnotation({ value: saved.cfi, color: saved.color })
      range.startContainer.ownerDocument?.getSelection()?.removeAllRanges()
      sel = null
      if (withNote) editing = { mark: saved, draft: saved.note }
    } catch (e) {
      say(errText(e))
    }
  }

  async function saveEdit(color?: HighlightColor) {
    if (!editing || !view) return
    try {
      const saved = await api.highlightSave({ ...$state.snapshot(editing.mark), note: editing.draft.trim(), color: color ?? editing.mark.color })
      marks = marks.map((m) => (m.id === saved.id ? saved : m))
      await view.addAnnotation({ value: saved.cfi, color: saved.color })
      if (color) editing = { mark: saved, draft: editing.draft }
      else editing = null
    } catch (e) {
      say(errText(e))
    }
  }

  async function removeMark(m: Highlight) {
    try {
      await api.highlightDelete(m.id)
      marks = marks.filter((x) => x.id !== m.id)
      await view?.deleteAnnotation({ value: m.cfi })
      editing = null
    } catch (e) {
      say(errText(e))
    }
  }

  /** One side panel at a time; the same button closes it. */
  function openPanel(which: 'find' | 'notes' | 'comfort') {
    const was = which === 'find' ? finding : which === 'notes' ? notes : panel
    finding = notes = panel = false
    if (!was) {
      if (which === 'find') finding = true
      else if (which === 'notes') notes = true
      else panel = true
    }
    wake()
  }

  async function exportNotes() {
    try {
      if (await api.highlightsExport(id)) say('Notes saved.')
    } catch (e) {
      say(errText(e))
    }
  }

  function jump(m: Highlight) {
    notes = false
    view?.goTo(m.cfi).catch(() => say('That passage could not be found in this copy of the book.'))
  }

  async function find(e?: Event) {
    e?.preventDefault()
    if (!view) return
    const run = ++findRun
    found = []
    searched = ''
    const q = findQuery.trim()
    if (!q) { view.clearSearch(); findProgress = null; return }
    findProgress = 0
    let n = 0
    try {
      for await (const step of view.search({ query: q, drawOptions: { color: '#c49a45', width: 2 } })) {
        if (run !== findRun) return
        if (step === 'done') break
        if ('progress' in step) findProgress = step.progress
        else if ('subitems' in step) {
          const items = step.subitems.slice(0, MAX_FOUND - n)
          n += items.length
          found = [...found, { label: step.label?.trim() ?? '', items }]
          if (n >= MAX_FOUND) break
        }
      }
    } catch (err) {
      say(errText(err))
    }
    if (run === findRun) { findProgress = null; searched = q }
  }

  function clearFind() {
    findRun++
    findQuery = ''
    searched = ''
    found = []
    findProgress = null
    view?.clearSearch()
  }

  function goFound(cfi: string) {
    if (dev === 'phone') finding = false
    view?.goTo(cfi).catch(() => {})
  }

  function say(m: string) {
    notice = m
    setTimeout(() => { if (notice === m) notice = '' }, 4000)
  }

  onMount(() => {
    let disposed = false
    let ready: () => void = () => {}
    const opened = new Promise<void>((r) => (ready = r))
    ;(async () => {
      try {
        const [shelf, comfort, bytes, , ov, saved] = await Promise.all([
          api.library(), api.comfortGet(dev, part), api.bookBytes(id), foliate(), overlayerModule(), api.highlights(id),
        ])
        if (disposed) return
        highlightFn = ov.Overlayer.highlight
        marks = saved
        meta = shelf.find((s) => s.book.id === id) ?? null
        settings = comfort.settings
        question = comfort.question
        history = comfort.history
        const v = document.createElement('foliate-view') as View
        view = v
        host.append(v)
        v.addEventListener('relocate', (e) => relocated((e as CustomEvent<Relocate>).detail))
        v.addEventListener('load', (e) => { const d = (e as CustomEvent<{ doc: Document; index: number }>).detail; loaded(d.doc, d.index) })
        // Each section gets its own overlay when shown: put its highlights back on it.
        v.addEventListener('create-overlay', (e) => {
          const { index } = (e as CustomEvent<{ index: number }>).detail
          for (const m of marks) {
            try { if (v.resolveCFI(m.cfi).index === index) v.addAnnotation({ value: m.cfi, color: m.color }) } catch { /* stale place */ }
          }
        })
        v.addEventListener('draw-annotation', (e) => {
          const { draw, annotation } = (e as CustomEvent<{ draw: Draw; annotation: { color?: HighlightColor } }>).detail
          draw(highlightFn, { color: SWATCH[annotation.color ?? 'yellow'] ?? SWATCH.yellow })
        })
        v.addEventListener('show-annotation', (e) => {
          const m = marks.find((x) => x.cfi === (e as CustomEvent<{ value: string }>).detail.value)
          if (m) editing = { mark: m, draft: m.note }
        })
        await v.open(fileOf(bytes, meta?.book.title ?? 'book', meta?.format ?? 'epub'))
        if (disposed) return
        fixed = v.isFixedLayout
        if (!fixed) {
          v.renderer.setAttribute('flow', 'paginated')
          if (!reducedMotion()) v.renderer.setAttribute('animated', '')
        }
        apply()
        try {
          await v.init({ lastLocation: meta?.location ?? null })
        } catch {
          // A place saved by the old reader that no longer resolves: start at the beginning.
          await v.init({ lastLocation: null })
        }
        ready()
        wake()
      } catch (e) {
        error = errText(e)
      }
    })()
    window.addEventListener('keydown', key)
    // Spread or single page depends on the width; watched once the book is open.
    let lastWidth = 0
    const sized = new ResizeObserver(() => {
      if (!lastWidth || Math.abs(host.clientWidth - lastWidth) < 24) return
      lastWidth = host.clientWidth
      apply()
    })
    sized.observe(host)
    opened.then(() => (lastWidth = host.clientWidth))
    return () => {
      disposed = true
      sized.disconnect()
      window.removeEventListener('keydown', key)
      clearTimeout(idle)
      view?.close()
      view?.remove()
    }
  })

  function key(e: KeyboardEvent) {
    // Typing a note or moving a slider must not turn the page.
    const t = e.target as HTMLElement | null
    if (t && (t.tagName === 'TEXTAREA' || t.tagName === 'INPUT' || t.tagName === 'SELECT')) {
      if (e.key === 'Escape') { editing = null; finding = false }
      return
    }
    if (e.key === 'ArrowRight') { e.preventDefault(); turn(1) }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); turn(-1) }
    else if (e.key === 'PageDown' || e.key === ' ') { e.preventDefault(); turn(1, true) }
    else if (e.key === 'PageUp') { e.preventDefault(); turn(-1, true) }
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') { e.preventDefault(); openPanel('find') }
    else if (e.key === 'Escape') {
      if (editing) editing = null
      else if (finding) finding = false
      else if (sel) sel = null
      else if (notes) notes = false
      else if (panel) panel = false
      else nav.back()
    }
  }

  const margin = $derived(settings ? `${(settings.margin * 100).toFixed(1)}%` : '8%')
  // Ink softening for fixed pages: 1 keeps the file's own contrast.
  const soft = $derived(settings ? Math.min(1, 1.25 - settings.contrast * 0.5) : 1)
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="reader"
  class:dark={look?.dark}
  class:fixed
  style="--paper:{look?.paper ?? 'var(--paper)'}; --ink:{look?.ink ?? 'var(--ink)'}; --faint:{look?.faint ?? 'var(--ink-faint)'}; --m:{margin}; --soft:{soft}; --lift:{1 / (0.5 + 0.5 * soft)}"
  onpointermove={wake}
>
  <header class="top" class:hide={!chrome}>
    <button onclick={() => nav.back()} aria-label="Back to the library">← Library</button>
    <div class="t"><strong>{meta?.book.title ?? ''}</strong>{#if chapter}<span> · {chapter}</span>{/if}</div>
    {#if !fixed}<button onclick={() => openPanel('find')} aria-expanded={finding}>Find</button>{/if}
    <button onclick={() => openPanel('notes')} aria-expanded={notes}>Notes{#if marks.length}<span class="count">{marks.length}</span>{/if}</button>
    <button onclick={() => openPanel('comfort')} aria-expanded={panel} aria-label="Reading comfort">Aa</button>
  </header>

  <div class="pagewrap">
    <button class="zone left" onclick={() => turn(-1)} aria-label="Previous page"></button>
    <div class="page" bind:this={host} style="view-transition-name: book-page"></div>
    {#if fixed}<div class="tint" aria-hidden="true"></div>{/if}
    <button class="zone right" onclick={() => turn(1)} aria-label="Next page"></button>
    {#key sweep}{#if sweep}<div class="sweep" class:back={sweep < 0} aria-hidden="true"></div>{/if}{/key}
    {#if spread}<div class="gutter" aria-hidden="true"></div>{/if}
  </div>

  <footer class="bottom" class:hide={!chrome}>
    <span>{pageInfo}</span>
    <div class="prog" aria-label="{Math.round(percent * 100)} percent read"><span style="width:{percent * 100}%"></span></div>
    <span>{Math.round(percent * 100)}%</span>
    {#if fixed}
      <span class="zoom" role="group" aria-label="Page size">
        <button onclick={() => setZoom(zoom === 'fit-width' ? 'fit-page' : 'fit-width')}>{zoom === 'fit-width' ? 'Whole page' : 'Fit width'}</button>
        <button onclick={() => zoomBy(1 / 1.2)} aria-label="Smaller">−</button>
        <button onclick={() => zoomBy(1.2)} aria-label="Larger">+</button>
      </span>
    {/if}
  </footer>

  {#if error}<p class="err" role="alert">This book could not be opened: {error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}

  {#if sel}
    <div class="selbar" class:below={sel.below} style="left:{sel.x}px; top:{sel.y}px" role="toolbar" aria-label="Highlight the selected passage">
      {#each HIGHLIGHT_COLORS as c}
        <button class="sw" style="--c:{SWATCH[c]}" onclick={() => mark(c)} aria-label="Highlight in {c}"></button>
      {/each}
      <button class="nb" onclick={() => mark(marks.find((m) => m.cfi === view?.getCFI(sel!.index, sel!.range))?.color ?? 'yellow', true)}>Note</button>
    </div>
  {/if}

  {#if editing}
    <div class="slip edit" role="dialog" aria-label="Highlight and note">
      <blockquote style="--c:{SWATCH[editing.mark.color]}">{editing.mark.text}</blockquote>
      <div class="colors" role="group" aria-label="Colour">
        {#each HIGHLIGHT_COLORS as c}
          <button class="sw" class:on={editing.mark.color === c} style="--c:{SWATCH[c]}" onclick={() => saveEdit(c)} aria-label="{c}" aria-pressed={editing.mark.color === c}></button>
        {/each}
      </div>
      <!-- svelte-ignore a11y_autofocus -->
      <textarea bind:value={editing.draft} rows="4" placeholder="Your note (optional)" aria-label="Note" autofocus></textarea>
      <div class="row">
        <button class="later" onclick={() => editing && removeMark(editing.mark)}>Remove highlight</button>
        <span></span>
        <button class="later" onclick={() => (editing = null)}>Cancel</button>
        <button class="save" onclick={() => saveEdit()}>Save</button>
      </div>
    </div>
  {/if}

  {#if finding}
    <aside class="panel" aria-label="Find in this book">
      <h2>Find in this book</h2>
      <form class="find" onsubmit={find}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={findQuery} placeholder="A word or phrase" aria-label="Find in this book" autofocus />
        <button>Find</button>
      </form>
      {#if findProgress !== null}
        <p class="hint" role="status">Searching… {Math.round(findProgress * 100)}%</p>
      {:else if searched && foundCount === 0}
        <p class="hint" role="status">No matches for “{searched}”.</p>
      {:else if foundCount}
        <p class="hint" role="status">{foundCount >= MAX_FOUND ? `First ${MAX_FOUND} matches` : `${foundCount} ${foundCount === 1 ? 'match' : 'matches'}`} · <button class="linkish" onclick={clearFind}>Clear</button></p>
      {/if}
      {#each found as g}
        {#if g.label}<h3>{g.label}</h3>{/if}
        <ol class="marks">
          {#each g.items as r (r.cfi)}
            <li><button onclick={() => goFound(r.cfi)}><span class="q">{r.excerpt.pre}<mark>{r.excerpt.match}</mark>{r.excerpt.post}</span></button></li>
          {/each}
        </ol>
      {/each}
    </aside>
  {/if}

  {#if notes}
    <aside class="panel" aria-label="Highlights and notes">
      <h2>Highlights and notes</h2>
      {#if fixed}
        <p class="hint">This book is made of fixed pages (a PDF or comic). Highlighting works in EPUB, MOBI, AZW3 and FB2 books; fixed pages can't be highlighted yet.</p>
      {:else if !marks.length}
        <p class="hint">Select some text to highlight it or add a note. Everything you mark is kept here, even if the book leaves your shelf.</p>
      {/if}
      {#if marks.length}
        <ol class="marks">
          {#each marks as m (m.id)}
            <li style="--c:{SWATCH[m.color]}">
              <button onclick={() => jump(m)}>
                {#if m.chapter}<small>{m.chapter}</small>{/if}
                <span class="q">{m.text}</span>
                {#if m.note}<span class="n">{m.note}</span>{/if}
              </button>
            </li>
          {/each}
        </ol>
        <button class="ask" onclick={exportNotes}>Save as Markdown…</button>
      {/if}
    </aside>
  {/if}

  {#if asking && question}
    <div class="slip" role="dialog" aria-label="A quick question about reading comfort">
      <p class="q">{question.prompt}</p>
      <div class="ans">
        {#each question.answers as a, i}<button onclick={() => answer(i)} class:mid={i === 1}>{a}</button>{/each}
      </div>
      <button class="later" onclick={() => (asking = false)}>Not now</button>
    </div>
  {/if}

  {#if panel && settings}
    <aside class="panel" aria-label="Reading comfort settings">
      <h2>Reading comfort</h2>
      <p class="hint">Tuned for {dev === 'phone' ? 'this phone' : dev === 'tablet' ? 'this tablet' : 'this screen'} in the {part === 'day' ? 'daytime' : 'evening'}. The library adjusts these from your answers; change anything here and it keeps your choice.</p>
      {#if !fixed}
        <label>Text size <span>{settings.font_size.toFixed(0)}px</span>
          <input type="range" min="13" max="34" step="0.5" value={settings.font_size} oninput={(e) => setManual({ font_size: +(e.target as HTMLInputElement).value })} /></label>
        <label>Line spacing <span>{settings.line_height.toFixed(2)}</span>
          <input type="range" min="1.2" max="2.1" step="0.05" value={settings.line_height} oninput={(e) => setManual({ line_height: +(e.target as HTMLInputElement).value })} /></label>
        <label>Margins <span>{Math.round(settings.margin * 100)}%</span>
          <input type="range" min="0.02" max="0.18" step="0.01" value={settings.margin} oninput={(e) => setManual({ margin: +(e.target as HTMLInputElement).value })} /></label>
      {:else}
        <p class="hint">This book is made of fixed pages, so text size and spacing come from the file. Paper colour, brightness and ink still apply.</p>
      {/if}
      <label>Page brightness
        <input type="range" min="0.2" max="0.95" step="0.01" value={settings.lightness} oninput={(e) => setManual({ lightness: +(e.target as HTMLInputElement).value })} /></label>
      <label>Warmth
        <input type="range" min="0" max="1" step="0.01" value={settings.warmth} oninput={(e) => setManual({ warmth: +(e.target as HTMLInputElement).value })} /></label>
      <label>Ink darkness
        <input type="range" min="0.3" max="0.95" step="0.01" value={settings.contrast} oninput={(e) => setManual({ contrast: +(e.target as HTMLInputElement).value })} /></label>
      {#if !fixed}
        <label>Typeface
          <select value={settings.font} onchange={(e) => setManual({ font: (e.target as HTMLSelectElement).value })}>
            {#each Object.keys(FONTS) as f}<option value={f}>{f}</option>{/each}
          </select></label>
      {/if}
      {#if question}<button class="ask" onclick={() => { asking = true; panel = false }}>Ask me a comfort question</button>{/if}
      {#if history.length}
        <h3>What the library changed</h3>
        <ul>{#each history.slice(0, 6) as h}<li><em>“{h.answer}”</em> → {h.summary}</li>{/each}</ul>
      {/if}
    </aside>
  {/if}
</div>

<style>
  .reader {
    position: fixed; inset: 0; background: var(--paper); color: var(--ink);
    display: grid; grid-template-rows: auto 1fr auto;
    /* One column no wider than the screen: a long title must not widen the page. */
    grid-template-columns: minmax(0, 1fr);
    transition: background 600ms, color 600ms;
  }
  .reader::before { content: ''; position: absolute; inset: 0; pointer-events: none; background-image: var(--grain); opacity: 0.5; mix-blend-mode: multiply; z-index: 1; }
  .dark::before { mix-blend-mode: screen; opacity: 0.15; }
  .top, .bottom {
    position: relative; z-index: 3; display: flex; align-items: center; gap: 0.8rem;
    padding: calc(env(safe-area-inset-top) + 0.5rem) 1rem 0.5rem; font-family: var(--font-type); font-size: 0.8rem; color: var(--faint);
    transition: opacity 500ms, translate 500ms var(--ease-out);
  }
  .bottom { padding: 0.4rem 1rem calc(env(safe-area-inset-bottom) + 0.6rem); }
  .hide { opacity: 0; }
  .top.hide { translate: 0 -6px; }
  .top button, .zoom button { background: none; border: 1px solid color-mix(in srgb, var(--faint) 50%, transparent); color: var(--faint); border-radius: 6px; padding: 0.25em 0.7em; font-family: var(--font-book); }
  .zoom { display: inline-flex; gap: 0.3rem; }
  .zoom button { padding: 0.1em 0.6em; font-size: 0.75rem; }
  .t { flex: 1; min-width: 0; text-align: center; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .t strong { font-family: var(--font-book); font-weight: 400; font-style: italic; color: var(--ink); opacity: 0.75; }
  .pagewrap { position: relative; overflow: hidden; }
  .page { height: 100%; position: relative; z-index: 0; }
  .page :global(foliate-view) { display: block; width: 100%; height: 100%; }
  /* Fixed pages are white pictures: tint them like paper and soften the ink; at night, turn them dark. */
  /* contrast() softens the ink; brightness() puts white back to white so the tint stays even. */
  .fixed .page { filter: contrast(var(--soft)) brightness(var(--lift)); background: #fff; }
  .tint { position: absolute; inset: 0; z-index: 1; pointer-events: none; background: var(--paper); mix-blend-mode: multiply; }
  .fixed.dark .page { filter: invert(0.92) hue-rotate(180deg) sepia(0.25); }
  .fixed.dark .tint { display: none; }
  .zone { position: absolute; top: 0; bottom: 0; width: max(var(--m), 44px); background: none; border: 0; z-index: 2; cursor: pointer; }
  .fixed .zone { width: 12%; }
  .zone.left { left: 0; cursor: w-resize; }
  .zone.right { right: 0; cursor: e-resize; }
  /* Spine shadow in two-page spreads on wide screens. */
  .gutter { position: absolute; top: 4%; bottom: 4%; left: 50%; width: 60px; translate: -50% 0; pointer-events: none; z-index: 1; background: linear-gradient(90deg, transparent, rgb(0 0 0 / 0.07) 45%, rgb(0 0 0 / 0.12) 50%, rgb(0 0 0 / 0.07) 55%, transparent); }
  /* Soft page-turn: a shadow sweeps across like a lifted leaf. No flash. */
  .sweep { position: absolute; inset: 0; z-index: 2; pointer-events: none; background: linear-gradient(90deg, transparent 0%, rgb(0 0 0 / 0.08) 48%, rgb(255 255 255 / 0.1) 52%, transparent 60%); background-size: 250% 100%; animation: sweep 420ms var(--ease-out) forwards; }
  .sweep.back { animation-direction: reverse; }
  @keyframes sweep { from { background-position: 100% 0; } to { background-position: -50% 0; opacity: 0; } }
  .prog { flex: 1; height: 2px; background: color-mix(in srgb, var(--faint) 30%, transparent); }
  .prog span { display: block; height: 100%; background: var(--faint); transition: width 400ms; }
  .err { position: absolute; inset: 40% 10% auto; text-align: center; font-family: var(--font-type); z-index: 5; }
  .slip {
    position: absolute; z-index: 6; left: 50%; bottom: calc(env(safe-area-inset-bottom) + 3.2rem); translate: -50% 0;
    width: min(92vw, 460px); padding: 1rem 1.2rem 0.8rem; text-align: center;
    background: color-mix(in srgb, var(--paper) 92%, var(--ink)); color: var(--ink);
    border: 1px solid color-mix(in srgb, var(--ink) 25%, transparent); border-radius: 4px;
    box-shadow: 0 12px 30px rgb(0 0 0 / 0.25); animation: slip 700ms var(--ease-spring) both;
  }
  .q { font-family: var(--font-book); font-size: 1.1rem; margin: 0 0 0.7rem; }
  .ans { display: flex; gap: 0.4rem; justify-content: center; flex-wrap: wrap; }
  .ans button { font-family: var(--font-book); border: 1px solid color-mix(in srgb, var(--ink) 35%, transparent); background: transparent; color: var(--ink); border-radius: 999px; padding: 0.4em 0.9em; }
  .ans button.mid { background: var(--ink); color: var(--paper); }
  .later { margin-top: 0.5rem; background: none; border: 0; color: var(--faint); font-family: var(--font-type); font-size: 0.75rem; }
  .panel {
    position: absolute; z-index: 6; top: calc(env(safe-area-inset-top) + 3rem); right: 0.8rem; width: min(92vw, 340px); max-height: 80dvh; overflow: auto;
    padding: 1rem 1.1rem; background: color-mix(in srgb, var(--paper) 94%, var(--ink)); color: var(--ink);
    border: 1px solid color-mix(in srgb, var(--ink) 20%, transparent); border-radius: 6px; box-shadow: 0 16px 40px rgb(0 0 0 / 0.3);
    animation: slip 500ms var(--ease-spring) both; display: grid; gap: 0.7rem; font-family: var(--font-book);
  }
  .panel h2 { margin: 0; font-weight: 400; font-size: 1.3rem; }
  .panel h3 { margin: 0.4rem 0 0; font-weight: 400; font-size: 1rem; }
  .hint { margin: 0; font-family: var(--font-type); font-size: 0.72rem; color: var(--faint); line-height: 1.4; }
  label { display: grid; grid-template-columns: 1fr auto; gap: 0.2rem; font-size: 0.9rem; }
  label input, label select { grid-column: 1 / -1; accent-color: var(--ink); }
  label select { font-family: var(--font-book); padding: 0.3rem; background: transparent; color: var(--ink); border: 1px solid color-mix(in srgb, var(--ink) 30%, transparent); border-radius: 4px; }
  label span { font-family: var(--font-type); font-size: 0.75rem; color: var(--faint); }
  .ask { font-family: var(--font-type); background: none; border: 1px dashed color-mix(in srgb, var(--ink) 40%, transparent); color: var(--ink); border-radius: 6px; padding: 0.4rem; }
  ul { margin: 0; padding-left: 1rem; font-size: 0.82rem; line-height: 1.5; }
  .page { --overlayer-highlight-opacity: 0.38; --overlayer-highlight-blend-mode: multiply; }
  .dark .page { --overlayer-highlight-opacity: 0.3; --overlayer-highlight-blend-mode: screen; }
  .count { margin-left: 0.35em; font-family: var(--font-type); font-size: 0.7em; opacity: 0.8; }
  .notice { position: absolute; z-index: 7; left: 50%; top: calc(env(safe-area-inset-top) + 3.2rem); translate: -50% 0; margin: 0; padding: 0.4rem 0.9rem; border-radius: 4px; background: var(--ink); color: var(--paper); font-family: var(--font-type); font-size: 0.78rem; }
  .selbar {
    position: fixed; z-index: 7; translate: -50% -100%; display: flex; gap: 0.35rem; align-items: center; padding: 0.35rem 0.45rem;
    background: color-mix(in srgb, var(--paper) 90%, var(--ink)); border: 1px solid color-mix(in srgb, var(--ink) 25%, transparent);
    border-radius: 999px; box-shadow: 0 8px 22px rgb(0 0 0 / 0.25);
  }
  .selbar.below { translate: -50% 0; }
  .sw { width: 30px; height: 30px; border-radius: 50%; border: 2px solid color-mix(in srgb, var(--ink) 20%, transparent); background: var(--c); padding: 0; cursor: pointer; }
  .sw.on { border-color: var(--ink); }
  .nb, .save { font-family: var(--font-type); font-size: 0.78rem; border: 1px solid color-mix(in srgb, var(--ink) 35%, transparent); background: transparent; color: var(--ink); border-radius: 999px; padding: 0.35em 0.9em; min-height: 30px; }
  .save { background: var(--ink); color: var(--paper); }
  .edit { text-align: left; bottom: auto; top: 50%; translate: -50% -50%; display: grid; gap: 0.6rem; }
  .edit blockquote { margin: 0; padding-left: 0.7rem; border-left: 4px solid var(--c); font-family: var(--font-book); font-size: 0.95rem; max-height: 9rem; overflow: auto; white-space: pre-wrap; }
  .colors { display: flex; gap: 0.4rem; }
  .edit textarea { font-family: var(--font-book); font-size: 0.95rem; background: transparent; color: var(--ink); border: 1px solid color-mix(in srgb, var(--ink) 30%, transparent); border-radius: 4px; padding: 0.5rem; resize: vertical; }
  .edit .row { display: flex; gap: 0.5rem; align-items: center; }
  .edit .row span { flex: 1; }
  .edit .later { margin: 0; }
  .find { display: flex; gap: 0.4rem; }
  .find input { flex: 1; min-width: 0; font-family: var(--font-book); font-size: 1rem; padding: 0.4rem 0.5rem; background: transparent; color: var(--ink); border: 1px solid color-mix(in srgb, var(--ink) 30%, transparent); border-radius: 4px; }
  .find button { font-family: var(--font-type); font-size: 0.8rem; background: var(--ink); color: var(--paper); border: 0; border-radius: 4px; padding: 0 0.8rem; }
  .linkish { all: unset; cursor: pointer; text-decoration: underline; }
  mark { background: rgb(226 185 59 / 0.45); color: inherit; border-radius: 2px; }
  .marks { list-style: none; margin: 0; padding: 0; display: grid; gap: 0.5rem; }
  .marks button { all: unset; box-sizing: border-box; display: grid; gap: 0.2rem; width: 100%; cursor: pointer; padding: 0.3rem 0 0.3rem 0.6rem; border-left: 4px solid var(--c); }
  .marks button:focus-visible { outline: 2px solid var(--ink); outline-offset: 2px; }
  .marks small { font-family: var(--font-type); font-size: 0.68rem; color: var(--faint); }
  .marks .q { font-size: 0.88rem; display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; }
  .marks .n { font-family: var(--font-type); font-size: 0.75rem; color: var(--faint); white-space: pre-wrap; }
  @keyframes slip { from { opacity: 0; transform: translateY(20px); } }
</style>
