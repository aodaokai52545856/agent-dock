import type { GrokSpend, GrokSpendPoint } from './types'

export const SPEND_PANEL_WIDTH = 860
export const SPEND_CHART_WIDTH = 812
export const SPEND_CHART_HEIGHT = 248

export type SpendPreset = 'today' | '1d' | '7d' | '14d' | '30d' | 'custom'

export const SPEND_PRESETS: { id: SpendPreset; label: string }[] = [
  { id: 'today', label: '当天' },
  { id: '1d', label: '1d' },
  { id: '7d', label: '7d' },
  { id: '14d', label: '14d' },
  { id: '30d', label: '30d' }
]

function trimFloat(value: number, digits: number) {
  return value
    .toFixed(digits)
    .replace(/(\.\d*?[1-9])0+$/, '$1')
    .replace(/\.0+$/, '')
}

export function formatTokenCount(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value) || value <= 0) return '0'
  const n = Math.round(value)
  if (n >= 100_000_000) {
    const v = n / 100_000_000
    return `${trimFloat(v, v >= 10 ? 1 : 2)}亿`
  }
  if (n >= 10_000) {
    const v = n / 10_000
    return `${trimFloat(v, v >= 1000 ? 0 : 1)}万`
  }
  return String(n)
}

export function formatExactTokens(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value) || value <= 0) return '0'
  return Math.round(value).toLocaleString('zh-CN')
}

export function formatSpendLine(spend: GrokSpend | null, _loading = false) {
  if (!spend?.ok) return 'token消耗'
  return `${formatTokenCount(spend.totalTokens)} token`
}

export function formatSpendCost(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value) || value <= 0) return '$0'
  return `$${trimFloat(value, value >= 10 ? 2 : 4)}`
}

export function formatCacheHit(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value) || value <= 0) return '0%'
  return `${trimFloat(value, 1)}%`
}

export function cacheHitPercent(input: number, cacheRead: number, cacheCreation = 0) {
  const denom = input + cacheRead + cacheCreation
  if (denom <= 0) return 0
  return (cacheRead / denom) * 100
}

export function pad2(value: number) {
  return String(value).padStart(2, '0')
}

export function toLocalDateTime(ts: number) {
  const date = new Date(ts * 1000)
  return {
    date: `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`,
    time: `${pad2(date.getHours())}:${pad2(date.getMinutes())}`
  }
}

export function fromLocalDateTime(date: string, time: string) {
  const [year, month, day] = date.split('-').map(Number)
  const [hours, minutes] = time.split(':').map(Number)
  if (!year || !month || !day) return Math.floor(Date.now() / 1000)
  return Math.floor(new Date(year, month - 1, day, hours || 0, minutes || 0, 0).getTime() / 1000)
}

export function startOfLocalDay(now = Date.now()) {
  const date = new Date(now)
  date.setHours(0, 0, 0, 0)
  return Math.floor(date.getTime() / 1000)
}

export function resolveSpendRange(
  preset: SpendPreset,
  now = Date.now(),
  custom?: { start: number; end: number }
) {
  const end = Math.floor(now / 1000)
  const today = startOfLocalDay(now)
  if (preset === 'today') return { start: today, end, followEnd: true, label: '当天' }
  if (preset === '1d') return { start: end - 86_400, end, followEnd: true, label: '1d' }
  if (preset === '7d') return { start: today - 6 * 86_400, end, followEnd: true, label: '7d' }
  if (preset === '14d') return { start: today - 13 * 86_400, end, followEnd: true, label: '14d' }
  if (preset === '30d') return { start: today - 29 * 86_400, end, followEnd: true, label: '30d' }
  const start = custom?.start ?? today
  const stop = custom?.end ?? end
  return {
    start: Math.min(start, stop),
    end: Math.max(start + 1, stop),
    followEnd: false,
    label: '自定义'
  }
}

export interface SpendChartSeries {
  key: string
  label: string
  color: string
  dashed?: boolean
  path: string
  area: string
  axis: 'token' | 'cost'
}

export interface SpendChartHit {
  x: number
  label: string
  values: { key: string; label: string; color: string; text: string }[]
  dots: { key: string; x: number; y: number; color: string }[]
}

export interface SpendChartModel {
  series: SpendChartSeries[]
  maxLabel: string
  maxCost: string
  ticks: { x: number; label: string }[]
  grid: { y: number; label: string }[]
  costTicks: { y: number; label: string }[]
  hits: SpendChartHit[]
  bottom: number
  padLeft: number
  padRight: number
  padTop: number
}

export function nearestHitIndex(hits: SpendChartHit[], x: number) {
  if (!hits.length) return -1
  let best = 0
  let dist = Number.POSITIVE_INFINITY
  hits.forEach((hit, index) => {
    const next = Math.abs(hit.x - x)
    if (next < dist) {
      dist = next
      best = index
    }
  })
  return best
}

export function dateInRange(value: string, start: string, end: string) {
  const from = start <= end ? start : end
  const to = start <= end ? end : start
  return value >= from && value <= to
}

export function formatFieldDate(date: string) {
  return date.replace(/-/g, '/')
}

const PAD = { l: 48, r: 48, t: 18, b: 32 }

function catmullPath(points: { x: number; y: number }[], minY?: number, maxY?: number) {
  if (!points.length) return ''
  if (points.length === 1) return `M ${points[0].x.toFixed(1)} ${points[0].y.toFixed(1)}`
  const clampY = (y: number) => {
    if (minY == null || maxY == null) return y
    return Math.min(maxY, Math.max(minY, y))
  }
  let d = `M ${points[0].x.toFixed(1)} ${points[0].y.toFixed(1)}`
  for (let i = 0; i < points.length - 1; i += 1) {
    const p0 = points[i - 1] ?? points[i]
    const p1 = points[i]
    const p2 = points[i + 1]
    const p3 = points[i + 2] ?? p2
    const c1x = p1.x + (p2.x - p0.x) / 6
    const c1y = clampY(p1.y + (p2.y - p0.y) / 6)
    const c2x = p2.x - (p3.x - p1.x) / 6
    const c2y = clampY(p2.y - (p3.y - p1.y) / 6)
    d += ` C ${c1x.toFixed(1)} ${c1y.toFixed(1)}, ${c2x.toFixed(1)} ${c2y.toFixed(1)}, ${p2.x.toFixed(1)} ${p2.y.toFixed(1)}`
  }
  return d
}

function areaPath(points: { x: number; y: number }[], bottom: number, top: number) {
  if (!points.length) return ''
  const line = catmullPath(points, top, bottom)
  const first = points[0]
  const last = points[points.length - 1]
  return `${line} L ${last.x.toFixed(1)} ${bottom.toFixed(1)} L ${first.x.toFixed(1)} ${bottom.toFixed(1)} Z`
}

export function buildSpendChart(rows: GrokSpendPoint[]): SpendChartModel {
  const points = rows.length ? rows : [{ ts: 0, label: '', inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0 }]
  const input = points.map((row) => row.inputTokens)
  const output = points.map((row) => row.outputTokens)
  const cache = points.map((row) => row.cacheReadTokens)
  const created = points.map((row) => row.cacheCreationTokens)
  const cost = points.map((row) => row.costUsd)
  const maxTok = Math.max(1, ...input, ...output, ...cache, ...created)
  const maxCost = Math.max(0.0001, ...cost)
  const innerW = SPEND_CHART_WIDTH - PAD.l - PAD.r
  const innerH = SPEND_CHART_HEIGHT - PAD.t - PAD.b
  const bottom = PAD.t + innerH
  const last = Math.max(points.length - 1, 1)
  const xAt = (i: number) => PAD.l + (innerW * i) / last
  const yTok = (v: number) => PAD.t + innerH * (1 - v / maxTok)
  const yCost = (v: number) => PAD.t + innerH * (1 - v / maxCost)
  const xy = (values: number[], y: (v: number) => number) => values.map((value, i) => ({ x: xAt(i), y: y(value) }))
  const tokenSeries = [
    { key: 'cache', label: '缓存命中', color: '#a78bfa', values: cache },
    { key: 'created', label: '缓存创建', color: '#fb923c', values: created, dashed: true },
    { key: 'input', label: '输入', color: '#60a5fa', values: input },
    { key: 'output', label: '输出', color: '#34d399', values: output }
  ]
  const series: SpendChartSeries[] = tokenSeries.map((item) => {
    const pts = xy(item.values, yTok)
    return {
      key: item.key,
      label: item.label,
      color: item.color,
      dashed: item.dashed,
      path: catmullPath(pts, PAD.t, bottom),
      area: item.dashed ? '' : areaPath(pts, bottom, PAD.t),
      axis: 'token' as const
    }
  })
  const costPts = xy(cost, yCost)
  series.unshift({
    key: 'cost',
    label: '成本',
    color: '#f87171',
    dashed: true,
    path: catmullPath(costPts, PAD.t, bottom),
    area: '',
    axis: 'cost'
  })
  const tickCount = Math.min(6, points.length)
  const ticks = Array.from({ length: tickCount }, (_, i) => {
    const index = tickCount === 1 ? 0 : Math.round((i * (points.length - 1)) / (tickCount - 1))
    return { x: xAt(index), label: points[index]?.label ?? '' }
  })
  const grid = [1, 0.75, 0.5, 0.25, 0].map((part) => ({
    y: PAD.t + innerH * (1 - part),
    label: part === 0 ? '0' : formatTokenCount(maxTok * part)
  }))
  const costTicks = [1, 0.5, 0].map((part) => ({
    y: PAD.t + innerH * (1 - part),
    label: part === 0 ? '$0' : formatSpendCost(maxCost * part)
  }))
  const tipMeta = [
    { key: 'input', label: '输入', color: '#60a5fa', value: input, kind: 'token' as const },
    { key: 'output', label: '输出', color: '#34d399', value: output, kind: 'token' as const },
    { key: 'created', label: '缓存创建', color: '#fb923c', value: created, kind: 'token' as const },
    { key: 'cache', label: '缓存命中', color: '#a78bfa', value: cache, kind: 'token' as const },
    { key: 'cost', label: '成本', color: '#f87171', value: cost, kind: 'cost' as const }
  ]
  const yFor = (key: string, value: number) => (key === 'cost' ? yCost(value) : yTok(value))
  const hits: SpendChartHit[] = points.map((row, index) => {
    const x = xAt(index)
    return {
      x,
      label: row.label,
      values: tipMeta.map((item) => ({
        key: item.key,
        label: item.label,
        color: item.color,
        text: item.kind === 'cost' ? formatSpendCost(item.value[index] ?? 0) : formatExactTokens(item.value[index] ?? 0)
      })),
      dots: tipMeta.map((item) => ({
        key: item.key,
        x,
        y: yFor(item.key, item.value[index] ?? 0),
        color: item.color
      }))
    }
  })
  return {
    series,
    maxLabel: formatTokenCount(maxTok),
    maxCost: formatSpendCost(maxCost),
    ticks,
    grid,
    costTicks,
    hits,
    bottom,
    padLeft: PAD.l,
    padRight: PAD.r,
    padTop: PAD.t
  }
}

export function monthCells(year: number, month: number) {
  const first = new Date(year, month, 1).getDay()
  const days = new Date(year, month + 1, 0).getDate()
  const cells: Array<number | null> = []
  for (let i = 0; i < first; i += 1) cells.push(null)
  for (let day = 1; day <= days; day += 1) cells.push(day)
  while (cells.length % 7) cells.push(null)
  return cells
}

function previewWeights(count: number) {
  if (count <= 1) return [1]
  return Array.from({ length: count }, (_, i) => {
    const t = i / (count - 1)
    if (t < 0.38) return 0
    const wave = Math.sin(((t - 0.38) / 0.62) * Math.PI)
    const bump = t > 0.72 && t < 0.82 ? 0.35 : 0
    return Math.max(0, wave * 0.9 + bump)
  })
}

export function previewSpend(start?: number, end?: number): GrokSpend {
  const now = Date.now()
  const range = resolveSpendRange('today', now)
  const rangeStart = start ?? range.start
  const rangeEnd = end ?? range.end
  const hourly = rangeEnd - rangeStart <= 36 * 3600
  const count = hourly
    ? Math.max(1, Math.ceil((rangeEnd - rangeStart) / 3600))
    : Math.max(1, Math.ceil((rangeEnd - rangeStart) / 86_400))
  const weights = previewWeights(Math.min(count, hourly ? 24 : 30))
  while (weights.length < count) weights.unshift(0)
  const sum = weights.reduce((total, item) => total + item, 0) || 1
  const inputTotal = 8_638_000
  const outputTotal = 994_000
  const cacheTotal = 143_000_000
  const costTotal = 20.6534
  const points: GrokSpendPoint[] = weights.slice(0, count).map((weight, index) => {
    const ts = hourly ? rangeStart + index * 3600 : rangeStart + index * 86_400
    const date = new Date(ts * 1000)
    const label = hourly
      ? `${date.getMonth() + 1}/${date.getDate()} ${pad2(date.getHours())}:00`
      : `${date.getMonth() + 1}/${date.getDate()}`
    const part = weight / sum
    return {
      ts,
      label,
      inputTokens: Math.round(inputTotal * part),
      outputTokens: Math.round(outputTotal * part),
      cacheReadTokens: Math.round(cacheTotal * part),
      cacheCreationTokens: 0,
      costUsd: costTotal * part
    }
  })
  const inputTokens = points.reduce((sum, row) => sum + row.inputTokens, 0)
  const outputTokens = points.reduce((sum, row) => sum + row.outputTokens, 0)
  const cacheReadTokens = points.reduce((sum, row) => sum + row.cacheReadTokens, 0)
  return {
    ok: true,
    totalTokens: inputTokens + outputTokens + cacheReadTokens,
    inputTokens,
    outputTokens,
    cacheReadTokens,
    cacheCreationTokens: 0,
    cacheHitPercent: cacheHitPercent(inputTokens, cacheReadTokens),
    turnCount: 54,
    costUsd: costTotal,
    granularity: hourly ? 'hour' : 'day',
    rangeStart,
    rangeEnd,
    points,
    scannedFiles: 12,
    fetchedAt: new Date().toISOString(),
    message: null
  }
}
