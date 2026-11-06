// Screenshot harness: fakes the Rust side with demo.json (from core/examples/demo.rs).
// Synthetic data only. Hash options:
//   view = home | paper | story | archive | library | catalogues | browse | reader | notes
//   q    = archive search text (view=archive)
//   marks = 0: open the reader without making highlights
import { mockIPC } from '@tauri-apps/api/mocks'
import demo from './demo.json'

const p = new URLSearchParams(location.hash.slice(1))
const view = p.get('view') ?? 'home'
const marks: any[] = []
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))

mockIPC((cmd, args: any) => {
  switch (cmd) {
    case 'edition_today': return demo.edition
    case 'edition_get': return args.date === demo.edition.date ? demo.edition : null
    case 'edition_dates': return demo.dates
    case 'story_open': return demo.opened
    case 'feedback_all': return {}
    case 'prefs_get': return demo.prefs
    case 'topics_list': return demo.topics
    case 'taste_get': return { genres: ['Adventure', 'Science fiction'], authors: [], length: 'any', done: true }
    case 'genres': return ['Adventure', 'Science fiction', 'Romance', 'Mystery']
    case 'library': return demo.library
    case 'books_suggest': return demo.catalogue.books.map((b) => ({ id: b.id, title: b.title, authors: b.authors, subjects: [], summary: '', cover: null, epub_url: b.downloads[0].url, source: 'Commons Library', popularity: 0 }))
    case 'book_bytes': return new TextEncoder().encode(demo.book.fb2).buffer
    case 'comfort_get': return demo.comfort
    case 'highlights_list': return marks.filter((m) => !args.bookId || m.book_id === args.bookId)
    case 'highlight_save': {
      const h = { ...args.highlight, id: `h${marks.length}`, created_at: '2026-11-05T07:20:00Z', updated_at: '2026-11-05T07:20:00Z' }
      marks.push(h)
      return h
    }
    case 'opds_presets': return []
    case 'opds_list': return demo.catalogues
    case 'opds_browse': return demo.catalogue
    case 'archive_list': return demo.editions
    case 'archive_search': return demo.hits
    default: return null
  }
})

await import('../src/main.ts')
const { nav } = await import('../src/lib/nav.svelte.ts')
await wait(300)
const lead = demo.edition.pages[0].stories[0].story.id
const go: Record<string, () => Promise<void> | void> = {
  home: () => {},
  paper: () => nav.go({ name: 'paper' }),
  story: () => nav.go({ name: 'paper', date: demo.edition.date, story: lead }),
  library: () => nav.go({ name: 'library' }),
  catalogues: () => nav.go({ name: 'catalogues' }),
  browse: async () => {
    nav.go({ name: 'catalogues' })
    await wait(300)
    ;[...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.includes('Commons Library'))?.click()
  },
  archive: async () => {
    nav.go({ name: 'archive' })
    await wait(300)
    const input = document.querySelector<HTMLInputElement>('.search input')
    if (input && p.get('q')) {
      input.value = p.get('q')!
      input.dispatchEvent(new Event('input', { bubbles: true }))
    }
  },
  reader: () => reader(false),
  notes: () => reader(true),
}
await go[view]?.()

/** Open the demo book, make two real highlights through the reader's own toolbar. */
async function reader(openNotes: boolean) {
  nav.go({ name: 'reader', id: demo.book.id })
  await wait(1500)
  const view = document.querySelector('foliate-view') as any
  await view.goTo(1) // past the title section, to chapter I
  await wait(1200)
  const { doc } = view.renderer.getContents()[0] as { doc: Document }
  const frame = doc.defaultView!.frameElement!.getBoundingClientRect()
  const visible = [...doc.querySelectorAll('p')].filter((x) => {
    const r = x.getBoundingClientRect()
    return (x.textContent?.length ?? 0) > 120 && frame.left + r.left >= 0 && frame.left + r.right <= innerWidth && frame.top + r.top > 60
  })
  const pick = async (para: Element | undefined, from: number, to: number, color: string | null) => {
    const t = para?.firstChild
    if (!t) return
    const r = doc.createRange()
    r.setStart(t, from)
    r.setEnd(t, Math.min(to, t.textContent!.length))
    doc.getSelection()!.removeAllRanges()
    doc.getSelection()!.addRange(r)
    await wait(500)
    if (color) {
      document.querySelector<HTMLButtonElement>(`.selbar button[aria-label="Highlight in ${color}"]`)?.click()
      await wait(400)
    }
  }
  if (p.get('marks') === '0') return
  await pick(visible[1], 0, 120, 'yellow')
  await pick(visible[3] ?? visible[2], 0, 90, 'green')
  if (openNotes) {
    doc.getSelection()!.removeAllRanges()
    ;[...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.startsWith('Notes'))?.click()
  } else {
    await pick(visible[2], 0, 70, null)
  }
}
