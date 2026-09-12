import type { BridgePipeline, BridgeReview, GateStatus } from '../types.ts'
import { firstCodexNode } from './channels.ts'
import type { FlowDef, FlowRun, ProjectFlows } from './types.ts'

export function gateFromRun(flow: FlowDef | null, run: FlowRun | null): GateStatus {
  if (!run || run.status === 'idle') return 'idle'
  if (run.status === 'waiting') return 'pendingReview'
  if (run.status === 'failed') return 'failed'
  if (run.status === 'completed') return 'passed'
  const node = flow?.nodes.find((item) => item.id === run.currentNodeId)
  if (node?.channel.kind === 'codexApp') return 'reviewing'
  if (node?.channel.kind === 'cursorSdk' || node?.channel.kind === 'pty') return 'developing'
  return 'developing'
}

export function reviewsFromRun(flow: FlowDef | null, run: FlowRun | null): BridgeReview[] {
  if (!flow || !run) return []
  const reviewerIds = new Set(flow.nodes.filter((node) => node.role === 'reviewer').map((node) => node.id))
  return run.steps
    .filter((step) => reviewerIds.has(step.nodeId) && step.envelope?.done)
    .map((step) => ({
      sourceThreadId: '',
      reviewThreadId: step.id,
      text: step.envelope?.done || '',
      verdict: step.envelope?.verdict || 'unknown'
    }))
}

export function flowAsPipeline(
  projectId: string,
  bundle: ProjectFlows | null,
  flow: FlowDef | null,
  run: FlowRun | null
): BridgePipeline {
  const codex = flow ? firstCodexNode(flow) : null
  const channel = codex?.channel.kind === 'codexApp' ? codex.channel : null
  const cursor = flow?.nodes.find((node) => node.channel.kind === 'cursorSdk')
  return {
    projectId,
    slice: flow?.slice ?? 1,
    gate: gateFromRun(flow, run),
    task: run?.task || flow?.draftTask || '',
    selectedThreadIds: channel?.threadIds ? [...channel.threadIds] : [],
    targetKind: channel?.targetKind ?? 'uncommittedChanges',
    commitSha: channel?.commitSha ?? '',
    baseBranch: channel?.baseBranch ?? 'main',
    customInstructions: channel?.customInstructions ?? '',
    reviews: reviewsFromRun(flow, run),
    cursorAgentId:
      run?.cursorAgentId || (cursor?.channel.kind === 'cursorSdk' ? cursor.channel.agentId : '') || '',
    retryCount: run?.retryCount ?? 0,
    maxRetries: flow?.maxRetries ?? 2,
    lastError: run?.lastError ?? '',
    git: run?.git ?? bundle?.git ?? null
  }
}
