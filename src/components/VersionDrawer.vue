<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { store, showToast } from '../lib/store'
import { toolLabel, TOOL_OFFICIAL_URLS, type ToolId, type ToolVersionInfo, type VersionTask } from '../lib/types'
import { canUpgradeTool, taskRunning, toolInstalled } from '../lib/versionCenter'
import { versionCenter } from '../lib/versionService'
import ToolMark from './ToolMark.vue'
import ConfirmDialog from './ConfirmDialog.vue'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: [] }>()
const { state, busy, running, batch, finished, active, updatable, appReady } = versionCenter
const tab = ref<'software' | 'tasks'>('software')
const expanded = ref<ToolId | ''>('')
const pendingUninstall = ref<ToolVersionInfo | null>(null)
const installConfirm = ref(false)
const helpOpen = ref(false)
const appIcon = new URL('../../src-tauri/icons/128x128.png', import.meta.url).href
const checking = computed(() => Object.values(state.checking).some(Boolean))
const history = computed(() => [...state.snapshot.tasks].reverse())
const failedCount = computed(() => batch.value.filter(task => task.status === 'failed').length)
const batchReady = computed(() => batch.value.some(task => task.status === 'ready'))
const progress = computed(() => active.value?.total
  ? Math.min(100, active.value.downloaded / active.value.total * 100)
  : batch.value.length ? finished.value / batch.value.length * 100 : 0)

watch(() => props.open, open => {
  if (open) versionCenter.open(store.settings.defaultProxyUrl || '')
  else { versionCenter.close(); pendingUninstall.value = null; installConfirm.value = false }
}, { immediate: true })

const lastTask = (target: string) => history.value.find(task => task.target === target)
const targetLabel = (target: string) => target === 'app' ? 'Agent Dock' : toolLabel(target as ToolId)
const taskLabel = (task: VersionTask) => ({ upgrade: '安装 / 升级', uninstall: '卸载', download: '下载更新', apply: '安装更新' })[task.action]
const bytes = (value: number) => `${(value / 1024 / 1024).toFixed(1)} MB`
const displayVersion = (value: string) => !value || value === '—' ? '—' : value
const taskTime = (task: VersionTask) => task.startedAt || task.finishedAt
function elapsed(task: VersionTask) {
  if (!task.startedAt) return '等待执行'
  const seconds = Math.max(0, Math.floor((Date.now() - task.startedAt) / 1000))
  return seconds >= 60 ? `已运行 ${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒` : `已运行 ${seconds} 秒`
}
function statusLabel(target: string, compare: string) {
  if (state.checking[target]) return '检测中'
  const task = lastTask(target)
  if (task && taskRunning(task)) return task.status === 'queued' ? '等待执行' : task.stage
  if (target === 'app' && appReady.value) return '待安装'
  if (state.errors[target]) return '检测失败'
  if (task?.status === 'failed') return '执行失败'
  return compare
}
function statusClass(label: string) {
  if (label.includes('失败') || label === '未通过检查') return 'status-error'
  if (label === '已是最新' || label === '已完成') return 'status-ok'
  if (label === '可更新' || label === '待安装') return 'status-update'
  return ''
}
function actionLabel(row: ToolVersionInfo) {
  const task = lastTask(row.toolId)
  if (task && taskRunning(task)) return task.status === 'queued' ? '等待中' : '执行中…'
  return toolInstalled(row) ? '升级' : '安装'
}
function uninstallBody(row: ToolVersionInfo) {
  const count = store.live.filter(live => live.toolId === row.toolId && live.alive).length
  return `将卸载本机 ${toolLabel(row.toolId)} 命令行，会话历史和配置会保留。${count ? `当前有 ${count} 个已打开的会话，卸载可能影响这些会话。` : '需要时可重新安装。'}`
}
function uninstall() {
  const row = pendingUninstall.value
  pendingUninstall.value = null
  if (row) void versionCenter.start([row.toolId], 'uninstall')
}
function installApp() { installConfirm.value = false; void versionCenter.start(['app'], 'apply') }
function retry(task: VersionTask) {
  if (task.action === 'apply') installConfirm.value = true
  else if (task.action === 'uninstall') pendingUninstall.value = state.rows.find(row => row.toolId === task.target) || null
  else void versionCenter.start([task.target], task.action)
}
async function copy(text: string) { showToast(await api.clipboardWrite(text) ? '已复制' : '复制失败，请手动选择文本') }
</script>

<template>
  <div v-if="open" class="ad-mask version-mask" @click.self="emit('close')">
    <section v-modal="() => emit('close')" class="ad-dialog version-dialog" role="dialog" aria-modal="true" aria-label="版本与更新">
      <header class="panel-header">
        <div><h2>版本与更新</h2><p>管理 Agent Dock 和本机命令行工具</p></div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">{{ running ? '后台运行' : '关闭' }}</button>
      </header>
      <div class="toolbar">
        <label class="proxy"><span>网络代理</span><input v-model="state.proxyUrl" type="text" :disabled="busy" placeholder="留空直连，例如 http://127.0.0.1:7890" /></label>
        <div class="toolbar-actions">
          <button type="button" class="btn btn-ghost btn-small" :disabled="busy" @click="versionCenter.check('all')">{{ checking ? '检测中…' : '全部检测' }}</button>
          <button type="button" class="btn btn-primary btn-small" :disabled="busy || !updatable.length" @click="versionCenter.start(updatable.map(row => row.toolId), 'upgrade')">升级可用项<span v-if="updatable.length"> ({{ updatable.length }})</span></button>
        </div>
      </div>
      <div v-if="!api.isTauri" class="preview-note">浏览器预览 · 安装与升级请在桌面端操作</div>
      <div v-if="state.error || state.syncError" class="error-banner" role="alert">
        <span>{{ state.error || state.syncError }}</span><button v-if="state.syncError" type="button" class="text-btn" @click="versionCenter.sync">刷新进度</button>
      </div>
      <section v-if="batch.length" class="batch-banner" :class="{ 'has-error': failedCount && !running }" aria-label="升级任务进度" aria-live="polite">
        <div class="batch-heading">
          <span class="task-dot" :class="{ pulsing: running }" />
          <strong>{{ running ? `${targetLabel(active?.target || '')} · ${active?.stage || '等待执行'}` : failedCount ? `${failedCount} 项失败，其余任务已处理` : batchReady ? '客户端更新已下载，等待安装' : '版本任务已完成' }}</strong>
          <span class="batch-count">{{ finished }} / {{ batch.length }}</span><button type="button" class="text-btn" @click="tab = 'tasks'">查看任务</button>
        </div>
        <div class="progress-track" role="progressbar" :aria-valuenow="Math.round(progress)" :aria-valuemin="0" :aria-valuemax="100" aria-label="任务完成进度"><span :style="{ width: `${progress}%` }" /></div>
        <div v-if="running" class="progress-note"><span>关闭面板后继续运行，可随时回来查看。</span><span v-if="active?.downloaded">{{ bytes(active.downloaded) }}<template v-if="active.total"> / {{ bytes(active.total) }}</template></span><span v-else-if="active">{{ elapsed(active) }}</span></div>
      </section>
      <div class="panel-tabs" role="tablist" aria-label="版本管理内容">
        <button id="version-software-tab" type="button" role="tab" :aria-selected="tab === 'software'" aria-controls="version-content" @click="tab = 'software'">软件版本</button>
        <button id="version-tasks-tab" type="button" role="tab" :aria-selected="tab === 'tasks'" aria-controls="version-content" @click="tab = 'tasks'">任务记录<span v-if="history.length">{{ history.length }}</span></button>
      </div>
      <div id="version-content" class="panel-content" role="tabpanel" :aria-labelledby="tab === 'software' ? 'version-software-tab' : 'version-tasks-tab'">
        <template v-if="tab === 'software'">
          <div class="section-label"><h3>桌面客户端</h3><span>下载完成后，由你选择安装时机</span></div>
          <article class="client-row">
            <div class="software-name"><img :src="appIcon" width="32" height="32" alt="" /><div><strong>Agent Dock</strong><span>{{ state.app.kindLabel || '当前桌面客户端' }}</span></div></div>
            <div class="client-version"><span>当前版本</span><strong>{{ displayVersion(state.appVersion) }}</strong></div>
            <div class="client-version"><span>最新版本</span><strong>{{ displayVersion(state.app.latestVersion) }}</strong></div>
            <span class="status" :class="statusClass(statusLabel('app', state.app.compare))">{{ statusLabel('app', state.app.compare) }}</span>
            <div class="row-actions">
              <button type="button" class="btn btn-ghost btn-small" :disabled="busy" @click="versionCenter.check('app')">检测</button>
              <button v-if="appReady" type="button" class="btn btn-primary btn-small" :disabled="busy" @click="installConfirm = true">安装更新</button>
              <button v-if="appReady && lastTask('app')?.status === 'failed'" type="button" class="text-btn" :disabled="busy" @click="versionCenter.start(['app'], 'download')">重新下载</button>
              <button v-else type="button" class="btn btn-ghost btn-small" :disabled="busy || state.app.compare !== '可更新'" @click="versionCenter.start(['app'], 'download')">下载更新</button>
            </div>
          </article>
          <div v-if="state.errors.app" class="row-error" role="alert">{{ state.errors.app }} <button type="button" class="text-btn" :disabled="busy" @click="versionCenter.check('app')">重新检测</button></div>
          <div class="section-label cli-label"><h3>命令行工具</h3><span>{{ state.rows.length }} 个工具 · 路径与环境检查可展开查看</span></div>
          <div class="table-wrap">
            <table class="software-table">
              <thead><tr><th>工具</th><th>当前版本</th><th>最新版本</th><th>状态</th><th class="actions-heading">操作</th></tr></thead>
              <tbody>
                <template v-for="row in state.rows" :key="row.toolId">
                  <tr class="software-row" :class="{ expanded: expanded === row.toolId }">
                    <td><div class="software-name"><ToolMark :id="row.toolId" /><div><strong>{{ toolLabel(row.toolId) }}</strong><span v-if="state.errors[row.toolId]" class="compact-error" :title="state.errors[row.toolId]">检测失败，可展开详情重试</span><span v-else-if="!canUpgradeTool(row) && row.compare !== '未检测'">请通过官方安装包安装</span></div></div></td>
                    <td class="version-number">{{ displayVersion(row.localVersion) }}</td><td class="version-number">{{ displayVersion(row.latestVersion) }}</td>
                    <td><span class="status" :class="statusClass(statusLabel(row.toolId, row.compare))">{{ statusLabel(row.toolId, row.compare) }}</span></td>
                    <td><div class="row-actions">
                      <button type="button" class="btn btn-ghost btn-small" :disabled="busy" @click="versionCenter.check(row.toolId)">检测</button>
                      <button type="button" class="btn btn-ghost btn-small" :disabled="busy || !canUpgradeTool(row)" @click="versionCenter.start([row.toolId], 'upgrade')">{{ actionLabel(row) }}</button>
                      <button type="button" class="text-btn detail-toggle" :aria-label="`${toolLabel(row.toolId)}详情`" :aria-expanded="expanded === row.toolId" @click="expanded = expanded === row.toolId ? '' : row.toolId">详情 <span aria-hidden="true">{{ expanded === row.toolId ? '⌃' : '⌄' }}</span></button>
                    </div></td>
                  </tr>
                  <tr v-if="expanded === row.toolId" class="details-row"><td colspan="5"><div class="tool-details">
                    <div class="path-line"><span>安装路径</span><code>{{ row.path || '检测后显示安装路径' }}</code><button v-if="row.path" type="button" class="text-btn" @click="copy(row.path)">复制</button></div>
                    <p v-if="state.errors[row.toolId]" class="row-error">{{ state.errors[row.toolId] }}</p>
                    <ul v-if="row.checks?.length" class="environment-checks"><li v-for="item in row.checks" :key="item.name"><span class="status" :class="item.ok ? 'status-ok' : 'status-error'">{{ item.ok ? '通过' : '未通过' }}</span><strong>{{ item.name }}</strong><span>{{ item.detail }}</span></li></ul>
                    <div class="detail-actions"><button type="button" class="text-btn" @click="api.openExternal(TOOL_OFFICIAL_URLS[row.toolId])">官网与安装说明 ↗</button><button v-if="toolInstalled(row)" type="button" class="text-btn danger-text" :disabled="busy" @click="pendingUninstall = row">卸载命令行</button></div>
                    <details v-if="lastTask(row.toolId)?.log" class="task-log"><summary>最近一次操作日志</summary><pre>{{ lastTask(row.toolId)?.log }}</pre></details>
                  </div></td></tr>
                </template>
              </tbody>
            </table>
          </div>
        </template>
        <template v-else>
          <div v-if="!history.length" class="empty-tasks">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3h8v4H8zM6 5H4v16h16V5h-2M8 11h8m-8 5h5" /></svg>
            <h3>还没有版本任务</h3><p>安装或升级后，可在这里查看进度、结果和日志。</p>
            <button type="button" class="btn btn-ghost btn-small" @click="tab = 'software'">查看软件版本</button>
          </div>
          <ul v-else class="task-list">
            <li v-for="task in history" :key="task.id" class="task-item" :data-status="task.status">
              <div class="task-item-heading"><span class="task-dot" :class="{ pulsing: task.status === 'running' }" /><strong>{{ targetLabel(task.target) }}</strong><span>{{ taskLabel(task) }}</span><time>{{ taskTime(task) ? new Date(taskTime(task)!).toLocaleString() : '排队中' }}</time><button v-if="task.status === 'failed'" type="button" class="text-btn" :disabled="busy" @click="retry(task)">重试</button></div>
              <p class="task-stage">{{ task.stage }}<span v-if="task.status === 'running' && task.downloaded"> · {{ bytes(task.downloaded) }}<template v-if="task.total"> / {{ bytes(task.total) }}</template></span></p>
              <details v-if="task.log" class="task-log"><summary>查看日志</summary><div class="log-actions"><button type="button" class="text-btn" @click="copy(task.log)">复制日志</button></div><pre>{{ task.log }}</pre></details>
            </li>
          </ul>
        </template>
      </div>
      <footer class="panel-footer"><span>{{ running ? '升级期间关闭主窗口会最小化，后台任务继续运行。' : '检测结果和任务记录会在关闭面板后保留。' }}</span><button type="button" class="text-btn" :aria-expanded="helpOpen" @click="helpOpen = !helpOpen">使用说明</button></footer>
      <div v-if="helpOpen" class="help-note">网络代理用于检测和下载安装包，留空时直连。批量升级按顺序执行，某项失败后仍会继续后面的任务。卸载只移除命令行，会话和配置会保留。</div>
    </section>
    <ConfirmDialog :open="!!pendingUninstall" :title="pendingUninstall ? `卸载 ${toolLabel(pendingUninstall.toolId)}` : ''" :body="pendingUninstall ? uninstallBody(pendingUninstall) : ''" action="卸载" danger @close="pendingUninstall = null" @confirm="uninstall" />
    <ConfirmDialog :open="installConfirm" title="安装 Agent Dock 更新" body="安装将替换当前客户端，Windows 和便携版会退出并完成更新。请先保存当前工作；macOS 和 Linux 安装包会交给系统安装器处理。" action="安装更新" @close="installConfirm = false" @confirm="installApp" />
  </div>
</template>

<style scoped>
.version-dialog { width: 1040px; max-width: calc(100vw - 48px); height: 816px; max-height: calc(100vh - 48px); padding: 0; display: flex; flex-direction: column; overflow: hidden; }
.panel-header { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 24px 24px 16px; flex-shrink: 0; }
h2 { margin: 0; font-size: 20px; line-height: 28px; font-weight: 600; }.panel-header p { margin: 4px 0 0; font-size: 12px; color: var(--ad-muted); line-height: 20px; }
.toolbar { display: flex; align-items: center; gap: 16px; padding: 0 24px 16px; flex-shrink: 0; }.proxy { flex: 1; min-width: 0; display: flex; align-items: center; gap: 12px; }.proxy > span { flex-shrink: 0; font-size: 12px; color: var(--ad-muted); }.proxy input { width: 100%; min-width: 0; height: 32px; font-size: 12px; }.toolbar-actions,.row-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }.toolbar-actions .btn { min-height: 32px; }
.preview-note { padding: 0 24px 12px; color: var(--ad-muted); font-size: 11px; }.error-banner { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin: 0 24px 12px; padding: 8px 12px; border-radius: 4px; background: color-mix(in srgb, var(--ad-error) 8%, transparent); color: var(--ad-error); font-size: 12px; line-height: 20px; overflow-wrap: anywhere; }
.batch-banner { margin: 0 24px 12px; padding: 12px; border: 1px solid var(--ad-border); border-radius: 6px; background: var(--ad-hover); flex-shrink: 0; }.batch-heading { display: flex; align-items: center; gap: 8px; font-size: 12px; line-height: 20px; }.batch-heading strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }.batch-count { margin-left: auto; flex-shrink: 0; font-size: 11px; font-variant-numeric: tabular-nums; color: var(--ad-muted); }.batch-heading .text-btn { margin-left: 8px; }.task-dot { display: inline-block; width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; background: var(--ad-success); }.has-error .task-dot,.task-item[data-status='failed'] .task-dot { background: var(--ad-error); }.pulsing { background: var(--ad-accent); animation: pulse 1.5s ease-in-out infinite; }.task-item[data-status='queued'] .task-dot { background: var(--ad-muted); }.task-item[data-status='ready'] .task-dot { background: var(--ad-warning); }
.progress-track { height: 3px; background: var(--ad-border); margin-top: 8px; border-radius: 2px; overflow: hidden; }.progress-track span { display: block; height: 100%; background: var(--ad-accent); }.progress-note { display: flex; justify-content: space-between; gap: 12px; margin-top: 8px; font-size: 11px; color: var(--ad-muted); line-height: 20px; }
.panel-tabs { display: flex; gap: 24px; padding: 0 24px; border-bottom: 1px solid var(--ad-border); flex-shrink: 0; }.panel-tabs button { display: flex; align-items: center; gap: 8px; height: 40px; font-size: 12px; color: var(--ad-muted); border-bottom: 2px solid transparent; }.panel-tabs button[aria-selected='true'] { border-bottom-color: var(--ad-text); color: var(--ad-text); }.panel-tabs button span { padding: 0 5px; border-radius: 4px; line-height: 18px; font-size: 10px; background: var(--ad-hover); }
.panel-content { flex: 1; min-height: 0; overflow: auto; padding: 20px 24px 24px; scrollbar-gutter: stable; }.section-label { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 12px; }.section-label h3 { margin: 0; font-size: 12px; font-weight: 600; line-height: 20px; }.section-label > span { font-size: 11px; line-height: 20px; color: var(--ad-muted); }.cli-label { margin-top: 24px; }
.client-row { display: flex; align-items: center; gap: 20px; border: 1px solid var(--ad-border); border-radius: 6px; padding: 16px; background: var(--ad-hover); }.client-row > .software-name { flex: 1.2; }.client-version { flex: .65; min-width: 0; display: flex; flex-direction: column; gap: 4px; }.client-version > span { color: var(--ad-muted); font-size: 11px; }.client-version strong { font-size: 13px; font-weight: 500; font-variant-numeric: tabular-nums; }.software-name { display: flex; align-items: center; gap: 12px; min-width: 0; }.software-name > :deep(.frame) { width: 28px; height: 28px; flex-shrink: 0; }.software-name > div { min-width: 0; display: flex; flex-direction: column; gap: 4px; }.software-name strong { font-size: 13px; font-weight: 600; line-height: 20px; white-space: nowrap; }.software-name div > span { font-size: 11px; color: var(--ad-muted); line-height: 16px; }.software-name img { border-radius: 6px; }
.status { display: inline-block; font-size: 11px; color: var(--ad-muted); line-height: 20px; overflow-wrap: anywhere; }.status-ok { color: var(--ad-success); }.status-error,.compact-error { color: var(--ad-error) !important; }.status-update { color: var(--ad-warning); }.row-actions { justify-content: flex-end; white-space: nowrap; }.row-actions .btn { min-width: 48px; padding-inline: 10px; }.text-btn { font-size: 12px; line-height: 20px; color: var(--ad-muted); padding: 0; background: transparent; flex-shrink: 0; }.text-btn:hover:not(:disabled) { color: var(--ad-text); }.text-btn:disabled { opacity: .45; cursor: not-allowed; }.detail-toggle { margin-left: 4px; }.danger-text { color: var(--ad-error); }
.table-wrap { overflow-x: auto; border: 1px solid var(--ad-border); border-radius: 6px; }.software-table { width: 100%; min-width: 780px; border-collapse: collapse; text-align: left; }.software-table th { background: var(--ad-hover); padding: 8px 12px; color: var(--ad-muted); font-size: 11px; line-height: 20px; font-weight: 400; white-space: nowrap; }.software-table td { padding: 12px; border-top: 1px solid var(--ad-border); }.software-table td:first-child { width: 27%; }.software-table .version-number { font-size: 12px; font-variant-numeric: tabular-nums; white-space: nowrap; }.software-table .actions-heading { text-align: right; }.software-row.expanded { background: var(--ad-hover); }.details-row td { padding: 0 !important; }.tool-details { padding: 16px; background: var(--ad-harbor); }.path-line { display: flex; gap: 12px; align-items: flex-start; font-size: 11px; line-height: 20px; color: var(--ad-muted); }.path-line > span { flex-shrink: 0; }.path-line code { flex: 1; min-width: 0; font-family: var(--ad-mono); overflow-wrap: anywhere; color: var(--ad-text); }.detail-actions { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-top: 12px; }.row-error { color: var(--ad-error); margin: 8px 0 0; font-size: 12px; line-height: 20px; overflow-wrap: anywhere; }.environment-checks { margin: 12px 0 0; padding: 0; list-style: none; display: grid; gap: 8px; }.environment-checks li { display: flex; align-items: baseline; gap: 12px; font-size: 11px; line-height: 20px; color: var(--ad-muted); }.environment-checks strong { color: var(--ad-text); font-weight: 500; }
.task-list { margin: 0; padding: 0; list-style: none; }.task-item { padding: 16px 0; border-bottom: 1px solid var(--ad-border); }.task-item:first-child { padding-top: 0; }.task-item-heading { display: flex; align-items: center; gap: 8px; font-size: 12px; line-height: 20px; }.task-item-heading > strong { font-size: 13px; font-weight: 600; }.task-item-heading > span:not(.task-dot) { color: var(--ad-muted); }.task-item-heading time { margin-left: auto; font-size: 11px; color: var(--ad-muted); font-variant-numeric: tabular-nums; }.task-stage { margin: 8px 0 0 14px; font-size: 12px; color: var(--ad-muted); line-height: 20px; }.task-item[data-status='failed'] .task-stage { color: var(--ad-error); }.task-log { margin-top: 12px; }.task-log summary { cursor: pointer; font-size: 12px; line-height: 20px; color: var(--ad-muted); }.task-log pre { max-height: 240px; overflow: auto; margin: 8px 0 0; padding: 12px; border-radius: 4px; background: var(--ad-hover); color: var(--ad-text); font-family: var(--ad-mono); font-size: 11px; line-height: 20px; white-space: pre-wrap; overflow-wrap: anywhere; }.log-actions { display: flex; justify-content: flex-end; padding-top: 8px; }
.empty-tasks { display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 280px; gap: 12px; text-align: center; }.empty-tasks svg { width: 32px; height: 32px; fill: none; stroke: var(--ad-muted); stroke-width: 1.2; }.empty-tasks h3 { margin: 0; font-size: 14px; font-weight: 600; }.empty-tasks p { margin: 0 0 4px; font-size: 12px; color: var(--ad-muted); line-height: 20px; }
.panel-footer { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 24px; border-top: 1px solid var(--ad-border); color: var(--ad-muted); font-size: 11px; line-height: 20px; flex-shrink: 0; }.help-note { padding: 0 24px 16px; color: var(--ad-muted); font-size: 12px; line-height: 20px; flex-shrink: 0; }
@keyframes pulse { 50% { opacity: .35; } }@media (prefers-reduced-motion: reduce) { .pulsing { animation: none; } }
@media (max-width: 920px) { .client-row { flex-wrap: wrap; gap: 16px; }.client-row > .software-name { flex: 1 0 calc(100% - 200px); }.client-version { flex: 1; }.client-row > .status { margin-right: auto; }.section-label > span { max-width: 55%; text-align: right; } }
@media (max-width: 680px) { .version-dialog { max-width: calc(100vw - 24px); max-height: calc(100vh - 24px); }.version-mask { padding: 12px; }.panel-header { padding: 16px; }h2 { font-size: 18px; }.toolbar { flex-wrap: wrap; padding: 0 16px 16px; gap: 12px; }.proxy { flex-basis: 100%; }.toolbar-actions { margin-left: auto; }.panel-tabs { padding-inline: 16px; }.panel-content { padding: 16px; }.batch-banner { margin-inline: 16px; }.client-row > .software-name { flex-basis: 100%; }.section-label { align-items: flex-start; }.section-label > span { max-width: 65%; }.panel-footer { padding-inline: 16px; }.panel-footer > span { max-width: 80%; }.task-item-heading { flex-wrap: wrap; }.task-item-heading time { width: calc(100% - 40px); margin-left: 14px; }.preview-note { padding-inline: 16px; }.batch-heading .text-btn { margin-left: 0; }.error-banner { margin-inline: 16px; } }
</style>
