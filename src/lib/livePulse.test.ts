import assert from 'node:assert/strict'
import {
  isPtyBusy,
  isSignificantPtyChunk,
  liveDotTitle,
  projectLiveCounts,
  projectLiveDot,
  sessionLiveDot
} from './livePulse.ts'
import type { LivePtyInfo } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const row = (over: Partial<LivePtyInfo> & Pick<LivePtyInfo, 'ptyId'>): LivePtyInfo => ({
  key: over.ptyId,
  projectId: 'p1',
  toolId: 'grokbuild',
  sessionId: 's1',
  title: 't',
  alive: true,
  ...over
})

test('tiny cursor redraws do not count as executing', () => {
  assert.equal(isSignificantPtyChunk('\x1b[?25h'), false)
  assert.equal(isSignificantPtyChunk('Worked for 6.5s and more'), true)
})

test('busy expires after the idle window', () => {
  assert.equal(isPtyBusy(1000, 2000), true)
  assert.equal(isPtyBusy(1000, 5000), false)
  assert.equal(isPtyBusy(undefined, 2000), false)
})

test('open session is a solid dot; recent output blinks', () => {
  const live = [row({ ptyId: 'a', sessionId: 's1' })]
  assert.equal(sessionLiveDot(live, 's1', 'grokbuild', 'p1', {}, 10_000), 'open')
  assert.equal(sessionLiveDot(live, 's1', 'grokbuild', 'p1', { a: 9000 }, 10_000), 'busy')
  assert.equal(sessionLiveDot(live, 'other', 'grokbuild', 'p1', { a: 9000 }, 10_000), 'off')
})

test('project blinks if any open session is executing', () => {
  const live = [row({ ptyId: 'a', sessionId: 's1' }), row({ ptyId: 'b', sessionId: 's2', toolId: 'kimi' })]
  assert.equal(projectLiveDot(live, 'p1', {}, 10_000), 'open')
  assert.equal(projectLiveDot(live, 'p1', { b: 9500 }, 10_000), 'busy')
  assert.equal(projectLiveDot(live, 'other', { b: 9500 }, 10_000), 'off')
})

test('dot titles distinguish open vs executing', () => {
  assert.equal(liveDotTitle('open', 'session'), '已打开')
  assert.equal(liveDotTitle('busy', 'session'), '正在执行')
  assert.equal(liveDotTitle('open', 'project'), '有打开的会话')
  assert.equal(liveDotTitle('busy', 'project'), '有会话正在执行')
})

test('project counts split open windows from running windows', () => {
  const live = [row({ ptyId: 'a', sessionId: 's1' }), row({ ptyId: 'b', sessionId: 's2', toolId: 'kimi' })]
  assert.deepEqual(projectLiveCounts(live, 'p1', {}, 10_000), { open: 2, busy: 0 })
  assert.deepEqual(projectLiveCounts(live, 'p1', { b: 9500 }, 10_000), { open: 2, busy: 1 })
  assert.deepEqual(projectLiveCounts(live, 'other', { b: 9500 }, 10_000), { open: 0, busy: 0 })
})
