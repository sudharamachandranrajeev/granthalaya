// Thin typed door to the vendored foliate-js (public/foliate-js, see VENDORED.md).
// It is served as plain files, not bundled, so it is loaded by URL at run time.

export type LangMap = string | Record<string, string> | undefined
type Contributor = string | { name?: LangMap } | (string | { name?: LangMap })[] | undefined

export type TocItem = { label: string; href: string; subitems?: TocItem[] }

export type FoliateBook = {
  metadata?: { title?: LangMap; author?: Contributor; language?: string | string[] }
  toc?: TocItem[]
  dir?: string
  sections: unknown[]
  rendition?: { layout?: string }
  getCover?: () => Promise<Blob | null> | Blob | null
}

/** Where the reader is: from foliate-view's `relocate` event. */
export type Relocate = {
  fraction: number
  cfi: string
  range: Range
  tocItem?: { label?: string; href?: string }
  pageItem?: { label?: string }
  location?: { current: number; next: number; total: number }
  section?: { current: number; total: number }
}

const base = () => new URL('/foliate-js/', location.href).href

let views: Promise<{ makeBook: (f: File) => Promise<FoliateBook> }> | null = null
/** Loads view.js once; it also registers the <foliate-view> element. */
export function foliate() {
  return (views ??= import(/* @vite-ignore */ `${base()}view.js`))
}

let overlayer: Promise<{ Overlayer: Record<string, unknown> }> | null = null
export function overlayerModule() {
  return (overlayer ??= import(/* @vite-ignore */ `${base()}overlayer.js`))
}

/** foliate-js tells ZIP kinds apart by file name, so keep the extension. */
export function fileOf(bytes: ArrayBuffer, name: string, format: string) {
  return new File([bytes], `${name.replace(/[^\w-]+/g, '_') || 'book'}.${format}`)
}

export function text(x: LangMap): string {
  if (!x) return ''
  if (typeof x === 'string') return x
  return x.en ?? x[Object.keys(x)[0]] ?? ''
}

export function authors(a: Contributor): string[] {
  const list = Array.isArray(a) ? a : a ? [a] : []
  return list.map((c) => (typeof c === 'string' ? c : text(c.name))).filter(Boolean)
}

/** Title and first author from any supported file; falls back to the file name. */
export async function readMeta(file: File): Promise<{ title: string; author: string }> {
  const fallback = file.name.replace(/\.[^.]+$/, '').replace(/[_-]+/g, ' ')
  try {
    const { makeBook } = await foliate()
    const book = await makeBook(file)
    return { title: text(book.metadata?.title).trim() || fallback, author: authors(book.metadata?.author)[0] ?? '' }
  } catch {
    return { title: fallback, author: '' }
  }
}

export const ACCEPT = '.epub,.pdf,.mobi,.azw3,.azw,.fb2,.fbz,.cbz,application/epub+zip,application/pdf'
