import assert from 'node:assert/strict'
import { createVersionCenter } from './versionCenter.ts'
import { seedVersionRows } from './toolVersions.ts'
import type { AppUpdateInfo, VersionTask, VersionTaskSnapshot } from './types.ts'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
function fixture() {
  let snapshot: VersionTaskSnapshot = { revision: 0, batchId: '', readyVersion: null, tasks: [] }
  let starts = 0
  const notifications: VersionTask[][] = []
  const rows = seedVersionRows().map(row => ({ ...row, found: true, localVersion: '1.0', latestVersion: '2.0', compare: '可更新' }))
  const app: AppUpdateInfo = { localVersion: '0.1.18', latestVersion: '0.1.19', compare: '可更新', kind: 'win-setup', kindLabel: 'Windows 安装包', assetName: 'setup.exe', htmlUrl: '' }
  const backend = {
    appVersion: async () => '0.1.18',
    checkAppUpdate: async (_proxy: string) => app,
    listToolVersions: async (_proxy: string, target: string) => rows.filter(row => row.toolId === target),
    versionTaskState: async () => snapshot,
    startVersionTasks: async (_targets: string[], _action: string, _proxy: string) => { starts++; return snapshot },
    settled: (tasks: VersionTask[]) => { notifications.push(tasks) }
  }
  return { backend, notifications, rows, app, set: (next: VersionTaskSnapshot) => { snapshot = next }, starts: () => starts }
}
const task = (status: VersionTask['status'], extra: Partial<VersionTask> = {}): VersionTask => ({
  id: 'task', batchId: 'batch', target: 'claude', action: 'upgrade', status, stage: '正在安装', log: 'saved installer output',
  localVersion: status === 'succeeded' ? '2.0' : '', downloaded: 0, total: null, startedAt: 100, finishedAt: null, restart: false, ...extra
})
const snap = (revision: number, tasks: VersionTask[]): VersionTaskSnapshot => ({ revision, batchId: 'batch', readyVersion: tasks.find(task => task.status === 'ready')?.localVersion || null, tasks })
async function test(name: string, run: () => Promise<void>) { await run(); console.log(`ok ${name}`) }

await test('closing and reopening retains checks, proxy, queued jobs and busy guard', async () => {
  const f = fixture(), center = createVersionCenter(f.backend)
  await center.sync(); center.open('http://proxy'); await center.check('all')
  const gate = deferred<VersionTaskSnapshot>()
  f.backend.startVersionTasks = async () => { return gate.promise }
  const start = center.start(['claude', 'kimi'], 'upgrade')
  center.state.proxyUrl = 'http://chosen-proxy'
  center.close(); center.open('http://default-proxy')
  assert.equal(center.busy.value, true)
  assert.equal(center.state.rows[0]!.localVersion, '1.0')
  assert.equal(center.state.proxyUrl, 'http://chosen-proxy')
  gate.resolve(snap(1, [task('running'), task('queued', { id: 'second', target: 'kimi' })])); await start
  center.close(); center.open('another')
  assert.equal(center.running.value, true)
  assert.equal(center.batch.value.length, 2)
  assert.equal(center.state.snapshot.tasks[0]!.log, 'saved installer output')
})

await test('a remounted client restores running jobs, keeps them on sync error, and ignores stale snapshots', async () => {
  const f = fixture(); f.set(snap(4, [task('running')]))
  const center = createVersionCenter(f.backend); await center.sync()
  assert.equal(center.running.value, true)
  f.backend.versionTaskState = async () => { throw new Error('IPC unavailable') }
  await center.sync(); assert.equal(center.running.value, true); assert.match(center.state.syncError, /不会因此取消/)
  center.accept(snap(5, [task('succeeded')]))
  center.accept(snap(3, [task('running')]))
  assert.equal(center.running.value, false)
  assert.equal(center.state.rows.find(row => row.toolId === 'claude')?.localVersion, '2.0')
})

await test('one batch completion notifies once and failed tasks do not hide successful results', async () => {
  const f = fixture(), center = createVersionCenter(f.backend); await center.sync()
  center.accept(snap(1, [task('running'), task('queued', { id: 'second', target: 'kimi' })]))
  center.accept(snap(2, [task('failed'), task('succeeded', { id: 'second', target: 'kimi' })]))
  center.accept(snap(2, [task('failed'), task('succeeded', { id: 'second', target: 'kimi' })]))
  assert.equal(f.notifications.length, 1)
  assert.equal(center.finished.value, 2)
  assert.equal(center.state.unread, true)
  center.open(''); assert.equal(center.state.unread, false)
})

await test('fast background completion beats an older start response', async () => {
  const f = fixture(), center = createVersionCenter(f.backend); await center.sync()
  const gate = deferred<VersionTaskSnapshot>(); f.backend.startVersionTasks = () => gate.promise
  const pending = center.start(['claude'], 'upgrade')
  center.accept(snap(3, [task('succeeded')]))
  gate.resolve(snap(1, [task('queued')])); await pending
  assert.equal(center.running.value, false)
  assert.equal(center.busy.value, false)
  assert.equal(f.notifications.length, 1)
})

await test('duplicate clicks submit only one background batch', async () => {
  const f = fixture(), center = createVersionCenter(f.backend); await center.sync()
  const gate = deferred<VersionTaskSnapshot>(); let calls = 0
  f.backend.startVersionTasks = () => { calls++; return gate.promise }
  const first = center.start(['claude'], 'upgrade')
  await center.start(['claude'], 'upgrade')
  assert.equal(calls, 1)
  gate.resolve(snap(1, [task('queued')])); await first
})

await test('a lost start response recovers authoritative running state', async () => {
  const f = fixture(), center = createVersionCenter(f.backend); await center.sync()
  f.backend.startVersionTasks = async () => { f.set(snap(2, [task('running')])); throw new Error('response lost') }
  await center.start(['claude'], 'upgrade')
  assert.equal(center.running.value, true)
  assert.match(center.state.error, /response lost/)
})

await test('download completion waits for a separate explicit install action', async () => {
  const f = fixture(), center = createVersionCenter(f.backend); await center.sync()
  f.set(snap(3, [task('ready', { target: 'app', action: 'download', localVersion: '0.1.19', downloaded: 200, total: 200 })]))
  await center.start(['app'], 'download')
  center.close(); center.open('')
  assert.equal(center.appReady.value, true)
  assert.equal(f.starts(), 1)
  assert.equal(center.running.value, false)
})

await test('failed refresh preserves previously detected versions and sends an explicit empty proxy for direct mode', async () => {
  const f = fixture(), center = createVersionCenter(f.backend); await center.check('claude')
  let proxy = 'not called'
  f.backend.listToolVersions = async (value) => { proxy = value; throw new Error('network timeout') }
  center.state.proxyUrl = ''; await center.check('claude')
  assert.equal(proxy, '')
  assert.equal(center.state.rows.find(row => row.toolId === 'claude')?.localVersion, '1.0')
  assert.equal(center.state.errors.claude, 'network timeout')
})

await test('ready installer remains visible even if its history row has been trimmed', async () => {
  const f = fixture(), center = createVersionCenter(f.backend)
  f.set({ ...snap(100, [task('succeeded')]), readyVersion: '0.1.19' })
  await center.sync()
  assert.equal(center.appReady.value, true)
  assert.equal(center.state.snapshot.tasks.some(task => task.target === 'app'), false)
})
