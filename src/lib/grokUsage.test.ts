import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  GROK_USAGE_FAST_INTERVAL_MS,
  GROK_USAGE_INTERVAL_MS,
  formatResetAt,
  formatUsageLine,
  formatUsageTooltip,
  grokUsagePollMs,
  shouldApplyUsageFetch,
  shouldShowGrokUsage,
  usageResetDue,
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
  usedCredits: null,
  creditLimit: null,
  fetchedAt: '2026-09-11T06:00:00.000Z',
  message: null,
  ...over
})

test('shows usage for grok filter or selected grok tool', () => {
  assert.equal(shouldShowGrokUsage('grokbuild', 'kimi'), true)
  assert.equal(shouldShowGrokUsage('all', 'grokbuild'), true)
  assert.equal(shouldShowGrokUsage('all', 'opencode'), false)
  assert.equal(shouldShowGrokUsage('opencode', 'grokbuild'), false)
  assert.equal(shouldShowGrokUsage('claude', 'grokbuild'), false)
  assert.equal(shouldShowGrokUsage('pi', 'grokbuild'), false)
  assert.equal(shouldShowGrokUsage('dsh', 'grokbuild'), false)
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

test('a reset time that already passed is labeled 即将重置, not 今天 HH:mm', () => {
  const now = Date.parse('2026-09-24T19:45:00+08:00')
  const resetsAt = '2026-09-24T09:31:00.000Z'
  assert.equal(formatResetAt(resetsAt, now), '即将重置')
  assert.equal(usageResetDue(sample({ remainingPercent: 4, resetsAt }), now), true)
  const line = formatUsageLine(sample({ remainingPercent: 4, resetsAt }), now)
  assert.match(line, /Grok 剩 4%/)
  assert.match(line, /即将重置/)
  assert.doesNotMatch(line, /17:31/)
})

test('failed refresh replaces a snapshot whose billing window already ended', () => {
  const now = Date.parse('2026-09-24T19:45:00+08:00')
  const stale = sample({ remainingPercent: 4, resetsAt: '2026-09-24T09:31:00.000Z' })
  const failed = sample({ ok: false, remainingPercent: null, resetsAt: null, message: '走代理读取 Grok 用量失败。' })
  const fresh = sample({ remainingPercent: 96, resetsAt: '2026-10-01T09:31:00.000Z' })
  assert.equal(shouldApplyUsageFetch(stale, failed, now), true)
  assert.equal(shouldApplyUsageFetch(stale, failed, Date.parse('2026-09-24T16:00:00+08:00')), false)
  assert.equal(shouldApplyUsageFetch(stale, fresh, now), true)
  assert.equal(shouldApplyUsageFetch(null, failed, now), true)
})

test('poll speeds up after the billed window ends', () => {
  const now = Date.parse('2026-09-24T19:45:00+08:00')
  assert.equal(grokUsagePollMs(sample({ resetsAt: '2026-09-24T09:31:00.000Z' }), now), GROK_USAGE_FAST_INTERVAL_MS)
  assert.equal(grokUsagePollMs(sample({ ok: false, resetsAt: null }), now), GROK_USAGE_FAST_INTERVAL_MS)
  assert.equal(grokUsagePollMs(sample({ resetsAt: '2026-10-01T09:31:00.000Z' }), now), GROK_USAGE_INTERVAL_MS)
  assert.equal(GROK_USAGE_FAST_INTERVAL_MS < GROK_USAGE_INTERVAL_MS, true)
})

test('status bar re-renders reset copy against a clock and refetches on focus', () => {
  const bar = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../components/StatusBar.vue'), 'utf8')
  assert.match(bar, /usageNow/)
  assert.match(bar, /formatUsageLine\(usage\.value,\s*usageNow/)
  assert.match(bar, /shouldApplyUsageFetch/)
  assert.match(bar, /grokUsagePollMs/)
  assert.match(bar, /addEventListener\('focus'/)
})

