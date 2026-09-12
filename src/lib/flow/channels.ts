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

export function describeChannel(channel: ChannelBinding) {
  if (channel.kind === 'codexApp') {
    const n = channel.threadIds.length
    return n ? `Codex · ${n} 条线程` : 'Codex · 新开审查线程'
  }
  if (channel.kind === 'pty') {
    const tool = channel.toolId === 'grokbuild' ? 'Grok' : channel.toolId
    return channel.sessionId ? `${tool} 窗口` : `未绑定的 ${tool} 窗口`
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

export function resolvePty(
  channel: PtyChannel,
  live: LivePtyInfo[],
  projectId: string
): LivePtyInfo | null {
  const ofProject = live.filter((item) => item.projectId === projectId && item.alive !== false)
  if (channel.ptyId) {
    const byId = ofProject.find((item) => item.ptyId === channel.ptyId)
    if (byId) return byId
  }
  if (channel.sessionId) {
    const bySession = ofProject.find(
      (item) => item.toolId === channel.toolId && item.sessionId === channel.sessionId
    )
    if (bySession) return bySession
  }
  return ofProject.find((item) => item.toolId === channel.toolId) ?? null
}

export function isCodexNode(node: FlowNode): node is FlowNode & { channel: CodexChannel } {
  return node.channel.kind === 'codexApp'
}
