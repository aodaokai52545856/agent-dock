<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { onMounted, onUnmounted, ref } from 'vue'
import { isTauri } from '../lib/api'
import { beginWindowMove, endWindowMove, noteWindowMove } from '../lib/layout'
import { setAppMode, store } from '../lib/store'
import { isTitlebarDoubleClick } from '../lib/titleDrag'
import { versionCenter } from '../lib/versionService'
import ClockMenu from './ClockMenu.vue'

const emit = defineEmits<{
  versions: []
  accounts: []
  'claude-accounts': []
  'antigravity-accounts': []
  deepseek: []
  'ccswitch-open': []
  'ccswitch-install': []
  'antigravity-tools-open': []
  'antigravity-tools-install': []
}>()

const fileOpen = ref(false)
const externalToolsOpen = ref(false)
const externalToolsButton = ref<HTMLButtonElement | null>(null)
const clockBurst = ref(0)
const maximized = ref(false)
let offResized: (() => void) | undefined
let offMoved: (() => void) | undefined

async function dragWindow(event: MouseEvent) {
  if (event.button !== 0 || !isTauri) return
  if (isTitlebarDoubleClick(event)) {
    await toggleMax()
    return
  }
  beginWindowMove()
  await getCurrentWindow().startDragging()
}

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

function shutClock() {
  clockBurst.value += 1
}

function toggleFile() {
  const next = !fileOpen.value
  externalToolsOpen.value = false
  shutClock()
  fileOpen.value = next
}

function closeMenus() {
  fileOpen.value = false
  externalToolsOpen.value = false
  shutClock()
}

function toggleExternalTools() {
  const next = !externalToolsOpen.value
  fileOpen.value = false
  shutClock()
  externalToolsOpen.value = next
}

function closeExternalTools() {
  closeMenus()
  externalToolsButton.value?.focus()
}

function runExternalToolAction(action: () => void) {
  closeExternalTools()
  action()
}

function onClockOpen() {
  fileOpen.value = false
  externalToolsOpen.value = false
}

function openVersions() {
  closeMenus()
  emit('versions')
}

function openAccounts() {
  closeMenus()
  emit('accounts')
}

function openClaudeAccounts() {
  closeMenus()
  emit('claude-accounts')
}

function openAntigravityAccounts() {
  closeMenus()
  emit('antigravity-accounts')
}

function openDeepseek() {
  closeMenus()
  emit('deepseek')
}

function onDocClick() {
  closeMenus()
}

function onKey(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  if (externalToolsOpen.value) closeExternalTools()
  else closeMenus()
}

onMounted(() => {
  document.addEventListener('click', onDocClick)
  window.addEventListener('keydown', onKey)
  window.addEventListener('mouseup', endWindowMove)
  if (!isTauri) return
  void syncMaximized()
  const win = getCurrentWindow()
  void win
    .onResized(() => {
      void syncMaximized()
    })
    .then((unlisten) => {
      offResized = unlisten
    })
  void win.onMoved(noteWindowMove).then((unlisten) => {
    offMoved = unlisten
  })
})

onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('mouseup', endWindowMove)
  offResized?.()
  offMoved?.()
})
</script>

<template>
  <header class="titlebar" @mousedown="dragWindow">
    <div class="left">
      <span class="brand">Agent Dock</span>
      <div class="file" @click.stop @mousedown.stop>
        <button
          type="button"
          class="file-btn"
          :class="{ 'is-open': fileOpen }"
          aria-haspopup="menu"
          :aria-expanded="fileOpen"
          @click="toggleFile"
        >
          账号管理
        </button>
        <div v-if="fileOpen" class="ad-menu file-menu" role="menu">
          <button type="button" class="ad-menu-item" role="menuitem" @click="openAccounts">Grok 账号</button>
          <button type="button" class="ad-menu-item" role="menuitem" @click="openClaudeAccounts">Claude 账号</button>
          <button type="button" class="ad-menu-item" role="menuitem" @click="openAntigravityAccounts">Antigravity 账号</button>
          <button type="button" class="ad-menu-item" role="menuitem" @click="openDeepseek">DeepSeek</button>
        </div>
      </div>
      <button type="button" class="file-btn version-button" aria-label="版本" :title="versionCenter.running.value ? '后台升级进行中，点击查看进度' : '版本与更新'" @click="openVersions" @mousedown.stop>
        版本
        <span v-if="versionCenter.running.value" class="version-progress">{{ versionCenter.finished.value }}/{{ versionCenter.batch.value.length }}</span>
        <span v-else-if="versionCenter.appReady.value" class="version-progress">待安装</span>
        <span v-else-if="versionCenter.state.unread" class="version-unread" aria-label="有新的版本任务结果" />
      </button>
      <div class="file" @click.stop @mousedown.stop>
        <button
          type="button"
          ref="externalToolsButton"
          class="file-btn"
          :class="{ 'is-open': externalToolsOpen }"
          :aria-expanded="externalToolsOpen"
          @click="toggleExternalTools"
        >
          外部工具
        </button>
        <div v-if="externalToolsOpen" class="ad-menu file-menu">
          <div class="external-tool" role="group" aria-label="CC Switch">
            <span>CC Switch</span>
            <button type="button" class="file-btn" aria-label="打开 CC Switch" @click="runExternalToolAction(() => emit('ccswitch-open'))">打开</button>
            <button type="button" class="file-btn" aria-label="下载安装 CC Switch" @click="runExternalToolAction(() => emit('ccswitch-install'))">下载安装</button>
          </div>
          <div class="external-tool" role="group" aria-label="Antigravity Tools">
            <span>Antigravity Tools</span>
            <button type="button" class="file-btn" aria-label="打开 Antigravity Tools" @click="runExternalToolAction(() => emit('antigravity-tools-open'))">打开</button>
            <button type="button" class="file-btn" aria-label="下载安装 Antigravity Tools" @click="runExternalToolAction(() => emit('antigravity-tools-install'))">下载安装</button>
          </div>
        </div>
      </div>
      <div class="modes" role="tablist" aria-label="顶层模式" @mousedown.stop>
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
          编排
        </button>
        <button
          type="button"
          role="tab"
          class="mode"
          :class="{ 'is-on': store.appMode === 'usage' }"
          :aria-selected="store.appMode === 'usage'"
          @click="setAppMode('usage')"
        >
          使用量
        </button>
        <button
          type="button"
          role="tab"
          class="mode"
          :class="{ 'is-on': store.appMode === 'postTool' }"
          :aria-selected="store.appMode === 'postTool'"
          @click="setAppMode('postTool')"
        >
          postTool
        </button>
      </div>
    </div>
    <div class="drag" />
    <ClockMenu :burst="clockBurst" @open="onClockOpen" />
    <div class="controls" role="group" aria-label="窗口控制" @mousedown.stop>
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
  cursor: default;
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

.version-button { display: inline-flex; align-items: center; gap: 6px; }
.version-progress { padding: 0 5px; font-size: 10px; line-height: 18px; color: var(--ad-text); background: var(--ad-hover); border-radius: 4px; font-variant-numeric: tabular-nums; }
.version-unread { width: 5px; height: 5px; border-radius: 50%; background: var(--ad-warning); }

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

.external-tool {
  display: grid;
  grid-template-columns: minmax(120px, 1fr) auto auto;
  align-items: center;
  gap: 4px;
  padding: 4px 6px;
  white-space: nowrap;
}

.external-tool + .external-tool {
  border-top: 1px solid var(--ad-border);
}

.external-tool > span {
  padding: 0 6px;
  color: var(--ad-text);
  font-size: 12px;
  font-weight: 600;
}

.external-tool > button {
  min-height: 30px;
  padding-inline: 8px;
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
  cursor: default;
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
  color: var(--ad-muted);
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
  background: var(--ad-hover);
  color: var(--ad-text);
}

.win-close:hover {
  background: #c42b1c;
  color: #fff;
}

@media (max-width: 900px) {
  .left { padding-left: 8px; }
  .file-btn { padding-inline: 8px; }
  .brand { padding-right: 4px; }
  .drag { min-width: 8px; }
  .modes { margin-left: 4px; }
  .win { width: 40px; }
}

@media (max-width: 720px) {
  .brand { display: none; }
}
</style>
