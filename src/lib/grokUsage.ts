import type { GrokUsage, SessionToolFilter, ToolId } from './types'

export const GROK_USAGE_INTERVAL_MS = 60_000

export function shouldShowGrokUsage(filter: SessionToolFilter, selectedTool: ToolId, appMode = 'console') {
  if (appMode !== 'console') return false
  if (filter === 'opencode' || filter === 'kimi') return false
  if (filter === 'grokbuild') return true
  return selectedTool === 'grokbuild'
}

export function usageTone(remaining: number | null | undefined): 'warn' | 'err' | '' {
  if (remaining == null || !Number.isFinite(remaining)) return ''
  if (remaining <= 5) return 'err'
  if (remaining <= 20) return 'warn'
  return ''
}

export function formatPercent(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) return '-'
  return String(Math.max(0, Math.min(100, Math.round(value))))
}

export function formatResetAt(iso: string | null | undefined, now = Date.now()) {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  if (date.getTime() <= now) return '即将重置'
  const month = date.getMonth() + 1
  const day = date.getDate()
  const hours = String(date.getHours()).padStart(2, '0')
  const minutes = String(date.getMinutes()).padStart(2, '0')
  const startOfToday = new Date(now)
  startOfToday.setHours(0, 0, 0, 0)
  const startOfTarget = new Date(date)
  startOfTarget.setHours(0, 0, 0, 0)
  const dayDiff = Math.round((startOfTarget.getTime() - startOfToday.getTime()) / 86_400_000)
  if (dayDiff === 0) return `今天 ${hours}:${minutes} 重置`
  return `${month}/${day} ${hours}:${minutes} 重置`
}

export function formatMoney(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) return '-'
  if (Number.isInteger(value)) return String(value)
  return value.toFixed(2)
}

export function formatUsageLine(usage: GrokUsage) {
  if (!usage.ok) return 'Grok 用量失败'
  const remaining = formatPercent(usage.remainingPercent)
  const reset = formatResetAt(usage.resetsAt)
  const parts = [`Grok 剩 ${remaining}%`]
  if (reset) parts.push(reset)
  if ((usage.prepaidBalance ?? 0) > 0) parts.push(`额度 ${formatMoney(usage.prepaidBalance)}`)
  return parts.join(' · ')
}

export function formatUsageTooltip(usage: GrokUsage) {
  if (!usage.ok) return usage.message || '读取 Grok 用量失败，点击重试'
  const period = usage.periodLabel || '本期'
  const lines = [
    `${period}已用 ${formatPercent(usage.usedPercent)}% · 剩余 ${formatPercent(usage.remainingPercent)}%`
  ]
  if (usage.grokBuildUsedPercent != null) {
    lines.push(`Build 已用 ${formatPercent(usage.grokBuildUsedPercent)}%`)
  }
  if (usage.prepaidBalance != null) {
    lines.push(`额度 ${formatMoney(usage.prepaidBalance)}`)
  }
  const reset = formatResetAt(usage.resetsAt)
  if (reset) lines.push(reset)
  if (usage.message) lines.push(usage.message)
  return lines.join('\n')
}


