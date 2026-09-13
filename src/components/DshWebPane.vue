<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  DSH_STATUS_BAR,
  DSH_TOAST_SELECTOR,
  clampDshEmbedBounds,
  dshEmbedBlocked,
  toastOverlayTop
} from '../lib/dshEmbed'
import { dshGlassApplyScript, dshGlassTheme, withDshGlassHash } from '../lib/dshGlass'
import { docRailPaneWidth, layout } from '../lib/layout'
import { glassRgba } from '../lib/uiGlass'
import { resumeDshEmbed, store } from '../lib/store'

const props = defineProps<{
  url: string
}>()

const hostRef = ref<HTMLElement | null>(null)
const error = ref('')

const theme = computed(() => dshGlassTheme(store.settings))
const surface = computed(() => glassRgba(theme.value.bg, store.settings.uiOpacity))

function measure() {
  const el = hostRef.value
  if (!el) return null
  const box = el.getBoundingClientRect()
  const status = document.querySelector('footer.bar')
  const statusBarHeight = status
    ? Math.round(status.getBoundingClientRect().height)
    : DSH_STATUS_BAR
  const toast = document.querySelector(DSH_TOAST_SELECTOR)
  return clampDshEmbedBounds(box, {
    windowWidth: window.innerWidth,
    windowHeight: window.innerHeight,
    docRailWidth: layout.docRailCollapsed ? 0 : docRailPaneWidth(),
    maximized: document.documentElement.classList.contains('ad-maximized'),
    statusBarHeight,
    overlayTop: toastOverlayTop(toast)
  })
}

function masked() {
  if (typeof document === 'undefined') return false
  const bounds = measure()
  const embed = bounds
    ? { left: bounds.x, top: bounds.y, width: bounds.width, height: bounds.height }
    : null
  return dshEmbedBlocked(document, embed)
}

let syncing = false
let queued: { navigate?: boolean } | undefined
let hasQueue = false
let released = false

function embedStopped() {
  return released || store.dshEmbedPaused
}

async function releaseEmbed() {
  await api.dshEmbedClose()
}

async function sync(opts?: { navigate?: boolean }) {
  if (embedStopped()) return
  queued = opts
  hasQueue = true
  if (syncing) return
  syncing = true
  try {
    while (hasQueue) {
      hasQueue = false
      const current = queued
      queued = undefined
      await syncOnce(current)
    }
  } finally {
    syncing = false
  }
}

async function syncOnce(opts?: { navigate?: boolean }) {
  if (!api.isTauri) return
  if (embedStopped()) {
    await releaseEmbed()
    return
  }
  const bounds = measure()
  try {
    if (!props.url || !bounds || masked()) {
      await api.dshEmbedSetVisible(false)
      return
    }
    if (opts?.navigate === false) {
      await api.dshEmbedSetBounds(bounds)
      await api.dshEmbedSetVisible(true)
    } else {
      await api.dshEmbedOpen(
        withDshGlassHash(props.url, theme.value),
        bounds,
        dshGlassApplyScript(theme.value)
      )
    }
    if (embedStopped()) {
      await releaseEmbed()
      return
    }
    error.value = ''
    await pushTheme()
  } catch (err) {
    if (embedStopped()) {
      await releaseEmbed()
      return
    }
    error.value = err instanceof Error ? err.message : String(err)
  }
}

async function pushTheme() {
  if (!api.isTauri || !props.url) return
  try {
    await api.dshEmbedApplyTheme(dshGlassApplyScript(theme.value))
  } catch {
    /* webview may not exist yet */
  }
}

function onResize() {
  void sync({ navigate: false })
}

let observer: ResizeObserver | undefined
let maskObserver: MutationObserver | undefined
let offResized: (() => void) | undefined

onMounted(() => {
  released = false
  resumeDshEmbed()
  observer = new ResizeObserver(onResize)
  if (hostRef.value) observer.observe(hostRef.value)
  window.addEventListener('resize', onResize)
  maskObserver = new MutationObserver(onResize)
  maskObserver.observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ['class', 'style']
  })
  if (api.isTauri) {
    void getCurrentWindow()
      .onResized(() => onResize())
      .then((off) => {
        offResized = off
      })
  }
  void sync()
})

onUnmounted(() => {
  released = true
  observer?.disconnect()
  maskObserver?.disconnect()
  offResized?.()
  window.removeEventListener('resize', onResize)
  void releaseEmbed()
})

watch(
  () => props.url,
  () => {
    void sync()
  }
)

watch(theme, () => {
  void pushTheme()
})

watch(
  () => [layout.docRailCollapsed, layout.docRailWidth, layout.sidebarWidth, layout.sidebarCollapsed] as const,
  () => {
    void sync({ navigate: false })
  }
)

watch(
  () => store.toast,
  () => {
    void sync({ navigate: false })
  }
)
</script>

<template>
  <div ref="hostRef" class="dsh-web" :style="{ background: surface, colorScheme: theme.scheme }">
    <p v-if="error" class="wait">{{ error }}</p>
    <p v-else-if="!url" class="wait">DeepSeek Web 还没有页面地址</p>
  </div>
</template>

<style scoped>
.dsh-web {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.wait {
  margin: 0;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--ad-muted);
}
</style>
