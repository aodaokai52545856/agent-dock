import { invoke } from '@tauri-apps/api/core'
import { previewSpend } from './grokSpend'
import type {
  AppSettings,
  AppState,
  AppUpdateInfo,
  AppUpgradeResult,
  CcswitchInstallResult,
  CcswitchLatest,
  CcswitchProbe,
  CodexProbe,
  CodexThreadList,
  DshBalance,
  DshKeyBundle,
  DshKeyStatus,
  CursorDevResult,
  GitSnapshot,
  GrokAccountList,
  GrokSpend,
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

export async function clipboardWrite(text: string): Promise<void> {
  if (!text) return
  if (isTauri) {
    try {
      await invoke('clipboard_write', { text })
      return
    } catch {
      /* fall through to the web clipboard */
    }
  }
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      /* copy is best-effort */
    }
  }
}

export async function clipboardRead(): Promise<string> {
  if (isTauri) {
    try {
      return await invoke('clipboard_read')
    } catch {
      /* fall through to the web clipboard */
    }
  }
  if (typeof navigator !== 'undefined' && navigator.clipboard?.readText) {
    try {
      return await navigator.clipboard.readText()
    } catch {
      return ''
    }
  }
  return ''
}

export async function revealMainWindow(): Promise<void> {
  if (!isTauri) return
  await invoke('reveal_main_window')
}

export async function setWindowFrost(frost: number): Promise<void> {
  if (!isTauri) return
  await invoke('set_window_frost', { frost })
}

export async function probeTools(): Promise<ToolProbeMap> {
  return invoke('probe_tools')
}

export async function listSessions(projectId: string, toolId: ToolId): Promise<SessionListResult> {
  return invoke('list_sessions', { projectId, toolId })
}

function previewDocs(toolId: ToolId): SessionDoc[] {
  if (toolId === 'opencode' || toolId === 'dsh') return []
  const now = Date.now()
  const summary: SessionDoc = {
    kind: 'summary',
    title: '本轮总结',
    path: `agent-dock:summary:${toolId}/preview`,
    relPath: null,
    updatedAt: now - 30000,
    source: toolId
  }
  if (toolId === 'kimi') {
    return [
      summary,
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
    summary,
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
  if (path.startsWith('agent-dock:summary:')) {
    return {
      path,
      title: '本轮总结',
      relPath: null,
      text: '已经把浅色对比和文档栏识别改好。Markdown 文件会出现在右侧，最后一轮说明也会单独列出。\n'
    }
  }
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
  uiTheme?: 'light' | 'dark'
}): Promise<PtyOpened> {
  return invoke('pty_open', payload)
}

export async function ptyWrite(ptyId: string, data: string): Promise<void> {
  if (!isTauri) return
  return invoke('pty_write', { ptyId, data })
}

export async function ptyResize(ptyId: string, cols: number, rows: number): Promise<void> {
  if (!isTauri) return
  return invoke('pty_resize', { ptyId, cols, rows })
}

export async function ptyKill(ptyId: string): Promise<void> {
  if (!isTauri) return
  return invoke('pty_kill', { ptyId })
}

export type DshEmbedBounds = {
  x: number
  y: number
  width: number
  height: number
}

export async function dshEmbedOpen(
  url: string,
  bounds: DshEmbedBounds,
  themeScript?: string
): Promise<void> {
  if (!isTauri) return
  return invoke('dsh_embed_open', { url, bounds, themeScript })
}

export async function dshEmbedSetBounds(bounds: DshEmbedBounds): Promise<void> {
  if (!isTauri) return
  return invoke('dsh_embed_set_bounds', { bounds })
}

export async function dshEmbedSetVisible(visible: boolean): Promise<void> {
  if (!isTauri) return
  return invoke('dsh_embed_set_visible', { visible })
}

export async function dshEmbedClose(): Promise<void> {
  if (!isTauri) return
  return invoke('dsh_embed_close')
}

export async function dshEmbedApplyTheme(script: string): Promise<void> {
  if (!isTauri) return
  return invoke('dsh_embed_apply_theme', { script })
}

export async function ptyBindSession(
  ptyId: string,
  sessionId: string,
  title?: string | null
): Promise<LivePtyInfo> {
  if (!isTauri) {
    return {
      ptyId,
      key: '',
      projectId: '',
      toolId: 'grokbuild',
      sessionId,
      title: title || '新会话',
      alive: true,
      openedAt: Date.now()
    }
  }
  return invoke('pty_bind_session', { ptyId, sessionId, title: title ?? null })
}

export async function listLivePtys(): Promise<LivePtyInfo[]> {
  return invoke('list_live_ptys')
}

export async function appVersion(): Promise<string> {
  if (!isTauri) return '0.1.5'
  return invoke('app_version')
}

export async function checkAppUpdate(proxyUrl?: string): Promise<AppUpdateInfo> {
  if (!isTauri) {
    return {
      localVersion: '0.1.5',
      latestVersion: '0.1.5',
      compare: '已是最新',
      kind: 'win-setup',
      kindLabel: 'Windows 安装包',
      assetName: 'AgentDock-0.1.5-Setup.exe',
      htmlUrl: 'https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.5'
    }
  }
  return invoke('check_app_update', { proxyUrl: proxyUrl || null })
}

export async function upgradeApp(proxyUrl?: string): Promise<AppUpgradeResult> {
  if (!isTauri) return { ok: false, log: '浏览器预览无法更新桌面客户端。', restart: false }
  return invoke('upgrade_app', { proxyUrl: proxyUrl || null })
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

export async function listToolVersions(proxyUrl?: string, toolId?: ToolId): Promise<ToolVersionInfo[]> {
  if (!isTauri) {
    const rows: ToolVersionInfo[] = [
      { toolId: 'opencode', name: 'OpenCode', found: true, path: 'C:\\Users\\me\\AppData\\Roaming\\npm\\opencode.cmd', localVersion: '1.18.30', latestVersion: '1.18.30', compare: '已是最新' },
      { toolId: 'grokbuild', name: 'Grok Build', found: true, path: 'C:\\Users\\me\\.grok\\bin\\grok.exe', localVersion: '1.0.25', latestVersion: '1.0.25', compare: '已是最新' },
      { toolId: 'kimi', name: 'Kimi', found: true, path: 'C:\\Users\\me\\.kimi-code\\bin\\kimi.exe', localVersion: '0.34.0', latestVersion: '0.42.0', compare: '可更新' },
      { toolId: 'claude', name: 'Claude Code', found: true, path: 'C:\\Users\\me\\.local\\bin\\claude.exe', localVersion: '2.1.269', latestVersion: '2.1.269', compare: '已是最新' },
      { toolId: 'pi', name: 'Pi', found: false, localVersion: '未安装', latestVersion: '0.85.1', compare: '未安装' },
      { toolId: 'dsh', name: 'DeepSeek Harness', found: false, localVersion: '未安装', latestVersion: '0.1.0', compare: '未安装' }
    ]
    return toolId ? rows.filter((row) => row.toolId === toolId) : rows
  }
  return invoke('list_tool_versions', { proxyUrl: proxyUrl ?? null, toolId: toolId ?? null })
}

export async function upgradeTool(toolId: ToolId, proxyUrl?: string): Promise<UpgradeResult> {
  if (!isTauri) {
    return { ok: false, log: '请在桌面端升级 CLI。', localVersion: '—' }
  }
  return invoke('upgrade_tool', { toolId, proxyUrl: proxyUrl ?? null })
}

export async function uninstallTool(toolId: ToolId): Promise<UpgradeResult> {
  if (!isTauri) {
    return { ok: false, log: '请在桌面端卸载 CLI。', localVersion: '—' }
  }
  return invoke('uninstall_tool', { toolId })
}

export async function probeCcswitch(): Promise<CcswitchProbe> {
  if (!isTauri) {
    return { found: false, path: null, downloadDir: '', installDir: '' }
  }
  return invoke('probe_ccswitch')
}

export async function ccswitchLatest(proxyUrl?: string): Promise<CcswitchLatest> {
  if (!isTauri) {
    return {
      version: '3.20.3',
      tag: 'v3.20.3',
      assetName: 'CC-Switch-v3.20.3-Windows-Portable.zip',
      size: 13_658_645,
      url: 'https://github.com/farion1231/cc-switch/releases/latest',
      homepage: 'https://ccswitch.io',
      releasesUrl: 'https://github.com/farion1231/cc-switch/releases'
    }
  }
  return invoke('ccswitch_latest', { proxyUrl: proxyUrl ?? null })
}

export async function installCcswitch(payload: {
  downloadDir: string
  installDir: string
  proxyUrl?: string
}): Promise<CcswitchInstallResult> {
  if (!isTauri) {
    return { ok: false, path: '', version: '', log: '请在桌面端下载安装 CC Switch。' }
  }
  return invoke('install_ccswitch', {
    downloadDir: payload.downloadDir,
    installDir: payload.installDir,
    proxyUrl: payload.proxyUrl ?? null
  })
}

export async function launchCcswitch(path?: string | null): Promise<void> {
  if (!isTauri) return
  return invoke('launch_ccswitch', { path: path ?? null })
}

export async function rememberCcswitchPath(path: string): Promise<AppState> {
  return invoke('remember_ccswitch_path', { path })
}

const emptyDshBundle = (): DshKeyBundle => ({
  status: {
    configured: false,
    writable: true,
    source: 'none',
    masked: '',
    dshHome: '~/.dsh',
    credentialsPath: '~/.dsh/.credentials.yaml',
    envBlocks: false
  },
  keys: isTauri
    ? []
    : [
        { id: 'preview-work', name: '工作号', masked: 'sk-ab…wxyz', updatedAt: '', active: true, managed: true },
        { id: 'preview-home', name: '个人号', masked: 'sk-cd…1234', updatedAt: '', active: false, managed: true }
      ]
})

export async function dshKeyStatus(projectPath?: string | null): Promise<DshKeyStatus> {
  if (!isTauri) return emptyDshBundle().status
  return invoke('dsh_key_status', { projectPath: projectPath ?? null })
}

export async function dshListKeys(projectPath?: string | null): Promise<DshKeyBundle> {
  if (!isTauri) return emptyDshBundle()
  return invoke('dsh_list_keys', { projectPath: projectPath ?? null })
}

export async function dshAddKey(name: string, key: string, projectPath?: string | null): Promise<DshKeyBundle> {
  return invoke('dsh_add_key', { name, key, projectPath: projectPath ?? null })
}

export async function dshSwitchKey(id: string, projectPath?: string | null): Promise<DshKeyBundle> {
  return invoke('dsh_switch_key', { id, projectPath: projectPath ?? null })
}

export async function dshDeleteKey(id: string, projectPath?: string | null): Promise<DshKeyBundle> {
  return invoke('dsh_delete_key', { id, projectPath: projectPath ?? null })
}

export async function dshRenameKey(id: string, name: string, projectPath?: string | null): Promise<DshKeyBundle> {
  return invoke('dsh_rename_key', { id, name, projectPath: projectPath ?? null })
}

const previewBalance = (): DshBalance => ({
  ok: true,
  available: true,
  currency: 'CNY',
  totalBalance: 110,
  grantedBalance: 10,
  toppedUpBalance: 100,
  fetchedAt: new Date().toISOString(),
  message: null
})

export async function dshBalance(projectId?: string | null): Promise<DshBalance> {
  if (!isTauri) return previewBalance()
  return invoke('dsh_balance', { projectId: projectId || null })
}

export async function openExternal(url: string): Promise<void> {
  if (!isTauri) {
    window.open(url, '_blank', 'noopener')
    return
  }
  return invoke('open_external', { url })
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
    usedCredits: 4277,
    creditLimit: 60000,
    fetchedAt: new Date().toISOString(),
    message: null
  }
}

export async function grokUsage(projectId?: string | null): Promise<GrokUsage> {
  if (!isTauri) return previewUsage()
  return invoke('grok_usage', { projectId: projectId || null })
}

export async function grokSpend(start?: number | null, end?: number | null): Promise<GrokSpend> {
  if (!isTauri) return previewSpend(start ?? undefined, end ?? undefined)
  return invoke('grok_spend', { start: start ?? null, end: end ?? null })
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
