import type { GitSnapshot, ReviewTargetKind, ReviewVerdict, ToolId } from '../types.ts'

export const FLOW_END = '__end__'
export const FLOW_START = '__start__'
export const MAX_STEPS_DEFAULT = 12
export const MAX_RETRIES_DEFAULT = 2
export const MAX_LOOPS_DEFAULT = 3
export const PTY_WAIT_MS_DEFAULT = 8 * 60 * 1000
export const ARTIFACT_CHAR_CAP = 200_000

export type RoleId = string
export type PaneMode = 'edit' | 'run'

export type ChannelKind = 'codexApp' | 'pty' | 'ompRpc' | 'cursorSdk' | 'human'

export type CodexChannel = {
  kind: 'codexApp'
  threadIds: string[]
  targetKind: ReviewTargetKind
  commitSha: string
  baseBranch: string
  customInstructions: string
}

export type PtyChannel = {
  kind: 'pty' | 'ompRpc'
  profileId?: string
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
export type EdgeGate = 'none' | 'passFail' | 'loop'

export interface NodeInput {
  ref: string
  required?: boolean
  when?: 'reworked'
  read?: 'index-first'
}

export type LessonRule = 'entries' | 'entries-or-na'

export interface NodeOutput {
  path: string
  capture?: 'file' | 'reply'
  template?: string
  sections?: string[]
  // Set by Dock for a single step; flow files cannot declare it.
  lesson?: LessonRule
}

export interface FlowNode {
  id: string
  title: string
  role: RoleId
  roleContract: string
  channel: ChannelBinding
  inputs?: NodeInput[]
  outputs?: NodeOutput[]
  x?: number
  y?: number
}

export interface FlowEdge {
  id: string
  from: string
  to: string
  mode: EdgeMode
  transform: EdgeTransform
  gate: EdgeGate
  backTo?: string
  maxLoops?: number
}

export type FlowTemplateId = 'dev-review' | 'codex-grok' | 'blank'

export type ExperiencePolicy = 'attempts' | 'always' | 'model'

export interface FlowExperience {
  policy: ExperiencePolicy
  minAttempts?: number
  authors: string[]
  judge?: string
}

export interface FlowDef {
  id: string
  name: string
  templateId?: FlowTemplateId
  nodes: FlowNode[]
  edges: FlowEdge[]
  experience?: FlowExperience
  maxRetries: number
  maxSteps: number
  slice: number
  draftTask: string
  startX?: number
  startY?: number
  endX?: number
  endY?: number
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

export interface StepFile {
  ref: string
  sha256: string | null
}

export interface RunStep {
  id: string
  nodeId: string
  status: TaskStatus
  startedAt: number
  endedAt?: number
  error?: string
  envelope?: Envelope
  inputs?: StepFile[]
  outputs?: StepFile[]
  // A turn that only fills in a lesson: it is not an attempt at the node's work.
  purpose?: 'lesson'
}

export interface LessonDraft {
  id: string
  file: string
  title: string
  scope: string
  body: string
  missingSections: string[]
  missingPaths: string[]
}

export interface LessonRecord {
  title: string
  scope: string
  // Empty when the reviewer left the lesson out.
  entry?: string
}

export type RunStatus = 'idle' | 'running' | 'waiting' | 'failed' | 'completed' | 'canceled'

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
  loopCounts: Record<string, number>
  stepCount: number
  lastError: string
  cursorAgentId: string
  git: GitSnapshot | null
  createdAt: number
  taskDir?: string
  // Attempts made by earlier runs of the same task, kept across a rerun.
  priorAttempts?: Record<string, number>
  lessonWanted?: string
  pendingLessons?: LessonDraft[]
  lessons?: LessonRecord[]
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
  unconfirmed?: boolean
}

export type EngineEvent =
  | { type: 'execute'; nodeId: string; envelope: Envelope; send: boolean; verbatim?: boolean }
  | { type: 'waitManual'; envelope: Envelope }
  | { type: 'waitHuman'; nodeId: string }
  | { type: 'waitLessons' }
  | { type: 'complete' }
  | { type: 'fail'; error: string }

export type EngineResult = {
  run: FlowRun
  event: EngineEvent
}
