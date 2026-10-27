import assert from 'node:assert/strict'
import { parseAppMode } from './types.ts'
import {
  USAGE_PRESETS,
  emptyUsageDashboard,
  formatSyncAgo,
  parseUsageRefreshMs,
  resolveUsageRange
} from './usage.ts'

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
