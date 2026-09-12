import type { BridgePipeline } from '../types.ts'
import { firstCodexNode } from './channels.ts'
import { makeId } from './ids.ts'
import { roleContract, roleLabel } from './roles.ts'
import { createDevReviewFlow } from './templates.ts'
import { ARTIFACT_CHAR_CAP, FLOW_END, type FlowDef, type FlowNode, type FlowRun, type ProjectFlows } from './types.ts'

export const FLOW_STORAGE_KEY = 'ad-bridge-flows'
export const RUN_STORAGE_KEY = 'ad-bridge-runs'
export const PIPELINE_STORAGE_KEY = 'ad-bridge-pipelines'

export type StorageLike = {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

function memoryStorage(): StorageLike {
  const map = new Map<string, string>()
  return {
    getItem(key) {
      return map.get(key) ?? null
    },
    setItem(key, value) {
      map.set(key, value)
    }
  }
}

export function defaultStorage(): StorageLike {
  try {
    if (typeof localStorage !== 'undefined') return localStorage
  } catch {
    /* ignore */
  }
  return memoryStorage()
}

function readJson<T>(storage: StorageLike, key: string): T | null {
  try {
    const raw = storage.getItem(key)
    if (!raw) return null
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

export function seedProjectFlows(): ProjectFlows {
  const pair = createDevReviewFlow()
  return {
    selectedFlowId: pair.id,
    flows: [pair],
    git: null
  }
}

export function migratePipeline(pipeline: BridgePipeline): FlowDef {
  const flow = createDevReviewFlow()
  flow.slice = pipeline.slice || 1
  flow.draftTask = pipeline.task || ''
  flow.maxRetries = pipeline.maxRetries || 2
  const cursor = flow.nodes.find((node) => node.channel.kind === 'cursorSdk')
  if (cursor && cursor.channel.kind === 'cursorSdk') {
    cursor.channel.agentId = pipeline.cursorAgentId || ''
  }
  const codex = firstCodexNode(flow)
  if (codex && codex.channel.kind === 'codexApp') {
    codex.channel.threadIds = [...(pipeline.selectedThreadIds || [])]
    codex.channel.targetKind = pipeline.targetKind || 'uncommittedChanges'
    codex.channel.commitSha = pipeline.commitSha || ''
    codex.channel.baseBranch = pipeline.baseBranch || 'main'
    codex.channel.customInstructions = pipeline.customInstructions || ''
  }
  return flow
}

function normalizeFlow(row: FlowDef): FlowDef | null {
  if (!row || !row.id || !Array.isArray(row.nodes) || !Array.isArray(row.edges)) return null
  return {
    id: row.id,
    name: row.name || '未命名流程',
    templateId: row.templateId,
    nodes: row.nodes.map(normalizeNode),
    edges: row.edges.map((edge) => ({
      id: edge.id || makeId('e'),
      from: edge.from,
      to: edge.to,
      mode: edge.mode === 'auto' ? 'auto' : 'manual',
      transform: edge.transform === 'verbatim' ? 'verbatim' : 'roleWrap',
      gate: edge.gate === 'passFail' ? 'passFail' : 'none',
      backTo: edge.backTo
    })),
    maxRetries: row.maxRetries ?? 2,
    maxSteps: row.maxSteps ?? 12,
    slice: row.slice || 1,
    draftTask: row.draftTask || ''
  }
}

function normalizeNode(node: FlowNode): FlowNode {
  const channel = node.channel
  return {
    id: node.id,
    title: node.title || '节点',
    role: node.role || 'custom',
    roleContract: node.roleContract || roleContract(node.role),
    channel: channel?.kind === 'codexApp'
      ? {
          kind: 'codexApp',
          threadIds: Array.isArray(channel.threadIds) ? channel.threadIds : [],
          targetKind: channel.targetKind || 'uncommittedChanges',
          commitSha: channel.commitSha || '',
          baseBranch: channel.baseBranch || 'main',
          customInstructions: channel.customInstructions || ''
        }
      : channel?.kind === 'pty'
        ? {
            kind: 'pty',
            toolId: channel.toolId || 'grokbuild',
            sessionId: channel.sessionId || '',
            ptyId: channel.ptyId || ''
          }
        : channel?.kind === 'cursorSdk'
          ? { kind: 'cursorSdk', agentId: channel.agentId || '' }
          : { kind: 'human' }
  }
}

export function loadAllFlows(storage: StorageLike = defaultStorage()): Record<string, ProjectFlows> {
  const stored = readJson<Record<string, ProjectFlows>>(storage, FLOW_STORAGE_KEY) || {}
  const out: Record<string, ProjectFlows> = {}
  for (const [projectId, row] of Object.entries(stored)) {
    if (!projectId || !row || !Array.isArray(row.flows)) continue
    const flows = row.flows.map(normalizeFlow).filter((item): item is FlowDef => Boolean(item))
    if (!flows.length) continue
    out[projectId] = {
      selectedFlowId: flows.some((item) => item.id === row.selectedFlowId)
        ? row.selectedFlowId
        : flows[0].id,
      flows,
      git: row.git ?? null
    }
  }

  const pipelines = readJson<Record<string, BridgePipeline>>(storage, PIPELINE_STORAGE_KEY) || {}
  for (const [projectId, pipeline] of Object.entries(pipelines)) {
    if (!projectId || !pipeline || out[projectId]) continue
    const migrated = migratePipeline({ ...pipeline, projectId })
    out[projectId] = {
      selectedFlowId: migrated.id,
      flows: [migrated],
      git: pipeline.git ?? null
    }
  }
  return out
}

export function loadAllRuns(storage: StorageLike = defaultStorage()): Record<string, FlowRun | null> {
  const stored = readJson<Record<string, FlowRun | null>>(storage, RUN_STORAGE_KEY) || {}
  const out: Record<string, FlowRun | null> = {}
  for (const [projectId, run] of Object.entries(stored)) {
    if (!projectId) continue
    out[projectId] = run && typeof run === 'object' ? slimRun(run) : null
  }
  return out
}

function slimRun(run: FlowRun): FlowRun {
  return {
    ...run,
    lastEnvelope: run.lastEnvelope ? slimEnvelope(run.lastEnvelope) : null,
    pendingHandoff: run.pendingHandoff ? slimEnvelope(run.pendingHandoff) : null,
    steps: (run.steps || []).map((step) => ({
      ...step,
      envelope: step.envelope ? slimEnvelope(step.envelope) : undefined
    }))
  }
}

function slimEnvelope<T extends { artifacts?: { body: string }[] }>(envelope: T): T {
  return {
    ...envelope,
    artifacts: (envelope.artifacts || []).map((item) => ({
      ...item,
      body: item.body.length > ARTIFACT_CHAR_CAP ? item.body.slice(0, ARTIFACT_CHAR_CAP) : item.body
    }))
  }
}

export function persistAllFlows(all: Record<string, ProjectFlows>, storage: StorageLike = defaultStorage()) {
  try {
    storage.setItem(FLOW_STORAGE_KEY, JSON.stringify(all))
  } catch {
    /* ignore quota */
  }
}

export function persistAllRuns(all: Record<string, FlowRun | null>, storage: StorageLike = defaultStorage()) {
  try {
    const slim: Record<string, FlowRun | null> = {}
    for (const [id, run] of Object.entries(all)) {
      slim[id] = run ? slimRun(run) : null
    }
    storage.setItem(RUN_STORAGE_KEY, JSON.stringify(slim))
  } catch {
    /* ignore quota */
  }
}

export function ensureProjectFlows(
  all: Record<string, ProjectFlows>,
  projectId: string
): ProjectFlows {
  if (!projectId) return seedProjectFlows()
  if (!all[projectId]) all[projectId] = seedProjectFlows()
  return all[projectId]
}

export function selectedFlow(bundle: ProjectFlows | null | undefined): FlowDef | null {
  if (!bundle?.flows.length) return null
  return bundle.flows.find((item) => item.id === bundle.selectedFlowId) ?? bundle.flows[0]
}

export function replaceFlow(bundle: ProjectFlows, flow: FlowDef): ProjectFlows {
  const index = bundle.flows.findIndex((item) => item.id === flow.id)
  const flows = index >= 0
    ? bundle.flows.map((item) => (item.id === flow.id ? flow : item))
    : [...bundle.flows, flow]
  return { ...bundle, flows, selectedFlowId: flow.id }
}

export function ensurePairLoop(flow: FlowDef): FlowDef {
  if (flow.nodes.length !== 2) return flow
  const [first, second] = flow.nodes
  return {
    ...flow,
    edges: [
      {
        id: makeId('e'),
        from: first.id,
        to: second.id,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'none'
      },
      {
        id: makeId('e'),
        from: second.id,
        to: FLOW_END,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'passFail',
        backTo: first.id
      }
    ]
  }
}

export function appendNode(flow: FlowDef, node: FlowNode): FlowDef {
  const last = flow.nodes[flow.nodes.length - 1]
  const nodes = [...flow.nodes, node]
  const edges = flow.edges.map((edge) => {
    if (last && edge.from === last.id && (!edge.to || edge.to === FLOW_END)) {
      return { ...edge, to: node.id }
    }
    return edge
  })
  const hasForward = last
    ? edges.some((edge) => edge.from === last.id && edge.to && edge.to !== FLOW_END)
    : true
  if (last && !hasForward) {
    edges.push({
      id: makeId('e'),
      from: last.id,
      to: node.id,
      mode: 'auto',
      transform: 'roleWrap',
      gate: 'none'
    })
  }
  const next = { ...flow, nodes, edges }
  if (next.nodes.length === 2) return ensurePairLoop(next)
  return next
}

export function removeNode(flow: FlowDef, nodeId: string): FlowDef {
  const index = flow.nodes.findIndex((item) => item.id === nodeId)
  if (index < 0) return flow
  const prev = flow.nodes[index - 1]
  const next = flow.nodes[index + 1]
  const nodes = flow.nodes.filter((item) => item.id !== nodeId)
  let edges = flow.edges.filter((edge) => edge.from !== nodeId && edge.to !== nodeId)
  edges = edges.map((edge) => (edge.backTo === nodeId ? { ...edge, backTo: undefined } : edge))
  if (prev && next && !edges.some((edge) => edge.from === prev.id && edge.to === next.id)) {
    edges.push({
      id: makeId('e'),
      from: prev.id,
      to: next.id,
      mode: 'manual',
      transform: 'roleWrap',
      gate: 'none'
    })
  }
  return { ...flow, nodes, edges }
}

export function createCustomNode(role: FlowNode['role'], channel: FlowNode['channel']): FlowNode {
  return {
    id: makeId('n'),
    title: roleLabel(role),
    role,
    roleContract: roleContract(role),
    channel
  }
}
