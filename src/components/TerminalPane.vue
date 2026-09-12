<script setup lang="ts">
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import { clampOverlayBox, isLayoutBusy, paneAnimating, paneDragging, windowMoving, windowResizing } from '../lib/layout'
import { isMac, isWindows } from '../lib/platform'
import { isSignificantPtyChunk } from '../lib/livePulse'
import { markPtyExit, notePtyData, selectedProject, store } from '../lib/store'
import { codeFontStack } from '../lib/appearance'
import { canMeasure, createFitScheduler } from '../lib/termFit'
import { attachTermClipboard } from '../lib/termClipboard'
import { termMenuItems, type TermMenuAction } from '../lib/termMenu'
import { attachOscBackground } from '../lib/termGlass'
import { attachSynchronizedOutput, createRefreshGate } from '../lib/termSync'
import { glassRgba } from '../lib/uiGlass'
import ToolMark from './ToolMark.vue'
import { TOOLS, type ToolId } from '../lib/types'

type Host = {
  term: Terminal
  fit: FitAddon
  el: HTMLDivElement
  offData: { dispose: () => void }
  offSurface: { dispose: () => void }
  offClipboard: { dispose: () => void }
  lastCols: number
  lastRows: number
  lastW: number
  lastH: number
}

const hosts = new Map<string, Host>()
let unlistenData: UnlistenFn | undefined
let unlistenExit: UnlistenFn | undefined
let observer: ResizeObserver | undefined
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
    const text = await api.clipboardRead()
    if (text) host.term.paste(text)
    return
  }
  if (action === 'selectAll') host.term.selectAll()
}

function fitAll() {
  const id = store.activePtyId
  if (!id) return
  const host = hosts.get(id)
  if (host) fitHost(id, host, true)
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

function theme() {
  const ink = cssVar('--ad-ink', '#0b0f13')
  const foreground = cssVar('--ad-text', '#ececec')
  const muted = cssVar('--ad-muted', '#8a8a8a')
  const opacity = Number.parseFloat(cssVar('--ad-ui-opacity', '0')) || 0
  return {
    background: glassRgba(ink, opacity),
    foreground,
    cursor: foreground,
    cursorAccent: ink,
    selectionBackground: '#ffffff22',
    black: ink,
    red: cssVar('--ad-error', '#e24b4a'),
    green: cssVar('--ad-success', '#3d9a6a'),
    yellow: cssVar('--ad-warning', '#c9a227'),
    blue: cssVar('--ad-dsh', '#4f7cff'),
    magenta: cssVar('--ad-grok', '#a78bfa'),
    cyan: cssVar('--ad-pi', '#22d3ee'),
    white: foreground,
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

function applyTermChrome() {
  hosts.forEach((host) => {
    host.term.options.fontFamily = termFontFamily()
    host.term.options.theme = theme()
  })
  if (store.activePtyId) show(store.activePtyId)
}

function ensureHost(ptyId: string) {
  if (hosts.has(ptyId)) return hosts.get(ptyId)!
  const el = document.createElement('div')
  el.className = 'term-host'
  if (ptyId !== store.activePtyId) el.classList.add('is-hidden')
  const mount = document.getElementById('term-mount')
  mount?.appendChild(el)
  const term = new Terminal({
    fontFamily: termFontFamily(),
    fontSize: props.fontSize,
    theme: theme(),
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
  const offData = term.onData((data) => {
    void api.ptyWrite(ptyId, data)
  })
  const offSurface = bindTermSurface(term, el, ptyId)
  const offClipboard = attachTermClipboard(term, el, {
    write: (text) => {
      void api.clipboardWrite(text)
    },
    mac: isMac
  })
  const host = { term, fit, el, offData, offSurface, offClipboard, lastCols: 0, lastRows: 0, lastW: 0, lastH: 0 }
  hosts.set(ptyId, host)
  return host
}

function bindTermSurface(term: Terminal, el: HTMLDivElement, ptyId: string) {
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
    ink: () => cssVar('--ad-ink', '#0b0f13')
  })
  const offScroll = term.onScroll(() => gate.request())
  const onPointer = () => gate.request()
  el.addEventListener('mousedown', onPointer)
  const offFocus = term.onSelectionChange(() => gate.request())
  return {
    dispose() {
      offSync.dispose()
      offOsc.dispose()
      offScroll.dispose()
      offFocus.dispose()
      el.removeEventListener('mousedown', onPointer)
      gate.dispose()
    }
  }
}

function fitHost(ptyId: string, host: Host, force = false) {
  if (!force && isLayoutBusy()) return
  const width = host.el.clientWidth
  const height = host.el.clientHeight
  if (!canMeasure(width, height)) return
  if (!force && width === host.lastW && height === host.lastH) return
  try {
    const proposed = host.fit.proposeDimensions()
    if (!proposed || proposed.cols < 20 || proposed.rows < 8) return
    host.fit.fit()
    host.term.refresh(0, Math.max(0, host.term.rows - 1))
    host.lastW = width
    host.lastH = height
    const cols = host.term.cols
    const rows = host.term.rows
    if (cols === host.lastCols && rows === host.lastRows) return
    host.lastCols = cols
    host.lastRows = rows
    void api.ptyResize(ptyId, cols, rows)
  } catch {
    /* mount not measured yet */
  }
}

function fitActive(force = false) {
  const id = store.activePtyId
  if (!id) return
  const host = hosts.get(id)
  if (host) fitHost(id, host, force)
}

function show(ptyId: string | '') {
  hosts.forEach((host, id) => {
    host.el.classList.toggle('is-hidden', id !== ptyId)
  })
  const host = ptyId ? hosts.get(ptyId) : undefined
  if (!host) return
  nextTick(() => {
    requestAnimationFrame(() => {
      fitHost(ptyId, host, true)
      host.term.focus()
    })
  })
}

function disposeHost(ptyId: string) {
  const host = hosts.get(ptyId)
  if (!host) return
  host.offData.dispose()
  host.offSurface.dispose()
  host.offClipboard.dispose()
  host.term.dispose()
  host.el.remove()
  hosts.delete(ptyId)
}

watch(
  () => store.activePtyId,
  (id) => {
    if (id && !hosts.has(id)) ensureHost(id)
    show(id)
  }
)

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
})

watch(windowMoving, (moving, was) => {
  if (was && !moving) fitActive(true)
})

onMounted(async () => {
  if (api.isTauri) {
    unlistenData = await listen<{ ptyId: string; data: string }>('pty-data', (event) => {
      const host = ensureHost(event.payload.ptyId)
      host.term.write(event.payload.data)
      if (isSignificantPtyChunk(event.payload.data)) notePtyData(event.payload.ptyId)
    })
    unlistenExit = await listen<{ ptyId: string }>('pty-exit', (event) => {
      disposeHost(event.payload.ptyId)
      markPtyExit(event.payload.ptyId)
    })
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
  window.addEventListener('keydown', onTermMenuKey)
})

function onTermMenuKey(event: KeyboardEvent) {
  if (event.key === 'Escape') closeTermMenu()
}

onUnmounted(() => {
  window.removeEventListener('ad-appearance', applyTermChrome)
  window.removeEventListener('click', closeTermMenu)
  window.removeEventListener('blur', closeTermMenu)
  window.removeEventListener('keydown', onTermMenuKey)
  unlistenData?.()
  unlistenExit?.()
  observer?.disconnect()
  fitScheduler.dispose()
  hosts.forEach((_, id) => disposeHost(id))
})

defineExpose({ ensureHost, show, dispose: disposeHost, fitActive })
</script>

<template>
  <section ref="workspaceEl" class="workspace" @contextmenu="onTermMenu">
    <div v-show="store.activePtyId" id="term-mount" class="mount" />
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
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.hero {
  width: min(780px, 100%);
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.mark {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  margin-bottom: 20px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 50%;
  color: var(--ad-muted);
  font-family: var(--ad-mono);
  font-size: 18px;
}

.empty-title {
  margin: 0 0 36px;
  max-width: 28ch;
  font-family: var(--ad-display);
  font-size: 28px;
  line-height: 36px;
  font-weight: 500;
  letter-spacing: -0.02em;
}

.tiles {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
  width: 100%;
}

.tile {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  min-height: 112px;
  padding: 20px 12px 16px;
  border: 1px solid transparent;
  border-radius: 18px;
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

@media (max-width: 900px) {
  .tiles {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
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
  contain: layout style;
  background: var(--ad-editor);
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
  overflow: hidden;
}

.term-host .xterm-bg-0 {
  background-color: var(--ad-editor) !important;
}

.term-host .xterm-viewport {
  background-color: var(--ad-editor);
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.16) transparent;
}

.term-host .xterm-viewport::-webkit-scrollbar {
  width: 8px;
}

.term-host .xterm-viewport::-webkit-scrollbar-track {
  background: transparent;
}

.term-host .xterm-viewport::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.16);
  border-radius: 999px;
}
</style>
