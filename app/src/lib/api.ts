// Typed wrappers over the Tauri commands in src-tauri/src/lib.rs.
import { invoke } from '@tauri-apps/api/core'

export type Link = { label: string; url: string | null }
export type OutletFacts = {
  id: string
  name: string
  country: string
  funding: string
  funding_text: string
  owners: Link[]
  funders: Link[]
  wikidata: string | null
}
export type Actor = { name: string; kind: string; mentions: number; quotes: number }
export type Component = { id: string; label: string; points: number; max: number; detail: string }
export type Report = {
  score: number
  verdict: string
  basis: string
  components: Component[]
  loaded: string[]
  unattributed: string[]
  actors: Actor[]
  influences: { who: string; how: string }[]
  corroboration: { outlets: string[]; independent: number; countries: number }
  opinion: boolean
  outlet: OutletFacts
}
export type Story = {
  id: string
  outlet_id: string
  outlet_name: string
  section: string
  title: string
  summary: string
  url: string
  published: string | null
  image: string | null
  topics: string[]
  paywall: boolean
}
export type Telling = { story_id: string; outlet_name: string; title: string; url: string }
export type EditionStory = {
  story: Story
  report: Report
  interest: number
  why: string[]
  others: Telling[]
  sensitive: boolean
}
export type Page = { title: string; stories: EditionStory[] }
export type Edition = {
  date: string
  built_at: string
  number: number
  pages: Page[]
  sources_ok: string[]
  sources_failed: string[]
  stories_read: number
}
export type Prefs = {
  blocked_outlets: string[]
  muted_topics: string[]
  followed_topics: string[]
  onboarded: boolean
}
export type TopicInfo = { id: string; label: string; sensitive: boolean }
export type Opened = { paragraphs: string[]; report: Report; note: string | null }
export type Feedback = Record<string, [number | null, number | null]>

export type Book = {
  id: string
  title: string
  authors: string[]
  subjects: string[]
  summary: string
  cover: string | null
  epub_url: string | null
  source: string
  popularity: number
}
export type Shelved = {
  book: Book
  progress: number
  location: string | null
  rating: number | null
  last_read: string | null
  format: 'epub' | 'pdf' | 'mobi' | 'fb2' | 'cbz' | 'fbz'
}
export type Taste = { genres: string[]; authors: string[]; length: string; done: boolean }
export type Settings = {
  font_size: number
  line_height: number
  margin: number
  warmth: number
  contrast: number
  lightness: number
  font: string
}
export type Question = { id: string; prompt: string; answers: [string, string, string] }
export type Change = { at: string; profile: string; question: string; answer: string; summary: string }
export type ComfortView = { settings: Settings; question: Question | null; history: Change[] }

export type Highlight = {
  id: string; book_id: string; book_title: string; book_author: string; cfi: string; text: string
  note: string; color: HighlightColor; chapter: string; progress: number; created_at: string; updated_at: string
}
export type HighlightColor = 'yellow' | 'green' | 'blue' | 'pink'
export const HIGHLIGHT_COLORS: HighlightColor[] = ['yellow', 'green', 'blue', 'pink']

export type Catalogue = { id: string; title: string; url: string; user: string }
export type Preset = { title: string; url: string; note: string; login: boolean }
export type NavLink = { title: string; url: string; summary: string; cover: string | null }
export type Pub = { id: string; title: string; authors: string[]; summary: string; cover: string | null; downloads: { url: string; format: string }[] }
export type CatalogueGroup = { title: string; links: NavLink[]; books: Pub[] }
export type CataloguePage = {
  title: string; url: string; links: NavLink[]; books: Pub[]; groups: CatalogueGroup[]
  next: string | null; prev: string | null; search: string | null
}

export type EditionInfo = { date: string; number: number; lead: string; stories: number }
/** `snippet` marks matches with \u0002 … \u0003. */
export type Hit = { story_id: string; date: string; title: string; outlet: string; snippet: string }

export const api = {
  editionToday: (force = false) => invoke<Edition>('edition_today', { force }),
  editionGet: (date: string) => invoke<Edition | null>('edition_get', { date }),
  editionDates: () => invoke<string[]>('edition_dates'),
  storyOpen: (id: string) => invoke<Opened>('story_open', { id }),
  storyFeedback: (id: string, vote: number | null, stars: number | null) => invoke<void>('story_feedback', { id, vote, stars }),
  feedbackAll: () => invoke<Feedback>('feedback_all'),
  prefsGet: () => invoke<Prefs>('prefs_get'),
  prefsSet: (prefs: Prefs) => invoke<void>('prefs_set', { prefs }),
  topics: () => invoke<TopicInfo[]>('topics_list'),

  tasteGet: () => invoke<Taste>('taste_get'),
  tasteSet: (taste: Taste) => invoke<void>('taste_set', { taste }),
  genres: () => invoke<string[]>('genres'),
  library: () => invoke<Shelved[]>('library'),
  suggest: () => invoke<Book[]>('books_suggest'),
  search: (query: string) => invoke<Book[]>('books_search', { query }),
  bookAdd: (book: Book) => invoke<void>('book_add', { book }),
  bookImport: (bytes: Uint8Array, name: string, title: string, author: string) =>
    invoke<string>('book_import', bytes, {
      headers: { 'x-name': encodeURIComponent(name), 'x-title': encodeURIComponent(title), 'x-author': encodeURIComponent(author) },
    }),
  bookBytes: (id: string) => invoke<ArrayBuffer>('book_bytes', { id }),
  bookProgress: (id: string, progress: number, location: string) => invoke<void>('book_progress', { id, progress, location }),
  bookRate: (id: string, rating: number | null) => invoke<void>('book_rate', { id, rating }),
  bookRemove: (id: string) => invoke<void>('book_remove', { id }),

  comfortGet: (device: string, daypart: string) => invoke<ComfortView>('comfort_get', { device, daypart }),
  comfortAnswer: (device: string, daypart: string, question: string, answer: number) =>
    invoke<Settings>('comfort_answer', { device, daypart, question, answer }),
  comfortSet: (device: string, daypart: string, settings: Settings) => invoke<void>('comfort_set', { device, daypart, settings }),
  highlightSave: (highlight: Partial<Highlight>) => invoke<Highlight>('highlight_save', { highlight }),
  highlights: (bookId: string | null) => invoke<Highlight[]>('highlights_list', { bookId }),
  highlightDelete: (id: string) => invoke<void>('highlight_delete', { id }),
  highlightsExport: (bookId: string | null) => invoke<boolean>('highlights_export', { bookId }),
  opdsPresets: () => invoke<Preset[]>('opds_presets'),
  opdsList: () => invoke<Catalogue[]>('opds_list'),
  opdsSave: (url: string, title: string, user: string, password: string | null) => invoke<Catalogue>('opds_save', { url, title, user, password }),
  opdsRemove: (id: string) => invoke<void>('opds_remove', { id }),
  opdsBrowse: (id: string, url: string | null, query: string | null = null, search: string | null = null) =>
    invoke<CataloguePage>('opds_browse', { id, url, query, search }),
  opdsGet: (id: string, book: Pub, format: string) => invoke<string>('opds_get', { id, book, format }),
  archiveList: () => invoke<EditionInfo[]>('archive_list'),
  archiveSearch: (query: string) => invoke<Hit[]>('archive_search', { query }),
}

export function device(): 'phone' | 'tablet' | 'desktop' {
  const w = Math.min(window.innerWidth, window.screen?.width || window.innerWidth)
  return w < 700 ? 'phone' : w < 1100 ? 'tablet' : 'desktop'
}

export function daypart(): 'day' | 'night' {
  const h = new Date().getHours()
  return h >= 7 && h < 19 ? 'day' : 'night'
}

export function errText(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
}
