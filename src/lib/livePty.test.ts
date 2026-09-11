import assert from 'node:assert/strict'
import {
  fallbackPtyAfterExit,
  focusedFromLive,
  forgetPty,
  groupLiveByProject,
  isCurrentSession,
  liveOfProject,
  pickPtyForProject,
  projectSwitchView
} from './livePty.ts'
import type { LivePtyInfo } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const pty = (
  over: Partial<LivePtyInfo> & Pick<LivePtyInfo, 'ptyId' | 'projectId' | 'title'>
): LivePtyInfo => ({
  key: over.ptyId,
  toolId: 'grokbuild',
  sessionId: over.sessionId ?? over.ptyId,
  alive: true,
  ...over
})

const live = [
  pty({ ptyId: 'a1', projectId: 'aitools', title: '修圆角', toolId: 'grokbuild' }),
  pty({ ptyId: 'a2', projectId: 'aitools', title: '扫会话', toolId: 'opencode' }),
  pty({ ptyId: 'd1', projectId: 'dock', title: '状态栏', toolId: 'kimi' }),
  pty({ ptyId: 'd2', projectId: 'dock', title: '代理核对', toolId: 'grokbuild' })
]

test('switching project does not drop live PTYs', () => {
  const next = projectSwitchView(live, 'aitools', 'dock', 'a1', {})
  assert.equal(next.live, live)
  assert.equal(next.live.length, 4)
  assert.equal(next.lastByProject.aitools, 'a1')
})

test('switching to a project restores its last live terminal', () => {
  const toDock = projectSwitchView(live, 'aitools', 'dock', 'a1', { dock: 'd2' })
  assert.equal(toDock.activePtyId, 'd2')
  const back = projectSwitchView(live, 'dock', 'aitools', toDock.activePtyId, toDock.lastByProject)
  assert.equal(back.activePtyId, 'a1')
  assert.equal(back.live.length, 4)
})

test('switching to a project with no live PTY hides the pane without killing others', () => {
  const next = projectSwitchView(live, 'aitools', 'empty', 'a1', {})
  assert.equal(next.activePtyId, '')
  assert.equal(next.live.length, 4)
})

test('pickPtyForProject prefers the remembered id when it is still alive', () => {
  assert.equal(pickPtyForProject(live, 'aitools', 'a2')?.ptyId, 'a2')
  assert.equal(pickPtyForProject(live, 'aitools', 'gone')?.ptyId, 'a1')
  assert.equal(pickPtyForProject(live, 'missing'), null)
})

test('liveOfProject ignores other projects and dead rows', () => {
  const mixed = [...live, pty({ ptyId: 'dead', projectId: 'aitools', title: '已关', alive: false })]
  assert.deepEqual(
    liveOfProject(mixed, 'aitools').map((item) => item.ptyId),
    ['a1', 'a2']
  )
})

test('groupLiveByProject keeps open order and falls back to the id', () => {
  const groups = groupLiveByProject(live, [{ id: 'aitools', name: 'aitools' }])
  assert.equal(groups.length, 2)
  assert.equal(groups[0]?.projectName, 'aitools')
  assert.equal(groups[0]?.items.length, 2)
  assert.equal(groups[1]?.projectName, 'dock')
  assert.equal(groups[1]?.items.length, 2)
})

test('a dead active PTY falls back to the same project, not another project', () => {
  const remaining = live.filter((item) => item.ptyId !== 'a1')
  assert.equal(fallbackPtyAfterExit(remaining, 'a1', 'aitools', 'a1')?.ptyId, 'a2')
  assert.equal(fallbackPtyAfterExit(remaining, 'a1', 'aitools', 'd1')?.ptyId, 'd1')
  const otherProjectOnly = live.filter((item) => item.projectId !== 'aitools')
  assert.equal(fallbackPtyAfterExit(otherProjectOnly, 'a1', 'aitools', 'a1'), null)
})

test('forgetPty clears only the remembered slot', () => {
  const last = forgetPty({ aitools: 'a1', dock: 'd1' }, 'a1')
  assert.equal(last.aitools, undefined)
  assert.equal(last.dock, 'd1')
})

test('isCurrentSession follows the active PTY, not just a live background session', () => {
  const current = live[0]!
  const background = live[1]!
  assert.equal(
    isCurrentSession(current.sessionId!, current.toolId, {
      activePtyId: current.ptyId,
      focused: focusedFromLive(current),
      live: current
    }),
    true
  )
  assert.equal(
    isCurrentSession(background.sessionId!, background.toolId, {
      activePtyId: current.ptyId,
      focused: focusedFromLive(current),
      live: background
    }),
    false
  )
})

test('isCurrentSession falls back to focused row when no window is open', () => {
  assert.equal(
    isCurrentSession('s1', 'grokbuild', {
      activePtyId: '',
      focused: { toolId: 'grokbuild', sessionId: 's1', title: '修圆角' },
      live: null
    }),
    true
  )
  assert.equal(
    isCurrentSession('s2', 'grokbuild', {
      activePtyId: '',
      focused: { toolId: 'grokbuild', sessionId: 's1', title: '修圆角' },
      live: null
    }),
    false
  )
})

test('focusedFromLive uses a pending id until the disk session exists', () => {
  assert.deepEqual(focusedFromLive(live[0]!), {
    toolId: 'grokbuild',
    sessionId: 'a1',
    title: '修圆角'
  })
  assert.deepEqual(
    focusedFromLive(pty({ ptyId: 'n', projectId: 'aitools', title: '新会话', sessionId: null })),
    { toolId: 'grokbuild', sessionId: '__pending:n', title: '新会话' }
  )
})
