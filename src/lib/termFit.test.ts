import { canMeasure, createFitScheduler, proposeGrid } from './termFit.ts'
import assert from 'node:assert/strict'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('rejects containers that are not yet laid out', () => {
  assert.equal(canMeasure(0, 400), false)
  assert.equal(canMeasure(800, 0), false)
  assert.equal(canMeasure(16, 16), false)
  assert.equal(canMeasure(80, 40), true)
})

test('proposeGrid floors to whole cells and ignores bad metrics', () => {
  assert.deepEqual(proposeGrid(805, 410, 10, 20), { cols: 80, rows: 20 })
  assert.equal(proposeGrid(0, 400, 10, 20), null)
  assert.equal(proposeGrid(800, 400, 0, 20), null)
})

test('scheduler does not run while layout is busy', () => {
  let runs = 0
  let busy = true
  const frames: Array<() => void> = []
  const s = createFitScheduler({
    isBusy: () => busy,
    run: () => {
      runs += 1
    },
    scheduleFrame: (cb) => {
      frames.push(cb)
      return frames.length
    },
    scheduleTimeout: () => 1,
    cancelTimeout: () => {},
    settleMs: 80
  })
  s.request()
  assert.equal(runs, 0)
  assert.equal(frames.length, 0)
  s.dispose()
})

test('scheduler runs after busy ends and retries once settled', () => {
  let runs = 0
  let busy = true
  const frames: Array<() => void> = []
  const timeouts: Array<() => void> = []
  const s = createFitScheduler({
    isBusy: () => busy,
    run: () => {
      runs += 1
    },
    scheduleFrame: (cb) => {
      frames.push(cb)
      return frames.length
    },
    scheduleTimeout: (cb) => {
      timeouts.push(cb)
      return timeouts.length
    },
    cancelTimeout: () => {},
    settleMs: 80
  })
  s.request()
  busy = false
  s.onBusyChange(false)
  assert.equal(frames.length, 1)
  frames[0]()
  assert.equal(runs, 1)
  assert.equal(timeouts.length, 1)
  timeouts[0]()
  assert.equal(runs, 2)
  s.dispose()
})

test('idle requests coalesce to one frame', () => {
  let runs = 0
  const frames: Array<() => void> = []
  const s = createFitScheduler({
    isBusy: () => false,
    run: () => {
      runs += 1
    },
    scheduleFrame: (cb) => {
      frames.push(cb)
      return 1
    },
    scheduleTimeout: () => 1,
    cancelTimeout: () => {},
    settleMs: 80
  })
  s.request()
  s.request()
  assert.equal(frames.length, 1)
  frames[0]()
  assert.equal(runs, 1)
  s.dispose()
})

test('flush while busy stays queued for the next idle', () => {
  let runs = 0
  let busy = false
  const frames: Array<() => void> = []
  const s = createFitScheduler({
    isBusy: () => busy,
    run: () => {
      runs += 1
    },
    scheduleFrame: (cb) => {
      frames.push(cb)
      return frames.length
    },
    scheduleTimeout: () => 1,
    cancelTimeout: () => {},
    settleMs: 80
  })
  s.request()
  busy = true
  frames[0]()
  assert.equal(runs, 0)
  busy = false
  s.onBusyChange(false)
  frames[1]()
  assert.equal(runs, 1)
  s.dispose()
})
