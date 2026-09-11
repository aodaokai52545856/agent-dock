import { invoke } from '@tauri-apps/api/core'
import type {
  AppSettings,
  AppState,
  CodexProbe,
  CodexThreadList,
  CursorDevResult,
  GitSnapshot,
  GrokAccountList,
  GrokUsage,
  LivePtyInfo,
  ProjectDraft,
  PtyOpened,
  RenameKind,
  ReviewStartResult,
  ReviewTargetKind,
  SessionDoc,
  SessionDocBody,
  SessionListResult,
  SessionTurn,
  ToolId,
  ToolProbeMap,
  ToolVersionInfo,
  UpgradeResult
} from './types'

export const isTauri = typeof window !== 'undefined' && !!(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__

export async function loadAppState(): Promise<AppState> {
  return invoke('load_app_state')
}

export async function saveSettings(settings: AppSettings): Promise<AppState> {
  return invoke('save_settings', { settings })
}

export async function addProject(draft: ProjectDraft): Promise<AppState> {
  return invoke('add_project', { draft })
}

export async function updateProject(draft: ProjectDraft & { id: string }): Promise<AppState> {
  return invoke('update_project', { draft })
}

export async function removeProject(projectId: string): Promise<AppState> {
  return invoke('remove_project', { projectId })
}

export async function folderLabel(path: string): Promise<string> {
  return invoke('folder_label', { path })
}

export async function revealMainWindow(): Promise<void> {
  if (!isTauri) return
  await invoke('reveal_main_window')
}

export async function probeTools(): Promise<ToolProbeMap> {
  return invoke('probe_tools')
}

export async function listSessions(projectId: string, toolId: ToolId): Promise<SessionListResult> {
  return invoke('list_sessions', { projectId, toolId })
}

function previewDocs(toolId: ToolId): SessionDoc[] {
  if (toolId === 'opencode') return []
  const now = Date.now()
  if (toolId === 'kimi') {
    return [
      {
        kind: 'plan',
        title: '写状态栏文案',
        path: 'D:\\preview\\kimi\\plans\\status-bar.md',
        relPath: null,
        updatedAt: now - 180000,
        source: 'kimi'
      }
    ]
  }
  return [
    {
      kind: 'plan',
      title: 'Plan',
      path: 'D:\\preview\\grok\\plan.md',
      relPath: null,
      updatedAt: now - 60000,
      source: 'grokbuild'
    },
    {
      kind: 'plan',
      title: '2026-09-11-cite-rail',
      path: 'D:\\idea_jidian_projects\\aitools\\docs\\superpowers\\plans\\2026-09-11-cite-rail.md',
      relPath: 'docs/superpowers/plans/2026-09-11-cite-rail.md',
      updatedAt: now - 120000,
      source: 'grokbuild'
    }
  ]
}

function previewDocBody(path: string): SessionDocBody {
  if (path.endsWith('cite-rail.md')) {
    return {
      path,
      title: '2026-09-11-cite-rail',
      relPath: 'docs/superpowers/plans/2026-09-11-cite-rail.md',
      text: '# 跨 CLI 引用\n\n- 右侧列出本 session 写出的 Markdown\n- 弹窗预览后写入目标终端提示行\n'
    }
  }
  if (path.includes('kimi')) {
    return {
      path,
      title: '写状态栏文案',
      relPath: null,
      text: '# 状态栏\n\n用量一行：`Grok 剩 58% · 9/15 09:53 重置`。\n'
    }
  }
  return {
    path,
    title: 'Plan',
    relPath: null,
    text: '# 修窗口圆角\n\n1. 先改 token\n2. 再核对毛玻璃\n'
  }
}

function previewTurns(toolId: ToolId): SessionTurn[] {
  if (toolId === 'opencode') return []
  const user = toolId === 'kimi' ? '把状态栏文案写短一点' : '修窗口圆角和毛玻璃'
  const assistant = toolId === 'kimi' ? '用量一行放在状态栏右侧，失败时保留上次结果。' : '已经把圆角和透明度改到现有 token。'
  return [
    { id: 't-0', role: 'user', excerpt: user, text: user },
    { id: 't-1', role: 'assistant', excerpt: assistant, text: assistant }
  ]
}

export async function listSessionDocs(
  projectId: string,
  toolId: ToolId,
  sessionId: string
): Promise<SessionDoc[]> {
  if (!isTauri) return previewDocs(toolId)
  return invoke('list_session_docs', { projectId, toolId, sessionId })
}

export async function readSessionDoc(projectId: string, path: string): Promise<SessionDocBody> {
  if (!isTauri) return previewDocBody(path)
  return invoke('read_session_doc', { projectId, path })
}

export async function listSessionTurns(
  projectId: string,
  toolId: ToolId,
  sessionId: string
): Promise<SessionTurn[]> {
  if (!isTauri) return previewTurns(toolId)
  return invoke('list_session_turns', { projectId, toolId, sessionId })
}

export async function renameSession(
  projectId: string,
  toolId: ToolId,
  sessionId: string,
  title: string
): Promise<{ kind: RenameKind }> {
  return invoke('rename_session', { projectId, toolId, sessionId, title })
}

export async function deleteSession(projectId: string, toolId: ToolId, sessionId: string): Promise<void> {
  return invoke('delete_session', { projectId, toolId, sessionId })
}

export async function ptyOpen(payload: {
  projectId: string
  toolId: ToolId
  sessionId?: string | null
  title: string
  cols: number
  rows: number
}): Promise<PtyOpened> {
  return invoke('pty_open', payload)
}

export async function ptyWrite(ptyId: string, data: string): Promise<void> {
  return invoke('pty_write', { ptyId, data })
}

export async function ptyResize(ptyId: string, cols: number, rows: number): Promise<void> {
  return invoke('pty_resize', { ptyId, cols, rows })
}

export async function ptyKill(ptyId: string): Promise<void> {
  return invoke('pty_kill', { ptyId })
}

export async function listLivePtys(): Promise<LivePtyInfo[]> {
  return invoke('list_live_ptys')
}

export async function appVersion(): Promise<string> {
  if (!isTauri) return '0.1.0'
  return invoke('app_version')
}

export async function hostPlatform(): Promise<'macos' | 'windows' | 'linux'> {
  if (!isTauri) return detectBrowserOs()
  return invoke('host_platform')
}

function detectBrowserOs(): 'macos' | 'windows' | 'linux' {
  const ua = typeof navigator === 'undefined' ? '' : navigator.userAgent
  if (/Mac|iPhone|iPad/.test(ua) && !/Win/.test(ua)) return 'macos'
  if (/Win/.test(ua)) return 'windows'
  return 'linux'
}

export async function listToolVersions(): Promise<ToolVersionInfo[]> {
  if (!isTauri) {
    return [
      { toolId: 'opencode', name: 'OpenCode', found: false, localVersion: '—', latestVersion: '—', compare: '无法对比' },
      { toolId: 'grokbuild', name: 'Grok Build', found: false, localVersion: '—', latestVersion: '—', compare: '无法对比' },
      { toolId: 'kimi', name: 'Kimi', found: false, localVersion: '—', latestVersion: '—', compare: '无法对比' }
    ]
  }
  return invoke('list_tool_versions')
}

export async function upgradeTool(toolId: ToolId): Promise<UpgradeResult> {
  if (!isTauri) {
    return { ok: false, log: '请在桌面端升级 CLI。', localVersion: '—' }
  }
  return invoke('upgrade_tool', { toolId })
}

const emptyAccounts = (): GrokAccountList => ({
  loggedIn: false,
  currentEmail: '',
  currentName: '',
  accounts: []
})

export async function listGrokAccounts(): Promise<GrokAccountList> {
  if (!isTauri) return emptyAccounts()
  return invoke('list_grok_accounts')
}

export async function saveGrokAccount(name: string): Promise<GrokAccountList> {
  if (!isTauri) return emptyAccounts()
  return invoke('save_grok_account', { name })
}

export async function switchGrokAccount(accountId: string): Promise<GrokAccountList> {
  if (!isTauri) return emptyAccounts()
  return invoke('switch_grok_account', { accountId })
}

export async function deleteGrokAccount(accountId: string): Promise<GrokAccountList> {
  if (!isTauri) return emptyAccounts()
  return invoke('delete_grok_account', { accountId })
}

export async function loginGrokAccount(): Promise<GrokAccountList> {
  if (!isTauri) return emptyAccounts()
  return invoke('login_grok_account')
}

const previewUsage = (): GrokUsage => {
  const resets = new Date()
  resets.setDate(resets.getDate() + 4)
  resets.setHours(9, 53, 0, 0)
  return {
    ok: true,
    usedPercent: 42,
    remainingPercent: 58,
    resetsAt: resets.toISOString(),
    periodLabel: '本周',
    prepaidBalance: 0,
    onDemandUsed: 0,
    onDemandCap: 0,
    grokBuildUsedPercent: 42,
    fetchedAt: new Date().toISOString(),
    message: null
  }
}

export async function grokUsage(projectId?: string | null): Promise<GrokUsage> {
  if (!isTauri) return previewUsage()
  return invoke('grok_usage', { projectId: projectId || null })
}

export async function codexProbe(projectId: string): Promise<CodexProbe> {
  return invoke('codex_probe', { projectId })
}

export async function listCodexThreads(projectId: string): Promise<CodexThreadList> {
  return invoke('list_codex_threads', { projectId })
}

export async function gitSnapshot(projectId: string): Promise<GitSnapshot> {
  return invoke('git_snapshot', { projectId })
}

export async function startCodexReview(payload: {
  projectId: string
  threadIds: string[]
  targetKind: ReviewTargetKind
  commitSha?: string
  baseBranch?: string
  customInstructions?: string
}): Promise<ReviewStartResult> {
  return invoke('start_codex_review', payload)
}

export async function cursorDevRun(payload: {
  projectId: string
  prompt: string
  agentId?: string | null
}): Promise<CursorDevResult> {
  return invoke('cursor_dev_run', payload)
}
