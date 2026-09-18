export type ToolId = 'opencode' | 'grokbuild' | 'kimi' | 'claude' | 'pi' | 'dsh'

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
  claudePath: string
  piPath: string
  dshPath: string
  powershellPath: string
  terminalFontSize: number
  uiFontSize: number
  uiTheme: 'system' | 'light' | 'dark'
  uiAccent: string
  uiBackground: string
  uiForeground: string
  uiAccentLight: string
  uiBackgroundLight: string
  uiForegroundLight: string
  uiFontFamily: string
  contentFontFamily: string
  codeFontFamily: string
  uiContrast: number
  translucentSidebar: boolean
  uiOpacity: number
  uiFrost: number
  grokFollowGlass: boolean
  sessionToolFilter: SessionToolFilter
  cursorApiKey: string
  codexPath: string
  ccswitchPath: string
  ccswitchDownloadDir: string
  ccswitchInstallDir: string
}

export interface CcswitchProbe {
  found: boolean
  path?: string | null
  downloadDir: string
  installDir: string
}

export interface CcswitchLatest {
  version: string
  tag: string
  assetName: string
  size: number
  url: string
  homepage: string
  releasesUrl: string
}

export interface CcswitchInstallResult {
  ok: boolean
  path: string
  version: string
  log: string
}

export interface CcswitchProgress {
  stage: string
  message: string
  received: number
  total: number
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

export type SessionDocKind = 'plan' | 'spec' | 'doc' | 'summary'

export interface SessionDoc {
  kind: SessionDocKind | string
  title: string
  path: string
  relPath?: string | null
  updatedAt: number
  source: string
}

export interface SessionDocBody {
  path: string
  title: string
  text: string
  relPath?: string | null
}

export interface SessionTurn {
  id: string
  role: 'user' | 'assistant' | string
  excerpt: string
  text: string
}

export interface FocusedSession {
  toolId: ToolId
  sessionId: string
  title: string
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
  claude: BinaryProbe
  pi: BinaryProbe
  dsh: BinaryProbe
}

export interface PtyOpened {
  ptyId: string
  key: string
  reused: boolean
  sessionId?: string | null
  title: string
  openedAt?: number | null
  kind?: 'pty' | 'web' | string | null
  url?: string | null
}

export interface LivePtyInfo {
  ptyId: string
  key: string
  projectId: string
  toolId: ToolId
  sessionId?: string | null
  title: string
  alive: boolean
  openedAt?: number | null
  kind?: 'pty' | 'web' | string | null
  url?: string | null
}

export interface ProjectDraft {
  name: string
  path: string
  proxyEnabled: boolean
  proxyUrl: string
}

export interface ToolCheck {
  name: string
  ok: boolean
  detail: string
}

export interface ToolVersionInfo {
  toolId: ToolId
  name: string
  found: boolean
  path?: string | null
  localVersion: string
  latestVersion: string
  compare: string
  checks?: ToolCheck[]
}

export interface UpgradeResult {
  ok: boolean
  log: string
  localVersion: string
}

export interface AppUpdateInfo {
  localVersion: string
  latestVersion: string
  compare: string
  kind: string
  kindLabel: string
  assetName: string
  htmlUrl: string
}

export interface AppUpgradeResult {
  ok: boolean
  log: string
  restart: boolean
}

export interface DshKeyStatus {
  configured: boolean
  writable: boolean
  source: string
  masked: string
  dshHome: string
  credentialsPath: string
  envBlocks: boolean
}

export interface DshKeyMeta {
  id: string
  name: string
  masked: string
  updatedAt: string
  active: boolean
  managed?: boolean
}

export interface DshKeyBundle {
  status: DshKeyStatus
  keys: DshKeyMeta[]
  vaultError?: string | null
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

export interface DshBalance {
  ok: boolean
  available: boolean | null
  currency: string | null
  totalBalance: number | null
  grantedBalance: number | null
  toppedUpBalance: number | null
  fetchedAt: string
  message: string | null
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
  usedCredits: number | null
  creditLimit: number | null
  fetchedAt: string
  message: string | null
}

export interface GrokSpendPoint {
  ts: number
  label: string
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  costUsd: number
}

export interface GrokSpend {
  ok: boolean
  totalTokens: number
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  cacheHitPercent: number
  turnCount: number
  costUsd: number
  granularity: 'hour' | 'day' | string
  rangeStart: number
  rangeEnd: number
  points: GrokSpendPoint[]
  scannedFiles: number
  fetchedAt: string
  message: string | null
}

export const TOOL_OFFICIAL_URLS: Record<ToolId, string> = {
  opencode: 'https://opencode.ai',
  grokbuild: 'https://github.com/xai-org/grok-build#installation',
  kimi: 'https://code.kimi.com',
  claude: 'https://code.claude.com/docs/en/quickstart',
  pi: 'https://pi.dev',
  dsh: 'https://github.com/deepseek-ai/deepseek-harness'
}

export const TOOLS: { id: ToolId; label: string; hint: string; tint: string }[] = [
  { id: 'opencode', label: 'OpenCode', hint: '打开编码会话', tint: 'var(--ad-opencode)' },
  { id: 'grokbuild', label: 'Grok', hint: '启动 Grok', tint: 'var(--ad-grok)' },
  { id: 'kimi', label: 'Kimi', hint: '启动 Kimi', tint: 'var(--ad-kimi)' },
  { id: 'claude', label: 'Claude Code', hint: '启动 Claude Code', tint: 'var(--ad-claude)' },
  { id: 'pi', label: 'Pi', hint: '启动 Pi', tint: 'var(--ad-pi)' },
  { id: 'dsh', label: 'DeepSeek', hint: '打开 DeepSeek Web', tint: 'var(--ad-dsh)' }
]

export function toolLabel(id: ToolId): string {
  return TOOLS.find((item) => item.id === id)?.label ?? id
}

export function parseSessionToolFilter(value: unknown): SessionToolFilter {
  if (value === 'all') return value
  const tool = TOOLS.find((item) => item.id === value)
  return tool?.id ?? 'all'
}

export function isToolInstalled(probes: ToolProbeMap | null | undefined, id: ToolId): boolean {
  if (!probes) return true
  return Boolean(probes[id]?.found)
}

export function toolsToScanForFilter(
  _filter: SessionToolFilter,
  _probes: ToolProbeMap | null | undefined
): ToolId[] {
  return TOOLS.map((tool) => tool.id)
}

export type SessionFilterAvail = {
  hasSessions?: boolean
  scanning?: boolean
  hasError?: boolean
}

export function sessionFilterBlock(
  id: SessionToolFilter,
  probes: ToolProbeMap | null | undefined,
  avail?: SessionFilterAvail
): 'missing' | 'empty' | null {
  if (id === 'all') return null
  if (avail?.hasSessions) return null
  if (!isToolInstalled(probes, id)) return 'missing'
  if (!avail) return null
  if (avail.scanning || avail.hasError) return null
  if (avail.hasSessions === false) return 'empty'
  return null
}

export function clampSessionToolFilter(
  filter: SessionToolFilter,
  probes: ToolProbeMap | null | undefined,
  avail?: SessionFilterAvail
): SessionToolFilter {
  if (filter === 'all') return 'all'
  if (sessionFilterBlock(filter, probes, avail) === 'missing') return 'all'
  return filter
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
