export const DSH_EMBED_EDGE = 12
export const DSH_EMBED_GUTTER = 6
export const DSH_STATUS_BAR = 28
export const DSH_OVERLAY_SELECTOR = '.ad-mask, .ad-menu, .toast'

export function dshEmbedBlocked(root: { querySelector: (selector: string) => unknown }) {
  return Boolean(root.querySelector(DSH_OVERLAY_SELECTOR))
}

export type Box = {
  left: number
  top: number
  width: number
  height: number
}

export type DshEmbedBounds = {
  x: number
  y: number
  width: number
  height: number
}

export function clampDshEmbedBounds(
  box: Box,
  opts: {
    windowWidth: number
    windowHeight: number
    docRailWidth: number
    maximized: boolean
    statusBarHeight?: number
  }
): DshEmbedBounds | null {
  const edge = opts.maximized ? 0 : DSH_EMBED_EDGE
  const bar = Math.max(0, Math.round(opts.statusBarHeight ?? DSH_STATUS_BAR))
  const rail = Math.max(0, Math.round(opts.docRailWidth))
  let x = Math.round(box.left) + DSH_EMBED_GUTTER
  const y = Math.round(box.top)
  let width = Math.round(box.width) - DSH_EMBED_GUTTER - rail
  if (rail > 0) width -= DSH_EMBED_GUTTER
  let height = Math.round(box.height)
  const maxRight = Math.round(opts.windowWidth) - edge
  const maxBottom = Math.round(opts.windowHeight) - Math.max(edge, bar)
  if (x + width > maxRight) width = maxRight - x
  if (y + height > maxBottom) height = maxBottom - y
  if (width < 80 || height < 80) return null
  return { x, y, width, height }
}
