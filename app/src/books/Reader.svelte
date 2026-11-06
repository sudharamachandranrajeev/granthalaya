<script lang="ts">
  import { onMount } from 'svelte'
  import ePub, { type Book as EpubBook, type Rendition, type NavItem } from 'epubjs'
  import { api, daypart, device, errText, type Change, type Question, type Settings, type Shelved } from '../lib/api'
  import { nav } from '../lib/nav.svelte'
  import { reducedMotion } from '../lib/motion'
  import { FONTS, fontFaces, palette } from './look'

  let spread = $state(false)
  let { id }: { id: string } = $props()

  const dev = device()
  const part = daypart()
  let host: HTMLDivElement
  let book: EpubBook | null = null
  let rendition: Rendition | null = null
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
  let toc: NavItem[] = []
  let locationsReady = false
  let startedAt = Date.now()
  let turns = 0
  let idle: ReturnType<typeof setTimeout> | undefined
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  const look = $derived(settings ? palette(settings) : null)

  function apply() {
    if (!rendition || !settings || !look) return
    rendition.themes.override('color', look.ink, true)
    rendition.themes.override('background', look.paper, true)
    rendition.themes.override('font-family', FONTS[settings.font] ?? FONTS.Literata, true)
    rendition.themes.override('line-height', String(settings.line_height), true)
    rendition.themes.fontSize(`${settings.font_size}px`)
  }

  function wake() {
    chrome = true
    clearTimeout(idle)
    idle = setTimeout(() => { if (!panel && !asking) chrome = false }, 3500)
  }

  function turn(d: 1 | -1) {
    if (!rendition) return
    if (!reducedMotion()) sweep = d * (sweep && Math.sign(sweep) === d ? Math.abs(sweep) + 1 : 1)
    d > 0 ? rendition.next() : rendition.prev()
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
    requestAnimationFrame(() => rendition?.resize(host.clientWidth, host.clientHeight))
    const v = await api.comfortGet(dev, part)
    question = v.question
    history = v.history
    asking = false
  }

  async function setManual(patch: Partial<Settings>) {
    if (!settings) return
    settings = { ...settings, ...patch }
    apply()
    if (patch.margin !== undefined) requestAnimationFrame(() => rendition?.resize(host.clientWidth, host.clientHeight))
    await api.comfortSet(dev, part, $state.snapshot(settings))
  }

  function relocated(loc: { start: { cfi: string; href: string; percentage: number; displayed: { page: number; total: number } } }) {
    const cfi = loc.start.cfi
    percent = locationsReady && book ? book.locations.percentageFromCfi(cfi) : loc.start.percentage
    pageInfo = loc.start.displayed.total ? `${loc.start.displayed.page} / ${loc.start.displayed.total} in chapter` : ''
    const item = toc.find((t) => loc.start.href.endsWith(t.href.split('#')[0]))
    if (item) chapter = item.label.trim()
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => api.bookProgress(id, percent, cfi).catch(() => {}), 600)
  }

  onMount(() => {
    let disposed = false
    ;(async () => {
      try {
        const [shelf, view, bytes] = await Promise.all([api.library(), api.comfortGet(dev, part), api.bookBytes(id)])
        if (disposed) return
        meta = shelf.find((s) => s.book.id === id) ?? null
        settings = view.settings
        question = view.question
        history = view.history
        book = ePub(bytes)
        rendition = book.renderTo(host, {
          width: '100%',
          height: '100%',
          flow: 'paginated',
          spread: dev === 'phone' ? 'none' : 'auto',
          minSpreadWidth: 900,
          allowScriptedContent: false,
        } as Parameters<EpubBook['renderTo']>[1])
        rendition.hooks.content.register((contents: any) => {
          // epub.js sizes a chapter from the bounding box of its text. Books that hide
          // text off-screen (a far negative left, as some publishers do for labels)
          // stretch that box to dozens of empty pages. Measure from the page's left edge.
          const measure = contents.textWidth.bind(contents)
          contents.textWidth = () => {
            const doc: Document = contents.document
            const range = doc.createRange()
            range.selectNodeContents(contents.content ?? doc.body)
            const r = range.getBoundingClientRect()
            const body = doc.body.getBoundingClientRect()
            const left = body.left + parseFloat(getComputedStyle(doc.body).paddingLeft || '0')
            return r.left >= left ? measure() : Math.round(r.right - left)
          }
          contents.addStylesheetCss(
            `${fontFaces}
             p, li { text-align: justify; hyphens: auto; -webkit-hyphens: auto; widows: 2; orphans: 2; }
             img, svg { max-width: 100%; mix-blend-mode: multiply; filter: grayscale(0.3); }
             a { color: inherit; }
             ::selection { background: rgb(196 154 69 / 0.35); }`,
            'granthalaya',
          )
          // Swipe inside the page.
          let sx = 0, sy = 0
          contents.document.addEventListener('touchstart', (e: TouchEvent) => { sx = e.touches[0].clientX; sy = e.touches[0].clientY }, { passive: true })
          contents.document.addEventListener('touchend', (e: TouchEvent) => {
            const dx = e.changedTouches[0].clientX - sx, dy = e.changedTouches[0].clientY - sy
            if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy) * 1.4) turn(dx < 0 ? 1 : -1)
            else if (Math.abs(dx) < 8 && Math.abs(dy) < 8) wake()
          })
          contents.document.addEventListener('keydown', key)
        })
        apply()
        // Two-page spreads get a spine shadow; epub.js decides from the page width.
        rendition.on('layout', (props: { spread: boolean }) => (spread = props.spread))
        rendition.on('relocated', relocated)
        await rendition.display(meta?.location ?? undefined)
        spread = ((rendition as any).manager?.layout?.divisor ?? 1) > 1
        const nav = await book.loaded.navigation
        toc = flatten(nav.toc)
        book.ready.then(() => book!.locations.generate(1600)).then(() => {
          locationsReady = true
          const cur = rendition?.currentLocation() as any
          if (cur?.start) relocated(cur)
        })
        wake()
      } catch (e) {
        error = errText(e)
      }
    })()
    window.addEventListener('keydown', key)
    return () => {
      disposed = true
      window.removeEventListener('keydown', key)
      clearTimeout(idle)
      rendition?.destroy()
      book?.destroy()
    }
  })

  function flatten(items: NavItem[]): NavItem[] {
    return items.flatMap((i) => [i, ...flatten(i.subitems ?? [])])
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowRight' || e.key === 'PageDown' || e.key === ' ') { e.preventDefault(); turn(1) }
    else if (e.key === 'ArrowLeft' || e.key === 'PageUp') turn(-1)
    else if (e.key === 'Escape') { if (panel) panel = false; else nav.back() }
  }

  const margin = $derived(settings ? `${(settings.margin * 100).toFixed(1)}%` : '8%')
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="reader"
  class:dark={look?.dark}
  style="--paper:{look?.paper ?? 'var(--paper)'}; --ink:{look?.ink ?? 'var(--ink)'}; --faint:{look?.faint ?? 'var(--ink-faint)'}; --m:{margin}"
  onpointermove={wake}
>
  <header class="top" class:hide={!chrome}>
    <button onclick={() => nav.back()} aria-label="Back to the library">← Library</button>
    <div class="t"><strong>{meta?.book.title ?? ''}</strong>{#if chapter}<span> · {chapter}</span>{/if}</div>
    <button onclick={() => { panel = !panel; wake() }} aria-expanded={panel} aria-label="Reading comfort">Aa</button>
  </header>

  <div class="pagewrap">
    <button class="zone left" onclick={() => turn(-1)} aria-label="Previous page"></button>
    <div class="page" bind:this={host} style="view-transition-name: book-page"></div>
    <button class="zone right" onclick={() => turn(1)} aria-label="Next page"></button>
    {#key sweep}{#if sweep}<div class="sweep" class:back={sweep < 0} aria-hidden="true"></div>{/if}{/key}
    {#if spread}<div class="gutter" aria-hidden="true"></div>{/if}
  </div>

  <footer class="bottom" class:hide={!chrome}>
    <span>{pageInfo}</span>
    <div class="prog" aria-label="{Math.round(percent * 100)} percent read"><span style="width:{percent * 100}%"></span></div>
    <span>{Math.round(percent * 100)}%</span>
  </footer>

  {#if error}<p class="err" role="alert">This book could not be opened: {error}</p>{/if}

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
      <label>Text size <span>{settings.font_size.toFixed(0)}px</span>
        <input type="range" min="13" max="34" step="0.5" value={settings.font_size} oninput={(e) => setManual({ font_size: +(e.target as HTMLInputElement).value })} /></label>
      <label>Line spacing <span>{settings.line_height.toFixed(2)}</span>
        <input type="range" min="1.2" max="2.1" step="0.05" value={settings.line_height} oninput={(e) => setManual({ line_height: +(e.target as HTMLInputElement).value })} /></label>
      <label>Margins <span>{Math.round(settings.margin * 100)}%</span>
        <input type="range" min="0.02" max="0.18" step="0.01" value={settings.margin} oninput={(e) => setManual({ margin: +(e.target as HTMLInputElement).value })} /></label>
      <label>Page brightness
        <input type="range" min="0.2" max="0.95" step="0.01" value={settings.lightness} oninput={(e) => setManual({ lightness: +(e.target as HTMLInputElement).value })} /></label>
      <label>Warmth
        <input type="range" min="0" max="1" step="0.01" value={settings.warmth} oninput={(e) => setManual({ warmth: +(e.target as HTMLInputElement).value })} /></label>
      <label>Ink darkness
        <input type="range" min="0.3" max="0.95" step="0.01" value={settings.contrast} oninput={(e) => setManual({ contrast: +(e.target as HTMLInputElement).value })} /></label>
      <label>Typeface
        <select value={settings.font} onchange={(e) => setManual({ font: (e.target as HTMLSelectElement).value })}>
          {#each Object.keys(FONTS) as f}<option value={f}>{f}</option>{/each}
        </select></label>
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
  .top button { background: none; border: 1px solid color-mix(in srgb, var(--faint) 50%, transparent); color: var(--faint); border-radius: 6px; padding: 0.25em 0.7em; font-family: var(--font-book); }
  .t { flex: 1; text-align: center; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .t strong { font-family: var(--font-book); font-weight: 400; font-style: italic; color: var(--ink); opacity: 0.75; }
  .pagewrap { position: relative; overflow: hidden; padding: 0.5rem var(--m) 0.5rem; }
  .page { height: 100%; position: relative; z-index: 0; }
  .zone { position: absolute; top: 0; bottom: 0; width: max(var(--m), 44px); background: none; border: 0; z-index: 2; cursor: pointer; }
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
  @keyframes slip { from { opacity: 0; transform: translateY(20px); } }
</style>
