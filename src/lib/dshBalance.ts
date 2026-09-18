import type { DshBalance, SessionToolFilter, ToolId } from './types'

export const DSH_BALANCE_INTERVAL_MS = 60_000

export function shouldShowDshBalance(filter: SessionToolFilter, selectedTool: ToolId, appMode = 'console') {
  if (appMode !== 'console') return false
  if (filter === 'opencode' || filter === 'kimi' || filter === 'pi' || filter === 'grokbuild') {
    return false
  }
  if (filter === 'dsh' || filter === 'claude') return true
  return selectedTool === 'dsh' || selectedTool === 'claude'
}

export function balanceTone(balance: DshBalance | null | undefined): 'warn' | 'err' | '' {
  if (!balance) return ''
  if (!balance.ok || balance.available === false) return 'err'
  const remaining = balance.totalBalance
  if (remaining == null || !Number.isFinite(remaining)) return ''
  if (remaining <= 1) return 'err'
  if (remaining <= 5) return 'warn'
  return ''
}

export function formatYuan(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) return '-'
  return value.toFixed(2)
}

export function currencyMark(currency: string | null | undefined) {
  return currency === 'USD' ? '$' : '¥'
}

export function formatBalanceLine(balance: DshBalance) {
  if (!balance.ok) return 'DeepSeek 余额失败'
  return `DeepSeek 剩 ${currencyMark(balance.currency)}${formatYuan(balance.totalBalance)}`
}

export function formatBalanceTooltip(balance: DshBalance) {
  if (!balance.ok) return balance.message || '读取 DeepSeek 余额失败，点击重试'
  const mark = currencyMark(balance.currency)
  const lines = [`剩余 ${mark}${formatYuan(balance.totalBalance)}`]
  if (balance.toppedUpBalance != null || balance.grantedBalance != null) {
    const parts = []
    if (balance.toppedUpBalance != null) parts.push(`充值 ${formatYuan(balance.toppedUpBalance)}`)
    if (balance.grantedBalance != null) parts.push(`赠金 ${formatYuan(balance.grantedBalance)}`)
    lines.push(parts.join(' · '))
  }
  if (balance.available === false) lines.push('余额不足，无法继续调用')
  if (balance.message) lines.push(balance.message)
  return lines.join('\n')
}

export const DSH_PEAK_TICK_MS = 15_000

const PEAK_WINDOWS_UTC = [
  { start: 60, end: 240 },
  { start: 360, end: 600 }
] as const

export function isDshPeak(now = new Date()) {
  const day = now.getUTCDay()
  if (day === 0 || day === 6) return false
  const minutes = now.getUTCHours() * 60 + now.getUTCMinutes()
  return PEAK_WINDOWS_UTC.some((window) => minutes >= window.start && minutes < window.end)
}

export function peakCopy(now = new Date()) {
  if (isDshPeak(now)) {
    return { peak: true, label: '高峰期', punch: '贵' }
  }
  return { peak: false, label: '非高峰', punch: '使劲蹬' }
}

export function formatPeakLine(now = new Date()) {
  const copy = peakCopy(now)
  return `${copy.label} ${copy.punch}`
}

export function formatPeakTooltip(now = new Date()) {
  if (isDshPeak(now)) {
    return '高峰期 · 价格翻倍\n工作日北京时间 09:00–12:00、14:00–18:00'
  }
  return '非高峰 · 半价，使劲蹬\n周末全天，以及工作日其余时段'
}
