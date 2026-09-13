import { glassRgba } from './uiGlass.ts'

export type Rgb = { r: number; g: number; b: number }

const SURFACE_CHROMA = 36
const DARK_LUMA_MAX = 82
const LIGHT_LUMA_MIN = 168

export function parseCssRgb(css: string): Rgb | null {
  const value = css.trim()
  if (!value) return null
  if (value[0] === '#') {
    const raw = value.slice(1)
    if (raw.length === 3) {
      return {
        r: Number.parseInt(raw[0] + raw[0], 16),
        g: Number.parseInt(raw[1] + raw[1], 16),
        b: Number.parseInt(raw[2] + raw[2], 16)
      }
    }
    if (raw.length === 6 || raw.length === 8) {
      return {
        r: Number.parseInt(raw.slice(0, 2), 16),
        g: Number.parseInt(raw.slice(2, 4), 16),
        b: Number.parseInt(raw.slice(4, 6), 16)
      }
    }
    return null
  }
  const rgb = value.match(/^rgba?\(\s*([0-9.]+)\s*[, ]\s*([0-9.]+)\s*[, ]\s*([0-9.]+)/i)
  if (!rgb) return null
  return {
    r: Number(rgb[1]),
    g: Number(rgb[2]),
    b: Number(rgb[3])
  }
}

export function chroma(rgb: Rgb) {
  return Math.max(rgb.r, rgb.g, rgb.b) - Math.min(rgb.r, rgb.g, rgb.b)
}

export function luma(rgb: Rgb) {
  return 0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b
}

export function isSurfaceFill(rgb: Rgb, dark: boolean) {
  if (chroma(rgb) > SURFACE_CHROMA) return false
  const L = luma(rgb)
  return dark ? L < DARK_LUMA_MAX : L > LIGHT_LUMA_MIN
}

export function paletteIndex(className: string | undefined) {
  if (!className) return null
  const match = className.match(/(?:^|\s)xterm-bg-(\d+)(?:\s|$)/)
  if (!match) return null
  const index = Number(match[1])
  return Number.isFinite(index) ? index : null
}

export function shouldClearFill(
  opts: { backgroundColor?: string; className?: string },
  dark: boolean
) {
  if (opts.backgroundColor) {
    const rgb = parseCssRgb(opts.backgroundColor)
    return Boolean(rgb && isSurfaceFill(rgb, dark))
  }
  const index = paletteIndex(opts.className)
  if (index == null) return false
  if (index === 0) return true
  if (dark) return index >= 232 && index <= 242
  return index === 7 || index === 15 || index >= 252
}

export function termThemeBackground(ink: string, opacityPercent: number) {
  return glassRgba(ink, opacityPercent)
}

export function glassifySpan(
  el: { style: { backgroundColor: string }; className: string },
  dark: boolean
) {
  if (shouldClearFill({ backgroundColor: el.style.backgroundColor, className: el.className }, dark)) {
    el.style.backgroundColor = 'transparent'
  }
}

export function glassifyRows(rowContainer: Element, start: number, end: number, dark: boolean) {
  const last = Math.min(end, rowContainer.children.length - 1)
  for (let i = Math.max(0, start); i <= last; i++) {
    const row = rowContainer.children[i]
    if (!row) continue
    for (let c = 0; c < row.children.length; c++) {
      glassifySpan(row.children[c] as HTMLElement, dark)
    }
  }
}

export function isOscQuery(data: string) {
  return data.trim().startsWith('?')
}

export function oscRgbFromHex(hex: string) {
  const raw = hex.trim().replace('#', '')
  if (raw.length !== 6) return 'rgb:0d0d/0d0d/0d0d'
  const r = raw.slice(0, 2)
  const g = raw.slice(2, 4)
  const b = raw.slice(4, 6)
  return `rgb:${r}${r}/${g}${g}/${b}${b}`
}

export function osc11Reply(hex: string) {
  return `\x1b]11;${oscRgbFromHex(hex)}\x1b\\`
}

export type OscTerm = {
  parser: {
    registerOscHandler: (
      ident: number,
      callback: (data: string) => boolean
    ) => { dispose: () => void }
  }
}

export function attachOscBackground(
  term: OscTerm,
  hooks: { send: (data: string) => void; ink: () => string }
) {
  return term.parser.registerOscHandler(11, (data) => {
    if (isOscQuery(data)) hooks.send(osc11Reply(hooks.ink()))
    return true
  })
}
