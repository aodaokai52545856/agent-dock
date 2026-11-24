<script setup lang="ts">
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { listen, TauriEvent, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import { clampOverlayBox, isLayoutBusy, paneAnimating, paneDragging, windowMoving, windowResizing } from '../lib/layout'
import { isMac, isWindows } from '../lib/platform'
import { antigravityBannerAccount } from '../lib/antigravityBanner'
import { isSignificantPtyChunk } from '../lib/livePulse'
import { isDshWeb } from '../lib/livePty'
import { markPtyExit, notePtyData, selectedProject, showToast, store } from '../lib/store'
import { codeFontStack } from '../lib/appearance'
import { canMeasure, createFitScheduler, rowsThatFit } from '../lib/termFit'
import {
  antigravityFileInputs,
  attachTermClipboard,
  resolveAntigravityPaste,
  resolveClaudePaste,
  shouldInterceptAntigravityPaste,
  shouldInterceptPaste
} from '../lib/termClipboard'
import { termMenuItems, type TermMenuAction } from '../lib/termMenu'
import {
  ansiPaletteBlack,
  ansiPaletteWhite,
  attachOscBackground,
  canvasPad,
  ptyCanvasInk,
  ptyForcesDark,
  termSchemeFromInk,
  termThemeBackground
} from '../lib/termGlass'
import { attachSynchronizedOutput, createRefreshGate } from '../lib/termSync'
import { createTermStream } from '../lib/termStream'
import { createTermRecovery } from '../lib/termRecovery'
import { createTermFollow } from '../lib/termFollow'
import ToolMark from './ToolMark.vue'
import { TOOLS, type ToolId } from '../lib/types'

type Host = {
  term: Terminal
  fit: FitAddon
  el: HTMLDivElement
  offData: { dispose: () => void }
  offSurface: { dispose: () => void }
  offClipboard: { dispose: () => void }
  offAccount?: { dispose: () => void }
  stream: ReturnType<typeof createTermStream>
  recovery?: ReturnType<typeof createTermRecovery>
  follow?: ReturnType<typeof createTermFollow>
  offViewport?: { dispose: () => void }
  unseenOutput: boolean
  lastCols: number
  lastRows: number
  lastW: number
  lastH: number
}

type FileDrop = { paths: string[]; position: { x: number; y: number } }

const hosts = new Map<string, Host>()
let unlistenData: UnlistenFn | undefined
let unlistenExit: UnlistenFn | undefined
let unlistenDrop: UnlistenFn | undefined
let observer: ResizeObserver | undefined
let mounted = false
const workspaceEl = ref<HTMLElement | null>(null)
const menu = ref<{ x: number; y: number; hasSelection: boolean } | null>(null)
const MENU_WIDTH = 200
const MENU_HEIGHT = 118

const menuItems = computed(() =>
  termMenuItems({
    hasSelection: Boolean(menu.value?.hasSelection),
    mac: isMac
  })
)

function closeTermMenu() {
  menu.value = null
}

function onTermMenu(event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  const host = store.activePtyId ? hosts.get(store.activePtyId) : undefined
  if (!host) {
    closeTermMenu()
    return
  }
  const pos = clampOverlayBox(
    { x: event.clientX, y: event.clientY, width: MENU_WIDTH, height: MENU_HEIGHT },
    { windowWidth: window.innerWidth, windowHeight: window.innerHeight }
  )
  menu.value = { ...pos, hasSelection: host.term.hasSelection() }
}

async function runTermMenu(action: TermMenuAction) {
  const id = store.activePtyId
  const host = id ? hosts.get(id) : undefined
  closeTermMenu()
  if (!host || !id) return
  if (action === 'copy') {
    const text = host.term.getSelection()
    if (text) await api.clipboardWrite(text)
    return
  }
  if (action === 'paste') {
    host.term.focus()
    await pasteFromClipboard(id, host.term)
    return
  }
  if (action === 'selectAll') host.term.selectAll()
}

function fitAll() {
  const id = store.activePtyId
  if (!id) return
  const host = hosts.get(id)
  if (host) fitHost(id, host)
}

const fitScheduler = createFitScheduler({
  isBusy: isLayoutBusy,
  run: fitAll,
  scheduleFrame: (cb) => requestAnimationFrame(cb),
  scheduleTimeout: (cb, ms) => window.setTimeout(cb, ms),
  cancelTimeout: (id) => window.clearTimeout(id),
  cancelFrame: (id) => cancelAnimationFrame(id),
  settleMs: 80
})

function scheduleFit() {
  fitScheduler.request()
}

const props = defineProps<{
  fontSize: number
  loading?: boolean
  loadingText?: string
}>()

const emit = defineEmits<{
  start: [toolId?: ToolId]
}>()

function cssVar(name: string, fallback: string) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

function liveForPty(ptyId: string) {
  return store.live.find((item) => item.ptyId === ptyId)
}

function toolForPty(ptyId: string) {
  return liveForPty(ptyId)?.toolId
}

async function pasteFromClipboard(ptyId: string, term: Terminal, fallbackText = '') {
  const tool = toolForPty(ptyId)
  try {
    const contents = await api.clipboardReadContents()
    if (hosts.get(ptyId)?.term !== term || store.activePtyId !== ptyId || store.appMode !== 'console') return
    if (tool === 'antigravity') {
      const action = resolveAntigravityPaste({ ...contents, text: contents.text || fallbackText })
      if (action.kind === 'image') await api.ptyWrite(ptyId, action.sequence)
      if (action.kind === 'text') await api.writeSessionText(ptyId, action.text.replaceAll('\x1b', ''))
      if (action.kind === 'files') await pasteAntigravityFiles(ptyId, action.paths)
      return
    }
    if (tool === 'claude') {
      const action = resolveClaudePaste(contents, isWindows)
      if (action.kind === 'image') {
        await api.ptyWrite(ptyId, action.sequence)
        return
      }
      if (action.kind === 'text') term.paste(action.text)
      return
    }
    if (contents.text) term.paste(contents.text)
  } catch {
    if (tool === 'antigravity') showToast('未能粘贴到 Antigravity 终端')
  }
}

async function pasteAntigravityFiles(ptyId: string, paths: string[]) {
  try {
    for (const input of antigravityFileInputs(paths, isWindows)) {
      if (store.activePtyId !== ptyId || store.appMode !== 'console' || !hosts.has(ptyId)) return
      await api.writeSessionText(ptyId, input)
    }
  } catch {
    showToast('未能将文件加入 Antigravity 终端')
  }
}

function onFileDrop(payload: FileDrop) {
  const id = store.activePtyId
  if (!id || toolForPty(id) !== 'antigravity' || store.appMode !== 'console') return
  const host = hosts.get(id)
  // WebView2 reports physical pixels. Wry's WebKit backends report logical coordinates.
  const scale = isWindows ? window.devicePixelRatio : 1
  const target = document.elementFromPoint(payload.position.x / scale, payload.position.y / scale)
  if (!host || !host.el.contains(target)) return
  host.term.focus()
  void pasteAntigravityFiles(id, payload.paths)
}

function canHost(ptyId: string) {
  return Boolean(ptyId) && !isDshWeb(liveForPty(ptyId)) && liveForPty(ptyId)?.kind !== 'ompRpc'
}

function theme(ptyId?: string) {
  const uiInk = cssVar('--ad-ink', '#0b0f13')
  const toolId = ptyId ? toolForPty(ptyId) : undefined
  const follow = !ptyForcesDark(toolId)
  const ink = ptyCanvasInk(uiInk, toolId)
  const foreground = follow ? cssVar('--ad-text', '#ececec') : '#ececec'
  const muted = follow ? cssVar('--ad-muted', '#8a8a8a') : '#8a8a8a'
  const opacity = Number.parseFloat(cssVar('--ad-ui-opacity', '0')) || 0
  const scheme = termSchemeFromInk(ink)
  return {
    background: termThemeBackground(ink, opacity),
    foreground,
    cursor: foreground,
    cursorAccent: scheme === 'dark' ? ink : foreground,
    selectionBackground: scheme === 'dark' ? '#ffffff22' : '#00000018',
    black: follow ? ink : ansiPaletteBlack(ink),
    red: cssVar('--ad-error', '#e24b4a'),
    green: cssVar('--ad-success', '#3d9a6a'),
    yellow: cssVar('--ad-warning', '#c9a227'),
    blue: cssVar('--ad-dsh', '#4f7cff'),
    magenta: cssVar('--ad-grok', '#a78bfa'),
    cyan: cssVar('--ad-pi', '#22d3ee'),
    white: follow ? foreground : ansiPaletteWhite(foreground, ink),
    brightBlack: muted,
    brightRed: cssVar('--ad-error', '#e24b4a'),
    brightGreen: cssVar('--ad-opencode', '#6ee7b7'),
    brightYellow: cssVar('--ad-warning', '#c9a227'),
    brightBlue: cssVar('--ad-dsh', '#4f7cff'),
    brightMagenta: cssVar('--ad-kimi', '#fb923c'),
    brightCyan: cssVar('--ad-pi', '#22d3ee'),
    brightWhite: '#ffffff'
  }
}

function termFontFamily() {
  return cssVar('--ad-mono', codeFontStack(store.settings.codeFontFamily))
}

function hostPad(ptyId?: string) {
  const opacity = Number.parseFloat(cssVar('--ad-ui-opacity', '0')) || 0
  return canvasPad(cssVar('--ad-ink', '#0b0f13'), opacity, ptyId ? toolForPty(ptyId) : undefined)
}

function paintHost(el: HTMLDivElement, ptyId: string) {
  const toolId = toolForPty(ptyId)
  const ink = ptyCanvasInk(cssVar('--ad-ink', '#0b0f13'), toolId)
  el.dataset.scheme = termSchemeFromInk(ink)
  if (toolId) el.dataset.tool = toolId
  else delete el.dataset.tool
  el.style.background = hostPad(ptyId)
}

function applyTermChrome() {
  hosts.forEach((host, ptyId) => {
    paintHost(host.el, ptyId)
    host.term.options.fontFamily = termFontFamily()
    host.term.options.theme = theme(ptyId)
  })
  if (store.activePtyId) show(store.activePtyId)
}

function ensureHost(ptyId: string) {
  if (!canHost(ptyId)) return undefined
  const existing = hosts.get(ptyId)
  if (existing) {
    configureViewportHost(ptyId, existing)
    return existing
  }
  const el = document.createElement('div')
  el.className = 'term-host'
  paintHost(el, ptyId)
  if (ptyId !== store.activePtyId) el.classList.add('is-hidden')
  const mount = document.getElementById('term-mount')
  mount?.appendChild(el)
  const term = new Terminal({
    cols: 120,
    rows: 32,
    fontFamily: termFontFamily(),
    fontSize: props.fontSize,
    theme: theme(ptyId),
    allowTransparency: true,
    cursorBlink: true,
    scrollback: 4000,
    convertEol: false,
    rescaleOverlappingGlyphs: true,
    ...(isWindows ? { windowsPty: { backend: 'conpty' as const, buildNumber: 22621 } } : {})
  })
  const fit = new FitAddon()
  term.loadAddon(fit)
  term.open(el)
  paintHost(el, ptyId)
  const offData = term.onData((data) => {
    void api.ptyWrite(ptyId, data)
  })
  const offSurface = bindTermSurface(term, ptyId)
  el.addEventListener('paste', (event) => {
    if (toolForPty(ptyId) !== 'antigravity') return
    event.preventDefault()
    event.stopPropagation()
    void pasteFromClipboard(ptyId, term, event.clipboardData?.getData('text/plain'))
  }, true)
  const offClipboard = attachTermClipboard(term, el, {
    write: (text) => {
      void api.clipboardWrite(text)
    },
    mac: isMac,
    onPaste: () => {
      void pasteFromClipboard(ptyId, term)
    },
    interceptPaste: (event) => {
      const tool = toolForPty(ptyId)
      return tool === 'antigravity'
        ? shouldInterceptAntigravityPaste(event, isMac)
        : tool === 'claude' && shouldInterceptPaste(event, isMac)
    }
  })
  const stream = createTermStream((data, done) => term.write(data, done))
  const host: Host = { term, fit, el, offData, offSurface, offClipboard, stream, unseenOutput: false, lastCols: 0, lastRows: 0, lastW: 0, lastH: 0 }
  hosts.set(ptyId, host)
  configureViewportHost(ptyId, host)
  configureAntigravityHost(ptyId, host)
  return host
}

function configureAntigravityHost(ptyId: string, host: Host) {
  const live = liveForPty(ptyId)
  if (host.offAccount || live?.toolId !== 'antigravity' || live.account) return

  const { term } = host
  const capture = () => {
    const current = liveForPty(ptyId)
    if (!current || current.account) {
      host.offAccount?.dispose()
      return
    }
    const buffer = term.buffer.active
    const cursor = buffer.baseY + buffer.cursorY
    const readFrom = (start: number) => antigravityBannerAccount(row => {
      const index = start + row
      // The current row can be partial when ConPTY splits a redraw.
      return index === cursor ? '' : buffer.getLine(index)?.translateToString(true) ?? ''
    }, Math.min(term.rows, 12))
    const account = readFrom(buffer.baseY) ?? (buffer.baseY > 0 ? readFrom(0) : null)
    if (!account) return

    current.account = account
    host.offAccount?.dispose()
  }
  host.offAccount = term.onWriteParsed(capture)
  capture()
}

function configureViewportHost(ptyId: string, host: Host) {
  const toolId = toolForPty(ptyId)
  if (host.follow || (toolId !== 'omp' && toolId !== 'antigravity')) return
  const { term, el } = host
  const atBottom = () => term.buffer.active.viewportY === term.buffer.active.baseY
  const inNormalBuffer = () => term.buffer.active.type === 'normal'
  const visible = () => store.appMode === 'console' && store.activePtyId === ptyId
    && !document.hidden && !el.classList.contains('is-hidden') && el.clientHeight > 0
  const follow = createTermFollow({
    canFollow: () => visible() && !term.hasSelection() && inNormalBuffer(),
    atBottom,
    scrollToBottom: () => term.scrollToBottom(),
    scheduleFrame: cb => requestAnimationFrame(cb),
    cancelFrame: id => cancelAnimationFrame(id),
    captureAnchor: () => {
      const buffer = term.buffer.active
      return buffer.type === 'normal' ? term.registerMarker(buffer.viewportY - buffer.baseY - buffer.cursorY) : undefined
    },
    scrollToLine: line => term.scrollToLine(line)
  })
  host.follow = follow
  paintHost(el, ptyId)
  term.options.theme = theme(ptyId)
  if (isWindows && toolId === 'omp') {
    const recovery = createTermRecovery({
      canRecover: () => visible() && !follow.isSuspended() && follow.isFollowing() && !isLayoutBusy() && !term.hasSelection()
        && inNormalBuffer(),
      recover: () => redrawHost(ptyId, host, false),
      scheduleTimeout: (cb, ms) => window.setTimeout(cb, ms),
      cancelTimeout: id => window.clearTimeout(id)
    })
    host.recovery = recovery
  }
  let userFrame: number | undefined
  const observeUserScroll = (down: boolean) => {
    if (!inNormalBuffer()) return
    if (userFrame !== undefined) cancelAnimationFrame(userFrame)
    userFrame = requestAnimationFrame(() => {
      userFrame = undefined
      if (!visible() || follow.isSuspended() || !inNormalBuffer()) return
      const wasFollowing = follow.isFollowing()
      // DOM and buffer positions can disagree after a TUI repaint. A downward
      // gesture at the physical bottom explicitly asks to return to live output.
      const viewport = el.querySelector('.xterm-viewport') as HTMLElement | null
      if (down && viewport && viewport.scrollTop + viewport.clientHeight >= viewport.scrollHeight - 1) term.scrollToBottom()
      follow.userScroll()
      if (down && follow.isFollowing()) {
        host.unseenOutput = false
        if (!wasFollowing) host.recovery?.request(true)
      }
    })
  }
  const onWheel = (event: WheelEvent) => {
    if (!inNormalBuffer()) return
    if (event.deltaY < 0 && term.buffer.active.baseY > 0) follow.pause()
    if (event.deltaY !== 0) observeUserScroll(event.deltaY > 0)
  }
  const onKey = (event: KeyboardEvent) => {
    if (!event.shiftKey || !inNormalBuffer()) return
    if (event.key === 'PageUp' || event.key === 'Home') follow.pause()
    if (event.key === 'PageDown' || event.key === 'End') observeUserScroll(true)
  }
  const offParsed = term.onWriteParsed(() => follow.refresh())
  const offScroll = term.onScroll(() => follow.refresh())
  const offSelection = term.onSelectionChange(() => {
    // xterm also clears selection during buffer activation/reset. This is not
    // user scroll intent: sampling that transient viewport can pin it at row 0.
    if (!term.hasSelection()) { follow.refresh(); host.recovery?.request() }
  })
  const viewport = el.querySelector('.xterm-viewport') as HTMLElement | null
  let scrollbarDragging = false
  const onPointerDown = (event: MouseEvent) => {
    if (!viewport || scrollbarDragging || !inNormalBuffer()) return
    const bounds = viewport.getBoundingClientRect()
    if (event.clientX < bounds.left + viewport.clientLeft + viewport.clientWidth || event.clientX > bounds.right) return
    scrollbarDragging = true
    follow.pause()
  }
  const onViewportScroll = () => {
    if (!visible() || follow.isSuspended() || !inNormalBuffer()) return
    if (scrollbarDragging || term.hasSelection()) follow.userScroll()
    else follow.refresh()
  }
  const onPointerUp = () => {
    if (!scrollbarDragging) return
    scrollbarDragging = false
    observeUserScroll(true)
  }
  viewport?.addEventListener('pointerdown', onPointerDown)
  viewport?.addEventListener('mousedown', onPointerDown)
  viewport?.addEventListener('scroll', onViewportScroll, { passive: true })
  window.addEventListener('pointerup', onPointerUp)
  window.addEventListener('mouseup', onPointerUp)
  window.addEventListener('pointercancel', onPointerUp)
  el.addEventListener('wheel', onWheel, { capture: true, passive: true })
  el.addEventListener('keydown', onKey, true)
  host.offViewport = {
    dispose() {
      if (userFrame !== undefined) cancelAnimationFrame(userFrame)
      offParsed.dispose()
      offScroll.dispose()
      offSelection.dispose()
      viewport?.removeEventListener('pointerdown', onPointerDown)
      viewport?.removeEventListener('mousedown', onPointerDown)
      viewport?.removeEventListener('scroll', onViewportScroll)
      window.removeEventListener('pointerup', onPointerUp)
      window.removeEventListener('mouseup', onPointerUp)
      window.removeEventListener('pointercancel', onPointerUp)
      el.removeEventListener('wheel', onWheel, true)
      el.removeEventListener('keydown', onKey, true)
    }
  }
  follow.refresh()
}

function refreshHost(host: Host) {
  try {
    host.term.refresh(0, Math.max(0, host.term.rows - 1))
  } catch {
    /* not measured yet */
  }
}

function bindTermSurface(term: Terminal, ptyId: string) {
  const refresh = () => {
    try {
      term.refresh(0, Math.max(0, term.rows - 1))
    } catch {
      /* not measured yet */
    }
  }
  const gate = createRefreshGate({
    refresh,
    scheduleFrame: (cb) => requestAnimationFrame(cb),
    cancelFrame: (id) => cancelAnimationFrame(id)
  })
  const send = (data: string) => {
    void api.ptyWrite(ptyId, data)
  }
  const offSync = attachSynchronizedOutput(term, {
    send,
    refresh: () => gate.request()
  })
  const offOsc = attachOscBackground(term, {
    send,
    ink: () => ptyCanvasInk(cssVar('--ad-ink', '#0b0f13'), toolForPty(ptyId))
  })
  const offScroll = term.onScroll(() => gate.request())
  return {
    dispose() {
      offSync.dispose()
      offOsc.dispose()
      offScroll.dispose()
      gate.dispose()
    }
  }
}

function fitHost(ptyId: string, host: Host, force = false) {
  void host.stream.resize(() => resizeHost(ptyId, host, force))
}

async function resizeHost(ptyId: string, host: Host, force = false) {
  if (!force && isLayoutBusy()) return
  const width = host.el.clientWidth
  const height = host.el.clientHeight
  if (!canMeasure(width, height)) return
  if (!force && width === host.lastW && height === host.lastH) return
  try {
    const proposed = host.fit.proposeDimensions()
    if (!proposed || proposed.cols < 20 || proposed.rows < 8) return
    host.fit.fit()
    const screen = host.el.querySelector('.xterm-screen') as HTMLElement | null
    const fittedRows = rowsThatFit(height, screen?.clientHeight ?? 0, host.term.rows)
    if (fittedRows < host.term.rows) host.term.resize(host.term.cols, fittedRows)
    host.lastW = width
    host.lastH = height
    const cols = host.term.cols
    const rows = host.term.rows
    if (cols === host.lastCols && rows === host.lastRows) return
    await api.ptyResize(ptyId, cols, rows)
    host.lastCols = cols
    host.lastRows = rows
  } catch {
    host.lastW = 0
    host.lastH = 0
    /* mount not measured yet */
  }
}

function fitActive(force = false) {
  const id = store.activePtyId
  if (!id) return
  const host = hosts.get(id)
  if (!host) return
  fitHost(id, host, force)
  refreshHost(host)
}

async function redrawActive() {
  const id = store.activePtyId
  const host = id ? hosts.get(id) : undefined
  if (!host || !id || toolForPty(id) !== 'omp') return
  await redrawHost(id, host, true)
}

function scrollToBottom() {
  const id = store.activePtyId
  const host = id ? hosts.get(id) : undefined
  if (!host || !id || toolForPty(id) !== 'omp') return
  host.term.clearSelection()
  host.term.scrollToBottom()
  host.follow?.resume()
  host.follow?.userScroll()
  host.unseenOutput = false
  host.recovery?.request(true)
  host.term.focus()
}

async function redrawHost(id: string, host: Host, focus: boolean) {
  await host.stream.resize(async () => {
    if (hosts.get(id) !== host) return
    // Recheck after queued output/resize jobs, since the user may have moved
    // into history or another pane while this automatic repaint was waiting.
    if (!focus && (store.appMode !== 'console' || store.activePtyId !== id || document.hidden
      || isLayoutBusy() || host.term.hasSelection() || !host.follow?.isFollowing())) return
    const { cols, rows } = host.term
    // A genuine size change asks OMP/ConPTY for a full repaint, even when
    // its current viewport already matches. This preserves input and tasks.
    host.term.resize(cols + 1, rows)
    try { await api.ptyResize(id, cols + 1, rows) }
    finally {
      host.term.resize(cols, rows)
      await api.ptyResize(id, cols, rows)
    }
    refreshHost(host)
    if (focus) host.term.focus()
  })
}

function show(ptyId: string | '') {
  const visible = ptyId && canHost(ptyId) ? ptyId : ''
  hosts.forEach((host, id) => {
    host.el.classList.toggle('is-hidden', id !== visible)
  })
  const host = visible ? hosts.get(visible) : undefined
  if (!host || !visible) return
  nextTick(() => {
    requestAnimationFrame(() => {
      void host.stream.resize(async () => {
        await resizeHost(visible, host, true)
        if (hosts.get(visible) !== host || store.activePtyId !== visible) return
        refreshHost(host)
        host.term.focus()
        resumeHostViewport(host)
      })
    })
  })
}

function disposeHost(ptyId: string) {
  const host = hosts.get(ptyId)
  if (!host) return
  host.offData.dispose()
  host.offSurface.dispose()
  host.offClipboard.dispose()
  host.offAccount?.dispose()
  host.offViewport?.dispose()
  host.recovery?.dispose()
  host.follow?.dispose()
  host.stream.dispose()
  host.term.dispose()
  host.el.remove()
  hosts.delete(ptyId)
}

watch(
  () => store.activePtyId,
  (id, previous) => {
    if (previous) hosts.get(previous)?.follow?.suspend()
    if (id && canHost(id) && !hosts.has(id)) ensureHost(id)
    show(id)
  },
  { immediate: true }
)

// pty-data can arrive before pty_open returns its tool metadata. Upgrade an
// existing host when that metadata arrives, instead of skipping setup forever.
watch(() => store.live.map(item => `${item.ptyId}:${item.toolId}:${item.kind}`), () => {
  hosts.forEach((host, id) => {
    configureViewportHost(id, host)
    configureAntigravityHost(id, host)
  })
})

// A list refresh can observe an exit before the backend monitor emits it.
watch(() => store.live.map(item => item.ptyId), (ids, previous) => {
  for (const id of previous) if (!ids.includes(id)) disposeHost(id)
})

watch(() => store.appMode, mode => {
  if (mode === 'console') void nextTick(() => resumeActiveViewport())
  else suspendViewports()
})

watch(
  () => props.fontSize,
  (size) => {
    hosts.forEach((host) => host.term.options.fontSize = size)
    if (store.activePtyId) show(store.activePtyId)
  }
)

watch(
  () => [
    store.settings.codeFontFamily,
    store.settings.uiTheme,
    store.settings.uiBackground,
    store.settings.uiForeground,
    store.settings.uiAccent,
    store.settings.uiContrast,
    store.settings.uiOpacity
  ],
  () => applyTermChrome()
)

watch([paneAnimating, paneDragging, windowResizing, windowMoving], (now) => {
  fitScheduler.onBusyChange(now[0] || now[1] || now[2] || now[3])
  if (!isLayoutBusy() && store.activePtyId) hosts.get(store.activePtyId)?.recovery?.request()
})

watch(windowMoving, (moving, was) => {
  if (was && !moving) fitActive()
})

onMounted(async () => {
  mounted = true
  if (api.isTauri) {
    unlistenData = await listen<{ ptyId: string; data: string; bytes?: number }>('pty-data', (event) => {
      const acknowledge = () => {
        if (event.payload.bytes) void api.ptyAck(event.payload.ptyId, event.payload.bytes).catch(() => {})
      }
      if (!mounted) { acknowledge(); return }
      const host = ensureHost(event.payload.ptyId)
      if (host) {
        if (store.appMode !== 'console' || store.activePtyId !== event.payload.ptyId || document.hidden || !document.hasFocus()) host.unseenOutput = true
        host.stream.write(event.payload.data, acknowledge)
        host.recovery?.noteOutput(event.payload.data)
      } else acknowledge()
      if (isSignificantPtyChunk(event.payload.data)) notePtyData(event.payload.ptyId)
    })
    if (!mounted) { unlistenData(); return }
    unlistenExit = await listen<{ ptyId: string }>('pty-exit', (event) => {
      disposeHost(event.payload.ptyId)
      markPtyExit(event.payload.ptyId)
    })
    if (!mounted) { unlistenExit(); return }
    void listen<FileDrop>(TauriEvent.DRAG_DROP, ({ payload }) => onFileDrop(payload)).then((unlisten) => {
      if (mounted) unlistenDrop = unlisten
      else unlisten()
    }).catch(() => {})
    // A reloaded webview cannot acknowledge frames delivered to the old page.
    await api.ptyResetOutput()
    if (!mounted) return
  }
  const target = workspaceEl.value ?? document.getElementById('term-mount')
  if (target) {
    observer = new ResizeObserver(() => {
      scheduleFit()
    })
    observer.observe(target)
  }
  window.addEventListener('ad-appearance', applyTermChrome)
  window.addEventListener('click', closeTermMenu)
  window.addEventListener('blur', closeTermMenu)
  window.addEventListener('blur', suspendViewports)
  window.addEventListener('keydown', onTermMenuKey)
  window.addEventListener('focus', resumeActiveViewport)
  document.addEventListener('visibilitychange', resumeActiveViewport)
  if (store.activePtyId) show(store.activePtyId)
})

function onTermMenuKey(event: KeyboardEvent) {
  if (event.key === 'Escape') closeTermMenu()
}

function resumeActiveViewport() {
  if (document.hidden) { suspendViewports(); return }
  const host = store.activePtyId ? hosts.get(store.activePtyId) : undefined
  if (!host || document.hidden || store.appMode !== 'console') return
  resumeHostViewport(host)
}

function resumeHostViewport(host: Host) {
  host.follow?.resume()
  if (host.follow?.isFollowing() && host.unseenOutput) {
    host.unseenOutput = false
    host.recovery?.request(true)
  } else host.recovery?.request()
}

function suspendViewports() {
  hosts.forEach(host => host.follow?.suspend())
}

onUnmounted(() => {
  mounted = false
  window.removeEventListener('ad-appearance', applyTermChrome)
  window.removeEventListener('click', closeTermMenu)
  window.removeEventListener('blur', closeTermMenu)
  window.removeEventListener('blur', suspendViewports)
  window.removeEventListener('keydown', onTermMenuKey)
  window.removeEventListener('focus', resumeActiveViewport)
  document.removeEventListener('visibilitychange', resumeActiveViewport)
  unlistenData?.()
  unlistenExit?.()
  unlistenDrop?.()
  observer?.disconnect()
  fitScheduler.dispose()
  hosts.forEach((_, id) => disposeHost(id))
})

defineExpose({ ensureHost, show, dispose: disposeHost, fitActive, redrawActive, scrollToBottom })
</script>

<template>
  <section ref="workspaceEl" class="workspace" @contextmenu="onTermMenu">
    <div v-show="store.activePtyId || loading" id="term-mount" class="mount" />
    <div v-if="loading" class="loading" aria-busy="true" aria-live="polite">
      <span class="spinner" aria-hidden="true" />
      <span>{{ loadingText || '正在打开会话' }}</span>
    </div>
    <div v-else-if="!store.activePtyId" class="empty">
      <div class="hero">
        <span class="mark" aria-hidden="true">~</span>
        <h1 class="empty-title">
          {{
            selectedProject
              ? `你想在 ${selectedProject.name} 中启动什么？`
              : store.projects.length
                ? '先选一个项目，再打开工具'
                : '先添加一个项目，再打开工具'
          }}
        </h1>
        <div class="tiles">
          <button
            v-for="tool in TOOLS"
            :key="tool.id"
            type="button"
            class="tile"
            @click="emit('start', tool.id)"
          >
            <span class="tile-mark" aria-hidden="true">
              <ToolMark :id="tool.id" />
            </span>
            <span class="tile-label">{{ tool.label }}</span>
            <span class="tile-hint">{{ tool.hint }}</span>
          </button>
        </div>
      </div>
    </div>
    <Teleport to="body">
      <div
        v-if="menu"
        class="ad-menu term-menu"
        role="menu"
        :style="{ left: menu.x + 'px', top: menu.y + 'px', width: MENU_WIDTH + 'px' }"
        @click.stop
        @contextmenu.prevent
      >
        <button
          v-for="item in menuItems"
          :key="item.id"
          type="button"
          role="menuitem"
          class="ad-menu-item"
          :disabled="!item.enabled"
          @click="runTermMenu(item.id)"
        >
          <span>{{ item.label }}</span>
          <span class="ad-menu-hint">{{ item.hint }}</span>
        </button>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.workspace {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  background: transparent;
  overflow: hidden;
  container-type: inline-size;
}

.mount {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.term-menu {
  position: fixed;
  z-index: 80;
}

.empty {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 24px;
  background: var(--ad-editor);
}

.hero {
  width: min(660px, 100%);
  min-height: 100%;
  margin: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
}

.mark {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  margin-bottom: 20px;
  border: 1px solid var(--ad-border-strong);
  border-radius: 50%;
  color: var(--ad-text);
  font-family: var(--ad-mono);
  font-size: 18px;
}

.empty-title {
  margin: 0 0 24px;
  max-width: 28ch;
  font-family: var(--ad-display);
  font-size: 22px;
  line-height: 30px;
  font-weight: 500;
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}

.tiles {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  width: 100%;
}

.tile {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  min-height: 96px;
  padding: 16px 12px;
  border: 1px solid var(--ad-border);
  border-radius: var(--ad-radius-control);
  background: var(--ad-hover);
  color: var(--ad-text);
}

.tile:hover {
  background: var(--ad-selected);
}

.tile-mark {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
}

.tile-label {
  font-size: 13px;
  line-height: 20px;
}

.tile-hint {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

@container (max-width: 640px) {
  .tiles {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@container (max-width: 360px) {
  .tiles { grid-template-columns: 1fr; }
}

@media (max-width: 720px) {
  .empty-title {
    font-size: 22px;
    line-height: 30px;
  }

  .tiles {
    grid-template-columns: 1fr;
  }
}

.loading {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  background: var(--ad-ink);
  color: var(--ad-muted);
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--ad-border);
  border-top-color: var(--ad-accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spinner {
    animation: none;
    border-top-color: var(--ad-border);
    background: var(--ad-accent);
  }
}
</style>

<style>
.term-host {
  position: absolute;
  inset: 0;
  overflow: hidden;
  isolation: isolate;
  contain: layout style paint;
  transform: translateZ(0);
  background: transparent;
}

.term-host:not(.is-hidden) {
  z-index: 1;
}

.term-host.is-hidden {
  visibility: hidden;
  pointer-events: none;
  z-index: 0;
}

.term-host .xterm {
  width: 100%;
  height: 100%;
  overflow: hidden;
  padding: 8px;
  box-sizing: border-box;
}

.term-host[data-scheme='dark'] .xterm {
  padding: 0;
}

.term-host[data-tool='pi'] .xterm {
  padding: 8px;
}

.term-host .xterm-bg-0 {
  background-color: transparent !important;
}

.term-host .xterm-viewport {
  scrollbar-width: auto;
  scrollbar-gutter: stable;
}

.term-host .xterm-viewport::-webkit-scrollbar {
  display: block;
  width: 10px;
  height: 10px;
}

</style>
