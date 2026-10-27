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
  return cell.tokens
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
