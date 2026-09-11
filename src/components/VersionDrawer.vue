<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import type { ToolId, ToolVersionInfo } from '../lib/types'
import { TOOLS, toolLabel } from '../lib/types'
import {
  UI_OPACITY_MAX,
  UI_OPACITY_MIN,
  applyUiOpacity,
  clampUiOpacity,
  saveAppSettings,
  store
} from '../lib/store'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
  changed: []
}>()

const INSTALL_COMMANDS = `# OpenCode
npm i -g opencode-ai@latest
# Kimi (macOS / Linux)
curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash
# Kimi (npm)
npm i -g @moonshot-ai/kimi-code@latest
# Grok
grok update`

const appVer = ref('')
const versions = ref<ToolVersionInfo[]>(seedRows())
const checking = ref(false)
const upgrading = ref<ToolId | ''>('')
const upgradingAll = ref(false)
const versionError = ref('')
const upgradeLog = ref('')
const commandsOpen = ref(false)
const copied = ref(false)

const busy = computed(() => checking.value || !!upgrading.value || upgradingAll.value)

const updatable = computed(() =>
  versions.value.filter((row) => row.compare === '可更新' && canStart(row))
)

function seedRows(): ToolVersionInfo[] {
  return TOOLS.map((tool) => ({
    toolId: tool.id,
    name: tool.label,
    found: false,
    localVersion: '检测中',
    latestVersion: '检测中',
    compare: '检测中'
  }))
}

function mergeRows(next: ToolVersionInfo[]) {
  versions.value = TOOLS.map((tool) => {
    return (
      next.find((row) => row.toolId === tool.id) ?? {
        toolId: tool.id,
        name: tool.label,
        found: false,
        localVersion: '查询失败',
        latestVersion: '查询失败',
        compare: '无法对比'
      }
    )
  })
}

watch(
  () => props.open,
  (open) => {
    if (!open) return
    versionError.value = ''
    upgradeLog.value = ''
    copied.value = false
    commandsOpen.value = false
    versions.value = seedRows()
    void loadAppVersion()
    void checkVersions()
  }
)

async function loadAppVersion() {
  try {
    appVer.value = await api.appVersion()
  } catch {
    appVer.value = '未知'
  }
}

async function checkVersions() {
  checking.value = true
  versionError.value = ''
  try {
    mergeRows(await api.listToolVersions())
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    mergeRows([])
  } finally {
    checking.value = false
  }
}

function displayVer(value: string) {
  return value && value !== '—' ? value : '-'
}

function compareClass(label: string) {
  if (label === '检测中') return ''
  if (label === '可更新') return 'ad-tag--warn'
  if (label === '已是最新') return 'ad-tag--ok'
  if (label === '新于远端') return 'ad-tag--info'
  return ''
}

function actionLabel(row: ToolVersionInfo) {
  if (upgrading.value === row.toolId) return row.found ? '升级中…' : '安装中…'
  if (row.compare === '检测中') return '检测中'
  return row.found ? '升级' : '安装'
}

function canStart(row: ToolVersionInfo) {
  if (row.compare === '检测中') return false
  if (row.toolId === 'grokbuild' && !row.found) return false
  return true
}

function actionTitle(row: ToolVersionInfo) {
  if (row.compare === '检测中') return '正在读取版本'
  if (row.toolId === 'grokbuild' && !row.found) return '未安装 Grok，请用官网安装包'
  if (!row.found) return '安装到最新版本'
  if (row.compare === '已是最新') return '再跑一遍升级'
  return '升级到最新版本'
}

async function runOne(row: ToolVersionInfo) {
  const result = await api.upgradeTool(row.toolId)
  const name = toolLabel(row.toolId)
  const text = result.log.trim() || (result.ok ? `${name} 完成。` : `${name} 失败，没有输出。`)
  upgradeLog.value = upgradeLog.value ? `${upgradeLog.value}\n${text}` : text
  if (!result.ok) {
    versionError.value = `${name} ${row.found ? '升级' : '安装'}失败`
  }
  return result.ok
}

async function upgrade(row: ToolVersionInfo) {
  if (busy.value || !canStart(row)) return
  upgrading.value = row.toolId
  versionError.value = ''
  upgradeLog.value = ''
  try {
    await runOne(row)
    await checkVersions()
    emit('changed')
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    upgradeLog.value = versionError.value
  } finally {
    upgrading.value = ''
  }
}

async function upgradeAll() {
  if (busy.value || !updatable.value.length) return
  upgradingAll.value = true
  versionError.value = ''
  upgradeLog.value = ''
  try {
    for (const row of updatable.value) {
      upgrading.value = row.toolId
      await runOne(row)
    }
    await checkVersions()
    emit('changed')
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    upgradeLog.value = versionError.value
  } finally {
    upgrading.value = ''
    upgradingAll.value = false
  }
}

function onOpacityInput(event: Event) {
  const value = clampUiOpacity(Number((event.target as HTMLInputElement).value))
  store.settings.uiOpacity = value
  applyUiOpacity(value)
}

async function onOpacityCommit(event: Event) {
  const uiOpacity = clampUiOpacity(Number((event.target as HTMLInputElement).value))
  applyUiOpacity(uiOpacity)
  if (!api.isTauri) {
    store.settings.uiOpacity = uiOpacity
    return
  }
  try {
    await saveAppSettings({ ...store.settings, uiOpacity })
  } catch {
    applyUiOpacity(store.settings.uiOpacity)
  }
}

async function copyCommands() {
  try {
    await navigator.clipboard.writeText(INSTALL_COMMANDS)
    copied.value = true
    window.setTimeout(() => {
      copied.value = false
    }, 2000)
  } catch {
    versionError.value = '复制失败，请手动选中命令'
  }
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="设置">
      <header>
        <div>
          <h2>设置</h2>
          <p class="app-ver">Agent Dock <strong>{{ appVer || '…' }}</strong></p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <label class="opacity">
        <span>界面透明度</span>
        <input
          type="range"
          :min="UI_OPACITY_MIN"
          :max="UI_OPACITY_MAX"
          :value="store.settings.uiOpacity"
          @input="onOpacityInput"
          @change="onOpacityCommit"
        />
        <span class="opacity-val">{{ store.settings.uiOpacity }}%</span>
      </label>
      <p class="hint">数字越大，越能透过窗口看到桌面。终端区域保持不透明。</p>

      <div class="toolbar">
        <p class="hint">对比本机与最新版本。OpenCode / Kimi 用 npm，Grok 用自身 update。</p>
        <div class="toolbar-actions">
          <button type="button" class="btn btn-ghost btn-small" :disabled="busy" @click="checkVersions">
            {{ checking ? '刷新中…' : '刷新' }}
          </button>
          <button
            type="button"
            class="btn btn-primary btn-small"
            :disabled="busy || !updatable.length"
            @click="upgradeAll"
          >
            {{ upgradingAll ? '升级中…' : '全部升级' }}
          </button>
        </div>
      </div>
      <p v-if="!api.isTauri" class="hint">浏览器预览无法检查本机 CLI，请在桌面端使用。</p>
      <p v-if="versionError" class="field-error">{{ versionError }}</p>

      <ul class="tools">
        <li v-for="row in versions" :key="row.toolId" class="tool">
          <div class="tool-top">
            <div class="tool-name">{{ toolLabel(row.toolId) }}</div>
            <span :class="['ad-tag', compareClass(row.compare)]">{{ row.compare }}</span>
          </div>
          <div v-if="row.path" class="tool-path" :title="row.path">{{ row.path }}</div>
          <div class="tool-meta">
            <div>当前 <strong>{{ displayVer(row.localVersion) }}</strong></div>
            <div>最新 <strong>{{ displayVer(row.latestVersion) }}</strong></div>
          </div>
          <button
            type="button"
            class="btn btn-ghost"
            :disabled="busy || !canStart(row)"
            :title="actionTitle(row)"
            @click="upgrade(row)"
          >
            {{ actionLabel(row) }}
          </button>
        </li>
      </ul>
      <pre v-if="upgradeLog" class="log">{{ upgradeLog }}</pre>

      <button type="button" class="fold" :aria-expanded="commandsOpen" @click="commandsOpen = !commandsOpen">
        <span>{{ commandsOpen ? '▾' : '▸' }} 手动安装命令</span>
      </button>
      <div v-if="commandsOpen" class="commands">
        <pre>{{ INSTALL_COMMANDS }}</pre>
        <button type="button" class="btn btn-ghost btn-small" @click="copyCommands">
          {{ copied ? '已复制' : '复制' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 880px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  overflow: auto;
}

header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

h2 {
  margin: 0;
  font-size: 16px;
  line-height: 24px;
}

.app-ver {
  margin: 4px 0 0;
  color: var(--ad-muted);
  font-size: 12px;
  line-height: 20px;
}

.app-ver strong {
  color: var(--ad-text);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.opacity {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  font-size: 13px;
  color: var(--ad-muted);
}

.opacity input {
  flex: 1;
  min-width: 0;
}

.opacity-val {
  width: 40px;
  text-align: right;
  color: var(--ad-text);
  font-variant-numeric: tabular-nums;
}

.toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.tools {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
}

.tool {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 164px;
  padding: 16px;
  background: var(--ad-hover);
  border: 1px solid var(--ad-border);
  border-radius: var(--ad-radius-card);
}

.tool-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.tool-name {
  font-weight: 600;
}

.tool-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.tool-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.tool-meta strong {
  color: var(--ad-text);
  font-family: var(--ad-mono);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.tool .btn {
  margin-top: auto;
  width: 100%;
}

.log {
  margin: 0 0 16px;
  padding: 12px;
  max-height: 140px;
  overflow: auto;
  background: var(--ad-harbor);
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  color: var(--ad-muted);
  font-family: var(--ad-mono);
  font-size: 12px;
  line-height: 18px;
  white-space: pre-wrap;
}

.fold {
  padding: 0;
  color: var(--ad-text);
  font-size: 13px;
  line-height: 22px;
}

.fold:hover {
  color: var(--ad-accent);
}

.commands {
  margin-top: 8px;
}

.commands pre {
  margin: 0 0 8px;
  padding: 12px;
  background: var(--ad-harbor);
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  color: var(--ad-muted);
  font-family: var(--ad-mono);
  font-size: 12px;
  line-height: 18px;
  white-space: pre-wrap;
}
</style>
