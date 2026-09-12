import assert from 'node:assert/strict'
import {
  SYNC_OUTPUT_MODE,
  attachSynchronizedOutput,
  csiParamsInclude,
  createRefreshGate,
  decrqmReply,
  flattenCsiParams,
  type CsiParams
} from './termSync.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('flattens CSI params including subparams', () => {
  assert.deepEqual(flattenCsiParams([2026, [1049, 25]]), [2026, 1049, 25])
  assert.equal(csiParamsInclude([1049, 2026], SYNC_OUTPUT_MODE), true)
  assert.equal(csiParamsInclude([[2026]], SYNC_OUTPUT_MODE), true)
  assert.equal(csiParamsInclude([1049], SYNC_OUTPUT_MODE), false)
})

test('DECRQM reply reports synchronized output as supported', () => {
  assert.equal(decrqmReply(2026), '\x1b[?2026;2$y')
})

test('refresh gate coalesces to one frame', () => {
  let runs = 0
  const frames: Array<() => void> = []
  const gate = createRefreshGate({
    refresh: () => {
      runs += 1
    },
    scheduleFrame: (cb) => {
      frames.push(cb)
      return frames.length
    },
    cancelFrame: () => {
      frames.length = 0
    }
  })
  gate.request()
  gate.request()
  assert.equal(runs, 0)
  assert.equal(frames.length, 1)
  frames[0]()
  assert.equal(runs, 1)
  gate.dispose()
})

test('CSI ?2026l refreshes and DECRQM 2026 answers the app', () => {
  const handlers = new Map<string, (params: CsiParams) => boolean>()
  const sent: string[] = []
  let refresh = 0
  const term = {
    parser: {
      registerCsiHandler(
        id: { prefix?: string; intermediates?: string; final: string },
        callback: (params: CsiParams) => boolean
      ) {
        handlers.set(`${id.prefix ?? ''}|${id.intermediates ?? ''}|${id.final}`, callback)
        return { dispose() {} }
      }
    }
  }
  const attached = attachSynchronizedOutput(term, {
    send: (data) => sent.push(data),
    refresh: () => {
      refresh += 1
    }
  })
  const reset = handlers.get('?||l')
  const query = handlers.get('?|$|p')
  assert.equal(reset?.([1049]), false)
  assert.equal(refresh, 0)
  assert.equal(reset?.([2026]), false)
  assert.equal(refresh, 1)
  assert.equal(query?.([25]), false)
  assert.equal(query?.([2026]), true)
  assert.deepEqual(sent, ['\x1b[?2026;2$y'])
  attached.dispose()
})
