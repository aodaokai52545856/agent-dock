import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  beginOrExtendBusy,
  doneNoticeCopy,
  doneNoticeTarget,
  PTY_DONE_IDLE_MS,
  PTY_DONE_MIN_BUSY_MS,
  PTY_DONE_STARTUP_MS,
  settleIdle
} from './ptyDone.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('first idle after a short startup burst does not notify', () => {
  let watch = beginOrExtendBusy(undefined, 1000)
  watch = beginOrExtendBusy(watch, 2500)
  const settled = settleIdle(watch, 2500 + PTY_DONE_IDLE_MS)
  assert.equal(settled.notify, false)
  assert.equal(settled.watch.phase, 'idle')
  assert.equal(settled.watch.cycles, 1)
})

test('second busy cycle notifies after a real run', () => {
  let watch = beginOrExtendBusy(undefined, 0)
  watch = settleIdle(watch, PTY_DONE_IDLE_MS).watch
  watch = beginOrExtendBusy(watch, 20_000)
  watch = beginOrExtendBusy(watch, 20_000 + PTY_DONE_MIN_BUSY_MS)
  const settled = settleIdle(watch, 20_000 + PTY_DONE_MIN_BUSY_MS + PTY_DONE_IDLE_MS)
  assert.equal(settled.notify, true)
  assert.equal(settled.watch.cycles, 2)
})

test('a long first cycle still notifies, like a resumed running session', () => {
  let watch = beginOrExtendBusy(undefined, 0)
  watch = beginOrExtendBusy(watch, PTY_DONE_STARTUP_MS)
  const settled = settleIdle(watch, PTY_DONE_STARTUP_MS + PTY_DONE_IDLE_MS)
  assert.equal(settled.notify, true)
})

test('brief flicker after the prompt is ready does not notify', () => {
  let watch = beginOrExtendBusy(undefined, 0)
  watch = settleIdle(watch, PTY_DONE_IDLE_MS).watch
  watch = beginOrExtendBusy(watch, 30_000)
  watch = beginOrExtendBusy(watch, 30_500)
  const settled = settleIdle(watch, 30_500 + PTY_DONE_IDLE_MS)
  assert.equal(settled.notify, false)
})

test('idle is ignored until the quiet window has passed', () => {
  const watch = beginOrExtendBusy(undefined, 0)
  const settled = settleIdle(watch, PTY_DONE_IDLE_MS - 1)
  assert.equal(settled.notify, false)
  assert.equal(settled.watch.phase, 'busy')
})

test('already-idle watches do not notify again', () => {
  let watch = beginOrExtendBusy(undefined, 0)
  watch = settleIdle(watch, PTY_DONE_IDLE_MS).watch
  const again = settleIdle(watch, PTY_DONE_IDLE_MS * 4)
  assert.equal(again.notify, false)
  assert.equal(again.watch.cycles, 1)
})

test('watching the focused window skips the toast', () => {
  assert.equal(doneNoticeTarget({ watching: true, windowFocused: true }), 'skip')
  assert.equal(doneNoticeTarget({ watching: false, windowFocused: true }), 'in-app')
  assert.equal(doneNoticeTarget({ watching: true, windowFocused: false }), 'native')
  assert.equal(doneNoticeTarget({ watching: false, windowFocused: false }), 'native')
})

test('notice copy names the tool and session', () => {
  assert.deepEqual(doneNoticeCopy({ toolLabel: 'Grok', title: '修圆角', projectName: 'aitools' }), {
    title: 'Grok 已完成',
    body: 'aitools · 修圆角'
  })
  assert.equal(doneNoticeCopy({ toolLabel: 'Claude Code', title: '  ', projectName: '' }).title, 'Claude Code 已完成')
})

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

test('project lists show open and running counts', () => {
  for (const file of ['components/SideBar.vue', 'components/BridgeSideBar.vue']) {
    const src = readFileSync(join(root, file), 'utf8')
    assert.match(src, /project-busy/)
    assert.match(src, /个正在运行/)
    assert.match(src, /个已打开窗口/)
  }
})

test('finished windows open a corner notice', () => {
  const src = readFileSync(join(root, 'App.vue'), 'utf8')
  assert.match(src, /done-toast/)
  assert.match(src, /openDoneNotice/)
  assert.match(src, /dismissDoneNotice/)
})
