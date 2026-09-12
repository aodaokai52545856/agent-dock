<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import type { ToolId, ToolVersionInfo } from '../lib/types'
import { TOOL_OFFICIAL_URLS, toolLabel } from '../lib/types'
import {
  isVersionChecking,
  markVersionChecking,
  mergeVersionRows,
  patchVersionRow,
  seedVersionRows
} from '../lib/toolVersions'
import { store } from '../lib/store'
import ConfirmDialog from './ConfirmDialog.vue'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
  changed: []
}>()

const INSTALL_COMMANDS = `# Claude Code (Windows)
irm https://claude.ai/install.ps1 | iex
# Claude Code (macOS / Linux)
curl -fsSL https://claude.ai/install.sh | bash
# OpenCode
npm i -g opencode-ai@latest
# Kimi (macOS / Linux)
curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash
# Kimi (Windows)
irm https://code.kimi.com/kimi-code/install.ps1 | iex
# Grok
# https://github.com/xai-org/grok-build#installation
# Pi
npm i -g --ignore-scripts @earendil-works/pi-coding-agent
# DeepSeek Harness（安装后同时有 dsh 与 dsh web）
# 需要 Node 22.18 或更高。22.14 上执行 dsh web 会直接退出，3080 也打不开。
npm i -g @deepseek-ai/dsh
# 交互界面（Agent Dock 默认嵌入当前编辑区）
dsh web
# 单次任务
dsh --profile headless "任务"`

const appVer = ref('')
const versions = ref<ToolVersionInfo[]>(seedVersionRows())
const checkingAll = ref(false)
const checkingId = ref<ToolId | ''>('')
const upgrading = ref<ToolId | ''>('')
const uninstalling = ref<ToolId | ''>('')
const upgradingAll = ref(false)
const versionError = ref('')
const upgradeLog = ref('')
const commandsOpen = ref(false)
const copied = ref(false)
const proxyUrl = ref('')
const pendingUninstall = ref<ToolVersionInfo | null>(null)

const busy = computed(
  () =>
    checkingAll.value ||
    !!checkingId.value ||
    !!upgrading.value ||
    upgradingAll.value ||
    !!uninstalling.value
)

const updatable = computed(() =>
  versions.value.filter((row) => row.compare === '可更新' && canStart(row))
)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    versionError.value = ''
    upgradeLog.value = ''
    copied.value = false
    commandsOpen.value = false
    pendingUninstall.value = null
    checkingAll.value = false
    checkingId.value = ''
    versions.value = seedVersionRows()
    proxyUrl.value = store.settings.defaultProxyUrl || ''
    void loadAppVersion()
  }
)

async function loadAppVersion() {
  try {
    appVer.value = await api.appVersion()
  } catch {
    appVer.value = '未知'
  }
}

function proxyArg() {
  return proxyUrl.value.trim() || undefined
}

async function checkVersions(force = false) {
  if (!force && busy.value) return
  checkingAll.value = true
  versionError.value = ''
  versions.value = versions.value.map(markVersionChecking)
  try {
    versions.value = mergeVersionRows(await api.listToolVersions(proxyArg()))
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    versions.value = mergeVersionRows([])
  } finally {
    checkingAll.value = false
  }
}

async function checkOne(toolId: ToolId, force = false) {
  if (!force && busy.value) return
  const current = versions.value.find((row) => row.toolId === toolId)
  if (!current) return
  checkingId.value = toolId
  versionError.value = ''
  versions.value = patchVersionRow(versions.value, markVersionChecking(current))
  try {
    const next = await api.listToolVersions(proxyArg(), toolId)
    const row = next.find((item) => item.toolId === toolId)
    if (row) versions.value = patchVersionRow(versions.value, row)
    else throw new Error('没有返回这项的检测结果')
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    versions.value = patchVersionRow(versions.value, {
      ...current,
      found: false,
      localVersion: '查询失败',
      latestVersion: '查询失败',
      compare: '无法对比',
      checks: []
    })
  } finally {
    checkingId.value = ''
  }
}

function displayVer(value: string) {
  return value && value !== '—' ? value : '-'
}

function compareClass(label: string) {
  if (label === '检测中' || label === '未检测') return ''
  if (label === '可更新' || label === '未通过检查') return 'ad-tag--warn'
  if (label === '已是最新') return 'ad-tag--ok'
  if (label === '新于远端') return 'ad-tag--info'
  return ''
}

function checksPass(row: ToolVersionInfo) {
  return !row.checks?.length || row.checks.every((item) => item.ok)
}

function isInstalled(row: ToolVersionInfo) {
  return (
    row.found &&
    checksPass(row) &&
    row.localVersion !== '未安装' &&
    row.compare !== '未安装' &&
    row.compare !== '未通过检查'
  )
}

function actionLabel(row: ToolVersionInfo) {
  if (upgrading.value === row.toolId) return isInstalled(row) ? '升级中…' : '安装中…'
  return isInstalled(row) ? '升级' : '安装'
}

function canStart(row: ToolVersionInfo) {
  if (isVersionChecking(row.compare)) return false
  if (row.toolId === 'grokbuild' && !row.found) return false
  return true
}

function actionTitle(row: ToolVersionInfo) {
  if (isVersionChecking(row.compare)) return '正在读取版本'
  if (row.toolId === 'grokbuild' && !isInstalled(row)) return '未安装 Grok，请打开官网安装包'
  if (row.compare === '未通过检查') return 'Node、dsh、dsh web 都要通过才算安装成功'
  if (!isInstalled(row)) return '按官方方式安装到本机'
  if (row.compare === '已是最新') return '再跑一遍升级'
  return '升级到最新版本'
}

function isRowChecking(row: ToolVersionInfo) {
  return checkingAll.value || checkingId.value === row.toolId || isVersionChecking(row.compare)
}

async function runOne(row: ToolVersionInfo) {
  const result = await api.upgradeTool(row.toolId, proxyUrl.value.trim() || undefined)
  const name = toolLabel(row.toolId)
  const text = result.log.trim() || (result.ok ? `${name} 完成。` : `${name} 失败，没有输出。`)
  upgradeLog.value = upgradeLog.value ? `${upgradeLog.value}\n${text}` : text
  if (!result.ok) {
    versionError.value = `${name} ${isInstalled(row) ? '升级' : '安装'}失败`
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
    await checkOne(row.toolId, true)
    emit('changed')
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    upgradeLog.value = versionError.value
  } finally {
    upgrading.value = ''
  }
}

function liveCount(toolId: ToolId) {
  return store.live.filter((item) => item.toolId === toolId && item.alive).length
}

function uninstallTitle(row: ToolVersionInfo) {
  return `卸载 ${toolLabel(row.toolId)}`
}

function uninstallBody(row: ToolVersionInfo) {
  const name = toolLabel(row.toolId)
  const running = liveCount(row.toolId)
  const base =
    row.toolId === 'kimi'
      ? '按官方方式卸载命令行：脚本安装删除 kimi 可执行文件；若还有 npm 安装会执行 npm uninstall -g @moonshot-ai/kimi-code。会话和配置留在 ~/.kimi-code，不会删。'
      : `只会删除本机上的 ${name} 命令行，会话记录和配置会保留。之后可以再从这里安装。`
  if (running) {
    return `当前有 ${running} 个正在运行的 ${name} 会话。${base}这些终端可能会立刻失败。`
  }
  return base
}

function askUninstall(row: ToolVersionInfo) {
  if (busy.value || !isInstalled(row)) return
  pendingUninstall.value = row
}

async function runUninstall(row: ToolVersionInfo) {
  const result = await api.uninstallTool(row.toolId)
  const name = toolLabel(row.toolId)
  const text = result.log.trim() || (result.ok ? `${name} 已卸载。` : `${name} 卸载失败，没有输出。`)
  upgradeLog.value = upgradeLog.value ? `${upgradeLog.value}\n${text}` : text
  if (!result.ok) {
    versionError.value = `${name} 卸载失败`
  }
  return result.ok
}

async function confirmUninstall() {
  const row = pendingUninstall.value
  pendingUninstall.value = null
  if (!row || busy.value) return
  uninstalling.value = row.toolId
  versionError.value = ''
  upgradeLog.value = ''
  try {
    await runUninstall(row)
    await checkOne(row.toolId, true)
    emit('changed')
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    upgradeLog.value = versionError.value
  } finally {
    uninstalling.value = ''
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
    await checkVersions(true)
    emit('changed')
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
    upgradeLog.value = versionError.value
  } finally {
    upgrading.value = ''
    upgradingAll.value = false
  }
}

function openOfficial(toolId: ToolId) {
  void api.openExternal(TOOL_OFFICIAL_URLS[toolId])
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
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="版本">
      <header>
        <div>
          <h2>版本</h2>
          <p class="app-ver">Agent Dock <strong>{{ appVer || '…' }}</strong></p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <label class="field">
        <span>下载代理</span>
        <input v-model="proxyUrl" type="text" placeholder="留空则直连，例如 http://127.0.0.1:7890" />
      </label>
      <p class="hint">安装和升级时写入 HTTPS_PROXY。卸载只删除可执行文件，会话和配置会保留。</p>

      <div class="toolbar">
        <p class="hint">打开后不会自动检测。请点全部检测，或在某一项上单独检测。DeepSeek 要 Node、dsh、dsh web 都通过才算安装成功。</p>
        <div class="toolbar-actions">
          <button type="button" class="btn btn-ghost btn-small" :disabled="busy" @click="checkVersions()">
            {{ checkingAll ? '检测中…' : '全部检测' }}
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
          <ul v-if="row.checks?.length" class="checks">
            <li v-for="item in row.checks" :key="item.name" :class="{ ok: item.ok, bad: !item.ok }">
              <strong>{{ item.ok ? '通过' : '未通过' }}</strong>
              {{ item.name }}
              <span>{{ item.detail }}</span>
            </li>
          </ul>
          <button type="button" class="official" @click="openOfficial(row.toolId)">
            官方地址
          </button>
          <div class="tool-meta">
            <div>当前 <strong>{{ displayVer(row.localVersion) }}</strong></div>
            <div>最新 <strong>{{ displayVer(row.latestVersion) }}</strong></div>
          </div>
          <div class="tool-actions">
            <button
              type="button"
              class="btn btn-ghost"
              :disabled="busy"
              :title="isRowChecking(row) ? '正在检测' : '只检测这一项'"
              @click="checkOne(row.toolId)"
            >
              {{ isRowChecking(row) ? '检测中…' : '检测' }}
            </button>
            <button
              type="button"
              class="btn btn-ghost"
              :disabled="busy || !canStart(row)"
              :title="actionTitle(row)"
              @click="upgrade(row)"
            >
              {{ actionLabel(row) }}
            </button>
            <button
              v-if="isInstalled(row)"
              type="button"
              class="btn btn-ghost btn-uninstall"
              :disabled="busy"
              title="卸载本机命令行，保留会话和配置"
              @click="askUninstall(row)"
            >
              {{ uninstalling === row.toolId ? '卸载中…' : '卸载' }}
            </button>
          </div>
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
    <ConfirmDialog
      :open="!!pendingUninstall"
      :title="pendingUninstall ? uninstallTitle(pendingUninstall) : ''"
      :body="pendingUninstall ? uninstallBody(pendingUninstall) : ''"
      action="卸载"
      danger
      @close="pendingUninstall = null"
      @confirm="confirmUninstall"
    />
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

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 8px;
  font-size: 13px;
  color: var(--ad-muted);
}

.field input {
  height: 32px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  background: var(--ad-harbor);
  color: var(--ad-text);
}

.official {
  align-self: flex-start;
  margin: 4px 0 8px;
  padding: 0;
  color: var(--ad-muted);
  font-size: 12px;
  text-decoration: underline;
  text-underline-offset: 2px;
}

.official:hover {
  color: var(--ad-text);
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
  min-height: 196px;
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

.checks {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.checks strong {
  margin-right: 6px;
  font-weight: 600;
}

.checks .ok strong {
  color: var(--ad-success);
}

.checks .bad strong {
  color: var(--ad-error);
}

.checks span {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
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

.tool-actions {
  display: flex;
  gap: 8px;
  margin-top: auto;
}

.tool-actions .btn {
  flex: 1;
  width: auto;
}

.btn-uninstall {
  flex: 0 0 auto;
  min-width: 72px;
  color: var(--ad-error);
  border-color: color-mix(in srgb, var(--ad-error) 40%, var(--ad-border));
}

.btn-uninstall:hover:not(:disabled) {
  background: color-mix(in srgb, var(--ad-error) 12%, transparent);
}

:deep(.ad-mask) {
  z-index: 26;
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
