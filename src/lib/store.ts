import { computed, reactive } from 'vue'
import * as api from './api'
import {
  isPendingSessionId,
  liveKey,
  matchUnboundLiveToSessions,
  mergeLiveFromServer,
  pendingSessionRows,
  ptyIdFromPending
} from './liveBind'
import {
  fallbackPtyAfterExit,
  focusedFromLive,
  forgetPty,
  pickPtyForProject,
  rememberPty
} from './livePty'
import { defaultShellPath } from './platform'
import { emptyPipeline, loadPipelines } from './pipelineModel'
import type {
  AppMode,
  AppSettings,
  AppState,
  BridgePipeline,
  CodexProbe,
  CodexThread,
  FocusedSession,
  LivePtyInfo,
  Project,
  ProjectDraft,
  SessionRow,
  SessionToolFilter,
  ToolId,
  ToolProbeMap
} from './types'

export const UI_OPACITY_MIN = 0
export const UI_OPACITY_MAX = 80
export const UI_OPACITY_DEFAULT = 10

const defaultSettings = (): AppSettings => ({
  defaultProxyUrl: 'http://127.0.0.1:7890',
  opencodePath: '',
  grokbuildPath: '',
  kimiPath: '',
  powershellPath: defaultShellPath(),
  terminalFontSize: 13,
  uiOpacity: UI_OPACITY_DEFAULT,
  sessionToolFilter: 'all',
  cursorApiKey: '',
  codexPath: ''
})

const FILTER_STORAGE_KEY = 'ad-session-tool-filter'
const MODE_STORAGE_KEY = 'ad-app-mode'
const lastPtyByProject: Record<string, string> = {}
const previewSessions: Record<string, SessionRow[]> = {}

export function parseAppMode(value: unknown): AppMode {
  return value === 'bridge' ? 'bridge' : 'console'
}

export function parseSessionToolFilter(value: unknown): SessionToolFilter {
  if (value === 'all' || value === 'opencode' || value === 'grokbuild' || value === 'kimi') return value
  return 'all'
}

export function clampUiOpacity(value: number) {
  const n = Number(value)
  if (!Number.isFinite(n)) return UI_OPACITY_DEFAULT
  return Math.min(UI_OPACITY_MAX, Math.max(UI_OPACITY_MIN, Math.round(n)))
}

export function applyUiOpacity(value: number) {
  document.documentElement.style.setProperty('--ad-ui-opacity', String(clampUiOpacity(value)))
}

export const store = reactive({
  ready: false,
  projects: [] as Project[],
  settings: defaultSettings(),
  selectedProjectId: '' as string,
  selectedTool: 'grokbuild' as ToolId,
  sessions: [] as SessionRow[],
  sessionStatus: 'idle' as 'idle' | 'loading' | 'ready' | 'empty' | 'error',
  sessionErrorKind: '' as string,
  sessionError: '',
  sessionErrors: {} as Partial<Record<ToolId, { kind: string; message: string }>>,
  sessionLoading: { opencode: false, grokbuild: false, kimi: false } as Record<ToolId, boolean>,
  sessionQuery: '',
  sessionToolFilter: 'all' as SessionToolFilter,
  probes: null as ToolProbeMap | null,
  live: [] as LivePtyInfo[],
  activePtyId: '' as string,
  focusedSession: null as FocusedSession | null,
  toast: '' as string,
  toastTimer: 0,
  grokAuthRev: 0,
  appMode: 'console' as AppMode,
  pipelines: {} as Record<string, BridgePipeline>,
  codexThreads: [] as CodexThread[],
  codexStatus: 'idle' as 'idle' | 'loading' | 'ready' | 'error',
  codexError: '',
  codexProbe: null as CodexProbe | null
})

export const selectedProject = computed(() => store.projects.find((item) => item.id === store.selectedProjectId) ?? null)

export const filteredSessions = computed(() => {
  const q = store.sessionQuery.trim().toLowerCase()
  if (!q) return store.sessions
  return store.sessions.filter((item) => item.title.toLowerCase().includes(q) || item.id.toLowerCase().includes(q))
})

export const visibleSessions = computed(() => {
  const pending = pendingSessionRows(store.live, store.sessions, store.selectedProjectId)
  const rows = [...pending, ...store.sessions]
  const seen = new Set<string>()
  const uniq = rows.filter((row) => {
    const key = `${row.toolId}:${row.id}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
  const q = store.sessionQuery.trim().toLowerCase()
  if (!q) return uniq
  return uniq.filter((item) => item.title.toLowerCase().includes(q) || item.id.toLowerCase().includes(q))
})

export function findLiveForSession(sessionId: string, toolId: ToolId, projectId = store.selectedProjectId) {
  if (isPendingSessionId(sessionId)) {
    const ptyId = ptyIdFromPending(sessionId)
    return store.live.find((item) => item.ptyId === ptyId && item.alive !== false) ?? null
  }
  return (
    store.live.find(
      (item) =>
        item.projectId === projectId &&
        item.toolId === toolId &&
        item.sessionId === sessionId &&
        item.alive !== false
    ) ?? null
  )
}

export const activeLive = computed(() => store.live.find((item) => item.ptyId === store.activePtyId) ?? null)

export const currentPipeline = computed(() => {
  const id = store.selectedProjectId
  if (!id) return null
  return ensurePipeline(id)
})

export const projectHasLivePty = computed(() =>
  store.live.some((item) => item.projectId === store.selectedProjectId && item.alive)
)

export function ensurePipeline(projectId: string) {
  if (!projectId) return emptyPipeline('')
  if (!store.pipelines[projectId]) {
    store.pipelines[projectId] = emptyPipeline(projectId)
  }
  return store.pipelines[projectId]
}

export function setAppMode(mode: AppMode) {
  store.appMode = mode
  try {
    localStorage.setItem(MODE_STORAGE_KEY, mode)
    if (store.selectedProjectId) {
      localStorage.setItem(`${MODE_STORAGE_KEY}:${store.selectedProjectId}`, mode)
    }
  } catch {
    /* ignore quota */
  }
}

function restoreAppMode() {
  try {
    const projectMode = store.selectedProjectId
      ? localStorage.getItem(`${MODE_STORAGE_KEY}:${store.selectedProjectId}`)
      : null
    store.appMode = parseAppMode(projectMode || localStorage.getItem(MODE_STORAGE_KEY))
  } catch {
    store.appMode = 'console'
  }
}

export function applyState(state: AppState) {
  store.projects = state.projects
  store.settings = {
    ...defaultSettings(),
    ...state.settings,
    uiOpacity: clampUiOpacity(state.settings.uiOpacity),
    sessionToolFilter: parseSessionToolFilter(state.settings.sessionToolFilter),
    cursorApiKey: state.settings.cursorApiKey ?? '',
    codexPath: state.settings.codexPath ?? ''
  }
  store.sessionToolFilter = store.settings.sessionToolFilter
  store.ready = true
  applyUiOpacity(store.settings.uiOpacity)
  if (!store.selectedProjectId || !store.projects.some((item) => item.id === store.selectedProjectId)) {
    store.selectedProjectId = store.projects[0]?.id ?? ''
  }
}

export function noteGrokAccountChange() {
  store.grokAuthRev += 1
}

export function showToast(message: string) {
  store.toast = message
  window.clearTimeout(store.toastTimer)
  store.toastTimer = window.setTimeout(() => {
    store.toast = ''
  }, 3000)
}

export async function boot() {
  if (!api.isTauri) {
    // Browser preview only: seed projects and sessions so the dock chrome can be reviewed without Tauri.
    const now = Date.now()
    store.ready = true
    store.projects = [
      {
        id: 'preview-aitools',
        name: 'aitools',
        path: 'D:\\idea_jidian_projects\\aitools',
        proxyEnabled: true,
        proxyUrl: 'http://127.0.0.1:7890',
        createdAt: new Date(now - 86400000).toISOString()
      },
      {
        id: 'preview-dock',
        name: 'agent-dock',
        path: 'D:\\idea_jidian_projects\\aitools\\agent-dock',
        proxyEnabled: false,
        proxyUrl: 'http://127.0.0.1:7890',
        createdAt: new Date(now - 3600000).toISOString()
      }
    ]
    store.selectedProjectId = 'preview-aitools'
    previewSessions['preview-aitools'] = [
      {
        toolId: 'grokbuild',
        id: 's1',
        title: '修窗口圆角和毛玻璃',
        cwd: '',
        updatedAt: now - 120000,
        renameKind: 'overlay'
      },
      {
        toolId: 'opencode',
        id: 's3',
        title: '扫会话列表',
        cwd: '',
        updatedAt: now - 86400000,
        renameKind: 'native'
      }
    ]
    previewSessions['preview-dock'] = [
      {
        toolId: 'kimi',
        id: 'd1',
        title: '写状态栏文案',
        cwd: '',
        updatedAt: now - 7200000,
        renameKind: 'overlay'
      },
      {
        toolId: 'grokbuild',
        id: 'd2',
        title: '核对代理只作用于终端',
        cwd: '',
        updatedAt: now - 3600000,
        renameKind: 'overlay'
      }
    ]
    store.sessions = [...(previewSessions['preview-aitools'] ?? [])]
    store.sessionStatus = 'ready'
    store.focusedSession = {
      toolId: 'grokbuild',
      sessionId: 's1',
      title: '修窗口圆角和毛玻璃'
    }
    store.live = [
      {
        ptyId: 'preview-a1',
        key: 'preview-aitools|grokbuild|s1',
        projectId: 'preview-aitools',
        toolId: 'grokbuild',
        sessionId: 's1',
        title: '修窗口圆角和毛玻璃',
        alive: true,
        openedAt: now - 120000
      },
      {
        ptyId: 'preview-a2',
        key: 'preview-aitools|opencode|s3',
        projectId: 'preview-aitools',
        toolId: 'opencode',
        sessionId: 's3',
        title: '扫会话列表',
        alive: true,
        openedAt: now - 90000
      },
      {
        ptyId: 'preview-d1',
        key: 'preview-dock|kimi|d1',
        projectId: 'preview-dock',
        toolId: 'kimi',
        sessionId: 'd1',
        title: '写状态栏文案',
        alive: true,
        openedAt: now - 60000
      },
      {
        ptyId: 'preview-d2',
        key: 'preview-dock|grokbuild|d2',
        projectId: 'preview-dock',
        toolId: 'grokbuild',
        sessionId: 'd2',
        title: '核对代理只作用于终端',
        alive: true,
        openedAt: now - 30000
      }
    ]
    store.activePtyId = 'preview-a1'
    rememberPty(lastPtyByProject, 'preview-aitools', 'preview-a1')
    store.sessionToolFilter = parseSessionToolFilter(localStorage.getItem(FILTER_STORAGE_KEY))
    store.settings.sessionToolFilter = store.sessionToolFilter
    applyUiOpacity(store.settings.uiOpacity)
    restoreAppMode()
    store.pipelines = loadPipelines()
    if (store.selectedProjectId) ensurePipeline(store.selectedProjectId)
    return
  }
  applyState(await api.loadAppState())
  restoreAppMode()
  store.pipelines = loadPipelines()
  if (store.selectedProjectId) ensurePipeline(store.selectedProjectId)
  void finishBoot()
}

async function finishBoot() {
  try {
    store.probes = await api.probeTools()
  } catch {
    /* chrome is already up */
  }
  try {
    store.live = await api.listLivePtys()
  } catch {
    /* live terminals can refresh later */
  }
  if (store.selectedProjectId) {
    await refreshSessions()
  }
}

let sessionRefresh: Promise<void> | null = null
let sessionRefreshAgain: { silent?: boolean } | null = null
let blurredAt = 0
let lastFocusRefreshAt = 0

export function markWindowBlurred() {
  blurredAt = Date.now()
}

export function refreshAfterWindowFocus() {
  const now = Date.now()
  // Frameless title-bar drag can blink blur/focus; that must not rescan CLIs.
  if (blurredAt && now - blurredAt < 1000) return
  if (now - lastFocusRefreshAt < 10_000) return
  lastFocusRefreshAt = now
  void refreshSessions({ silent: true })
  void refreshLive()
}

export function isToolScanning(toolId: ToolId) {
  return store.sessionLoading[toolId]
}

export function isSessionScanning() {
  return store.sessionLoading.opencode || store.sessionLoading.grokbuild || store.sessionLoading.kimi
}

export async function refreshSessions(opts?: { silent?: boolean }) {
  if (sessionRefresh) {
    sessionRefreshAgain = opts ?? {}
    return sessionRefresh
  }
  sessionRefresh = refreshSessionsNow(opts).finally(() => {
    sessionRefresh = null
    const again = sessionRefreshAgain
    sessionRefreshAgain = null
    if (again) void refreshSessions(again)
  })
  return sessionRefresh
}

async function refreshSessionsNow(opts?: { silent?: boolean }) {
  if (!store.selectedProjectId) {
    store.sessions = []
    store.sessionErrors = {}
    store.sessionLoading = { opencode: false, grokbuild: false, kimi: false }
    store.sessionStatus = 'idle'
    return
  }
  if (!api.isTauri) {
    applyPreviewSessions()
    return
  }
  const projectId = store.selectedProjectId
  const tools = toolsToScan()
  store.sessionError = ''
  store.sessionErrorKind = ''
  store.sessionStatus = 'ready'
  for (const toolId of tools) {
    store.sessionLoading[toolId] = true
    if (!opts?.silent) delete store.sessionErrors[toolId]
  }
  await Promise.all(
    tools.map(async (toolId) => {
      try {
        const result = await api.listSessions(projectId, toolId)
        if (store.selectedProjectId !== projectId) return
        applyToolScan(toolId, result)
      } catch (err) {
        if (store.selectedProjectId !== projectId) return
        store.sessionErrors[toolId] = {
          kind: 'scan_failed',
          message: err instanceof Error ? err.message : String(err)
        }
        store.sessions = store.sessions.filter((item) => item.toolId !== toolId)
      } finally {
        if (store.selectedProjectId === projectId) {
          store.sessionLoading[toolId] = false
        }
      }
    })
  )
  if (store.selectedProjectId !== projectId) return
  await reconcileLiveBinds(projectId)
  store.sessionStatus = store.sessions.length || pendingSessionRows(store.live, store.sessions, projectId).length
    ? 'ready'
    : 'empty'
}

function applyToolScan(toolId: ToolId, result: Awaited<ReturnType<typeof api.listSessions>>) {
  if (result.ok) {
    delete store.sessionErrors[toolId]
    store.sessions = [...store.sessions.filter((item) => item.toolId !== toolId), ...result.sessions].sort(
      (a, b) => b.updatedAt - a.updatedAt
    )
    return
  }
  store.sessionErrors[toolId] = {
    kind: result.errorKind ?? 'scan_failed',
    message: result.message ?? '加载会话失败'
  }
  store.sessions = store.sessions.filter((item) => item.toolId !== toolId)
}

export function focusSession(session: FocusedSession | null) {
  store.focusedSession = session
}

function applyPreviewSessions() {
  const rows = previewSessions[store.selectedProjectId] ?? []
  store.sessions = [...rows]
  store.sessionErrors = {}
  store.sessionLoading = { opencode: false, grokbuild: false, kimi: false }
  store.sessionStatus = rows.length ? 'ready' : 'empty'
}

function rememberActivePty() {
  if (store.selectedProjectId && store.activePtyId) {
    rememberPty(lastPtyByProject, store.selectedProjectId, store.activePtyId)
  }
}

function activateLiveItem(item: LivePtyInfo) {
  store.activePtyId = item.ptyId
  store.selectedTool = item.toolId
  rememberPty(lastPtyByProject, item.projectId, item.ptyId)
  store.focusedSession = focusedFromLive(item)
}

function applyProjectPty(projectId: string) {
  const next = pickPtyForProject(store.live, projectId, lastPtyByProject[projectId])
  if (!next) {
    store.activePtyId = ''
    store.focusedSession = null
    return
  }
  activateLiveItem(next)
}

export async function selectProject(id: string) {
  if (!id) return
  if (store.selectedProjectId === id) {
    await refreshSessions()
    return
  }
  rememberActivePty()
  store.selectedProjectId = id
  store.sessions = []
  store.sessionErrors = {}
  store.sessionQuery = ''
  ensurePipeline(id)
  applyProjectPty(id)
  await refreshSessions()
}

export async function jumpToLive(ptyId: string) {
  const item = store.live.find((row) => row.ptyId === ptyId && row.alive !== false)
  if (!item) {
    showToast('这个终端已经关闭')
    return
  }
  rememberActivePty()
  rememberPty(lastPtyByProject, item.projectId, item.ptyId)
  if (store.appMode !== 'console') setAppMode('console')
  if (store.selectedProjectId !== item.projectId) {
    await selectProject(item.projectId)
  }
  activateLiveItem(item)
}

export async function selectTool(id: ToolId) {
  store.selectedTool = id
}

export async function saveProject(draft: ProjectDraft, id?: string) {
  const state = id ? await api.updateProject({ ...draft, id }) : await api.addProject(draft)
  applyState(state)
  if (!id) {
    store.selectedProjectId = store.projects[0]?.id ?? ''
  }
  if (store.selectedProjectId) ensurePipeline(store.selectedProjectId)
}

export async function dropProject(id: string) {
  applyState(await api.removeProject(id))
  await refreshSessions()
}

export async function saveAppSettings(settings: AppSettings) {
  applyState(await api.saveSettings(settings))
}

function toolsToScan(): ToolId[] {
  const filter = store.sessionToolFilter
  if (filter === 'opencode' || filter === 'grokbuild' || filter === 'kimi') {
    return [filter]
  }
  return ['opencode', 'grokbuild', 'kimi']
}

export async function setSessionToolFilter(id: SessionToolFilter) {
  store.sessionToolFilter = id
  store.settings.sessionToolFilter = id
  if (!api.isTauri) {
    localStorage.setItem(FILTER_STORAGE_KEY, id)
    return
  }
  try {
    await saveAppSettings({ ...store.settings, sessionToolFilter: id })
  } catch (error) {
    showToast(error instanceof Error ? error.message : '筛选未能保存')
  }
  void refreshSessions()
}

export async function renameCurrentSession(session: SessionRow, title: string) {
  if (!store.selectedProjectId) return
  const result = await api.renameSession(store.selectedProjectId, session.toolId, session.id, title)
  await refreshSessions()
  if (result.kind === 'overlay') {
    showToast('已保存显示名。OpenCode 自己的标题不会改。')
  } else {
    showToast('已写入会话标题')
  }
}

export async function deleteCurrentSession(session: SessionRow) {
  if (!store.selectedProjectId) return
  await api.deleteSession(store.selectedProjectId, session.toolId, session.id)
  if (store.focusedSession?.sessionId === session.id && store.focusedSession.toolId === session.toolId) {
    store.focusedSession = null
  }
  await refreshSessions()
  showToast('已删除会话')
}

function adoptLiveFallback(deadPtyId: string, deadProjectId?: string | null) {
  forgetPty(lastPtyByProject, deadPtyId)
  if (store.activePtyId && store.activePtyId !== deadPtyId && store.live.some((item) => item.ptyId === store.activePtyId)) {
    return
  }
  const next = fallbackPtyAfterExit(store.live, deadPtyId, deadProjectId, store.activePtyId)
  if (!next) {
    store.activePtyId = ''
    store.focusedSession = null
    return
  }
  activateLiveItem(next)
}

export async function refreshLive() {
  if (!api.isTauri) return
  const previous = store.activePtyId
  const previousProject = store.live.find((item) => item.ptyId === previous)?.projectId ?? store.selectedProjectId
  const local = store.live.slice()
  store.live = mergeLiveFromServer(await api.listLivePtys(), local)
  if (previous && !store.live.some((item) => item.ptyId === previous)) {
    adoptLiveFallback(previous, previousProject)
  }
}

export function markPtyExit(ptyId: string) {
  stopPendingWatch(ptyId)
  const dying = store.live.find((item) => item.ptyId === ptyId)
  store.live = store.live.filter((item) => item.ptyId !== ptyId)
  adoptLiveFallback(ptyId, dying?.projectId ?? store.selectedProjectId)
}

export function rememberOpened(info: LivePtyInfo) {
  const openedAt = info.openedAt ?? store.live.find((item) => item.ptyId === info.ptyId)?.openedAt ?? Date.now()
  const next = { ...info, openedAt, alive: true }
  const idx = store.live.findIndex((item) => item.ptyId === info.ptyId)
  if (idx >= 0) store.live[idx] = next
  else store.live.push(next)
  activateLiveItem(next)
}

const pendingWatchers = new Map<string, number>()

function stopPendingWatch(ptyId: string) {
  const id = pendingWatchers.get(ptyId)
  if (!id) return
  window.clearInterval(id)
  pendingWatchers.delete(ptyId)
}

export function watchPendingSession(ptyId: string) {
  stopPendingWatch(ptyId)
  let tries = 0
  const tick = () => {
    const live = store.live.find((item) => item.ptyId === ptyId)
    if (!live?.alive || live.sessionId) {
      stopPendingWatch(ptyId)
      return
    }
    tries += 1
    void refreshSessions({ silent: true })
    if (tries >= 8) stopPendingWatch(ptyId)
  }
  pendingWatchers.set(ptyId, window.setInterval(tick, 2000))
  window.setTimeout(tick, 800)
}

export async function bindLiveSession(ptyId: string, sessionId: string, title?: string) {
  const current = store.live.find((item) => item.ptyId === ptyId)
  if (!current) return
  current.sessionId = sessionId
  current.key = liveKey(current.projectId, current.toolId, sessionId)
  if (title?.trim()) current.title = title.trim()
  if (store.activePtyId === ptyId) {
    store.focusedSession = focusedFromLive(current)
  }
  if (!api.isTauri) return
  try {
    const bound = await api.ptyBindSession(ptyId, sessionId, title ?? null)
    const targetId = bound.ptyId || ptyId
    const idx = store.live.findIndex((item) => item.ptyId === targetId)
    if (idx >= 0) {
      store.live[idx] = {
        ...store.live[idx],
        ...bound,
        openedAt: bound.openedAt ?? store.live[idx]?.openedAt
      }
    }
    if (targetId !== ptyId) {
      store.activePtyId = targetId
    }
    const live = store.live.find((item) => item.ptyId === store.activePtyId)
    if (live) store.focusedSession = focusedFromLive(live)
  } catch {
    /* keep the local bind until the next live list */
  }
}

async function reconcileLiveBinds(projectId: string) {
  const binds = matchUnboundLiveToSessions(store.live, store.sessions, projectId)
  for (const bind of binds) {
    await bindLiveSession(bind.ptyId, bind.sessionId, bind.title)
    stopPendingWatch(bind.ptyId)
  }
}
