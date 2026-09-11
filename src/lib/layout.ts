import { reactive, ref, watch } from 'vue'

const STORAGE_KEY = 'agent-dock-layout'

export const COLLAPSED_WIDTH = 40
export const SIDEBAR_MIN = 240
export const SIDEBAR_MAX = 420

export type LayoutState = {
  sidebarWidth: number
  sidebarCollapsed: boolean
}

function clamp(n: number, min: number, max: number) {
  return Math.min(max, Math.max(min, Math.round(n)))
}

function readStored(): Partial<LayoutState> & { sessionWidth?: number; projectCollapsed?: boolean } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

const stored = readStored()

export const layout = reactive<LayoutState>({
  sidebarWidth: clamp(stored.sidebarWidth ?? stored.sessionWidth ?? 280, SIDEBAR_MIN, SIDEBAR_MAX),
  sidebarCollapsed: Boolean(stored.sidebarCollapsed ?? stored.projectCollapsed)
})

export const paneDragging = ref(false)
export const paneAnimating = ref(false)
export const windowResizing = ref(false)

let animTimer = 0
let windowResizeTimer = 0
let persistTimer = 0
let windowResizeBound = false

export function isLayoutBusy() {
  return paneAnimating.value || paneDragging.value || windowResizing.value
}

function setResizingClass(on: boolean) {
  if (typeof document === 'undefined') return
  document.documentElement.classList.toggle('ad-resizing', on)
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
      localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
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
  return layout.sidebarCollapsed ? COLLAPSED_WIDTH : layout.sidebarWidth
}

export function toggleSidebar() {
  if (!paneDragging.value) beginPaneAnim()
  layout.sidebarCollapsed = !layout.sidebarCollapsed
}

let sidebarRaf = 0
let sidebarPending: number | null = null

function applySidebarSize(next: number) {
  if (next < SIDEBAR_MIN - 24) {
    layout.sidebarCollapsed = true
    return
  }
  layout.sidebarCollapsed = false
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
