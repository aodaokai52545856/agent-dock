import type { BridgePipeline } from '../types.ts'
import { firstCodexNode } from './channels.ts'
import { makeId } from './ids.ts'
import { roleContract, roleLabel } from './roles.ts'
import { defaultNodePoint } from './chartLayout.ts'
import { forgetNode } from './routing.ts'
import { createDevReviewFlow } from './templates.ts'
import { ARTIFACT_CHAR_CAP, FLOW_END, FLOW_START, type FlowDef, type FlowExperience, type FlowNode, type FlowRun, type NodeExit, type NodeInput, type NodeOutput, type ProjectFlows } from './types.ts'

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

// A bad exit stays in so the run refuses it. Dropping it would quietly change the route.
function normalizeExits(rows: NodeExit[] | undefined) {
  if (!Array.isArray(rows) || !rows.length) return undefined
  return rows.map((row) => ({
    on: String(row?.on ?? '').toUpperCase() as NodeExit['on'],
    owner: typeof row?.owner === 'string' && row.owner.trim() ? row.owner.trim() : undefined,
    to: String(row?.to ?? '')
  }))
}

function normalizeProfiles(rows: FlowDef['profiles']) {
  if (!rows || typeof rows !== 'object' || Array.isArray(rows)) return undefined
  const entries = Object.entries(rows).map(([name, row]) => [
    name,
    { skip: Array.isArray(row?.skip) ? row.skip.filter((id) => typeof id === 'string') : [] }
  ])
  return entries.length ? Object.fromEntries(entries) : undefined
}

function normalizeExperience(row: FlowExperience | undefined): FlowExperience | undefined {
  if (row?.policy !== 'attempts' && row?.policy !== 'always' && row?.policy !== 'model') return undefined
  return {
    policy: row.policy,
    minAttempts: row.policy === 'attempts' ? Math.max(1, Math.floor(Number(row.minAttempts)) || 2) : undefined,
    authors: Array.isArray(row.authors) ? row.authors.filter((id) => typeof id === 'string' && id.trim()) : [],
    judge: row.policy === 'model' && typeof row.judge === 'string' && row.judge.trim() ? row.judge.trim() : undefined
  }
}

export function normalizeFlow(row: FlowDef): FlowDef | null {
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
      gate: edge.gate === 'passFail' ? 'passFail' : edge.gate === 'loop' ? 'loop' : 'none',
      backTo: edge.backTo,
      maxLoops: edge.gate === 'loop'
        ? Math.min(20, Math.max(1, Number(edge.maxLoops) || 3))
        : undefined
    })),
    experience: normalizeExperience(row.experience),
    profiles: normalizeProfiles(row.profiles),
    draftProfile: typeof row.draftProfile === 'string' ? row.draftProfile : undefined,
    maxRetries: row.maxRetries ?? 2,
    maxSteps: row.maxSteps ?? 12,
    slice: row.slice || 1,
    draftTask: row.draftTask || '',
    startX: Number.isFinite(row.startX) ? row.startX : undefined,
    startY: Number.isFinite(row.startY) ? row.startY : undefined,
    endX: Number.isFinite(row.endX) ? row.endX : undefined,
    endY: Number.isFinite(row.endY) ? row.endY : undefined
  }
}

function normalizeInputs(rows: NodeInput[] | undefined): NodeInput[] | undefined {
  if (!Array.isArray(rows)) return undefined
  const inputs = rows.filter((row) => typeof row?.ref === 'string' && row.ref.trim()).map((row) => ({
    ref: row.ref.trim(),
    required: row.required === true ? true : undefined,
    when: row.when === 'reworked' ? 'reworked' as const : undefined,
    read: row.read === 'index-first' ? 'index-first' as const : undefined
  }))
  return inputs.length ? inputs : undefined
}

function normalizeOutputs(rows: NodeOutput[] | undefined): NodeOutput[] | undefined {
  if (!Array.isArray(rows)) return undefined
  const outputs = rows.filter((row) => typeof row?.path === 'string' && row.path.trim()).map((row) => ({
    path: row.path.trim(),
    capture: row.capture === 'file' || row.capture === 'reply' ? row.capture : undefined,
    template: typeof row.template === 'string' && row.template.trim() ? row.template.trim() : undefined,
    sections: Array.isArray(row.sections) ? row.sections.filter((item) => typeof item === 'string' && item.trim()) : undefined
  }))
  return outputs.length ? outputs : undefined
}

function normalizeNode(node: FlowNode): FlowNode {
  const channel = node.channel
  return {
    id: node.id,
    title: node.title || '节点',
    role: node.role || 'custom',
    roleContract: node.roleContract || roleContract(node.role),
    x: Number.isFinite(node.x) ? node.x : undefined,
    y: Number.isFinite(node.y) ? node.y : undefined,
    channel: channel?.kind === 'codexApp'
      ? {
          kind: 'codexApp',
          threadIds: Array.isArray(channel.threadIds) ? channel.threadIds : [],
          targetKind: channel.targetKind || 'uncommittedChanges',
          commitSha: channel.commitSha || '',
          baseBranch: channel.baseBranch || 'main',
          customInstructions: channel.customInstructions || ''
        }
      : (channel?.kind === 'pty' || channel?.kind === 'ompRpc')
        ? {
            kind: channel.toolId==='omp'?'ompRpc':channel.kind,
            profileId: channel.profileId,
            toolId: channel.toolId || 'grokbuild',
            sessionId: channel.sessionId || '',
            ptyId: channel.ptyId || ''
          }
        : channel?.kind === 'cursorSdk'
          ? { kind: 'cursorSdk', agentId: channel.agentId || '' }
          : channel?.kind === 'script'
            ? {
                kind: 'script',
                command: typeof channel.command === 'string' ? channel.command : '',
                timeoutSec: Number(channel.timeoutSec) > 0 ? Math.floor(Number(channel.timeoutSec)) : undefined
              }
            : { kind: 'human' },
    inputs: normalizeInputs(node.inputs),
    outputs: normalizeOutputs(node.outputs),
    session: node.session === 'fresh' || node.session === 'reuse' ? node.session : undefined,
    exits: normalizeExits(node.exits),
    maxAttempts: Number.isInteger(node.maxAttempts) && Number(node.maxAttempts) > 0 ? node.maxAttempts : undefined
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
    loopCounts: run.loopCounts || {},
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
        from: FLOW_START,
        to: first.id,
        mode: 'auto',
        gate: 'none'
      },
      {
        id: makeId('e'),
        from: first.id,
        to: second.id,
        mode: 'auto',
        gate: 'none'
      },
      {
        id: makeId('e'),
        from: second.id,
        to: FLOW_END,
        mode: 'auto',
        gate: 'passFail',
        backTo: first.id
      }
    ]
  }
}

export function appendNode(flow: FlowDef, node: FlowNode): FlowDef {
  const point = defaultNodePoint(flow.nodes.length)
  const placed: FlowNode = {
    ...node,
    x: Number.isFinite(node.x) ? node.x : point.x,
    y: Number.isFinite(node.y) ? node.y : point.y
  }
  return { ...flow, nodes: [...flow.nodes, placed] }
}

export function removeNode(flow: FlowDef, nodeId: string): FlowDef {
  const rest = forgetNode(flow, nodeId)
  const nodes = rest.nodes.filter((item) => item.id !== nodeId)
  const edges = flow.edges
    .filter((edge) => edge.from !== nodeId && edge.to !== nodeId)
    .map((edge) => (edge.backTo === nodeId ? { ...edge, backTo: undefined } : edge))
  return { ...rest, nodes, edges }
}

export function createCustomNode(
  role: FlowNode['role'],
  channel: FlowNode['channel'],
  point?: { x: number; y: number }
): FlowNode {
  return {
    id: makeId('n'),
    title: roleLabel(role),
    role,
    roleContract: roleContract(role),
    channel,
    x: point?.x,
    y: point?.y
  }
}
