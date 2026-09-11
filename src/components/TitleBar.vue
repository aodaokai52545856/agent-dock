<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { onMounted, onUnmounted, ref } from 'vue'
import { isTauri } from '../lib/api'
import { setAppMode, store } from '../lib/store'

const emit = defineEmits<{
  settings: []
  accounts: []
}>()

const fileOpen = ref(false)
const maximized = ref(false)
let offResized: (() => void) | undefined

async function syncMaximized() {
  if (!isTauri) return
  const max = await getCurrentWindow().isMaximized()
  maximized.value = max
  document.documentElement.classList.toggle('ad-maximized', max)
}

async function minimize() {
  if (!isTauri) return
  await getCurrentWindow().minimize()
}

async function toggleMax() {
  if (!isTauri) return
  await getCurrentWindow().toggleMaximize()
  await syncMaximized()
}

async function closeWindow() {
  if (!isTauri) return
  await getCurrentWindow().close()
}

function toggleFile() {
  fileOpen.value = !fileOpen.value
}

function closeFile() {
  fileOpen.value = false
}

function openSettings() {
  closeFile()
  emit('settings')
}

function openAccounts() {
  closeFile()
  emit('accounts')
}

function onDocClick() {
  closeFile()
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') closeFile()
}

onMounted(() => {
  document.addEventListener('click', onDocClick)
  window.addEventListener('keydown', onKey)
  if (!isTauri) return
  void syncMaximized()
  void getCurrentWindow()
    .onResized(() => {
      void syncMaximized()
    })
    .then((unlisten) => {
      offResized = unlisten
    })
})

onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
  window.removeEventListener('keydown', onKey)
  offResized?.()
})
</script>

<template>
  <header class="titlebar">
    <div class="left">
      <span class="brand" data-tauri-drag-region>Agent Dock</span>
      <div class="file" @click.stop>
        <button
          type="button"
          class="file-btn"
          :class="{ 'is-open': fileOpen }"
          aria-haspopup="menu"
          :aria-expanded="fileOpen"
          @click="toggleFile"
        >
          文件
        </button>
        <div v-if="fileOpen" class="ad-menu file-menu" role="menu">
          <button type="button" class="ad-menu-item" role="menuitem" @click="openSettings">设置</button>
          <button type="button" class="ad-menu-item" role="menuitem" @click="openAccounts">Grok 账号</button>
        </div>
      </div>
      <div class="modes" role="tablist" aria-label="顶层模式">
        <button
          type="button"
          role="tab"
          class="mode"
          :class="{ 'is-on': store.appMode === 'console' }"
          :aria-selected="store.appMode === 'console'"
          @click="setAppMode('console')"
        >
          控制台
        </button>
        <button
          type="button"
          role="tab"
          class="mode"
          :class="{ 'is-on': store.appMode === 'bridge' }"
          :aria-selected="store.appMode === 'bridge'"
          @click="setAppMode('bridge')"
        >
          编排（施工中）
        </button>
      </div>
    </div>
    <div class="drag" data-tauri-drag-region @dblclick="toggleMax" />
    <div class="controls" role="group" aria-label="窗口控制">
      <button type="button" class="win" aria-label="最小化" @click="minimize">
        <svg viewBox="0 0 12 12" aria-hidden="true">
          <path d="M2 6h8" />
        </svg>
      </button>
      <button type="button" class="win" :aria-label="maximized ? '还原' : '最大化'" @click="toggleMax">
        <svg v-if="!maximized" viewBox="0 0 12 12" aria-hidden="true">
          <rect x="2" y="2" width="8" height="8" rx="0.6" />
        </svg>
        <svg v-else viewBox="0 0 12 12" aria-hidden="true">
          <path d="M3.6 2.6h5.2A.6.6 0 0 1 9.4 3.2V8.4" />
          <rect x="2.4" y="3.6" width="6.2" height="6.2" rx="0.5" />
        </svg>
      </button>
      <button type="button" class="win win-close" aria-label="关闭" @click="closeWindow">
        <svg viewBox="0 0 12 12" aria-hidden="true">
          <path d="M3 3l6 6M9 3l-6 6" />
        </svg>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  position: relative;
  z-index: 6;
  height: max(40px, calc(var(--ad-font) + 27px));
  flex-shrink: 0;
  display: flex;
  align-items: center;
  background: var(--ad-sidebar);
  border-bottom: 1px solid var(--ad-border);
  color: var(--ad-muted);
  font-size: var(--ad-font);
  font-family: var(--ad-sans);
  user-select: none;
}

.left {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  height: 100%;
  padding-left: 14px;
  gap: 2px;
}

.brand {
  font-size: var(--ad-font);
  line-height: 1.5;
  font-weight: 600;
  color: var(--ad-text);
  padding-right: 8px;
}

.file {
  position: relative;
  height: 100%;
  display: flex;
  align-items: center;
}

.file-btn {
  height: 100%;
  padding: 0 12px;
  font-size: var(--ad-font);
  line-height: 1.5;
  color: var(--ad-muted);
}

.file-btn:hover,
.file-btn.is-open {
  background: var(--ad-hover);
  color: var(--ad-text);
}

.file-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 50;
  min-width: 176px;
}

.modes {
  display: flex;
  align-items: center;
  height: calc(var(--ad-font) + 15px);
  margin-left: 8px;
  padding: 2px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
  background: var(--ad-harbor);
}

.mode {
  height: 100%;
  padding: 0 10px;
  border-radius: 6px;
  font-size: var(--ad-font-sm);
  color: var(--ad-muted);
  white-space: nowrap;
}

.mode:hover {
  color: var(--ad-text);
}

.mode.is-on {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.drag {
  flex: 1;
  height: 100%;
  min-width: 24px;
}

.controls {
  display: flex;
  height: 100%;
}

.win {
  width: 48px;
  height: 100%;
  display: grid;
  place-items: center;
  color: #cfcfcf;
  background: transparent;
}

.win svg {
  width: 12px;
  height: 12px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.15;
  stroke-linecap: square;
  stroke-linejoin: miter;
}

.win:hover {
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
}

.win-close:hover {
  background: #c42b1c;
  color: #fff;
}
</style>
