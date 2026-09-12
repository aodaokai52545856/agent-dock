import { parseHex, resolvedTheme, systemPrefersLight, themeDefaults } from './appearance.ts'
import { clampUiFrost, UI_FROST_DEFAULT, veilAlpha } from './uiGlass.ts'
import type { AppSettings } from './types.ts'

export type DshGlassTheme = {
  bg: string
  fg: string
  veil: number
  frost: number
  scheme: 'light' | 'dark'
}

type GlassSettings = Pick<AppSettings, 'uiTheme' | 'uiBackground' | 'uiForeground' | 'uiOpacity'> & {
  uiFrost?: number
}

function clampVeil(value: number) {
  if (!Number.isFinite(value)) return 0.9
  return Math.max(0.12, Math.min(0.9, value))
}

export function dshGlassTheme(settings: GlassSettings): DshGlassTheme {
  const scheme = resolvedTheme(
    settings.uiTheme,
    typeof window !== 'undefined' ? systemPrefersLight() : false
  )
  const defaults = themeDefaults(scheme)
  return {
    bg: parseHex(settings.uiBackground) || defaults.background,
    fg: parseHex(settings.uiForeground) || defaults.foreground,
    veil: Number(veilAlpha(settings.uiOpacity).toFixed(3)),
    frost: clampUiFrost(settings.uiFrost ?? UI_FROST_DEFAULT),
    scheme
  }
}

export function dshGlassHash(theme: DshGlassTheme) {
  const bg = theme.bg.replace('#', '')
  const fg = theme.fg.replace('#', '')
  const scheme = theme.scheme === 'light' ? 'l' : 'd'
  return `ad=1&bg=${bg}&fg=${fg}&v=${theme.veil.toFixed(3)}&s=${scheme}&f=${theme.frost}`
}

export function parseDshGlassHash(hash: string): DshGlassTheme | null {
  const raw = hash.replace(/^#/, '')
  if (!raw) return null
  const query = new URLSearchParams(raw)
  if (query.get('ad') !== '1') return null
  const bg = parseHex(query.get('bg') || '')
  if (!bg) return null
  const veil = Number(query.get('v'))
  const frost = Number(query.get('f'))
  return {
    bg,
    fg: parseHex(query.get('fg') || '') || '',
    veil: clampVeil(veil),
    frost: clampUiFrost(Number.isFinite(frost) ? frost : UI_FROST_DEFAULT),
    scheme: query.get('s') === 'l' ? 'light' : 'dark'
  }
}

export function withDshGlassHash(url: string, theme: DshGlassTheme) {
  try {
    const next = new URL(url)
    next.hash = dshGlassHash(theme)
    return next.toString()
  } catch {
    const base = url.split('#')[0] ?? url
    return `${base}#${dshGlassHash(theme)}`
  }
}

export function dshGlassMessage(theme: DshGlassTheme) {
  return { type: 'ad-dsh-glass' as const, theme }
}

export function dshGlassCss(theme: DshGlassTheme) {
  const raw = theme.bg.replace('#', '')
  const r = Number.parseInt(raw.slice(0, 2), 16)
  const g = Number.parseInt(raw.slice(2, 4), 16)
  const b = Number.parseInt(raw.slice(4, 6), 16)
  const rgb = [r, g, b].every((n) => Number.isFinite(n)) ? `${r}, ${g}, ${b}` : '11, 15, 19'
  const lift = Math.min(0.96, theme.veil + 0.08)
  const panel = Math.min(0.97, theme.veil + 0.14)
  const bubble = Math.min(0.98, theme.veil + 0.18)
  return {
    rgb,
    canvas: `rgba(${rgb}, ${theme.veil})`,
    lift: `rgba(${rgb}, ${lift})`,
    panel: `rgba(${rgb}, ${panel})`,
    bubble: `rgba(${rgb}, ${bubble})`,
    transparent: 'transparent'
  }
}

export function dshGlassVars(theme: DshGlassTheme) {
  const css = dshGlassCss(theme)
  const ink = theme.fg || (theme.scheme === 'light' ? '#171717' : '#ECECEC')
  return {
    '--dsw-alias-bg-base': css.transparent,
    '--dsw-alias-bg-layer-1': css.transparent,
    '--dsw-alias-bg-layer-2': css.lift,
    '--dsw-alias-bg-layer-3': css.panel,
    '--dsw-alias-bg-module-platform': css.transparent,
    '--dsw-alias-bg-overlay': css.panel,
    '--dsw-specific-sidebar-fill': css.transparent,
    '--dsw-specific-input-major': css.panel,
    '--dsw-specific-bubble': css.panel,
    '--dsw-specific-bubble-highlight': css.bubble,
    '--dsw-alias-label-primary': ink,
    '--dsh-boot-bg': css.transparent
  }
}

export function dshGlassFrostPx(theme: DshGlassTheme) {
  return Number((clampUiFrost(theme.frost) * 0.42).toFixed(1))
}

export function dshGlassCssText(theme: DshGlassTheme) {
  const css = dshGlassCss(theme)
  const vars = dshGlassVars(theme)
  const frost = dshGlassFrostPx(theme)
  const saturate = Number((1 + clampUiFrost(theme.frost) * 0.003).toFixed(3))
  const tokens = Object.entries(vars)
    .map(([name, value]) => `  ${name}: ${value} !important;`)
    .join('\n')
  return [
    'html, body, #app, #__next, [data-ds-root] {',
    `  background: ${css.transparent} !important;`,
    `  background-color: ${css.transparent} !important;`,
    '}',
    '#root, [data-ad-dsh-glass] {',
    `  background: ${css.canvas} !important;`,
    `  background-color: ${css.canvas} !important;`,
    `  backdrop-filter: blur(${frost}px) saturate(${saturate});`,
    `  -webkit-backdrop-filter: blur(${frost}px) saturate(${saturate});`,
    '}',
    ':root, html, body, body[data-ds-dark-theme] {',
    `  color-scheme: ${theme.scheme} !important;`,
    tokens,
    '}'
  ].join('\n')
}

export function dshGlassApplyScript(theme: DshGlassTheme) {
  const css = dshGlassCssText(theme)
  const vars = dshGlassVars(theme)
  const payload = { ...theme, css, vars }
  const stamps = Object.keys(vars)
    .map((name) => `el.style.setProperty('${name}',vars['${name}'],'important');`)
    .join('')
  return [
    '(function(){',
    `var t=${JSON.stringify(payload)};`,
    "var id='ad-dsh-glass';",
    'function stamp(el){',
    '  if(!el||!el.style)return;',
    '  var vars=t.vars||{};',
    `  ${stamps}`,
    "  el.style.setProperty('background','transparent','important');",
    "  el.style.setProperty('background-color','transparent','important');",
    '}',
    'function apply(){',
    '  try{sessionStorage.setItem(id,JSON.stringify({bg:t.bg,fg:t.fg,veil:t.veil,frost:t.frost,scheme:t.scheme}))}catch(e){}',
    '  var s=document.getElementById(id);',
    "  if(!s){s=document.createElement('style');s.id=id;(document.head||document.documentElement).appendChild(s)}",
    '  s.textContent=t.css;',
    '  var r=document.documentElement;',
    '  r.style.colorScheme=t.scheme;',
    "  r.setAttribute('data-ad-dsh-glass',t.scheme);",
    '  stamp(r);',
    '  if(document.body){',
    "    if(t.scheme==='dark')document.body.setAttribute('data-ds-dark-theme','');",
    "    else document.body.removeAttribute('data-ds-dark-theme');",
    '    stamp(document.body);',
    '  }',
    '}',
    'apply();',
    "if(!document.body)document.addEventListener('DOMContentLoaded',apply);",
    '})();'
  ].join('')
}
