import assert from 'node:assert/strict'
import { createTermRecovery, RECOVERY_OUTPUT_CHARS } from './termRecovery.ts'

function fixture() {
  const timers = new Map<number, () => void>()
  let serial = 0, repairs = 0, allowed = true
  let repair: () => Promise<void> = async () => {}
  const recovery = createTermRecovery({
    canRecover: () => allowed,
    recover: async () => { repairs++; await repair() },
    scheduleTimeout: cb => { timers.set(++serial, cb); return serial },
    cancelTimeout: id => { timers.delete(id) }
  })
  return {
    recovery,
    get repairs() { return repairs },
    get pending() { return timers.size },
    allow(value: boolean) { allowed = value },
    repairWith(fn: () => Promise<void>) { repair = fn },
    async settle() {
      const callbacks = [...timers.values()]
      timers.clear()
      callbacks.forEach(cb => cb())
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
console.log('ok long native paints recover once after settling, preserve history reading, and ignore their own replay')
