import assert from 'node:assert/strict'
import { parseAppMode } from './types.ts'
import {
  buildUsageCalendar,
  completeUsagePoints,
  heatMetric,
  USAGE_PRESETS,
  emptyUsageDashboard,
  formatSyncAgo,
  parseUsageRefreshMs,
  resolveUsageRange
} from './usage.ts'
import type { UsageHeatCell, UsagePoint } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('parseAppMode keeps usage and falls back to console', () => {
  assert.equal(parseAppMode('usage'), 'usage')
  assert.equal(parseAppMode('bridge'), 'bridge')
  assert.equal(parseAppMode('console'), 'console')
  assert.equal(parseAppMode('nope'), 'console')
  assert.equal(parseAppMode(null), 'console')
})

test('usage presets add 全部 on top of spend presets', () => {
  assert.deepEqual(
    USAGE_PRESETS.map((item) => item.id),
    ['today', '1d', '7d', '14d', '30d', 'all']
  )
})

test('all range starts at unix 0 and follows now', () => {
  const now = Date.parse('2026-09-11T20:28:00+08:00')
  const range = resolveUsageRange('all', now)
  assert.equal(range.label, '全部')
  assert.equal(range.start, 0)
  assert.equal(range.end, Math.floor(now / 1000) + 1)
  assert.equal(range.followEnd, true)
})

test('today preset still starts at local midnight', () => {
  const now = Date.parse('2026-09-11T20:28:00+08:00')
  const range = resolveUsageRange('today', now)
  assert.equal(range.label, '当天')
  assert.equal(range.followEnd, true)
  assert.equal(range.end, Math.floor(now / 1000))
})

test('refresh interval only accepts 5/10/30/60 seconds', () => {
  assert.equal(parseUsageRefreshMs('60000'), 60_000)
  assert.equal(parseUsageRefreshMs('5000'), 5_000)
  assert.equal(parseUsageRefreshMs('10000'), 10_000)
  assert.equal(parseUsageRefreshMs('30000'), 30_000)
  assert.equal(parseUsageRefreshMs('7'), 60_000)
  assert.equal(parseUsageRefreshMs(undefined), 60_000)
})

test('sync label is 刚刚 then minutes', () => {
  const fetched = '2026-09-11T12:00:00.000Z'
  const now = Date.parse(fetched)
  assert.equal(formatSyncAgo(fetched, now + 3_000), '刚刚')
  assert.equal(formatSyncAgo(fetched, now + 180_000), '3 分钟前')
})

test('browser fallback dashboard does not invent usage', () => {
  const dash = emptyUsageDashboard()
  assert.equal(dash.totalTokens, 0)
  assert.equal(dash.totalRequests, 0)
  assert.equal(dash.dbOk, false)
  assert.equal(dash.message, '请在桌面窗口里查看用量')
})

const dayRow = (date: Date, tokens = 100): UsageHeatCell => ({
  ts: date.getTime() / 1000, label: '', tokens, requests: 1, costUsd: 0.5,
  inputTokens: 10, cacheReadTokens: 60, cacheCreationTokens: 30
})

test('the all-history calendar includes current usage and supports earlier records', () => {
  const today = new Date(2026, 9, 29, 12)
  const rows = [dayRow(new Date(2024, 0, 1)), dayRow(new Date(2026, 9, 29))]
  const calendar = buildUsageCalendar(rows, 'tokens', today.getTime() / 1000)
  assert.equal(calendar.weeks.length, 53)
  assert.equal(calendar.weeks.flat().length, 371)
  assert.equal(calendar.activeDays, 1)
  assert.equal(calendar.weeks.flat().find(day => day.key === '2026-10-29')?.row?.tokens, 100)
  assert.ok(calendar.hasOlder)
  const older = buildUsageCalendar(rows, 'tokens', today.getTime() / 1000, 1)
  assert.equal(new Date(older.end).getTime() < new Date(calendar.start).getTime(), true)
})

test('calendar days stay on local dates and cache shading uses the actual hit rate', () => {
  const date = new Date(2026, 2, 10)
  const row = dayRow(date)
  assert.equal(heatMetric(row, 'cache'), 60)
  const calendar = buildUsageCalendar([row], 'cache', new Date(2026, 2, 11).getTime() / 1000)
  const cell = calendar.weeks.flat().find(day => day.key === '2026-03-10')!
  assert.equal(cell.value, 60)
  assert.equal(cell.date.getHours(), 0)
  assert.ok(cell.level > 0)
  assert.ok(calendar.weeks.flat().some(day => day.future))
  const zeroRate = buildUsageCalendar([{ ...row, cacheReadTokens: 0 }], 'cache', date.getTime() / 1000)
  assert.equal(zeroRate.weeks.flat().find(day => day.key === '2026-03-10')?.value, 0)
})

test('sparse hourly trends fill missing hours without inventing requests', () => {
  const start = new Date(2026, 9, 29, 0).getTime() / 1000
  const row: UsagePoint = { ...dayRow(new Date(2026, 9, 29, 1)), outputTokens: 0 }
  const points = completeUsagePoints([row], start, start + 3 * 3600)
  assert.equal(points.length, 3)
  assert.deepEqual(points.map(point => point.requests), [0, 1, 0])
  assert.deepEqual(points.map(point => point.ts), [start, start + 3600, start + 7200])
  assert.deepEqual(completeUsagePoints([], start, start), [])
})
