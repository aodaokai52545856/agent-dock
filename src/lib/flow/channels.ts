import type { LivePtyInfo, ToolId } from '../types.ts'
import { findRole, roleLabel } from './roles.ts'
import type { ChannelBinding, ChannelKind, CodexChannel, FlowDef, FlowNode, PtyChannel, RoleId } from './types.ts'

export const CHANNEL_LABEL: Record<ChannelKind, string> = {
  codexApp: 'Codex 应用程序',
  pty: '终端窗口',
  cursorSdk: 'Cursor SDK',
  human: '人工'
}

export const AUTO_PTY_TOOLS: ToolId[] = ['grokbuild', 'kimi']

export function channelKindLabel(kind: ChannelKind) {
  return CHANNEL_LABEL[kind]
}

export function defaultCodexChannel(): CodexChannel {
  return {
    kind: 'codexApp',
    threadIds: [],
    targetKind: 'uncommittedChanges',
    commitSha: '',
    baseBranch: 'main',
    customInstructions: ''
  }
}

export function defaultPtyChannel(toolId: ToolId = 'grokbuild'): PtyChannel {
  return { kind: 'pty', toolId, sessionId: '', ptyId: '' }
}

export function defaultChannel(role: RoleId): ChannelBinding {
  const kind = findRole(role)?.defaultChannel || 'pty'
  if (kind === 'codexApp') return defaultCodexChannel()
  if (kind === 'pty') return defaultPtyChannel()
  if (kind === 'cursorSdk') return { kind: 'cursorSdk', agentId: '' }
  return { kind: 'human' }
}

export function channelSupportsAuto(channel: ChannelBinding) {
  if (channel.kind === 'codexApp' || channel.kind === 'cursorSdk' || channel.kind === 'human') return true
  return channel.kind === 'pty' && AUTO_PTY_TOOLS.includes(channel.toolId)
}

export function ptyToolLabel(toolId: ToolId) {
  return toolId === 'grokbuild' ? 'Grok' : toolId
}

export function describeChannel(channel: ChannelBinding, live: LivePtyInfo[] = []) {
  if (channel.kind === 'codexApp') {
    const n = channel.threadIds.length
    return n ? `Codex · ${n} 条线程` : 'Codex · 新开审查线程'
  }
  if (channel.kind === 'pty') {
    const tool = ptyToolLabel(channel.toolId)
    if (!channel.ptyId) return `未绑定的 ${tool} 窗口`
    const found = inspectPty(channel, live, live.find((item) => item.ptyId === channel.ptyId)?.projectId || '')
    if (found.ok) {
      const title = found.live.title.trim() || found.live.ptyId.slice(-4)
      return `${tool} · ${title.length > 24 ? `${title.slice(0, 23)}…` : title}`
    }
    if (found.reason === 'closed') return `${tool} · 已关闭`
    return `未绑定的 ${tool} 窗口`
  }
  if (channel.kind === 'cursorSdk') return 'Cursor SDK'
  return '人工确认'
}

export function firstCodexNode(flow: FlowDef | null | undefined) {
  return flow?.nodes.find((node) => node.channel.kind === 'codexApp') ?? null
}

export function firstPtyNode(flow: FlowDef | null | undefined) {
  return flow?.nodes.find((node) => node.channel.kind === 'pty') ?? null
}

export function defaultNodeTitle(role: RoleId) {
  return roleLabel(role)
}

export type PtyInspectReason = 'unbound' | 'closed' | 'mismatch' | 'ambiguous'

export type PtyInspect =
  | { ok: true; live: LivePtyInfo }
  | { ok: false; reason: PtyInspectReason }

export function inspectPty(
  channel: PtyChannel,
  live: LivePtyInfo[],
  projectId: string
): PtyInspect {
  const ofProject = live.filter((item) => item.projectId === projectId && item.alive !== false)
  if (channel.ptyId) {
    const byId = ofProject.find((item) => item.ptyId === channel.ptyId)
    if (byId) {
      if (byId.toolId !== channel.toolId) return { ok: false, reason: 'mismatch' }
      return { ok: true, live: byId }
    }
    const known = live.find((item) => item.ptyId === channel.ptyId)
    if (known) return { ok: false, reason: known.toolId === channel.toolId ? 'closed' : 'mismatch' }
    return { ok: false, reason: 'closed' }
  }
  if (channel.sessionId) {
    const matches = ofProject.filter(
      (item) => item.toolId === channel.toolId && item.sessionId === channel.sessionId
    )
    if (matches.length === 1) return { ok: true, live: matches[0] }
    if (matches.length > 1) return { ok: false, reason: 'ambiguous' }
  }
  return { ok: false, reason: 'unbound' }
}

export function resolvePty(
  channel: PtyChannel,
  live: LivePtyInfo[],
  projectId: string
): LivePtyInfo | null {
  const found = inspectPty(channel, live, projectId)
  return found.ok ? found.live : null
}

export function ptyWindowLabel(item: LivePtyInfo, occupiedBy?: string) {
  const tool = ptyToolLabel(item.toolId)
  const short = item.ptyId.slice(-4)
  const taken = occupiedBy ? `（已被「${occupiedBy}」使用）` : ''
  return `${tool} · ${item.title} · ${short}${taken}`
}

export function ptyOccupant(flow: FlowDef | null | undefined, ptyId: string, exceptNodeId?: string) {
  if (!ptyId || !flow) return null
  return flow.nodes.find((node) => (
    node.id !== exceptNodeId
    && node.channel.kind === 'pty'
    && node.channel.ptyId === ptyId
  )) ?? null
}

export function ptyBindError(node: FlowNode, reason: PtyInspectReason) {
  const tool = node.channel.kind === 'pty' ? ptyToolLabel(node.channel.toolId) : '窗口'
  const name = node.title || roleLabel(node.role)
  if (reason === 'closed') return `节点「${name}」绑定的窗口已关闭，请重新选择`
  if (reason === 'mismatch') return `节点「${name}」绑定的窗口类型不匹配，请重新选择`
  if (reason === 'ambiguous') return `节点「${name}」匹配到多个 ${tool} 窗口，请显式选择一扇`
  return `节点「${name}」未绑定 ${tool} 窗口`
}

function nodeNeedsAutoSession(flow: FlowDef, node: FlowNode) {
  if (node.channel.kind !== 'pty' || !channelSupportsAuto(node.channel)) return false
  const edge = flow.edges.find((item) => item.from === node.id)
  return !edge || edge.mode === 'auto'
}

export function validateFlowPtyBindings(
  flow: FlowDef,
  live: LivePtyInfo[],
  projectId: string
): string | null {
  const owners = new Map<string, FlowNode[]>()
  for (const node of flow.nodes) {
    if (node.channel.kind !== 'pty') continue
    const found = inspectPty(node.channel, live, projectId)
    if (!found.ok) return ptyBindError(node, found.reason)
    const bucket = owners.get(found.live.ptyId) || []
    bucket.push(node)
    owners.set(found.live.ptyId, bucket)
    if (nodeNeedsAutoSession(flow, node) && !found.live.sessionId) {
      return `先在「${found.live.title || ptyToolLabel(node.channel.toolId)}」窗口里完成至少一轮对话，再自动接驳`
    }
  }
  for (const group of owners.values()) {
    if (group.length < 2) continue
    const names = group.map((node) => node.title || roleLabel(node.role)).join('、')
    return `多个节点绑了同一扇窗口（${names}）。两个 Grok 窗口必须各绑各的。`
  }
  return null
}

export function isCodexNode(node: FlowNode): node is FlowNode & { channel: CodexChannel } {
  return node.channel.kind === 'codexApp'
}

export type PtyRunPrep =
  | { ok: true; live: LivePtyInfo; waitForTurn: boolean; sessionId: string }
  | { ok: false; error: string }

export function preparePtyRun(
  node: FlowNode,
  live: LivePtyInfo[],
  projectId: string,
  opts: { send: boolean }
): PtyRunPrep {
  if (node.channel.kind !== 'pty') return { ok: false, error: '不是窗口节点' }
  const found = inspectPty(node.channel, live, projectId)
  if (!found.ok) return { ok: false, error: ptyBindError(node, found.reason) }
  const sessionId = found.live.sessionId || ''
  const waitForTurn = (opts.send || channelSupportsAuto(node.channel)) && Boolean(sessionId)
  if (opts.send && channelSupportsAuto(node.channel) && !sessionId) {
    const title = found.live.title.trim() || ptyToolLabel(node.channel.toolId)
    return { ok: false, error: `先在「${title}」窗口里完成至少一轮对话，再自动接驳` }
  }
  return { ok: true, live: found.live, waitForTurn, sessionId }
}

export function ptyWaitTimeoutError(live: Pick<LivePtyInfo, 'title' | 'ptyId'>) {
  const title = live.title.trim() || live.ptyId.slice(-4)
  return `等待「${title}」窗口回复超时。可改成手动桥接，或在窗口里确认是否已发送。`
}
