import type { ToolId } from './types'

function toolName(toolId: ToolId) {
  if (toolId === 'grokbuild') return 'Grok'
  if (toolId === 'kimi') return 'Kimi'
  if (toolId === 'claude') return 'Claude Code'
  if (toolId === 'pi') return 'Pi'
  if (toolId === 'dsh') return 'DeepSeek'
  return 'OpenCode'
}

export function formatCiteHeader(toolId: ToolId, sessionTitle: string) {
  const title = sessionTitle.trim() || '会话'
  return `【引用自 ${toolName(toolId)} · ${title}】`
}

export function formatCiteBlock(opts: {
  toolId: ToolId
  sessionTitle: string
  relPath?: string | null
  body?: string | null
}) {
  const lines = [formatCiteHeader(opts.toolId, opts.sessionTitle)]
  const rel = opts.relPath?.trim()
  if (rel) {
    lines.push(`@${rel.replace(/^@/, '')}`)
  }
  const body = opts.body?.trim()
  if (body) {
    if (rel) lines.push('')
    lines.push(body)
  }
  return lines.join('\n')
}

export function wrapBracketedPaste(text: string) {
  return `\x1b[200~${text}\x1b[201~`
}

export function kindLabel(kind: string) {
  if (kind === 'plan') return 'Plan'
  if (kind === 'spec') return 'Spec'
  if (kind === 'summary') return '总结'
  return '文档'
}
