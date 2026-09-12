import { reactive, ref, watch } from 'vue'

const STORAGE_KEY = 'agent-dock-layout'
export const LAYOUT_VERSION = 3

export const COLLAPSED_WIDTH = 40
export const SIDEBAR_MIN = 240
export const SIDEBAR_MAX = 420
export const DOCRAIL_MIN = 240
export const DOCRAIL_MAX = 320

export type LayoutState = {
  sidebarWidth: number
  sidebarCollapsed: boolean
  projectsCollapsed: boolean
  docRailWidth: number
  docRailCollapsed: boolean
}

export type StoredLayout = Partial<LayoutState> & {
  sessionWidth?: number
  projectCollapsed?: boolean
  layoutVersion?: number
}

export function migrateStoredLayout(stored: StoredLayout): StoredLayout {
  const version = stored.layoutVersion ?? 0
  if (version >= LAYOUT_VERSION) return stored
  const next = { ...stored, layoutVersion: LAYOUT_VERSION }
  if (version < 2) next.docRailCollapsed = true
  if (version < 3) next.sidebarCollapsed = false
  return next
}

function clamp(n: number, min: number, max: number) {
  return Math.min(max, Math.max(min, Math.round(n)))
}

function readStored(): StoredLayout {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return migrateStoredLayout(raw ? JSON.parse(raw) : {})
  } catch {
    return migrateStoredLayout({})
  }
}

const stored = readStored()

export const layout = reactive<LayoutState>({
  sidebarWidth: clamp(stored.sidebarWidth ?? stored.sessionWidth ?? 280, SIDEBAR_MIN, SIDEBAR_MAX),
  sidebarCollapsed: false,
  projectsCollapsed: Boolean(stored.projectsCollapsed),
  docRailWidth: clamp(stored.docRailWidth ?? 260, DOCRAIL_MIN, DOCRAIL_MAX),
  docRailCollapsed: stored.docRailCollapsed ?? true
})

export const paneDragging = ref(false)
export const paneAnimating = ref(false)
export const windowResizing = ref(false)
export const windowMoving = ref(false)

let animTimer = 0
let windowResizeTimer = 0
let windowMoveTimer = 0
let persistTimer = 0
let windowResizeBound = false

export function isLayoutBusy() {
  return paneAnimating.value || paneDragging.value || windowResizing.value || windowMoving.value
}

function setResizingClass(on: boolean) {
  if (typeof document === 'undefined') return
  document.documentElement.classList.toggle('ad-resizing', on)
}

function setMovingClass(on: boolean) {
  if (typeof document === 'undefined') return
  document.documentElement.classList.toggle('ad-moving', on)
}

export function beginWindowMove() {
  if (windowMoving.value) return
  windowMoving.value = true
  setMovingClass(true)
}

export function endWindowMove() {
  if (typeof window !== 'undefined') window.clearTimeout(windowMoveTimer)
  windowMoveTimer = 0
  if (!windowMoving.value) return
  windowMoving.value = false
  setMovingClass(false)
}

export function noteWindowMove() {
  beginWindowMove()
  if (typeof window === 'undefined') return
  window.clearTimeout(windowMoveTimer)
  windowMoveTimer = window.setTimeout(() => {
    windowMoveTimer = 0
    windowMoving.value = false
    setMovingClass(false)
  }, 160)
}

export function noteWindowResize() {
  if (!windowResizing.value) {
    windowResizing.value = true
    setResizingClass(true)
  }
  window.clearTimeout(windowResizeTimer)
  windowResizeTimer = window.setTimeout(() => {
    windowResizing.value = false
    setResizingClass(false)
  }, 200)
}

export function bindWindowResize() {
  if (windowResizeBound || typeof window === 'undefined') return
  windowResizeBound = true
  window.addEventListener('resize', noteWindowResize, { passive: true })
  const observe = () => {
    if (typeof ResizeObserver === 'undefined') return
    const target = document.getElementById('app') ?? document.body
    if (!target) return
    const observer = new ResizeObserver(() => noteWindowResize())
    observer.observe(target)
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', observe, { once: true })
  } else {
    observe()
  }
}

function animBudget() {
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 32 : 280
  } catch {
    return 280
  }
}

export function beginPaneAnim() {
  paneAnimating.value = true
  window.clearTimeout(animTimer)
  animTimer = window.setTimeout(() => {
    paneAnimating.value = false
  }, animBudget())
}

export function endPaneAnim() {
  window.clearTimeout(animTimer)
  paneAnimating.value = false
}

watch(
  layout,
  (value) => {
    if (typeof window === 'undefined') return
    window.clearTimeout(persistTimer)
    persistTimer = window.setTimeout(() => {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...value, layoutVersion: LAYOUT_VERSION }))
    }, 200)
  },
  { deep: true }
)

bindWindowResize()

export function beginPaneDrag() {
  paneDragging.value = true
}

export function endPaneDrag() {
  paneDragging.value = false
}

export function sidebarPaneWidth() {
  return layout.sidebarWidth
}

export function clampOverlayBox(
  box: { x: number; y: number; width: number; height: number },
  bounds: { windowWidth: number; windowHeight: number; containRight?: number }
) {
  const pad = 8
  const right = bounds.containRight ?? bounds.windowWidth - pad
  const maxX = right - box.width
  const maxY = bounds.windowHeight - pad - box.height
  return {
    x: Math.max(0, Math.min(Math.round(box.x), maxX)),
    y: Math.max(pad, Math.min(Math.round(box.y), maxY))
  }
}

export function toggleProjects() {
  layout.projectsCollapsed = !layout.projectsCollapsed
}

let sidebarRaf = 0
let sidebarPending: number | null = null

function applySidebarSize(next: number) {
  layout.sidebarWidth = clamp(next, SIDEBAR_MIN, SIDEBAR_MAX)
}

export function resizeSidebar(next: number) {
  sidebarPending = next
  if (sidebarRaf) return
  const run = () => {
    sidebarRaf = 0
    if (sidebarPending == null) return
    applySidebarSize(sidebarPending)
    sidebarPending = null
  }
  if (typeof requestAnimationFrame === 'undefined') {
    run()
    return
  }
  sidebarRaf = requestAnimationFrame(run)
}

export function docRailPaneWidth() {
  return layout.docRailCollapsed ? 0 : layout.docRailWidth
}

export function toggleDocRail() {
  layout.docRailCollapsed = !layout.docRailCollapsed
}

let docRailRaf = 0
let docRailPending: number | null = null

function applyDocRailSize(next: number) {
  if (next < DOCRAIL_MIN - 24) {
    layout.docRailCollapsed = true
    return
  }
  layout.docRailCollapsed = false
  layout.docRailWidth = clamp(next, DOCRAIL_MIN, DOCRAIL_MAX)
}

export function resizeDocRail(next: number) {
  docRailPending = next
  if (docRailRaf) return
  const run = () => {
    docRailRaf = 0
    if (docRailPending == null) return
    applyDocRailSize(docRailPending)
    docRailPending = null
  }
  if (typeof requestAnimationFrame === 'undefined') {
    run()
    return
  }
  docRailRaf = requestAnimationFrame(run)
}
