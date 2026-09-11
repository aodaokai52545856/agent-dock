import assert from 'node:assert/strict'
import {
  formatResetAt,
  formatUsageLine,
  formatUsageTooltip,
  shouldShowGrokUsage,
  usageTone
} from './grokUsage.ts'
import type { GrokUsage } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const sample = (over: Partial<GrokUsage> = {}): GrokUsage => ({
  ok: true,
  usedPercent: 42,
  remainingPercent: 58,
  resetsAt: '2026-09-15T01:53:09.930Z',
  periodLabel: '本周',
  prepaidBalance: 0,
  onDemandUsed: 0,
  onDemandCap: 0,
  grokBuildUsedPercent: 80,
  fetchedAt: '2026-09-11T06:00:00.000Z',
  message: null,
  ...over
})

test('shows usage for grok filter or selected grok tool', () => {
  assert.equal(shouldShowGrokUsage('grokbuild', 'kimi'), true)
  assert.equal(shouldShowGrokUsage('all', 'grokbuild'), true)
  assert.equal(shouldShowGrokUsage('all', 'opencode'), false)
  assert.equal(shouldShowGrokUsage('opencode', 'grokbuild'), false)
  assert.equal(shouldShowGrokUsage('all', 'grokbuild', 'bridge'), false)
})

test('warns when the weekly pool is almost gone', () => {
  assert.equal(usageTone(58), '')
  assert.equal(usageTone(20), 'warn')
  assert.equal(usageTone(5), 'err')
  assert.equal(usageTone(null), '')
})

test('formats a compact status line', () => {
  const now = Date.parse('2026-09-11T08:00:00+08:00')
  const line = formatUsageLine({
    ...sample(),
    resetsAt: '2026-09-15T09:53:00+08:00'
  })
  assert.match(line, /Grok 剩 58%/)
  assert.match(formatResetAt('2026-09-15T09:53:00+08:00', now), /9\/15/)
  assert.match(formatUsageLine(sample({ prepaidBalance: 12.5 })), /额度 12.50/)
})

test('tooltip keeps the fuller breakdown', () => {
  const tip = formatUsageTooltip(sample())
  assert.match(tip, /本周已用 42%/)
  assert.match(tip, /Build 已用 80%/)
})

test('failed usage stays explicit', () => {
  assert.equal(formatUsageLine(sample({ ok: false, message: '请重新登录' })), 'Grok 用量失败')
  assert.equal(formatUsageTooltip(sample({ ok: false, message: '请重新登录' })), '请重新登录')
})
