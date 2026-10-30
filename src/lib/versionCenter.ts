import { computed, reactive } from 'vue'
import { seedVersionRows, patchVersionRow } from './toolVersions.ts'
import { TOOLS, type AppUpdateInfo, type ToolId, type ToolVersionInfo, type VersionAction, type VersionTask, type VersionTaskSnapshot } from './types.ts'

interface Backend {
  appVersion(): Promise<string>
  checkAppUpdate(proxyUrl: string): Promise<AppUpdateInfo>
  listToolVersions(proxyUrl: string, toolId: ToolId): Promise<ToolVersionInfo[]>
  versionTaskState(): Promise<VersionTaskSnapshot>
  startVersionTasks(targets: string[], action: VersionAction, proxyUrl: string): Promise<VersionTaskSnapshot>
  settled?(tasks: VersionTask[]): void
}

export function taskRunning(task: VersionTask) { return task.status === 'queued' || task.status === 'running' }
export function toolInstalled(row: ToolVersionInfo) {
  return row.found && row.localVersion !== '未安装' && row.compare !== '未通过检查' && (!row.checks?.length || row.checks.every(check => check.ok))
}
export function canUpgradeTool(row: ToolVersionInfo) { return row.toolId !== 'grokbuild' || row.found }

export function createVersionCenter(backend: Backend) {
  const state = reactive({
    appVersion: '',
    app: { localVersion: '', latestVersion: '-', compare: '未检测', kind: '', kindLabel: '', assetName: '', htmlUrl: '' } as AppUpdateInfo,
    rows: seedVersionRows(),
    checking: {} as Record<string, boolean>,
    errors: {} as Record<string, string>,
    proxyUrl: '', proxyInitialized: false, panelOpen: false, starting: false, error: '', syncError: '', unread: false,
    snapshot: { revision: 0, batchId: '', readyVersion: null, tasks: [] } as VersionTaskSnapshot
  })
  let versionLoaded = false
  let synced = false
  let notifiedBatch = ''
  const reconciled = new Set<string>()
  const running = computed(() => state.snapshot.tasks.some(taskRunning))
  const busy = computed(() => running.value || state.starting || Object.values(state.checking).some(Boolean))
  const batch = computed(() => state.snapshot.tasks.filter(task => task.batchId === state.snapshot.batchId))
  const finished = computed(() => batch.value.filter(task => !taskRunning(task)).length)
  const active = computed(() => batch.value.find(task => task.status === 'running') || batch.value.find(task => task.status === 'queued'))
  const updatable = computed(() => state.rows.filter(row => row.compare === '可更新' && canUpgradeTool(row)))
  const appReady = computed(() => Boolean(state.snapshot.readyVersion))

  function accept(snapshot: VersionTaskSnapshot) {
    if (snapshot.revision < state.snapshot.revision) return
    const initial = !synced
    synced = true
    state.snapshot = snapshot
    state.syncError = ''
    for (const task of snapshot.tasks) {
      if (taskRunning(task) || reconciled.has(task.id)) continue
      reconciled.add(task.id)
      if (task.target === 'app') continue
      const row = state.rows.find(row => row.toolId === task.target)
      if (!row || !task.localVersion) continue
      const removed = task.action === 'uninstall' && task.status === 'succeeded'
      const installed = !removed && task.status === 'succeeded'
      const localVersion = removed ? '未安装' : task.localVersion
      state.rows = patchVersionRow(state.rows, { ...row, found: installed || (!removed && row.found), localVersion,
        compare: removed ? '未安装' : installed ? (row.latestVersion === '-' ? '待检测' : localVersion === row.latestVersion ? '已是最新' : '待检测') : row.compare,
        checks: installed ? [] : row.checks })
    }
    const current = snapshot.tasks.filter(task => task.batchId === snapshot.batchId)
    if (current.length && current.every(task => !taskRunning(task)) && snapshot.batchId !== notifiedBatch) {
      notifiedBatch = snapshot.batchId
      if (!initial) { state.unread = !state.panelOpen; backend.settled?.(current) }
    }
  }

  async function sync() {
    try { accept(await backend.versionTaskState()) }
    catch (error) { state.syncError = `暂时无法读取后台进度：${message(error)}。任务不会因此取消，可稍后刷新。` }
  }
  async function init() {
    if (!versionLoaded) {
      versionLoaded = true
      try { state.appVersion = await backend.appVersion() }
      catch { state.appVersion = '未知'; versionLoaded = false }
    }
  }
  function open(defaultProxy: string) {
    state.panelOpen = true
    state.unread = false
    if (!state.proxyInitialized) { state.proxyUrl = defaultProxy; state.proxyInitialized = true }
    void init()
    void sync()
  }
  function close() { state.panelOpen = false }

  async function check(target: ToolId | 'app' | 'all') {
    if (busy.value) return
    const targets = target === 'all' ? ['app', ...TOOLS.map(tool => tool.id)] : [target]
    const proxy = state.proxyUrl.trim()
    state.error = ''
    for (const id of targets) { state.checking[id] = true; delete state.errors[id] }
    await Promise.all(targets.map(async id => {
      try {
        if (id === 'app') { state.app = await backend.checkAppUpdate(proxy); state.appVersion = state.app.localVersion || state.appVersion }
        else {
          const result = await backend.listToolVersions(proxy, id as ToolId)
          const row = result.find(row => row.toolId === id)
          if (!row) throw new Error('未返回版本信息，请重试')
          state.rows = patchVersionRow(state.rows, row)
        }
      } catch (error) { state.errors[id] = message(error) }
      finally { state.checking[id] = false }
    }))
  }

  async function start(targets: string[], action: VersionAction) {
    if (busy.value || !targets.length) return
    state.starting = true
    state.error = ''
    try { accept(await backend.startVersionTasks([...targets], action, state.proxyUrl.trim())) }
    catch (error) { state.error = message(error); await sync() }
    finally { state.starting = false }
  }
  return { state, running, busy, batch, finished, active, updatable, appReady, accept, sync, init, open, close, check, start }
}
const message = (error: unknown) => error instanceof Error ? error.message : String(error)
