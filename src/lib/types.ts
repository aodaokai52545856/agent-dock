export type ToolId = 'opencode' | 'grokbuild' | 'kimi'

export type RenameKind = 'native' | 'overlay'

export interface Project {
  id: string
  name: string
  path: string
  proxyEnabled: boolean
  proxyUrl: string
  createdAt: string
}

export type SessionToolFilter = 'all' | ToolId

export type AppMode = 'console' | 'bridge'

export type GateStatus = 'idle' | 'developing' | 'pendingReview' | 'reviewing' | 'failed' | 'passed'

export type ReviewTargetKind = 'uncommittedChanges' | 'baseBranch' | 'commit' | 'custom'

export type ReviewVerdict = 'pass' | 'fail' | 'unknown'

export interface AppSettings {
  defaultProxyUrl: string
  opencodePath: string
  grokbuildPath: string
  kimiPath: string
  powershellPath: string
  terminalFontSize: number
  uiOpacity: number
  sessionToolFilter: SessionToolFilter
  cursorApiKey: string
  codexPath: string
}

export interface AppState {
  projects: Project[]
  settings: AppSettings
  titleOverlays: Record<string, string>
}

export interface SessionRow {
  toolId: ToolId
  id: string
  title: string
  cwd: string
  updatedAt: number
  renameKind: RenameKind
}

export interface SessionListResult {
  ok: boolean
  sessions: SessionRow[]
  toolFound: boolean
  toolPath?: string | null
  errorKind?: string | null
  message?: string | null
}

export interface BinaryProbe {
  found: boolean
  path?: string | null
  message?: string | null
}

export interface ToolProbeMap {
  opencode: BinaryProbe
  grokbuild: BinaryProbe
  kimi: BinaryProbe
}

export interface PtyOpened {
  ptyId: string
  key: string
  reused: boolean
  sessionId?: string | null
  title: string
}

export interface LivePtyInfo {
  ptyId: string
  key: string
  projectId: string
  toolId: ToolId
  sessionId?: string | null
  title: string
  alive: boolean
}

export interface ProjectDraft {
  name: string
  path: string
  proxyEnabled: boolean
  proxyUrl: string
}

export interface ToolVersionInfo {
  toolId: ToolId
  name: string
  found: boolean
  path?: string | null
  localVersion: string
  latestVersion: string
  compare: string
}

export interface UpgradeResult {
  ok: boolean
  log: string
  localVersion: string
}

export interface GrokAccount {
  id: string
  name: string
  email: string
  userId: string
  updatedAt: string
  active: boolean
}

export interface GrokAccountList {
  loggedIn: boolean
  currentEmail: string
  currentName: string
  accounts: GrokAccount[]
}

export interface GrokUsage {
  ok: boolean
  usedPercent: number | null
  remainingPercent: number | null
  resetsAt: string | null
  periodLabel: string | null
  prepaidBalance: number | null
  onDemandUsed: number | null
  onDemandCap: number | null
  grokBuildUsedPercent: number | null
  fetchedAt: string
  message: string | null
}

export const TOOLS: { id: ToolId; label: string; hint: string; tint: string }[] = [
  { id: 'opencode', label: 'OpenCode', hint: '打开编码会话', tint: 'var(--ad-opencode)' },
  { id: 'grokbuild', label: 'Grok', hint: '启动 Grok', tint: 'var(--ad-grok)' },
  { id: 'kimi', label: 'Kimi', hint: '启动 Kimi', tint: 'var(--ad-kimi)' }
]

export function toolLabel(id: ToolId): string {
  return TOOLS.find((item) => item.id === id)?.label ?? id
}

export interface CodexThread {
  id: string
  name: string
  preview: string
  cwd?: string | null
  sourceKind?: string | null
  isPinned: boolean
  createdAt: number
  updatedAt: number
  modelProvider?: string | null
}

export interface GitSnapshot {
  branch: string
  head: string
  dirty: boolean
  summary: string
}

export interface BridgeReview {
  sourceThreadId: string
  reviewThreadId: string
  text: string
  verdict: ReviewVerdict
}

export interface BridgePipeline {
  projectId: string
  slice: number
  gate: GateStatus
  task: string
  selectedThreadIds: string[]
  targetKind: ReviewTargetKind
  commitSha: string
  baseBranch: string
  customInstructions: string
  reviews: BridgeReview[]
  cursorAgentId: string
  retryCount: number
  maxRetries: number
  lastError: string
  git: GitSnapshot | null
}

export interface CodexProbe {
  ok: boolean
  binary: string
  initialized: boolean
  listed: number
  cwdMatched: number
  threads: CodexThread[]
  writerSafe: boolean
  note: string
}

export interface CodexThreadList {
  threads: CodexThread[]
  listed: number
  cwdMatched: number
  note: string
}

export interface ReviewStartResult {
  reviews: BridgeReview[]
  verdict: ReviewVerdict
}

export interface CursorDevResult {
  ok: boolean
  agentId: string
  status: string
  text: string
}

export const GATE_LABEL: Record<GateStatus, string> = {
  idle: '空闲',
  developing: '开发中',
  pendingReview: '待审',
  reviewing: '审查中',
  failed: '未通过',
  passed: '已通过'
}
