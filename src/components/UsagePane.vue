<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  buildSpendChart,
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
  heatMetric,
  parseUsageRefreshMs,
  pointMetric,
  resolveUsageRange,
  toDatetimeLocal,
  toSpendPoints,
  type UsageMetric,
  type UsagePreset
} from '../lib/usage'
import type { UsageDashboard, UsageLogPage, UsageQuery } from '../lib/types'
import SpendChart from './SpendChart.vue'

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
const hasTrend = computed(() => (dash.value?.points.length ?? 0) > 0)
const chart = computed(() => buildSpendChart(toSpendPoints(dash.value?.points ?? [])))
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
const heatMax = computed(() =>
  Math.max(0, ...(dash.value?.heatmap ?? []).map((cell) => heatMetric(cell, metric.value)))
)
const pageCount = computed(() => Math.max(1, Math.ceil((logs.value.total || 0) / USAGE_PAGE_SIZE)))
const metricBars = computed(() => {
  const points = dash.value?.points ?? []
  const values = points.map((row) => pointMetric(row, metric.value))
  const max = Math.max(0.0001, ...values)
  return points.map((row, index) => ({
    label: row.label,
    value: values[index] ?? 0,
    height: Math.max(2, ((values[index] ?? 0) / max) * 100)
  }))
})

function metricText(id: UsageMetric) {
  const row = dash.value
  if (id === 'cost') return formatSpendCost(row?.totalCostUsd)
  if (id === 'requests') return formatExactTokens(row?.totalRequests)
  if (id === 'cache') return formatCacheHit(row?.cacheHitPercent)
  return formatTokenCount(row?.totalTokens)
}

function heatStyle(value: number) {
  if (heatMax.value <= 0 || value <= 0) {
    return { background: 'var(--ad-harbor)' }
  }
  const t = Math.min(1, value / heatMax.value)
  return { background: `color-mix(in srgb, var(--ad-primary) ${Math.round(12 + t * 72)}%, var(--ad-harbor))` }
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
    const next = await api.usageDashboard(query.value)
    if (token !== seq) return
    if (!next.dbOk && next.message?.includes('占用') && dash.value) {
      dash.value = { ...dash.value, message: next.message, dbOk: false }
    } else {
      dash.value = next
    }
    logs.value = await api.usageLogs({ ...query.value, page: page.value, pageSize: USAGE_PAGE_SIZE })
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

agoTimer = setInterval(() => {
  nowTick.value = Date.now()
}, 15_000)

onUnmounted(() => {
  stopPoll()
  if (agoTimer) clearInterval(agoTimer)
})
</script>

<template>
  <section class="pane" aria-label="使用量">
    <header class="head">
      <div>
        <h1>使用量</h1>
        <p>本机会话 token，含 Claude 副号。不请求官方额度。</p>
      </div>
      <div class="head-actions">
        <button type="button" class="btn btn-ghost btn-small" :disabled="loading" @click="reload(true)">
          {{ loading ? '同步中…' : `立即同步 · ${syncLabel}` }}
        </button>
        <button type="button" class="btn btn-ghost btn-small" :disabled="exporting" @click="exportToCcswitch">
          {{ exporting ? '写入中…' : '同步到 CC Switch' }}
        </button>
        <button type="button" class="btn btn-ghost btn-small" @click="settingsOpen = true">设置</button>
      </div>
    </header>

    <p v-if="banner" class="banner" role="status">{{ banner }}</p>
    <p v-if="exportNotice" class="banner" role="status">{{ exportNotice }}</p>

    <div class="filters">
      <div class="chips" role="group" aria-label="时间范围">
        <button
          v-for="item in USAGE_PRESETS"
          :key="item.id"
          type="button"
          class="chip"
          :class="{ on: preset === item.id }"
          @click="pickPreset(item.id)"
        >
          {{ item.label }}
        </button>
        <button type="button" class="chip" :class="{ on: preset === 'custom' }" @click="setCustom">自定义</button>
      </div>
      <div v-if="preset === 'custom'" class="custom">
        <label>
          开始
          <input :value="toDatetimeLocal(start)" type="datetime-local" @change="onCustomStart(($event.target as HTMLInputElement).value)" />
        </label>
        <label>
          结束
          <input :value="toDatetimeLocal(end)" type="datetime-local" @change="onCustomEnd(($event.target as HTMLInputElement).value)" />
        </label>
      </div>
      <div class="chips" role="group" aria-label="应用">
        <button type="button" class="chip" :class="{ on: appFilter === 'all' }" @click="appFilter = 'all'">全部</button>
        <button
          v-for="app in dash?.apps ?? []"
          :key="app.id"
          type="button"
          class="chip"
          :class="{ on: appFilter === app.id }"
          @click="appFilter = app.id"
        >
          {{ app.label }}
        </button>
      </div>
      <div class="selects">
        <label>
          供应商
          <select v-model="providerName">
            <option value="">全部供应商</option>
            <option v-for="row in dash?.providers ?? []" :key="row.name" :value="row.name">{{ row.name }}</option>
          </select>
        </label>
        <label>
          模型
          <select v-model="model">
            <option value="">全部模型</option>
            <option v-for="row in dash?.models ?? []" :key="row.name" :value="row.name">{{ row.name }}</option>
          </select>
        </label>
      </div>
    </div>

    <div class="cards" role="group" aria-label="汇总指标">
      <button
        v-for="item in METRICS"
        :key="item.id"
        type="button"
        class="card"
        :class="{ on: metric === item.id }"
        @click="metric = item.id"
      >
        <span>{{ item.label }}</span>
        <strong>{{ metricText(item.id) }}</strong>
      </button>
    </div>

    <div class="viz">
      <SpendChart
        v-if="metric === 'tokens'"
        :chart="chart"
        :has-data="hasTrend"
        :loading="loading"
        :range-label="rangeLabel"
        :empty-text="emptyText"
      />
      <div v-else class="metric-chart">
        <div class="chart-head">
          <p>{{ METRICS.find((item) => item.id === metric)?.label }}</p>
          <span>{{ rangeLabel }}</span>
        </div>
        <div v-if="hasTrend" class="bars">
          <div v-for="bar in metricBars" :key="bar.label" class="bar">
            <i :style="{ height: bar.height + '%' }" />
            <span>{{ bar.label }}</span>
          </div>
        </div>
        <p v-else class="empty">{{ emptyText }}</p>
      </div>
      <div class="heat">
        <div class="chart-head">
          <p>热力图</p>
          <span>按日</span>
        </div>
        <div v-if="(dash?.heatmap.length ?? 0) > 0" class="heat-grid">
          <span
            v-for="cell in dash?.heatmap ?? []"
            :key="cell.ts"
            class="heat-cell"
            :style="heatStyle(heatMetric(cell, metric))"
            :title="`${cell.label} · ${formatTokenCount(cell.tokens)} · ${formatSpendCost(cell.costUsd)} · ${cell.requests} 次`"
          />
        </div>
        <p v-else class="empty">{{ emptyText }}</p>
      </div>
    </div>

    <div class="detail">
      <div class="tabs" role="tablist" aria-label="明细">
        <button
          v-for="item in TABS"
          :key="item.id"
          type="button"
          role="tab"
          class="tab"
          :class="{ on: tab === item.id }"
          :aria-selected="tab === item.id"
          @click="tab = item.id"
        >
          {{ item.label }}
        </button>
      </div>

      <div v-if="tab === 'logs'" class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>时间</th>
              <th>应用</th>
              <th>供应商</th>
              <th>模型</th>
              <th class="num">Tokens</th>
              <th class="num">成本</th>
              <th class="num">状态</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="!logs.rows.length">
              <td colspan="7" class="empty-cell">{{ emptyText }}</td>
            </tr>
            <tr v-for="row in logs.rows" :key="row.requestId">
              <td>{{ formatLogTime(row.createdAt) }}</td>
              <td>{{ row.appLabel }}</td>
              <td>{{ row.providerName || '-' }}</td>
              <td>{{ row.model || '-' }}</td>
              <td class="num">{{ formatExactTokens(row.inputTokens + row.outputTokens + row.cacheReadTokens + row.cacheCreationTokens) }}</td>
              <td class="num">{{ row.unpriced ? '未定价' : formatSpendCost(row.costUsd) }}</td>
              <td class="num">{{ row.statusCode }}</td>
            </tr>
          </tbody>
        </table>
        <div class="pager">
          <button type="button" class="btn btn-ghost btn-small" :disabled="page <= 0" @click="page = Math.max(0, page - 1)">上一页</button>
          <span>{{ page + 1 }} / {{ pageCount }}</span>
          <button type="button" class="btn btn-ghost btn-small" :disabled="page + 1 >= pageCount" @click="page += 1">下一页</button>
        </div>
      </div>

      <div v-else class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>{{ tab === 'providers' ? '供应商' : '模型' }}</th>
              <th class="num">请求</th>
              <th class="num">Tokens</th>
              <th class="num">成本</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="!(tab === 'providers' ? dash?.providers : dash?.models)?.length">
              <td colspan="4" class="empty-cell">{{ emptyText }}</td>
            </tr>
            <tr v-for="row in (tab === 'providers' ? dash?.providers : dash?.models) ?? []" :key="row.name">
              <td>{{ row.name }}</td>
              <td class="num">{{ row.requestCount }}</td>
              <td class="num">{{ formatExactTokens(row.tokens) }}</td>
              <td class="num">{{ row.unpriced ? '未定价' : formatSpendCost(row.costUsd) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div v-if="settingsOpen" class="ad-mask ad-mask--flush" @click.self="settingsOpen = false">
      <aside class="ad-drawer drawer" role="dialog" aria-modal="true" aria-label="用量设置">
        <header>
          <h2>用量设置</h2>
          <button type="button" class="btn btn-ghost btn-small" @click="settingsOpen = false">关闭</button>
        </header>
        <p class="section">扫描间隔</p>
        <div class="chips">
          <button
            v-for="ms in USAGE_REFRESH_MS"
            :key="ms"
            type="button"
            class="chip"
            :class="{ on: refreshMs === ms }"
            @click="setRefresh(ms)"
          >
            {{ ms / 1000 }} 秒
          </button>
        </div>
        <p class="section">同步到 CC Switch</p>
        <p class="hint">把 Claude 副号 jsonl 写入 CC Switch 库。应用芯片合并进 Claude，供应商列按账号分开。已有记录不会覆盖。</p>
        <button type="button" class="btn btn-ghost btn-small" :disabled="exporting" @click="exportToCcswitch">
          {{ exporting ? '写入中…' : '同步副号到 CC Switch' }}
        </button>
        <p class="section">定价（只读）</p>
        <p class="hint">改价请到 CC Switch。Dock 只读这份价表给副号估算成本。</p>
        <table v-if="dash?.pricing.length">
          <thead>
            <tr>
              <th>模型</th>
              <th class="num">输入 / M</th>
              <th class="num">输出 / M</th>
              <th class="num">命中 / M</th>
              <th class="num">写入 / M</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in dash.pricing" :key="row.modelId">
              <td>{{ row.displayName || row.modelId }}</td>
              <td class="num">{{ formatSpendCost(row.inputPerMillion) }}</td>
              <td class="num">{{ formatSpendCost(row.outputPerMillion) }}</td>
              <td class="num">{{ formatSpendCost(row.cacheReadPerMillion) }}</td>
              <td class="num">{{ formatSpendCost(row.cacheCreationPerMillion) }}</td>
            </tr>
          </tbody>
        </table>
        <p v-else class="empty">打开 CC Switch 扫描后才会有定价表</p>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 16px 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  background: var(--ad-ink);
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.head h1 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.head p {
  margin: 4px 0 0;
  font-size: var(--ad-font-sm);
  color: var(--ad-muted);
}

.head-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.banner {
  margin: 0;
  padding: 8px 12px;
  border-radius: 8px;
  background: var(--ad-hover);
  border: 1px solid var(--ad-border);
  color: var(--ad-text);
  font-size: var(--ad-font-sm);
}

.filters,
.viz,
.detail {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chips,
.selects,
.cards,
.pager,
.head-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.chip,
.tab {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  background: var(--ad-hover);
  color: var(--ad-muted);
  font: inherit;
  font-size: var(--ad-font-sm);
}

.chip.on,
.tab.on {
  background: var(--ad-selected);
  color: var(--ad-text);
  border-color: var(--ad-border-strong);
}

.custom,
.selects {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.custom label,
.selects label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--ad-font-sm);
  color: var(--ad-muted);
}

.custom input,
.selects select {
  height: 28px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
  background: var(--ad-harbor);
  color: var(--ad-text);
  padding: 0 8px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  min-height: 72px;
  padding: 12px;
  border-radius: 10px;
  border: 1px solid var(--ad-border);
  background: var(--ad-harbor);
  color: inherit;
  text-align: left;
}

.card span {
  font-size: 12px;
  color: var(--ad-muted);
}

.card strong {
  font-size: 20px;
  font-variant-numeric: tabular-nums;
}

.card.on {
  border-color: var(--ad-border-strong);
  background: var(--ad-selected);
}

.viz {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(220px, 1fr);
  gap: 12px;
}

.metric-chart,
.heat {
  min-height: 220px;
  padding: 12px;
  border: 1px solid var(--ad-border);
  border-radius: 10px;
  background: var(--ad-harbor);
}

.chart-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 8px;
}

.chart-head p {
  margin: 0;
  font-weight: 600;
}

.chart-head span,
.empty,
.hint {
  font-size: 12px;
  color: var(--ad-muted);
}

.bars {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  height: 160px;
  overflow-x: auto;
}

.bar {
  flex: 1;
  min-width: 18px;
  height: 100%;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: center;
  gap: 4px;
}

.bar i {
  display: block;
  width: 100%;
  border-radius: 4px 4px 0 0;
  background: var(--ad-primary);
}

.bar span {
  font-size: 10px;
  color: var(--ad-faint);
  white-space: nowrap;
}

.heat-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.heat-cell {
  width: 14px;
  height: 14px;
  border-radius: 3px;
}

.tabs {
  display: flex;
  gap: 4px;
}

.table-wrap {
  overflow: auto;
  border: 1px solid var(--ad-border);
  border-radius: 10px;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--ad-font-sm);
}

th,
td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--ad-border);
  text-align: left;
  white-space: nowrap;
}

th {
  color: var(--ad-muted);
  font-weight: 500;
  background: var(--ad-harbor);
}

.num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.empty-cell,
.empty {
  text-align: center;
  color: var(--ad-muted);
  padding: 16px;
}

.pager {
  justify-content: flex-end;
  padding: 8px 10px;
}

.drawer header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.section {
  margin: 16px 0 8px;
  font-weight: 600;
}

.hint {
  margin: 0 0 8px;
}

@media (max-width: 960px) {
  .cards,
  .viz {
    grid-template-columns: 1fr;
  }
}
</style>
