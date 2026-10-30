import {
  cacheHitPercent,
  pad2,
  resolveSpendRange,
  SPEND_PRESETS,
  type SpendPreset
} from './grokSpend.ts'
import type {
  GrokSpendPoint,
  UsageDashboard,
  UsageHeatCell,
  UsageLogPage,
  UsagePoint
} from './types.ts'

export type UsagePreset = SpendPreset | 'all'
export type UsageMetric = 'tokens' | 'cost' | 'requests' | 'cache'

export const USAGE_PRESETS: { id: UsagePreset; label: string }[] = [
  ...SPEND_PRESETS,
  { id: 'all', label: '全部' }
]

export const USAGE_REFRESH_MS = [5_000, 10_000, 30_000, 60_000] as const
export const USAGE_REFRESH_KEY = 'ad-usage-refresh-ms'
export const USAGE_PAGE_SIZE = 20
export const DESKTOP_USAGE_HINT = '请在桌面窗口里查看用量'

export function resolveUsageRange(
  preset: UsagePreset,
  now = Date.now(),
  custom?: { start: number; end: number }
) {
  if (preset === 'all') {
    return { start: 0, end: Math.floor(now / 1000) + 1, followEnd: true, label: '全部' }
  }
  return resolveSpendRange(preset, now, custom)
}

export function parseUsageRefreshMs(value: unknown): number {
  const n = Number(value)
  return (USAGE_REFRESH_MS as readonly number[]).includes(n) ? n : 60_000
}

export function formatSyncAgo(fetchedAt: string, now = Date.now()): string {
  const t = Date.parse(fetchedAt)
  if (!Number.isFinite(t)) return ''
  const sec = Math.max(0, Math.floor((now - t) / 1000))
  if (sec < 15) return '刚刚'
  if (sec < 60) return `${sec} 秒前`
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min} 分钟前`
  const hour = Math.floor(min / 60)
  if (hour < 24) return `${hour} 小时前`
  return `${Math.floor(hour / 24)} 天前`
}

export function emptyUsageDashboard(message = DESKTOP_USAGE_HINT): UsageDashboard {
  return {
    ok: true,
    dbOk: false,
    message,
    totalCostUsd: 0,
    totalRequests: 0,
    totalTokens: 0,
    cacheHitPercent: 0,
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
    points: [],
    heatmap: [],
    providers: [],
    models: [],
    apps: [],
    pricing: [],
    fetchedAt: new Date().toISOString()
  }
}

export function emptyUsageLogs(): UsageLogPage {
  return { total: 0, page: 0, pageSize: USAGE_PAGE_SIZE, rows: [] }
}

export function toSpendPoints(points: UsagePoint[]): GrokSpendPoint[] {
  return points.map((row) => ({
    ts: row.ts,
    label: row.label,
    inputTokens: row.inputTokens,
    outputTokens: row.outputTokens,
    cacheReadTokens: row.cacheReadTokens,
    cacheCreationTokens: row.cacheCreationTokens,
    costUsd: row.costUsd
  }))
}

export function pointMetric(row: UsagePoint, metric: UsageMetric): number {
  if (metric === 'cost') return row.costUsd
  if (metric === 'requests') return row.requests
  if (metric === 'cache') return cacheHitPercent(row.inputTokens, row.cacheReadTokens, row.cacheCreationTokens)
  return row.tokens
}

export function heatMetric(cell: UsageHeatCell, metric: UsageMetric): number {
  if (metric === 'cost') return cell.costUsd
  if (metric === 'requests') return cell.requests
  if (metric === 'cache') return cacheHitPercent(cell.inputTokens, cell.cacheReadTokens, cell.cacheCreationTokens)
  return cell.tokens
}

export const USAGE_METRIC_LABELS: Record<UsageMetric, string> = {
  cost: '总成本', requests: '总请求', tokens: '真实消耗 Tokens', cache: '缓存命中率'
}

export function usageDateKey(date: Date) {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`
}

function addCalendarDays(date: Date, days: number) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days)
}

// Calendar layout and quantile shading follow CC Switch v4.0.21. See THIRD_PARTY_NOTICES.md.
export function buildUsageCalendar(rows: UsageHeatCell[], metric: UsageMetric, end: number, page = 0) {
  const today = new Date(end * 1000)
  const monday = addCalendarDays(today, -((today.getDay() + 6) % 7))
  const start = addCalendarDays(monday, -364 - page * 371)
  const byDay = new Map(rows.map(row => [usageDateKey(new Date(row.ts * 1000)), row]))
  const values = rows.filter(row => metric === 'cache'
    ? row.inputTokens + row.cacheReadTokens + row.cacheCreationTokens > 0
    : heatMetric(row, metric) > 0).map(row => heatMetric(row, metric)).sort((a, b) => a - b)
  const thresholds = [0.25, 0.5, 0.75].map(q => values[Math.min(values.length - 1, Math.floor(values.length * q))] ?? Infinity)
  const weeks = Array.from({ length: 53 }, (_, week) => Array.from({ length: 7 }, (_, day) => {
    const date = addCalendarDays(start, week * 7 + day)
    const key = usageDateKey(date)
    const row = byDay.get(key)
    const future = date.getTime() > today.getTime()
    const known = row && !future && (metric === 'cache'
      ? row.inputTokens + row.cacheReadTokens + row.cacheCreationTokens > 0
      : heatMetric(row, metric) > 0)
    const value = known ? heatMetric(row!, metric) : null
    const level = value == null ? 0 : value <= thresholds[0] ? 1 : value <= thresholds[1] ? 2 : value <= thresholds[2] ? 3 : 4
    return { key, date, row, value, level, future }
  }))
  let lastLabel = -4
  let lastMonth = -1
  const months = weeks.map((week, index) => {
    const month = week[0].date.getMonth()
    const changed = month !== lastMonth
    lastMonth = month
    if (!changed || index - lastLabel < 3) return ''
    lastLabel = index
    return `${month + 1}月`
  })
  const visible = weeks.flat().filter(day => !day.future)
  const first = rows.reduce((min, row) => Math.min(min, row.ts), end)
  return {
    weeks, months, start: usageDateKey(start), end: visible.at(-1)!.key,
    activeDays: visible.filter(day => (day.row?.requests ?? 0) > 0).length,
    requests: visible.reduce((sum, day) => sum + (day.row?.requests ?? 0), 0),
    hasOlder: first < start.getTime() / 1000
  }
}

export function completeUsagePoints(rows: UsagePoint[], start: number, end: number) {
  if (end <= start) return []
  const hourly = end - start <= 36 * 3600
  if ((end - start) / (hourly ? 3600 : 86400) > 800) return [...rows].sort((a, b) => a.ts - b.ts)
  const byTime = new Map(rows.map(row => [row.ts, row]))
  const first = hourly ? Math.floor(start / 3600) * 3600 : new Date(new Date(start * 1000).setHours(0, 0, 0, 0)).getTime() / 1000
  const result: UsagePoint[] = []
  for (let ts = first; ts < end && result.length < 800; ts = hourly ? ts + 3600 : addCalendarDays(new Date(ts * 1000), 1).getTime() / 1000) {
    const date = new Date(ts * 1000)
    const label = hourly ? `${pad2(date.getHours())}:00` : `${date.getMonth() + 1}/${date.getDate()}`
    result.push(byTime.get(ts) ?? { ts, label, inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, tokens: 0, costUsd: 0, requests: 0 })
  }
  return result
}

export function formatLogTime(ts: number): string {
  const date = new Date(ts * 1000)
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())} ${pad2(date.getHours())}:${pad2(date.getMinutes())}`
}

export function toDatetimeLocal(ts: number): string {
  const date = new Date(ts * 1000)
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}T${pad2(date.getHours())}:${pad2(date.getMinutes())}`
}

export function fromDatetimeLocal(value: string): number {
  const [date, time] = value.split('T')
  if (!date) return Math.floor(Date.now() / 1000)
  const [year, month, day] = date.split('-').map(Number)
  const [hours, minutes] = (time || '00:00').split(':').map(Number)
  if (!year || !month || !day) return Math.floor(Date.now() / 1000)
  return Math.floor(new Date(year, month - 1, day, hours || 0, minutes || 0, 0).getTime() / 1000)
}
