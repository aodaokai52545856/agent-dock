import { normalizeFlow } from './model.ts'
import type { ChannelBinding, FlowDef, FlowNode } from './types.ts'

export const FLOWS_DIR = 'flows'

export function flowFilePath(flowId: string) {
  return `${FLOWS_DIR}/${flowId}.json`
}

// Session, window and thread ids only mean something on this machine.
function portableChannel(channel: ChannelBinding): Partial<ChannelBinding> {
  if (channel.kind === 'codexApp') {
    return {
      kind: 'codexApp',
      targetKind: channel.targetKind,
      baseBranch: channel.baseBranch,
      customInstructions: channel.customInstructions
    }
  }
  if (channel.kind === 'pty' || channel.kind === 'ompRpc') return { kind: channel.kind, toolId: channel.toolId }
  if (channel.kind === 'script') return channel
  return { kind: channel.kind }
}

function withBindings(channel: ChannelBinding, local: ChannelBinding): ChannelBinding {
  if (channel.kind === 'codexApp' && local.kind === 'codexApp') {
    return { ...channel, threadIds: local.threadIds, commitSha: local.commitSha }
  }
  if ((channel.kind === 'pty' || channel.kind === 'ompRpc') && channel.kind === local.kind && channel.toolId === local.toolId) {
    return { ...channel, sessionId: local.sessionId, ptyId: local.ptyId, profileId: local.profileId }
  }
  if (channel.kind === 'cursorSdk' && local.kind === 'cursorSdk') return { ...channel, agentId: local.agentId }
  return channel
}

export function flowFileText(flow: FlowDef) {
  const { draftTask: _task, slice: _slice, draftProfile: _profile, ...definition } = flow
  const nodes = flow.nodes.map((node) => ({ ...node, channel: portableChannel(node.channel) }))
  return `${JSON.stringify({ ...definition, nodes }, null, 2)}\n`
}

export function parseFlowFile(name: string, text: string): FlowDef | null {
  try {
    const row = JSON.parse(text)
    if (!row || typeof row !== 'object') return null
    return normalizeFlow({ ...row, id: name.replace(/\.json$/, '') })
  } catch {
    return null
  }
}

export function mergeRepoFlow(repo: FlowDef, local: FlowDef | undefined): FlowDef {
  const known = new Map<string, FlowNode>((local?.nodes ?? []).map((node) => [node.id, node]))
  return {
    ...repo,
    draftTask: local?.draftTask ?? '',
    draftProfile: local?.draftProfile,
    slice: local?.slice ?? 1,
    nodes: repo.nodes.map((node) => {
      const old = known.get(node.id)
      return old ? { ...node, channel: withBindings(node.channel, old.channel) } : node
    })
  }
}

export function mergeRepoFlows(local: FlowDef[], repo: FlowDef[]): FlowDef[] {
  const fromRepo = new Map(repo.map((flow) => [flow.id, flow]))
  const merged = local.map((flow) => {
    const row = fromRepo.get(flow.id)
    return row ? mergeRepoFlow(row, flow) : flow
  })
  const seen = new Set(local.map((flow) => flow.id))
  return [...merged, ...repo.filter((flow) => !seen.has(flow.id)).map((flow) => mergeRepoFlow(flow, undefined))]
}
