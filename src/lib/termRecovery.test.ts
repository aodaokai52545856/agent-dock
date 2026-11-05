import assert from 'node:assert/strict'
import { createTermRecovery, RECOVERY_OUTPUT_CHARS } from './termRecovery.ts'

function fixture() {
  const timers = new Map<number, { cb: () => void; at: number }>()
  let clock = 0
  let serial = 0, repairs = 0, allowed = true
  let repair: () => Promise<void> = async () => {}
  const recovery = createTermRecovery({
    canRecover: () => allowed,
    recover: async () => { repairs++; await repair() },
    scheduleTimeout: (cb, ms) => { timers.set(++serial, { cb, at: clock + ms }); return serial },
    cancelTimeout: id => { timers.delete(id) },
    now: () => clock
  })
  return {
    recovery,
    get repairs() { return repairs },
    get pending() { return timers.size },
    get delay() { return Math.min(...[...timers.values()].map(t => t.at)) - clock },
    allow(value: boolean) { allowed = value },
    repairWith(fn: () => Promise<void>) { repair = fn },
    async settle() {
      const callbacks = [...timers.values()]
      timers.clear()
      callbacks.forEach(({ cb, at }) => { clock = at; cb() })
      for (let i = 0; i < 5; i++) await Promise.resolve()
    }
  }
}

const longPaint = '中'.repeat(RECOVERY_OUTPUT_CHARS)
{
  const f = fixture()
  for (let i = 0; i < 4; i++) f.recovery.noteOutput(longPaint.slice(0, RECOVERY_OUTPUT_CHARS / 4))
  assert.equal(f.repairs, 0)
  assert.equal(f.pending, 1)
  await f.settle()
  assert.equal(f.repairs, 1)
  // ConPTY's full redraw can be much larger than the original answer.
  f.recovery.noteOutput(longPaint.repeat(4))
  await f.settle()
  await f.settle()
  assert.equal(f.repairs, 1)
  f.recovery.noteOutput(longPaint)
  await f.settle()
  assert.equal(f.repairs, 2)
}
{
  const f = fixture()
  f.allow(false)
  f.recovery.noteOutput(longPaint)
  await f.settle()
  assert.equal(f.repairs, 0)
  assert.equal(f.pending, 0)
  f.allow(true)
  f.recovery.request()
  await f.settle()
  assert.equal(f.repairs, 1)
}
{
  const f = fixture()
  f.recovery.noteOutput('small prompt edit')
  await f.settle()
  assert.equal(f.repairs, 0)
  f.recovery.noteOutput(longPaint)
  f.recovery.dispose()
  await f.settle()
  f.recovery.request()
  f.recovery.noteOutput(longPaint)
  assert.equal(f.pending, 0)
  assert.equal(f.repairs, 0)
}
{
  const f = fixture()
  f.repairWith(async () => { throw new Error('PTY closed') })
  f.recovery.noteOutput(longPaint)
  await f.settle()
  await f.settle()
  assert.equal(f.repairs, 1)
  assert.equal(f.pending, 0)
}
{
  const f = fixture()
  f.recovery.noteOutput('short answer')
  f.recovery.request(true)
  for (let i = 0; i < 50; i++) f.recovery.noteOutput('\x1b[?25h')
  await f.settle()
  assert.equal(f.repairs, 1, 'periodic cursor controls must not starve a downward scroll request')
  f.recovery.request(true)
  await f.settle()
  assert.equal(f.repairs, 1, 'reaching the bottom again must not replay an already recovered answer')
  await f.settle()
  for (let i = 0; i < 50; i++) {
    f.recovery.noteOutput('\x1b[?25h\x1b[0m\x1b]0;OMP\x07')
    f.recovery.request(true)
  }
  await f.settle()
  assert.equal(f.repairs, 1, 'cursor, style and title controls must not rearm recovery')
  f.recovery.noteOutput('a new answer')
  f.recovery.request(true)
  await f.settle()
  assert.equal(f.repairs, 2)
}
{
  const f = fixture()
  for (let i = 0; i < 300; i++) f.recovery.request(true)
  await f.settle()
  assert.equal(f.repairs, 0, 'a free-spinning wheel without new output must never resize the PTY')
}
{
  const f = fixture()
  f.allow(false)
  f.recovery.noteOutput('short background answer')
  f.recovery.request(true)
  await f.settle()
  assert.equal(f.repairs, 0)
  f.allow(true)
  f.recovery.request()
  await f.settle()
  assert.equal(f.repairs, 1, 'a deferred explicit recovery must survive a layout transition')
}
console.log('ok long native paints recover once after settling, preserve history reading, and ignore their own replay')
