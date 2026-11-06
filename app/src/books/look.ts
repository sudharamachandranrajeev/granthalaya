// Turn comfort settings into page colours and fonts for the reader.
import type { Settings } from '../lib/api'
import literata400 from '@fontsource/literata/files/literata-latin-400-normal.woff2?url'
import literata400i from '@fontsource/literata/files/literata-latin-400-italic.woff2?url'
import literata700 from '@fontsource/literata/files/literata-latin-700-normal.woff2?url'
import oldStd400 from '@fontsource/old-standard-tt/files/old-standard-tt-latin-400-normal.woff2?url'
import oldStd400i from '@fontsource/old-standard-tt/files/old-standard-tt-latin-400-italic.woff2?url'
import oldStd700 from '@fontsource/old-standard-tt/files/old-standard-tt-latin-700-normal.woff2?url'
import fell400 from '@fontsource/im-fell-english/files/im-fell-english-latin-400-normal.woff2?url'
import fell400i from '@fontsource/im-fell-english/files/im-fell-english-latin-400-italic.woff2?url'

export const FONTS: Record<string, string> = {
  Literata: "'Literata', Georgia, serif",
  'Old Standard': "'Old Standard TT', Georgia, serif",
  'IM Fell': "'IM Fell English', Georgia, serif",
  'System serif': 'Georgia, "Times New Roman", serif',
  'System sans': 'system-ui, sans-serif',
}

const abs = (u: string) => new URL(u, location.href).href
const face = (family: string, url: string, weight: number, style = 'normal') =>
  `@font-face{font-family:'${family}';src:url('${abs(url)}') format('woff2');font-weight:${weight};font-style:${style};font-display:block}`

/** @font-face rules injected into each book chapter's frame. */
export const fontFaces = [
  face('Literata', literata400, 400), face('Literata', literata400i, 400, 'italic'), face('Literata', literata700, 700),
  face('Old Standard TT', oldStd400, 400), face('Old Standard TT', oldStd400i, 400, 'italic'), face('Old Standard TT', oldStd700, 700),
  face('IM Fell English', fell400, 400), face('IM Fell English', fell400i, 400, 'italic'),
].join('\n')

/**
 * E-ink-like palette: warm grey paper, never pure white; ink never pure black.
 * warmth shifts hue and saturation toward candle-light; lightness dims the page.
 */
export function palette(s: Settings) {
  const hue = 42 - s.warmth * 10
  const sat = 8 + s.warmth * 32
  const paperL = 30 + s.lightness * 60 // 42%..87%
  const inkL = paperL > 55 ? 34 - s.contrast * 28 : 70 + s.contrast * 22
  const paper = `hsl(${hue} ${sat}% ${paperL}%)`
  const ink = `hsl(${hue - 10} ${10 + s.warmth * 10}% ${inkL}%)`
  const faint = `hsl(${hue - 10} ${10}% ${paperL > 55 ? inkL + 30 : inkL - 30}%)`
  return { paper, ink, faint, dark: paperL <= 55 }
}
