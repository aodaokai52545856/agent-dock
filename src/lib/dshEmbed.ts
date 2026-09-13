export const DSH_EMBED_EDGE = 12
export const DSH_EMBED_GUTTER = 6
export const DSH_STATUS_BAR = 28
export const DSH_MASK_SELECTOR = '.ad-mask'
export const DSH_MENU_SELECTOR = '.ad-menu'
export const DSH_TOAST_SELECTOR = '.toast'

export type OverlayRoot = {
  querySelector: (selector: string) => unknown
  querySelectorAll?: (selector: string) => ArrayLike<{ getBoundingClientRect: () => Box }>
}

export function rectsOverlap(a: Box, b: Box) {
  return a.left < b.left + b.width && a.left + a.width > b.left && a.top < b.top + b.height && a.top + a.height > b.top
}

export function dshEmbedBlocked(root: OverlayRoot, embed?: Box | null) {
  if (root.querySelector(DSH_MASK_SELECTOR)) return true
  if (!embed) return false
  const menus = root.querySelectorAll?.(DSH_MENU_SELECTOR)
  if (!menus?.length) return false
  for (let i = 0; i < menus.length; i++) {
    if (rectsOverlap(embed, menus[i].getBoundingClientRect())) return true
  }
  return false
}

export function toastOverlayTop(
  toast: { getBoundingClientRect: () => { bottom: number } } | null
): number {
  if (!toast) return 0
  return Math.ceil(toast.getBoundingClientRect().bottom) + DSH_EMBED_GUTTER
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
    overlayTop?: number
  }
): DshEmbedBounds | null {
  const edge = opts.maximized ? 0 : DSH_EMBED_EDGE
  const bar = Math.max(0, Math.round(opts.statusBarHeight ?? DSH_STATUS_BAR))
  const rail = Math.max(0, Math.round(opts.docRailWidth))
  let x = Math.round(box.left) + DSH_EMBED_GUTTER
  let y = Math.round(box.top)
  let width = Math.round(box.width) - DSH_EMBED_GUTTER - rail
  if (rail > 0) width -= DSH_EMBED_GUTTER
  let height = Math.round(box.height)
  const overlayTop = Math.max(0, Math.round(opts.overlayTop ?? 0))
  if (overlayTop > y) {
    height -= overlayTop - y
    y = overlayTop
  }
  const maxRight = Math.round(opts.windowWidth) - edge
  const maxBottom = Math.round(opts.windowHeight) - Math.max(edge, bar)
  if (x + width > maxRight) width = maxRight - x
  if (y + height > maxBottom) height = maxBottom - y
  if (width < 80 || height < 80) return null
  return { x, y, width, height }
}
