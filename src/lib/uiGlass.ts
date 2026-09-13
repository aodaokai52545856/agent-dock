export const UI_OPACITY_MIN = 0
export const UI_OPACITY_MAX = 100
export const UI_OPACITY_DEFAULT = 0

export const UI_FROST_MIN = 0
export const UI_FROST_MAX = 100
export const UI_FROST_DEFAULT = 100

function clampPercent(value: number, fallback: number) {
  const n = Number(value)
  if (!Number.isFinite(n)) return fallback
  return Math.min(100, Math.max(0, Math.round(n)))
}

export function clampUiOpacity(value: number) {
  return clampPercent(value, UI_OPACITY_DEFAULT)
}

export function clampUiFrost(value: number) {
  return clampPercent(value, UI_FROST_DEFAULT)
}

export function inkBoostPercent(opacityPercent: number) {
  return Math.min(95, Math.round(clampUiOpacity(opacityPercent) * 1.8))
}

export function applyUiGlass(opacity: number, frost: number) {
  if (typeof document === 'undefined') return
  const nextOpacity = clampUiOpacity(opacity)
  const nextFrost = clampUiFrost(frost)
  const root = document.documentElement
  root.style.setProperty('--ad-ui-opacity', String(nextOpacity))
  root.style.setProperty('--ad-ui-frost', String(nextFrost))
  root.style.setProperty('--ad-ink-boost', `${inkBoostPercent(nextOpacity)}%`)
  if (nextFrost <= 0) root.setAttribute('data-frost', 'off')
  else root.removeAttribute('data-frost')
}

export function applyUiOpacity(value: number, frost = UI_FROST_DEFAULT) {
  applyUiGlass(value, frost)
}

export function veilAlpha(opacityPercent: number) {
  return Math.max(0, Math.min(1, 0.9 - (clampUiOpacity(opacityPercent) / 100) * 0.78))
}

export function glassRgba(hex: string, opacityPercent: number) {
  const raw = hex.trim().replace('#', '')
  const alpha = veilAlpha(opacityPercent).toFixed(3)
  if (raw.length !== 6) return `rgba(11, 15, 19, ${alpha})`
  const r = Number.parseInt(raw.slice(0, 2), 16)
  const g = Number.parseInt(raw.slice(2, 4), 16)
  const b = Number.parseInt(raw.slice(4, 6), 16)
  if (![r, g, b].every((n) => Number.isFinite(n))) return `rgba(11, 15, 19, ${alpha})`
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}
