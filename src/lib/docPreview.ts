export const DOC_PREVIEW_SIZE_KEY = 'ad-doc-preview-size'
export const DOC_PREVIEW_MIN_W = 720
export const DOC_PREVIEW_MIN_H = 420
export const DOC_PREVIEW_DEFAULT_W = 1280
export const DOC_PREVIEW_DEFAULT_H = 800

export type DocPreviewSize = { w: number; h: number }
export type DocPreviewEdge = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw'

export const DOC_PREVIEW_EDGES: DocPreviewEdge[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw']

export function clampDocPreviewSize(
  w: number,
  h: number,
  viewW = 1280,
  viewH = 800
): DocPreviewSize {
  const maxW = Math.max(320, Math.round(viewW) - 48)
  const maxH = Math.max(240, Math.round(viewH) - 48)
  const minW = Math.min(DOC_PREVIEW_MIN_W, maxW)
  const minH = Math.min(DOC_PREVIEW_MIN_H, maxH)
  const width = Number.isFinite(w) ? Math.round(w) : DOC_PREVIEW_DEFAULT_W
  const height = Number.isFinite(h) ? Math.round(h) : DOC_PREVIEW_DEFAULT_H
  return {
    w: Math.min(maxW, Math.max(minW, width)),
    h: Math.min(maxH, Math.max(minH, height))
  }
}

export function parseDocPreviewSize(
  raw: string | null,
  viewW?: number,
  viewH?: number
): DocPreviewSize {
  if (!raw) {
    return clampDocPreviewSize(DOC_PREVIEW_DEFAULT_W, DOC_PREVIEW_DEFAULT_H, viewW, viewH)
  }
  try {
    const parsed = JSON.parse(raw) as { w?: unknown; h?: unknown }
    return clampDocPreviewSize(Number(parsed.w), Number(parsed.h), viewW, viewH)
  } catch {
    return clampDocPreviewSize(DOC_PREVIEW_DEFAULT_W, DOC_PREVIEW_DEFAULT_H, viewW, viewH)
  }
}

export function nextDocPreviewSize(
  start: DocPreviewSize,
  edge: DocPreviewEdge,
  dx: number,
  dy: number,
  viewW?: number,
  viewH?: number
) {
  let w = start.w
  let h = start.h
  if (edge.includes('e')) w = start.w + dx
  if (edge.includes('w')) w = start.w - dx
  if (edge.includes('s')) h = start.h + dy
  if (edge.includes('n')) h = start.h - dy
  return clampDocPreviewSize(w, h, viewW, viewH)
}

export function loadDocPreviewSize(viewW?: number, viewH?: number): DocPreviewSize {
  if (typeof localStorage === 'undefined') {
    return clampDocPreviewSize(DOC_PREVIEW_DEFAULT_W, DOC_PREVIEW_DEFAULT_H, viewW, viewH)
  }
  return parseDocPreviewSize(localStorage.getItem(DOC_PREVIEW_SIZE_KEY), viewW, viewH)
}

export function saveDocPreviewSize(size: DocPreviewSize) {
  if (typeof localStorage === 'undefined') return
  localStorage.setItem(DOC_PREVIEW_SIZE_KEY, JSON.stringify({ w: size.w, h: size.h }))
}
