import type { GitSnapshot, ReviewTargetKind, ReviewVerdict, ToolId } from '../types.ts'

export const FLOW_END = '__end__'
export const MAX_STEPS_DEFAULT = 12
export const MAX_RETRIES_DEFAULT = 2
export const PTY_WAIT_MS_DEFAULT = 8 * 60 * 1000
export const ARTIFACT_CHAR_CAP = 200_000

export type RoleId = string
export type PaneMode = 'edit' | 'run'

export type ChannelKind = 'codexApp' | 'pty' | 'cursorSdk' | 'human'

export type CodexChannel = {
  kind: 'codexApp'
  threadIds: string[]
  targetKind: ReviewTargetKind
  commitSha: string
  baseBranch: string
  customInstructions: string
}

export type PtyChannel = {
  kind: 'pty'
  toolId: ToolId
  sessionId: string
  ptyId: string
}

export type CursorChannel = {
  kind: 'cursorSdk'
  agentId: string
}

export type HumanChannel = {
  kind: 'human'
}

export type ChannelBinding = CodexChannel | PtyChannel | CursorChannel | HumanChannel

export type EdgeMode = 'auto' | 'manual'
export type EdgeTransform = 'roleWrap' | 'verbatim'
export type EdgeGate = 'none' | 'passFail'

export interface FlowNode {
  id: string
  title: string
  role: RoleId
  roleContract: string
  channel: ChannelBinding
}

export interface FlowEdge {
  id: string
  from: string
  to: string
  mode: EdgeMode
  transform: EdgeTransform
  gate: EdgeGate
  backTo?: string
}

export type FlowTemplateId = 'dev-review' | 'codex-grok' | 'blank'

export interface FlowDef {
  id: string
  name: string
  templateId?: FlowTemplateId
  nodes: FlowNode[]
  edges: FlowEdge[]
  maxRetries: number
  maxSteps: number
  slice: number
  draftTask: string
}

export type ArtifactKind = 'text' | 'diff' | 'verdict' | 'file'

export interface Artifact {
  kind: ArtifactKind
  body: string
  path?: string
}

export interface Envelope {
  id: string
  runId: string
  causationId: string
  fromNode: string
  toNode: string
  fromRole: RoleId
  toRole: RoleId
  task: string
  done: string
  remaining: string
  blockers: string[]
  artifacts: Artifact[]
  verdict?: ReviewVerdict
}

export type TaskStatus = 'submitted' | 'working' | 'input-required' | 'completed' | 'failed' | 'canceled'

export interface RunStep {
  id: string
  nodeId: string
  status: TaskStatus
  startedAt: number
  endedAt?: number
  error?: string
  envelope?: Envelope
}

export type RunStatus = 'idle' | 'running' | 'waiting' | 'failed' | 'completed'

export interface FlowRun {
  id: string
  projectId: string
  flowId: string
  status: RunStatus
  currentNodeId: string
  task: string
  steps: RunStep[]
  lastEnvelope: Envelope | null
  pendingHandoff: Envelope | null
  retryCount: number
  stepCount: number
  lastError: string
  cursorAgentId: string
  git: GitSnapshot | null
  createdAt: number
}

export interface ProjectFlows {
  selectedFlowId: string
  flows: FlowDef[]
  git: GitSnapshot | null
}

export type AdapterOutcome = {
  ok: boolean
  text: string
  verdict?: ReviewVerdict
  cursorAgentId?: string
  error?: string
}

export type EngineEvent =
  | { type: 'execute'; nodeId: string; envelope: Envelope; send: boolean; verbatim?: boolean }
  | { type: 'waitManual'; envelope: Envelope }
  | { type: 'complete' }
  | { type: 'fail'; error: string }

export type EngineResult = {
  run: FlowRun
  event: EngineEvent
}
