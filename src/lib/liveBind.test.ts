import assert from 'node:assert/strict'
import {
  isPendingSessionId,
  keepLiveTitle,
  matchUnboundLiveToSessions,
  mergeLiveFromServer,
  pendingSessionId,
  pendingSessionRows,
  pickActivePtyForProject,
  resolveOpenTarget
} from './liveBind.ts'
import type { LivePtyInfo, SessionRow } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const live = (over: Partial<LivePtyInfo> & Pick<LivePtyInfo, 'ptyId'>): LivePtyInfo => ({
  ptyId: over.ptyId,
  key: over.key ?? `${over.projectId ?? 'a'}|${over.toolId ?? 'grokbuild'}|new:${over.ptyId}`,
  projectId: over.projectId ?? 'a',
  toolId: over.toolId ?? 'grokbuild',
  sessionId: over.sessionId ?? null,
  title: over.title ?? '新会话',
  alive: over.alive ?? true,
  openedAt: 'openedAt' in over ? over.openedAt : 1_000_000
})

const session = (over: Partial<SessionRow> & Pick<SessionRow, 'id'>): SessionRow => ({
  toolId: over.toolId ?? 'grokbuild',
  id: over.id,
  title: over.title ?? over.id,
  cwd: over.cwd ?? '',
  updatedAt: over.updatedAt ?? 1_000_000,
  renameKind: over.renameKind ?? 'native'
})

test('new live pty binds to a session that appeared after it opened', () => {
  const binds = matchUnboundLiveToSessions(
    [live({ ptyId: 'p1', openedAt: 1_000_000 })],
    [session({ id: 'old', updatedAt: 100 }), session({ id: 'fresh', title: '修圆角', updatedAt: 1_000_500 })],
    'a'
  )
  assert.deepEqual(binds, [{ ptyId: 'p1', sessionId: 'fresh', title: '修圆角', toolId: 'grokbuild' }])
})

test('does not bind an old disk session to a new unbound pty', () => {
  const binds = matchUnboundLiveToSessions(
    [live({ ptyId: 'p1', openedAt: 1_000_000 })],
    [session({ id: 'old', updatedAt: 200 })],
    'a'
  )
  assert.deepEqual(binds, [])
})

test('does not guess when the pty has no openedAt', () => {
  const binds = matchUnboundLiveToSessions(
    [live({ ptyId: 'p1', openedAt: undefined })],
    [session({ id: 'fresh', updatedAt: Date.now() })],
    'a'
  )
  assert.deepEqual(binds, [])
})

test('pairs two new sessions newest-to-newest', () => {
  const binds = matchUnboundLiveToSessions(
    [
      live({ ptyId: 'older', openedAt: 1_000_000 }),
      live({ ptyId: 'newer', openedAt: 1_000_800 })
    ],
    [
      session({ id: 's-a', title: '先开', updatedAt: 1_000_100 }),
      session({ id: 's-b', title: '后开', updatedAt: 1_000_900 })
    ],
    'a'
  )
  assert.deepEqual(
    binds.sort((x, y) => x.ptyId.localeCompare(y.ptyId)),
    [
      { ptyId: 'newer', sessionId: 's-b', title: '后开', toolId: 'grokbuild' },
      { ptyId: 'older', sessionId: 's-a', title: '先开', toolId: 'grokbuild' }
    ]
  )
})

test('does not rebind a session already owned by another live pty', () => {
  const binds = matchUnboundLiveToSessions(
    [
      live({ ptyId: 'owned', sessionId: 'fresh', key: 'a|grokbuild|fresh', openedAt: 900_000 }),
      live({ ptyId: 'open', openedAt: 1_000_000 })
    ],
    [session({ id: 'fresh', updatedAt: 1_000_500 })],
    'a'
  )
  assert.deepEqual(binds, [])
})

test('DeepSeek reuses the project web instead of opening another session', () => {
  const liveDsh = live({
    ptyId: 'w1',
    toolId: 'dsh',
    sessionId: 'session-old',
    key: 'a|dsh|web',
    title: 'DeepSeek Web'
  })
  assert.deepEqual(
    resolveOpenTarget([liveDsh], { projectId: 'a', toolId: 'dsh', sessionId: 'deepseek' }),
    { action: 'bind-and-switch', ptyId: 'w1', sessionId: 'deepseek' }
  )
  assert.deepEqual(
    resolveOpenTarget(
      [live({ ...liveDsh, sessionId: 'deepseek', key: 'a|dsh|deepseek' })],
      { projectId: 'a', toolId: 'dsh', sessionId: null }
    ),
    { action: 'switch', ptyId: 'w1' }
  )
  assert.deepEqual(resolveOpenTarget([], { projectId: 'a', toolId: 'dsh', sessionId: null }), {
    action: 'open',
    sessionId: 'deepseek'
  })
})

test('DeepSeek does not sprout a pending row beside the fixed session', () => {
  const rows = pendingSessionRows(
    [live({ ptyId: 'w1', toolId: 'dsh', sessionId: null, title: 'DeepSeek Web' })],
    [session({ id: 'deepseek', toolId: 'dsh', title: 'deepseek' })],
    'a'
  )
  assert.deepEqual(rows, [])
})

test('clicking a live session switches instead of opening a second pty', () => {
  const target = resolveOpenTarget(
    [live({ ptyId: 'p1', sessionId: 's1', key: 'a|grokbuild|s1' })],
    { projectId: 'a', toolId: 'grokbuild', sessionId: 's1' }
  )
  assert.deepEqual(target, { action: 'switch', ptyId: 'p1' })
})

test('clicking a just-created session binds the unbound pty instead of resuming', () => {
  const target = resolveOpenTarget(
    [live({ ptyId: 'p1', openedAt: 1_000_000 })],
    { projectId: 'a', toolId: 'grokbuild', sessionId: 'fresh', sessionUpdatedAt: 1_000_400 }
  )
  assert.deepEqual(target, { action: 'bind-and-switch', ptyId: 'p1', sessionId: 'fresh' })
})

test('clicking an old session still opens a resume even if a new pty is running', () => {
  const target = resolveOpenTarget(
    [live({ ptyId: 'p1', openedAt: 1_000_000 })],
    { projectId: 'a', toolId: 'grokbuild', sessionId: 'old', sessionUpdatedAt: 200 }
  )
  assert.deepEqual(target, { action: 'open', sessionId: 'old' })
})

test('pending sidebar id switches the live pty', () => {
  const target = resolveOpenTarget(
    [live({ ptyId: 'p1' })],
    { projectId: 'a', toolId: 'grokbuild', sessionId: pendingSessionId('p1') }
  )
  assert.deepEqual(target, { action: 'switch', ptyId: 'p1' })
})

test('switching project keeps the current pty when it belongs to the next project', () => {
  const next = pickActivePtyForProject(
    [live({ ptyId: 'p1', projectId: 'a' }), live({ ptyId: 'p2', projectId: 'b' })],
    'a',
    'p1'
  )
  assert.equal(next, 'p1')
})

test('switching project restores that project\'s latest live pty instead of killing others', () => {
  const next = pickActivePtyForProject(
    [
      live({ ptyId: 'p1', projectId: 'a', openedAt: 1 }),
      live({ ptyId: 'p2', projectId: 'b', openedAt: 2 }),
      live({ ptyId: 'p3', projectId: 'b', openedAt: 9 })
    ],
    'b',
    'p1'
  )
  assert.equal(next, 'p3')
})

test('switching to a project with no live pty clears the active terminal without dropping others', () => {
  const rows = [live({ ptyId: 'p1', projectId: 'a' })]
  assert.equal(pickActivePtyForProject(rows, 'b', 'p1'), '')
  assert.equal(rows.length, 1)
})

test('unbound live pty appears as a pending session until the disk row exists', () => {
  const rows = pendingSessionRows(
    [live({ ptyId: 'p1', title: '新会话', openedAt: 50 })],
    [session({ id: 'old', updatedAt: 10 })],
    'a'
  )
  assert.equal(rows.length, 1)
  assert.equal(rows[0]?.id, pendingSessionId('p1'))
  assert.equal(rows[0]?.title, '新会话')
  assert.equal(isPendingSessionId(rows[0]!.id), true)
})

test('pending row disappears once the live pty is bound and listed', () => {
  const rows = pendingSessionRows(
    [live({ ptyId: 'p1', sessionId: 'fresh', key: 'a|grokbuild|fresh' })],
    [session({ id: 'fresh' })],
    'a'
  )
  assert.deepEqual(rows, [])
})

test('refreshLive keeps a local bind if the server has not caught up', () => {
  const merged = mergeLiveFromServer(
    [live({ ptyId: 'p1', sessionId: null, title: '新会话' })],
    [live({ ptyId: 'p1', sessionId: 'fresh', title: '修圆角', key: 'a|grokbuild|fresh' })]
  )
  assert.equal(merged[0]?.sessionId, 'fresh')
  assert.equal(merged[0]?.title, '修圆角')
})

test('keeps a user-typed live title when the disk row later appears', () => {
  assert.equal(keepLiveTitle('登录超时', 'Claude 会话'), '登录超时')
  assert.equal(keepLiveTitle('新会话', 'login-fix'), 'login-fix')
  assert.equal(keepLiveTitle('', 'login-fix'), 'login-fix')
})
