<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  formatCacheHit,
  formatExactTokens,
  formatSpendCost,
  formatTokenCount
} from '../lib/grokSpend'
import { store } from '../lib/store'
import {
  USAGE_PAGE_SIZE,
  USAGE_PRESETS,
  USAGE_REFRESH_KEY,
  USAGE_REFRESH_MS,
  emptyUsageDashboard,
  emptyUsageLogs,
  formatLogTime,
  formatSyncAgo,
  fromDatetimeLocal,
  parseUsageRefreshMs,
  resolveUsageRange,
  toDatetimeLocal,
  type UsageMetric,
  type UsagePreset
} from '../lib/usage'
import { TOOLS, type UsageDashboard, type UsageLogPage, type UsageQuery, type UsageRecord, type UsageGroupRow } from '../lib/types'
import { modal as vModal } from '../lib/modal'
import ToolMark from './ToolMark.vue'
import UsageTrend from './UsageTrend.vue'
import UsageCalendar from './UsageCalendar.vue'

const METRICS: { id: UsageMetric; label: string }[] = [
  { id: 'cost', label: '总成本' },
  { id: 'requests', label: '总请求' },
  { id: 'tokens', label: '真实消耗 Tokens' },
  { id: 'cache', label: '缓存命中率' }
]

const TABS = [
  { id: 'logs', label: '请求日志' },
  { id: 'providers', label: '供应商' },
  { id: 'models', label: '模型' }
] as const

const initialRange = resolveUsageRange('today')
const preset = ref<UsagePreset>('today')
const start = ref(initialRange.start)
const end = ref(initialRange.end)
const followEnd = ref(true)
const appFilter = ref('all')
const providerName = ref('')
const model = ref('')
const metric = ref<UsageMetric>('tokens')
const tab = ref<(typeof TABS)[number]['id']>('logs')
const page = ref(0)
const pageSize = ref<number>(USAGE_PAGE_SIZE)
const showTokenParts = ref(false)
const showLogTokens = ref(false)
const selectedLog = ref<UsageRecord | null>(null)
const providerOptions = ref<UsageGroupRow[]>([])
const modelOptions = ref<UsageGroupRow[]>([])
const loading = ref(false)
const exporting = ref(false)
const exportNotice = ref('')
const settingsOpen = ref(false)
const refreshMs = ref(parseUsageRefreshMs(typeof localStorage === 'undefined' ? undefined : localStorage.getItem(USAGE_REFRESH_KEY)))
const nowTick = ref(Date.now())
const dash = ref<UsageDashboard | null>(null)
const logs = ref<UsageLogPage>(emptyUsageLogs())
let pollTimer: ReturnType<typeof setInterval> | undefined
let agoTimer: ReturnType<typeof setInterval> | undefined
let seq = 0

const query = computed<UsageQuery>(() => ({
  start: start.value,
  end: end.value,
  appFilter: appFilter.value,
  providerName: providerName.value || null,
  model: model.value || null
}))
const banner = computed(() => dash.value?.message || '')
const rangeLabel = computed(() =>
  preset.value === 'custom' ? '自定义' : (USAGE_PRESETS.find((item) => item.id === preset.value)?.label ?? '当天')
)
const emptyText = computed(() => (loading.value ? '正在读取用量…' : '这个时间段还没有用量记录'))
const syncLabel = computed(() => {
  if (loading.value && !dash.value) return '同步中'
  const fetched = dash.value?.fetchedAt
  if (!fetched) return '立即同步'
  return formatSyncAgo(fetched, nowTick.value)
})
const pageCount = computed(() => Math.max(1, Math.ceil((logs.value.total || 0) / pageSize.value)))
const appChoices = computed(() => {
  const apps = (dash.value?.apps ?? []).filter(app => !app.id.startsWith('claude:'))
  if ((dash.value?.apps ?? []).some(app => app.id.startsWith('claude:')) && !apps.some(app => app.id === 'claude')) {
    return [...apps, { id: 'claude', label: 'Claude' }]
  }
  return apps
})
const claudeAccounts = computed(() => (dash.value?.apps ?? []).filter(app => app.id.startsWith('claude:')))
const appFamily = computed(() => appFilter.value.startsWith('claude:') ? 'claude' : appFilter.value)
const tokenParts = computed(() => [
  { label: '新增输入', value: dash.value?.inputTokens }, { label: '输出', value: dash.value?.outputTokens },
  { label: '缓存写入', value: dash.value?.cacheCreationTokens }, { label: '缓存命中', value: dash.value?.cacheReadTokens }
])
const groupRows = computed(() => (tab.value === 'providers' ? dash.value?.providers : dash.value?.models) ?? [])
const tabCounts = computed(() => ({ logs: logs.value.total, providers: dash.value?.providers.length ?? 0, models: dash.value?.models.length ?? 0 }))
const filterActive = computed(() => appFilter.value !== 'all' || !!providerName.value || !!model.value)
const dateLabel = computed(() => `${new Date(start.value * 1000).toLocaleDateString()} — ${new Date(end.value * 1000).toLocaleDateString()}`)
const logColumnCount = computed(() => showLogTokens.value ? 12 : 8)
const logTokens = (row: UsageRecord) => row.inputTokens + row.outputTokens + row.cacheReadTokens + row.cacheCreationTokens
const appTool = (id: string) => TOOLS.find(tool => tool.id === id)?.id
const statusKind = (code: number) => code >= 200 && code < 400 ? 'success' : code >= 400 ? 'error' : 'unknown'

function chooseApp(id: string) {
  appFilter.value = id
  providerName.value = ''; model.value = ''
}

function clearFilters() { chooseApp('all') }
function closeDrawers() { settingsOpen.value = false; selectedLog.value = null }
function closePopovers(event?: MouseEvent) {
  document.querySelectorAll<HTMLDetailsElement>('.usage-popover[open]').forEach(el => {
    if (!event || !el.contains(event.target as Node)) el.open = false
  })
}
onMounted(() => document.addEventListener('click', closePopovers))

function metricText(id: UsageMetric) {
  const row = dash.value
  if (!row) return loading.value ? '…' : '—'
  if (!row.totalRequests) return '—'
  if (id === 'cost') return row.totalCostUsd.toLocaleString('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 2 })
  if (id === 'requests') return formatExactTokens(row?.totalRequests)
  if (id === 'cache') return row.inputTokens + row.cacheReadTokens + row.cacheCreationTokens > 0 ? formatCacheHit(row.cacheHitPercent) : '—'
  return formatTokenCount(row?.totalTokens)
}

function applyRange(id: UsagePreset, custom?: { start: number; end: number }) {
  const next = resolveUsageRange(id, Date.now(), custom)
  preset.value = id
  start.value = next.start
  end.value = next.end
  followEnd.value = next.followEnd
  if (page.value !== 0) page.value = 0
  else if (store.appMode === 'usage') void reload(false)
}

function pickPreset(id: UsagePreset) {
  applyRange(id)
  closePopovers()
}

function setCustom() {
  applyRange('custom', { start: start.value, end: end.value })
}

function onCustomStart(value: string) {
  applyRange('custom', { start: fromDatetimeLocal(value), end: end.value })
}

function onCustomEnd(value: string) {
  applyRange('custom', { start: start.value, end: fromDatetimeLocal(value) })
}

function setRefresh(ms: number) {
  refreshMs.value = parseUsageRefreshMs(ms)
  try {
    localStorage.setItem(USAGE_REFRESH_KEY, String(refreshMs.value))
  } catch {
    /* ignore quota */
  }
  startPoll()
}

function refreshFollowedRange() {
  if (!followEnd.value) return
  const next = resolveUsageRange(preset.value, Date.now(), { start: start.value, end: end.value })
  start.value = next.start
  end.value = next.end
}

async function reload(force: boolean) {
  if (store.appMode !== 'usage') return
  const token = ++seq
  loading.value = true
  try {
    if (force) {
      refreshFollowedRange()
      await api.usageSync()
    }
    if (token !== seq) return
    const request = { ...query.value }
    const [next, nextLogs] = await Promise.all([
      api.usageDashboard(request),
      api.usageLogs({ ...request, page: page.value, pageSize: pageSize.value })
    ])
    if (token !== seq) return
    if (!next.dbOk && next.message?.includes('占用') && dash.value) {
      dash.value = { ...dash.value, message: next.message, dbOk: false }
    } else {
      dash.value = next
    }
    logs.value = nextLogs
    if (!request.providerName && !request.model) providerOptions.value = next.providers
    if (!request.model) modelOptions.value = next.models
  } catch (err) {
    if (token !== seq) return
    const message = err instanceof Error ? err.message : String(err)
    const fallback = dash.value ?? emptyUsageDashboard(message)
    dash.value = { ...fallback, message }
  } finally {
    if (token === seq) loading.value = false
  }
}

async function exportToCcswitch() {
  if (store.appMode !== 'usage' || exporting.value) return
  exporting.value = true
  try {
    const result = await api.usageExportCcswitch()
    exportNotice.value = result.message || (result.dbOk ? '已同步到 CC Switch' : '同步到 CC Switch 失败')
    await reload(true)
  } catch (err) {
    exportNotice.value = err instanceof Error ? err.message : String(err)
  } finally {
    exporting.value = false
  }
}

function startPoll() {
  stopPoll()
  pollTimer = setInterval(() => {
    nowTick.value = Date.now()
    refreshFollowedRange()
    void reload(true)
  }, refreshMs.value)
}

function stopPoll() {
  if (pollTimer) clearInterval(pollTimer)
  pollTimer = undefined
}

watch(
  () => store.appMode,
  (mode) => {
    if (mode === 'usage') {
      nowTick.value = Date.now()
      refreshFollowedRange()
      void reload(true)
      startPoll()
    } else {
      stopPoll()
    }
  },
  { immediate: true }
)

watch([appFilter, providerName, model], () => {
  if (page.value !== 0) page.value = 0
  else if (store.appMode === 'usage') void reload(false)
})

watch(page, () => {
  if (store.appMode === 'usage') void reload(false)
})

watch(pageSize, () => {
  if (page.value) page.value = 0
  else if (store.appMode === 'usage') void reload(false)
})

agoTimer = setInterval(() => {
  nowTick.value = Date.now()
}, 15_000)

onUnmounted(() => {
  document.removeEventListener('click', closePopovers)
  ++seq
  stopPoll()
  if (agoTimer) clearInterval(agoTimer)
})
</script>

<template>
  <section class="pane" aria-label="使用量" @keydown.esc="closePopovers()">
    <header class="head">
      <div class="page-title"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19V9m8 10V4m8 15v-7" /></svg><h1>使用量</h1><span class="help" title="统计本机会话与 Claude 独立账号的请求、Token 和估算成本。所有筛选条件统一作用于概览和明细。">?</span></div>
      <div class="head-actions"><span class="sync-time">更新于 {{ syncLabel }}</span><button type="button" class="btn btn-ghost btn-small" :disabled="loading" @click="reload(true)"><svg class="action-icon" :class="{ spinning: loading }" viewBox="0 0 16 16" aria-hidden="true"><path d="M13 6a5 5 0 1 0 0 4M13 2v4H9" /></svg>{{ loading ? '同步中…' : '立即同步' }}</button><button type="button" class="btn btn-ghost btn-small" :disabled="exporting" @click="exportToCcswitch">{{ exporting ? '写入中…' : '同步到 CC Switch' }}</button><button type="button" class="btn btn-ghost btn-small" @click="settingsOpen = true">设置</button></div>
    </header>

    <div class="filters">
      <details class="usage-popover range-picker"><summary class="filter-trigger"><svg class="action-icon" viewBox="0 0 16 16" aria-hidden="true"><rect x="2" y="3" width="12" height="11" rx="2" /><path d="M5 1v4m6-4v4M2 7h12" /></svg>{{ rangeLabel }}<span class="caret">⌄</span></summary><div class="popover-body range-options"><p>时间范围</p><div class="preset-grid"><button v-for="item in USAGE_PRESETS" :key="item.id" type="button" :class="{ on: preset === item.id }" @click="pickPreset(item.id)">{{ item.label }}</button><button type="button" :class="{ on: preset === 'custom' }" @click="setCustom">自定义</button></div><div v-if="preset === 'custom'" class="custom"><label>开始<input :value="toDatetimeLocal(start)" type="datetime-local" @change="onCustomStart(($event.target as HTMLInputElement).value)" /></label><label>结束<input :value="toDatetimeLocal(end)" type="datetime-local" @change="onCustomEnd(($event.target as HTMLInputElement).value)" /></label><button type="button" class="btn btn-ghost btn-small" @click="closePopovers()">完成</button></div><span v-else class="range-hint">{{ preset === 'all' ? '查看全部历史记录' : dateLabel }}</span></div></details>
      <span class="filter-divider" />
      <div class="app-segments" role="group" aria-label="应用"><button type="button" :aria-pressed="appFilter === 'all'" @click="chooseApp('all')">全部</button><button v-for="app in appChoices" :key="app.id" type="button" :aria-pressed="appFamily === app.id" :title="app.label" @click="chooseApp(app.id)"><span v-if="appTool(app.id)" class="app-mark"><ToolMark :id="appTool(app.id)!" /></span>{{ app.label }}</button></div>
      <label v-if="appFamily === 'claude' && claudeAccounts.length" class="select-filter"><span class="sr-only">Claude 账号</span><select :value="appFilter" aria-label="Claude 账号" @change="chooseApp(($event.target as HTMLSelectElement).value)"><option value="claude">系统 Claude</option><option v-for="account in claudeAccounts" :key="account.id" :value="account.id">{{ account.label.replace(/^Claude · /, '') }}</option></select></label>
      <label class="select-filter"><span class="sr-only">供应商</span><select v-model="providerName" aria-label="供应商" @change="model = ''"><option value="">全部供应商</option><option v-for="row in providerOptions.length ? providerOptions : dash?.providers ?? []" :key="row.name" :value="row.name">{{ row.name }}</option></select></label>
      <label class="select-filter model-filter"><span class="sr-only">模型</span><select v-model="model" aria-label="模型"><option value="">全部模型</option><option v-for="row in modelOptions.length ? modelOptions : dash?.models ?? []" :key="row.name" :value="row.name">{{ row.name }}</option></select></label>
      <button v-if="filterActive" type="button" class="clear-filter" @click="clearFilters">清除筛选</button>
      <span class="scope-date" :title="dateLabel">{{ preset === 'all' ? '全部历史' : dateLabel }}</span>
    </div>

    <div class="content">
      <p v-if="banner" class="notice" role="status">{{ banner }}</p>
      <div v-if="exportNotice" class="notice export-notice" role="status"><span>{{ exportNotice }}</span><button type="button" aria-label="关闭同步提示" @click="exportNotice = ''">×</button></div>
      <section class="overview" aria-label="用量概览" :aria-busy="loading">
        <div class="metric-strip" role="group" aria-label="汇总指标"><button v-for="item in METRICS" :key="item.id" type="button" class="metric-cell" :aria-pressed="metric === item.id" @click="metric = item.id"><span>{{ item.label }} <span v-if="item.id === 'tokens'" class="help" title="新增输入 + 输出 + 缓存写入 + 缓存命中">?</span><span v-if="item.id === 'cache'" class="help" title="缓存命中 ÷（新增输入 + 缓存写入 + 缓存命中）">?</span></span><strong :title="item.id === 'tokens' ? formatExactTokens(dash?.totalTokens) : undefined">{{ metricText(item.id) }}</strong></button></div>
        <div v-if="showTokenParts" id="usage-token-parts" class="token-parts"><div v-for="part in tokenParts" :key="part.label"><span>{{ part.label }}</span><strong :title="formatExactTokens(part.value)">{{ dash?.totalRequests ? formatTokenCount(part.value) : '—' }}</strong></div></div>
        <div class="breakdown-divider"><button type="button" :aria-expanded="showTokenParts" aria-controls="usage-token-parts" :aria-label="showTokenParts ? '收起 Token 明细' : '展开 Token 明细'" :title="showTokenParts ? '收起 Token 明细' : '展开 Token 明细'" @click="showTokenParts = !showTokenParts"><svg viewBox="0 0 16 16" :class="{ rotated: showTokenParts }" aria-hidden="true"><path d="m4 6 4 4 4-4" /></svg></button></div>
        <UsageCalendar v-if="preset === 'all'" :key="appFilter + '|' + providerName + '|' + model" :rows="dash?.heatmap ?? []" :metric="metric" :end="end" :loading="loading" />
        <UsageTrend v-else :points="dash?.points ?? []" :metric="metric" :start="start" :end="end" :loading="loading" />
      </section>

      <section class="detail" aria-label="使用量明细">
        <div class="detail-toolbar"><div class="tabs" role="tablist" aria-label="明细"><button v-for="item in TABS" :id="'usage-tab-' + item.id" :key="item.id" type="button" role="tab" class="tab" :aria-selected="tab === item.id" aria-controls="usage-detail-panel" @click="tab = item.id">{{ item.label }}<span class="count">{{ formatExactTokens(tabCounts[item.id]) }}</span></button></div><label v-if="tab === 'logs'" class="column-toggle"><input v-model="showLogTokens" type="checkbox" />Token 分项</label></div>
        <div id="usage-detail-panel" role="tabpanel" :aria-labelledby="'usage-tab-' + tab" class="table-wrap" :aria-busy="loading">
          <table v-if="tab === 'logs'" class="logs-table"><colgroup><col class="time-col" /><col class="app-col" /><col class="provider-col" /><col class="model-col" /></colgroup><thead><tr><th>时间</th><th>应用 / 账号</th><th>供应商</th><th>模型</th><template v-if="showLogTokens"><th class="num">新增输入</th><th class="num">输出</th><th class="num">缓存写入</th><th class="num">缓存命中</th></template><th class="num">Tokens</th><th class="num">成本</th><th class="num">状态</th><th class="row-action"><span class="sr-only">详情</span></th></tr></thead><tbody><tr v-if="!logs.rows.length"><td :colspan="logColumnCount" class="empty-cell"><strong>{{ emptyText }}</strong><span v-if="filterActive">可以调整时间范围或清除筛选后重试。</span><button v-if="filterActive" type="button" @click="clearFilters">清除筛选</button></td></tr><tr v-for="row in logs.rows" :key="row.requestId" @click="selectedLog = row"><td class="time-cell">{{ formatLogTime(row.createdAt) }}</td><td :title="row.appLabel"><span class="ellipsis">{{ row.appLabel }}</span></td><td :title="row.providerName"><span class="ellipsis muted">{{ row.providerName || '—' }}</span></td><td :title="row.model"><button type="button" class="model-link" @click.stop="selectedLog = row">{{ row.model || '—' }}</button></td><template v-if="showLogTokens"><td class="num">{{ formatExactTokens(row.inputTokens) }}</td><td class="num">{{ formatExactTokens(row.outputTokens) }}</td><td class="num">{{ formatExactTokens(row.cacheCreationTokens) }}</td><td class="num">{{ formatExactTokens(row.cacheReadTokens) }}</td></template><td class="num" :title="'新增输入 ' + formatExactTokens(row.inputTokens) + ' · 输出 ' + formatExactTokens(row.outputTokens) + ' · 缓存写入 ' + formatExactTokens(row.cacheCreationTokens) + ' · 缓存命中 ' + formatExactTokens(row.cacheReadTokens)">{{ formatExactTokens(logTokens(row)) }}</td><td class="num">{{ row.unpriced ? '未定价' : formatSpendCost(row.costUsd) }}</td><td class="num"><span class="status" :class="statusKind(row.statusCode)"><i />{{ row.statusCode || '—' }}</span></td><td class="row-action"><button type="button" :aria-label="'查看请求详情：' + row.requestId" @click.stop="selectedLog = row">↗</button></td></tr></tbody></table>
          <table v-else class="groups-table"><thead><tr><th>{{ tab === 'providers' ? '供应商' : '模型' }}</th><th class="num">请求</th><th class="num">Tokens</th><th class="num">成本</th><th class="num">Token 占比</th></tr></thead><tbody><tr v-if="!groupRows.length"><td colspan="5" class="empty-cell">{{ emptyText }}</td></tr><tr v-for="row in groupRows" :key="row.name"><td :title="row.name">{{ row.name || '—' }}</td><td class="num">{{ formatExactTokens(row.requestCount) }}</td><td class="num">{{ formatExactTokens(row.tokens) }}</td><td class="num">{{ row.unpriced ? '未定价' : formatSpendCost(row.costUsd) }}</td><td class="num share-cell"><span class="share-track"><i :style="{ width: (dash?.totalTokens ? row.tokens / dash.totalTokens * 100 : 0) + '%' }" /></span>{{ dash?.totalTokens ? (row.tokens / dash.totalTokens * 100).toFixed(1) : '0.0' }}%</td></tr></tbody></table>
        </div>
        <div v-if="tab === 'logs'" class="pager"><span>共 {{ formatExactTokens(logs.total) }} 条记录</span><label>每页<select v-model.number="pageSize" aria-label="每页记录数"><option v-for="size in [20, 50, 100]" :key="size" :value="size">{{ size }}</option></select>条</label><span class="page-position">{{ page + 1 }} / {{ pageCount }}</span><button type="button" class="btn btn-ghost btn-small" :disabled="page <= 0 || loading" @click="page = Math.max(0, page - 1)">上一页</button><button type="button" class="btn btn-ghost btn-small" :disabled="page + 1 >= pageCount || loading" @click="page += 1">下一页</button></div>
      </section>
      <p class="source-note">本地会话统计 · 含 Claude 独立账号 · 成本按 CC Switch 价表估算</p>
    </div>

    <Teleport to="body"><div v-if="settingsOpen || selectedLog" class="ad-mask ad-mask--flush" @click.self="closeDrawers"><aside v-modal="closeDrawers" class="ad-drawer usage-drawer" role="dialog" aria-modal="true" :aria-label="selectedLog ? '请求详情' : '用量设置'" tabindex="-1"><header><h2>{{ selectedLog ? '请求详情' : '用量设置' }}</h2><button type="button" class="btn btn-ghost btn-small" @click="closeDrawers">关闭</button></header>
      <template v-if="selectedLog"><p class="drawer-eyebrow">{{ formatLogTime(selectedLog.createdAt) }}<span class="status" :class="statusKind(selectedLog.statusCode)"><i />{{ selectedLog.statusCode }}</span></p><h3 class="request-model">{{ selectedLog.model || '未知模型' }}</h3><div class="request-totals"><div><span>真实消耗 Tokens</span><strong>{{ formatExactTokens(logTokens(selectedLog)) }}</strong></div><div><span>成本</span><strong>{{ selectedLog.unpriced ? '未定价' : formatSpendCost(selectedLog.costUsd) }}</strong></div></div><dl class="request-facts"><dt>应用 / 账号</dt><dd>{{ selectedLog.appLabel }}</dd><dt>供应商</dt><dd>{{ selectedLog.providerName || '—' }}</dd><dt>新增输入</dt><dd>{{ formatExactTokens(selectedLog.inputTokens) }}</dd><dt>输出</dt><dd>{{ formatExactTokens(selectedLog.outputTokens) }}</dd><dt>缓存写入</dt><dd>{{ formatExactTokens(selectedLog.cacheCreationTokens) }}</dd><dt>缓存命中</dt><dd>{{ formatExactTokens(selectedLog.cacheReadTokens) }}</dd><dt>会话 ID</dt><dd><code>{{ selectedLog.sessionId || '—' }}</code></dd><dt>请求 ID</dt><dd><code>{{ selectedLog.requestId }}</code></dd><dt>数据来源</dt><dd>{{ selectedLog.dataSource || '—' }}</dd></dl></template>
      <template v-else><h3 class="section-label">自动同步</h3><p class="hint">定期扫描本机会话，更新统计和明细。</p><div class="intervals"><button v-for="ms in USAGE_REFRESH_MS" :key="ms" type="button" :aria-pressed="refreshMs === ms" @click="setRefresh(ms)">{{ ms / 1000 }} 秒</button></div><h3 class="section-label">同步到 CC Switch</h3><p class="hint">将 Claude 独立账号用量写入 CC Switch，供应商按账号区分。已有记录保持不变。</p><button type="button" class="btn btn-ghost btn-small" :disabled="exporting" @click="exportToCcswitch">{{ exporting ? '写入中…' : '同步独立账号用量' }}</button><h3 class="section-label">模型定价</h3><p class="hint">读取 CC Switch 的价表；价格可在 CC Switch 中修改。</p><div class="pricing-wrap"><table v-if="dash?.pricing.length"><thead><tr><th>模型</th><th class="num">输入 / M</th><th class="num">输出 / M</th><th class="num">命中 / M</th><th class="num">写入 / M</th></tr></thead><tbody><tr v-for="row in dash.pricing" :key="row.modelId"><td>{{ row.displayName || row.modelId }}</td><td class="num">{{ formatSpendCost(row.inputPerMillion) }}</td><td class="num">{{ formatSpendCost(row.outputPerMillion) }}</td><td class="num">{{ formatSpendCost(row.cacheReadPerMillion) }}</td><td class="num">{{ formatSpendCost(row.cacheCreationPerMillion) }}</td></tr></tbody></table><p v-else class="hint">暂无定价，请先在 CC Switch 中扫描或配置。</p></div></template>
    </aside></div></Teleport>
  </section>
</template>

<style scoped>
.pane { --usage-chart: #8b9cf7; flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--ad-ink); }
.head { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 20px 24px 12px; flex-shrink: 0; }
.page-title, .head-actions { display: flex; align-items: center; gap: 8px; }
.page-title > svg { width: 20px; height: 20px; fill: none; stroke: var(--ad-text); stroke-width: 1.6; stroke-linecap: round; }
h1 { margin: 0; font-size: 18px; font-weight: 600; line-height: 28px; }
.help { display: inline-flex; align-items: center; justify-content: center; width: 14px; height: 14px; border: 1px solid var(--ad-border-strong); border-radius: 50%; color: var(--ad-muted); font-size: 10px; font-weight: 400; }
.sync-time { margin-right: 8px; color: var(--ad-muted); font-size: 12px; white-space: nowrap; }
.head-actions { flex-wrap: wrap; justify-content: flex-end; }
.btn { gap: 6px; }
.action-icon { width: 14px; height: 14px; fill: none; stroke: currentColor; stroke-width: 1.3; }
.filters { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 8px 24px 16px; flex-shrink: 0; }
.filter-divider { width: 1px; height: 16px; background: var(--ad-border-strong); margin: 0 4px; }
.filter-trigger, .select-filter select { display: flex; align-items: center; gap: 8px; height: 32px; border: 1px solid var(--ad-border-strong); border-radius: 6px; background: var(--ad-harbor); padding: 0 10px; color: var(--ad-text); font-size: 12px; cursor: pointer; }
.filter-trigger { list-style: none; min-width: 112px; }
.filter-trigger::-webkit-details-marker { display: none; }
.caret { margin-left: auto; color: var(--ad-muted); }
.usage-popover { position: relative; }
.popover-body { position: absolute; z-index: 12; top: calc(100% + 8px); left: 0; width: 320px; padding: 16px; background: var(--ad-float-solid); border: 1px solid var(--ad-border-strong); box-shadow: var(--ad-shadow-menu); border-radius: 8px; }
.range-options > p { margin: 0 0 12px; font-size: 12px; color: var(--ad-muted); }
.preset-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.preset-grid button, .intervals button { height: 32px; border-radius: 4px; color: var(--ad-muted); font-size: 12px; border: 1px solid var(--ad-border); }
.preset-grid button:hover, .preset-grid .on { background: var(--ad-selected); color: var(--ad-text); }
.range-hint { display: block; margin-top: 12px; font-size: 11px; color: var(--ad-muted); }
.custom { display: flex; flex-direction: column; gap: 12px; margin-top: 16px; }
.custom label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--ad-muted); }
.custom input { width: 100%; height: 32px; font-size: 12px; }
.app-segments { display: flex; align-items: center; flex-wrap: wrap; gap: 2px; padding: 2px; border: 1px solid var(--ad-border-strong); border-radius: 6px; }
.app-segments button { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: 4px; font-size: 12px; color: var(--ad-muted); white-space: nowrap; }
.app-segments button[aria-pressed='true'] { color: var(--ad-text); background: var(--ad-selected); }
.app-segments button:hover { color: var(--ad-text); }
.app-mark { width: 16px; height: 16px; }
.select-filter { min-width: 0; }
.select-filter select { max-width: 200px; text-overflow: ellipsis; }
.model-filter select { max-width: 180px; }
.clear-filter { height: 28px; color: var(--ad-muted); font-size: 12px; }
.clear-filter:hover { color: var(--ad-text); }
.scope-date { margin-left: auto; font-size: 11px; color: var(--ad-muted); white-space: nowrap; }
.content { flex: 1; min-height: 0; min-width: 0; overflow-y: auto; scrollbar-gutter: stable; padding: 0 24px 24px; display: flex; flex-direction: column; gap: 24px; }
.content > * { flex-shrink: 0; }
.notice { margin: 0; padding: 8px 12px; border: 1px solid var(--ad-border); border-radius: 6px; background: var(--ad-harbor); font-size: 12px; line-height: 20px; color: var(--ad-muted); }
.export-notice { display: flex; gap: 12px; align-items: flex-start; }
.export-notice button { margin-left: auto; font-size: 18px; line-height: 20px; color: var(--ad-muted); }
.overview, .detail { border: 1px solid var(--ad-border); border-radius: 8px; background: var(--ad-harbor); min-width: 0; }
.metric-strip, .token-parts { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); }
.metric-cell { display: flex; flex-direction: column; gap: 4px; text-align: left; padding: 16px 20px; min-width: 0; position: relative; }
.metric-cell:hover { background: var(--ad-hover); }
.metric-cell[aria-pressed='true'] { box-shadow: inset 0 -2px 0 var(--ad-text); }
.metric-cell > span { display: flex; align-items: center; gap: 6px; font-size: 12px; line-height: 20px; color: var(--ad-muted); }
.metric-cell strong { font-size: 26px; line-height: 32px; font-weight: 600; font-variant-numeric: tabular-nums; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
.token-parts { border-top: 1px solid var(--ad-border); }
.token-parts > div { display: flex; flex-direction: column; gap: 4px; padding: 12px 20px; }
.token-parts span { font-size: 11px; color: var(--ad-muted); }
.token-parts strong { font-size: 16px; font-variant-numeric: tabular-nums; font-weight: 500; }
.breakdown-divider { position: relative; height: 1px; background: var(--ad-border); }
.breakdown-divider button { position: absolute; z-index: 2; left: 50%; top: 0; transform: translate(-50%, -50%); width: 24px; height: 24px; border-radius: 50%; background: var(--ad-float-solid); border: 1px solid var(--ad-border-strong); display: grid; place-items: center; }
.breakdown-divider svg { width: 14px; height: 14px; stroke: var(--ad-muted); stroke-width: 1.4; fill: none; }
.breakdown-divider .rotated { transform: rotate(180deg); }
.detail-toolbar { min-height: 48px; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px; border-bottom: 1px solid var(--ad-border); padding: 0 16px; }
.tabs { display: flex; align-self: stretch; gap: 20px; }
.tab { display: inline-flex; align-items: center; gap: 8px; padding: 0; height: 48px; border-bottom: 2px solid transparent; color: var(--ad-muted); font-size: 13px; white-space: nowrap; }
.tab[aria-selected='true'] { border-bottom-color: var(--ad-text); color: var(--ad-text); font-weight: 600; }
.count { padding: 0 6px; border-radius: 4px; background: var(--ad-hover); color: var(--ad-muted); font-size: 11px; line-height: 20px; font-weight: 400; font-variant-numeric: tabular-nums; }
.column-toggle { display: inline-flex; align-items: center; gap: 6px; color: var(--ad-muted); font-size: 12px; }
.column-toggle input { accent-color: var(--ad-text); }
.table-wrap { overflow-x: auto; max-width: 100%; }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
.logs-table { table-layout: fixed; min-width: 1100px; }
.logs-table:has(th:nth-child(12)) { min-width: 1480px; }
.time-col { width: 152px; }.app-col { width: 200px; }.provider-col { width: 190px; }.model-col { width: 220px; }
th, td { padding: 10px 16px; border-bottom: 1px solid var(--ad-border); text-align: left; white-space: nowrap; height: 40px; }
th { color: var(--ad-muted); font-size: 11px; font-weight: 500; background: var(--ad-float-inset); }
tbody tr:last-child td { border-bottom: none; }
tbody tr:hover { background: var(--ad-hover); }
.logs-table tbody tr { cursor: pointer; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.time-cell { color: var(--ad-muted); font-size: 11px; }
.ellipsis, .model-link { max-width: 100%; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.model-link { color: var(--ad-text); text-align: left; font-family: var(--ad-mono); font-size: 11px; }
.model-link:hover { text-decoration: underline; }
.muted { color: var(--ad-muted); }
.row-action { position: relative; width: 40px; padding-left: 4px; padding-right: 12px; text-align: right; }
.row-action button { color: var(--ad-muted); }
.status { display: inline-flex; align-items: center; gap: 6px; color: var(--ad-muted); font-size: 11px; }
.status i { width: 5px; height: 5px; border-radius: 50%; background: currentColor; }
.status.success { color: var(--ad-success); }.status.error { color: var(--ad-error); }
.pager { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 12px 16px; border-top: 1px solid var(--ad-border); font-size: 12px; color: var(--ad-muted); }
.pager > span:first-child { margin-right: auto; }
.pager label { display: inline-flex; align-items: center; gap: 6px; }
.pager select { height: 28px; padding: 0 4px; min-width: 48px; font-size: 12px; }
.page-position { margin: 0 12px; font-variant-numeric: tabular-nums; }
.groups-table { min-width: 640px; }.groups-table td:first-child { max-width: 360px; overflow: hidden; text-overflow: ellipsis; }
.share-cell { min-width: 180px; }.share-track { display: inline-block; width: 72px; height: 4px; margin-right: 12px; vertical-align: middle; background: var(--ad-hover); border-radius: 2px; overflow: hidden; }.share-track i { display: block; height: 100%; background: var(--usage-chart); }
.empty-cell { padding: 32px; height: 140px; text-align: center; color: var(--ad-muted); }
.empty-cell strong, .empty-cell span { display: block; font-weight: 400; margin-bottom: 8px; }.empty-cell button { color: var(--ad-text); text-decoration: underline; }
.source-note { margin: 0; font-size: 11px; color: var(--ad-muted); }
.usage-drawer { width: 560px; max-width: calc(100vw - 24px); padding: 24px; overflow-y: auto; outline: none; }
.usage-drawer header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 24px; }.usage-drawer h2 { margin: 0; font-size: 18px; }.section-label { margin: 24px 0 8px; font-size: 13px; }.hint { margin: 0 0 12px; color: var(--ad-muted); font-size: 12px; line-height: 20px; }
.intervals { display: flex; gap: 8px; }.intervals button { padding: 0 12px; }.intervals button[aria-pressed='true'] { background: var(--ad-selected); color: var(--ad-text); }.pricing-wrap { overflow-x: auto; }.pricing-wrap table { min-width: 520px; }.drawer-eyebrow { display: flex; align-items: center; gap: 16px; color: var(--ad-muted); font-size: 12px; }.request-model { margin: 12px 0 20px; font-size: 18px; overflow-wrap: anywhere; }.request-totals { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; padding: 16px; background: var(--ad-hover); border-radius: 6px; }.request-totals div { display: flex; flex-direction: column; gap: 8px; }.request-totals span { font-size: 12px; color: var(--ad-muted); }.request-totals strong { font-size: 20px; font-variant-numeric: tabular-nums; }.request-facts { display: grid; grid-template-columns: 88px minmax(0, 1fr); gap: 16px; margin: 24px 0; font-size: 12px; }.request-facts dt { color: var(--ad-muted); }.request-facts dd { margin: 0; overflow-wrap: anywhere; }.request-facts code { font-family: var(--ad-mono); font-size: 11px; }
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
.spinning { animation: usage-spin 1s linear infinite; }@keyframes usage-spin { to { transform: rotate(360deg); } }
@media(max-width: 1100px) { .scope-date { display: none; }.sync-time { display: none; }.app-col { width: 168px; }.provider-col { width: 168px; }.model-col { width: 200px; } }
@media(max-width: 720px) { .head { padding: 16px; flex-wrap: wrap; }.filters { padding: 0 16px 12px; }.content { padding: 0 16px 16px; gap: 16px; }.metric-strip, .token-parts { grid-template-columns: repeat(2,minmax(0,1fr)); }.metric-cell { padding: 12px 16px; }.metric-cell strong { font-size: 22px; }.metric-cell:nth-child(-n+2) { border-bottom: 1px solid var(--ad-border); }.tabs { gap: 12px; }.tab { font-size: 12px; }.column-toggle { padding-bottom: 8px; }.select-filter select { max-width: 160px; }.popover-body { max-width: calc(100vw - 64px); }.filter-divider { display: none; } }
@media(prefers-reduced-motion: reduce) { .spinning { animation: none; } }
</style>
