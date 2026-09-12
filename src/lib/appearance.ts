export type UiTheme = 'system' | 'light' | 'dark'

export type Appearance = {
  theme: UiTheme
  uiFontSize: number
  accent: string
  background: string
  foreground: string
  uiFontFamily: string
  contentFontFamily: string
  codeFontFamily: string
  contrast: number
  translucentSidebar: boolean
}

type Rgb = { r: number; g: number; b: number }

export const UI_FONT_MIN = 11
export const UI_FONT_MAX = 18
export const UI_FONT_DEFAULT = 13
export const UI_CONTRAST_DEFAULT = 60

export const UI_THEMES: { id: UiTheme; label: string }[] = [
  { id: 'system', label: '系统' },
  { id: 'light', label: '浅色' },
  { id: 'dark', label: '深色' }
]

export const UI_FONTS: { id: string; label: string }[] = [
  { id: '', label: '系统默认' },
  { id: 'Segoe UI', label: 'Segoe UI' },
  { id: 'Microsoft YaHei UI', label: '微软雅黑 UI' },
  { id: 'Microsoft YaHei', label: '微软雅黑' },
  { id: 'Inter', label: 'Inter' },
  { id: 'PingFang SC', label: '苹方' }
]

export const CONTENT_FONTS: { id: string; label: string }[] = [
  { id: '', label: '与界面字体相同' },
  ...UI_FONTS.filter((item) => item.id)
]

export const CODE_FONTS: { id: string; label: string }[] = [
  { id: '', label: '系统默认' },
  { id: 'Cascadia Mono', label: 'Cascadia Mono' },
  { id: 'Cascadia Code', label: 'Cascadia Code' },
  { id: 'Consolas', label: 'Consolas' },
  { id: 'JetBrains Mono', label: 'JetBrains Mono' },
  { id: 'Fira Code', label: 'Fira Code' },
  { id: 'SF Mono', label: 'SF Mono' }
]

const UI_STACK =
  '"Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI", "PingFang SC", "Hiragino Sans GB", sans-serif'
const MONO_STACK = '"Cascadia Mono", "Cascadia Code", "SF Mono", Menlo, Monaco, Consolas, monospace'

export function parseTheme(value: unknown): UiTheme {
  if (value === 'light' || value === 'dark' || value === 'system') return value
  return 'system'
}

export function clampUiFontSize(value: number) {
  const n = Number(value)
  if (!Number.isFinite(n)) return UI_FONT_DEFAULT
  return Math.min(UI_FONT_MAX, Math.max(UI_FONT_MIN, Math.round(n)))
}

export function clampContrast(value: number) {
  const n = Number(value)
  if (!Number.isFinite(n)) return UI_CONTRAST_DEFAULT
  return Math.min(100, Math.max(0, Math.round(n)))
}

export function parseHex(value: unknown): string {
  if (typeof value !== 'string') return ''
  const raw = value.trim().replace('#', '')
  if (/^[0-9a-fA-F]{3}$/.test(raw)) {
    return `#${raw[0]}${raw[0]}${raw[1]}${raw[1]}${raw[2]}${raw[2]}`.toUpperCase()
  }
  if (/^[0-9a-fA-F]{6}$/.test(raw)) return `#${raw.toUpperCase()}`
  return ''
}

export function parseFamily(value: unknown, allowed: { id: string }[]) {
  if (typeof value !== 'string') return ''
  return allowed.some((item) => item.id === value) ? value : ''
}

export function resolvedTheme(theme: UiTheme, preferLight = false): 'light' | 'dark' {
  if (theme === 'light' || theme === 'dark') return theme
  return preferLight ? 'light' : 'dark'
}

export function systemPrefersLight() {
  return Boolean(window.matchMedia?.('(prefers-color-scheme: light)')?.matches)
}

export function themeDefaults(resolved: 'light' | 'dark') {
  if (resolved === 'light') {
    return { background: '#F3F3F3', foreground: '#171717', accent: '#171717' }
  }
  return { background: '#0B0F13', foreground: '#ECECEC', accent: '#ECECEC' }
}

export function defaultAppearance(): Appearance {
  return {
    theme: 'system',
    uiFontSize: UI_FONT_DEFAULT,
    accent: '',
    background: '',
    foreground: '',
    uiFontFamily: '',
    contentFontFamily: '',
    codeFontFamily: '',
    contrast: UI_CONTRAST_DEFAULT,
    translucentSidebar: true
  }
}

export function appearanceFromSettings(settings: {
  uiTheme?: unknown
  uiFontSize?: number
  uiAccent?: string
  uiBackground?: string
  uiForeground?: string
  uiFontFamily?: string
  contentFontFamily?: string
  codeFontFamily?: string
  uiContrast?: number
  translucentSidebar?: boolean
}): Appearance {
  return parseAppearance({
    theme: settings.uiTheme,
    uiFontSize: settings.uiFontSize,
    accent: settings.uiAccent,
    background: settings.uiBackground,
    foreground: settings.uiForeground,
    uiFontFamily: settings.uiFontFamily,
    contentFontFamily: settings.contentFontFamily,
    codeFontFamily: settings.codeFontFamily,
    contrast: settings.uiContrast,
    translucentSidebar: settings.translucentSidebar
  })
}

export function appearanceToSettings(appearance: Appearance) {
  return {
    uiTheme: appearance.theme,
    uiFontSize: appearance.uiFontSize,
    uiAccent: appearance.accent,
    uiBackground: appearance.background,
    uiForeground: appearance.foreground,
    uiFontFamily: appearance.uiFontFamily,
    contentFontFamily: appearance.contentFontFamily,
    codeFontFamily: appearance.codeFontFamily,
    uiContrast: appearance.contrast,
    translucentSidebar: appearance.translucentSidebar
  }
}

export function parseAppearance(input: Partial<Omit<Appearance, 'theme'>> & { theme?: unknown }): Appearance {
  const base = defaultAppearance()
  return {
    theme: parseTheme(input.theme),
    uiFontSize: clampUiFontSize(input.uiFontSize ?? base.uiFontSize),
    accent: parseHex(input.accent),
    background: parseHex(input.background),
    foreground: parseHex(input.foreground),
    uiFontFamily: parseFamily(input.uiFontFamily, UI_FONTS),
    contentFontFamily: parseFamily(input.contentFontFamily, CONTENT_FONTS),
    codeFontFamily: parseFamily(input.codeFontFamily, CODE_FONTS),
    contrast: clampContrast(input.contrast ?? base.contrast),
    translucentSidebar: input.translucentSidebar !== false
  }
}

export function fontStack(name: string, fallback: string) {
  if (!name) return fallback
  return `"${name}", ${fallback}`
}

export function codeFontStack(name: string) {
  return fontStack(name, MONO_STACK)
}

function toRgb(hex: string): Rgb {
  return {
    r: Number.parseInt(hex.slice(1, 3), 16),
    g: Number.parseInt(hex.slice(3, 5), 16),
    b: Number.parseInt(hex.slice(5, 7), 16)
  }
}

function toHex(rgb: Rgb) {
  const part = (n: number) => Math.max(0, Math.min(255, n)).toString(16).padStart(2, '0')
  return `#${part(rgb.r)}${part(rgb.g)}${part(rgb.b)}`.toUpperCase()
}

export function mixHex(from: string, to: string, amount: number) {
  const a = toRgb(from)
  const b = toRgb(to)
  const t = Math.max(0, Math.min(1, amount))
  return toHex({
    r: Math.round(a.r * t + b.r * (1 - t)),
    g: Math.round(a.g * t + b.g * (1 - t)),
    b: Math.round(a.b * t + b.b * (1 - t))
  })
}

function rgbChannels(hex: string) {
  const rgb = toRgb(hex)
  return `${rgb.r} ${rgb.g} ${rgb.b}`
}

export function glassFill(hex: string, alpha = 'var(--ad-veil)') {
  return `rgb(${rgbChannels(hex)} / ${alpha})`
}

function setVar(name: string, value: string | null) {
  const root = document.documentElement
  if (!value) root.style.removeProperty(name)
  else root.style.setProperty(name, value)
}

export function applyAppearance(input: Appearance) {
  const appearance = parseAppearance(input)
  const resolved = resolvedTheme(
    appearance.theme,
    typeof window !== 'undefined' ? systemPrefersLight() : false
  )
  const defaults = themeDefaults(resolved)
  const background = appearance.background || defaults.background
  const foreground = appearance.foreground || defaults.foreground
  const accent = appearance.accent || defaults.accent
  const contrast = appearance.contrast / 100
  const muted = mixHex(foreground, background, 0.28 + contrast * 0.32)
  const faint = mixHex(foreground, background, 0.14 + contrast * 0.22)
  const harbor = mixHex(foreground, background, 0.06)
  const raised = mixHex(foreground, background, 0.09)
  const hover = mixHex(foreground, background, 0.12)
  const selected = mixHex(foreground, background, 0.18)
  const ui = fontStack(appearance.uiFontFamily, UI_STACK)
  const content = appearance.contentFontFamily ? fontStack(appearance.contentFontFamily, UI_STACK) : ui
  const root = document.documentElement
  root.dataset.theme = resolved
  root.style.colorScheme = resolved
  setVar('--ad-ui-font-size', String(appearance.uiFontSize))
  setVar('--ad-ink', background)
  setVar('--ad-bg', background)
  setVar('--ad-editor', glassFill(background))
  setVar('--ad-harbor', harbor)
  setVar('--ad-raised', raised)
  setVar('--ad-hover', hover)
  setVar('--ad-selected', selected)
  setVar('--ad-text', foreground)
  setVar('--ad-muted', muted)
  setVar('--ad-faint', faint)
  setVar('--ad-accent', accent)
  setVar('--ad-accent-hover', mixHex(accent, foreground, 0.65))
  setVar('--ad-accent-active', mixHex(accent, background, 0.82))
  setVar('--ad-filament', accent)
  setVar('--ad-primary', resolved === 'light' ? foreground : mixHex(foreground, '#FFFFFF', 0.92))
  setVar('--ad-primary-text', resolved === 'light' ? background : '#111111')
  setVar('--ad-range-accent', accent)
  setVar(
    '--ad-sidebar',
    appearance.translucentSidebar ? `rgb(${rgbChannels(background)} / var(--ad-veil))` : background
  )
  setVar('--ad-sans', ui)
  setVar('--ad-display', ui)
  setVar('--ad-content', content)
  setVar('--ad-mono', codeFontStack(appearance.codeFontFamily))
  setVar('--ad-border', `rgba(${rgbChannels(foreground)} / ${0.06 + contrast * 0.06})`)
  setVar('--ad-border-strong', `rgba(${rgbChannels(foreground)} / ${0.12 + contrast * 0.08})`)
  if (typeof window !== 'undefined') window.dispatchEvent(new Event('ad-appearance'))
}

export function watchSystemTheme(getAppearance: () => Appearance) {
  const media = window.matchMedia?.('(prefers-color-scheme: light)')
  if (!media) return () => {}
  const onChange = () => {
    if (getAppearance().theme === 'system') applyAppearance(getAppearance())
  }
  media.addEventListener('change', onChange)
  return () => media.removeEventListener('change', onChange)
}
