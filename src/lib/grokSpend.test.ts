import assert from 'node:assert/strict'
import {
  buildSpendChart,
  cacheHitPercent,
  dateInRange,
  formatCacheHit,
  formatExactTokens,
  formatFieldDate,
  formatSpendLine,
  formatTokenCount,
  nearestHitIndex,
  previewSpend,
  resolveSpendRange
} from './grokSpend.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('compact token counts use 万 and 亿', () => {
  assert.equal(formatTokenCount(0), '0')
  assert.equal(formatTokenCount(8800), '8800')
  assert.equal(formatTokenCount(12_340), '1.2万')
  assert.equal(formatTokenCount(8_149_000), '814.9万')
  assert.equal(formatTokenCount(145_147_696), '1.45亿')
})

test('status line stays token消耗 until spend is known', () => {
  assert.equal(formatSpendLine(null), 'token消耗')
  assert.equal(formatSpendLine({ ...previewSpend(), ok: false, totalTokens: 0 }), 'token消耗')
})

test('status line shows compact token count', () => {
  const spend = previewSpend()
  assert.match(formatSpendLine(spend), /token$/)
  assert.match(formatSpendLine(spend), /亿|万/)
  assert.equal(formatExactTokens(145_147_696), '145,147,696')
})

test('cache hit rate matches uncached input plus cache read', () => {
  assert.equal(formatCacheHit(cacheHitPercent(8_638_000, 143_000_000)), '94.3%')
})

test('today preset starts at local midnight and follows now', () => {
  const now = Date.parse('2026-09-11T20:28:00+08:00')
  const range = resolveSpendRange('today', now)
  assert.equal(range.label, '当天')
  assert.equal(range.followEnd, true)
  assert.equal(range.end, Math.floor(now / 1000))
  assert.ok(range.start < range.end)
})

test('chart builds smooth paths and dual-axis cost', () => {
  const chart = buildSpendChart(previewSpend().points)
  assert.ok(chart.series.find((item) => item.key === 'cache')?.path.startsWith('M '))
  assert.ok(chart.series.find((item) => item.key === 'cost')?.dashed)
  assert.ok(chart.ticks.length >= 2)
  assert.ok(chart.grid.length >= 3)
  assert.ok(chart.hits.length >= 2)
  assert.ok(chart.hits[0]?.dots.length)
})

test('hover picks the nearest chart sample', () => {
  const chart = buildSpendChart(previewSpend().points)
  const first = chart.hits[0]
  const last = chart.hits[chart.hits.length - 1]
  assert.ok(first && last)
  assert.equal(nearestHitIndex(chart.hits, first.x - 8), 0)
  assert.equal(nearestHitIndex(chart.hits, last.x + 8), chart.hits.length - 1)
})

test('calendar range includes days between start and end', () => {
  assert.equal(dateInRange('2026-09-10', '2026-09-10', '2026-09-11'), true)
  assert.equal(dateInRange('2026-09-11', '2026-09-10', '2026-09-11'), true)
  assert.equal(dateInRange('2026-09-09', '2026-09-10', '2026-09-11'), false)
  assert.equal(dateInRange('2026-09-11', '2026-09-11', '2026-09-10'), true)
})

test('datetime field shows slash dates', () => {
  assert.equal(formatFieldDate('2026-09-10'), '2026/09/10')
})
