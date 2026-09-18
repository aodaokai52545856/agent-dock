import assert from 'node:assert/strict'
import { shouldShowGrokUsage } from './grokUsage.ts'
import {
  balanceTone,
  formatBalanceLine,
  formatBalanceTooltip,
  formatPeakLine,
  formatPeakTooltip,
  formatYuan,
  isDshPeak,
  peakCopy,
  shouldShowDshBalance
} from './dshBalance.ts'
import type { DshBalance } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const sample = (over: Partial<DshBalance> = {}): DshBalance => ({
  ok: true,
  available: true,
  currency: 'CNY',
  totalBalance: 110,
  grantedBalance: 10,
  toppedUpBalance: 100,
  fetchedAt: '2026-09-13T06:00:00.000Z',
  message: null,
  ...over
})

test('shows balance for deepseek filter or selected deepseek tool', () => {
  assert.equal(shouldShowDshBalance('dsh', 'grokbuild'), true)
  assert.equal(shouldShowDshBalance('all', 'dsh'), true)
  assert.equal(shouldShowDshBalance('all', 'grokbuild'), false)
  assert.equal(shouldShowDshBalance('grokbuild', 'dsh'), false)
  assert.equal(shouldShowDshBalance('opencode', 'dsh'), false)
  assert.equal(shouldShowDshBalance('kimi', 'dsh'), false)
  assert.equal(shouldShowDshBalance('pi', 'dsh'), false)
  assert.equal(shouldShowDshBalance('all', 'dsh', 'bridge'), false)
})

test('shows balance for claude code as well as deepseek', () => {
  assert.equal(shouldShowDshBalance('claude', 'grokbuild'), true)
  assert.equal(shouldShowDshBalance('claude', 'claude'), true)
  assert.equal(shouldShowDshBalance('all', 'claude'), true)
  assert.equal(shouldShowDshBalance('dsh', 'claude'), true)
  assert.equal(shouldShowDshBalance('all', 'claude', 'bridge'), false)
})

test('grok usage and deepseek balance never show together', () => {
  const filters = ['all', 'opencode', 'grokbuild', 'kimi', 'claude', 'pi', 'dsh'] as const
  const tools = ['opencode', 'grokbuild', 'kimi', 'claude', 'pi', 'dsh'] as const
  for (const filter of filters) {
    for (const tool of tools) {
      const grok = shouldShowGrokUsage(filter, tool)
      const dsh = shouldShowDshBalance(filter, tool)
      assert.equal(grok && dsh, false, `${filter}/${tool}`)
    }
  }
})

test('warns when remaining yuan is low', () => {
  assert.equal(balanceTone(sample()), '')
  assert.equal(balanceTone(sample({ totalBalance: 5 })), 'warn')
  assert.equal(balanceTone(sample({ totalBalance: 1 })), 'err')
  assert.equal(balanceTone(sample({ totalBalance: 0 })), 'err')
  assert.equal(balanceTone(sample({ available: false, totalBalance: 20 })), 'err')
  assert.equal(balanceTone(sample({ ok: false })), 'err')
  assert.equal(balanceTone(null), '')
})

test('formats remaining RMB on the status line', () => {
  assert.equal(formatYuan(110), '110.00')
  assert.equal(formatYuan(9.7), '9.70')
  assert.equal(formatBalanceLine(sample()), 'DeepSeek 剩 ¥110.00')
  assert.equal(formatBalanceLine(sample({ currency: 'USD', totalBalance: 12.5 })), 'DeepSeek 剩 $12.50')
})

test('tooltip keeps recharge and grant split', () => {
  const tip = formatBalanceTooltip(sample())
  assert.match(tip, /剩余 ¥110\.00/)
  assert.match(tip, /充值 100\.00/)
  assert.match(tip, /赠金 10\.00/)
  assert.match(formatBalanceTooltip(sample({ available: false, totalBalance: 0 })), /余额不足/)
})

test('failed balance stays explicit', () => {
  assert.equal(formatBalanceLine(sample({ ok: false, message: '请添加 API Key' })), 'DeepSeek 余额失败')
  assert.equal(formatBalanceTooltip(sample({ ok: false, message: '请添加 API Key' })), '请添加 API Key')
})

function at(iso: string) {
  return new Date(iso)
}

test('weekday utc windows are peak, lunch gap and weekends are off-peak', () => {
  assert.equal(isDshPeak(at('2026-09-14T00:59:00.000Z')), false)
  assert.equal(isDshPeak(at('2026-09-14T01:00:00.000Z')), true)
  assert.equal(isDshPeak(at('2026-09-14T02:30:00.000Z')), true)
  assert.equal(isDshPeak(at('2026-09-14T03:59:00.000Z')), true)
  assert.equal(isDshPeak(at('2026-09-14T04:00:00.000Z')), false)
  assert.equal(isDshPeak(at('2026-09-14T05:30:00.000Z')), false)
  assert.equal(isDshPeak(at('2026-09-14T06:00:00.000Z')), true)
  assert.equal(isDshPeak(at('2026-09-14T09:59:00.000Z')), true)
  assert.equal(isDshPeak(at('2026-09-14T10:00:00.000Z')), false)
  assert.equal(isDshPeak(at('2026-09-18T08:00:00.000Z')), true)
  assert.equal(isDshPeak(at('2026-09-19T02:00:00.000Z')), false)
  assert.equal(isDshPeak(at('2026-09-20T08:00:00.000Z')), false)
})

test('peak copy stresses expensive, off-peak stresses going hard', () => {
  const peak = peakCopy(at('2026-09-17T02:00:00.000Z'))
  const off = peakCopy(at('2026-09-17T12:00:00.000Z'))
  assert.equal(peak.peak, true)
  assert.equal(peak.label, '高峰期')
  assert.equal(peak.punch, '贵')
  assert.equal(formatPeakLine(at('2026-09-17T02:00:00.000Z')), '高峰期 贵')
  assert.equal(off.peak, false)
  assert.equal(off.label, '非高峰')
  assert.equal(off.punch, '使劲蹬')
  assert.equal(formatPeakLine(at('2026-09-17T12:00:00.000Z')), '非高峰 使劲蹬')
  assert.match(formatPeakTooltip(at('2026-09-17T02:00:00.000Z')), /翻倍/)
  assert.match(formatPeakTooltip(at('2026-09-17T12:00:00.000Z')), /半价/)
})
