import assert from 'node:assert/strict'
import type { LivePtyInfo } from '../types.ts'
import { channelSupportsFresh, defaultCodexChannel, defaultPtyChannel, nodeSession, validateFlowPtyBindings, validateSessions } from './channels.ts'
import { normalizeFlow } from './model.ts'
import { resetOmpSession, resetWindow, startsFresh, type WindowResetIo } from './session.ts'
import type { ChannelBinding, FlowDef, FlowNode, FlowRun, RunStep } from './types.ts'

const tests: [string, () => void | Promise<void>][] = []
const test = (name: string, fn: () => void | Promise<void>) => tests.push([name, fn])

const claude = (): ChannelBinding => defaultPtyChannel('claude')
const output = [{ path: 'task:a.md' }]

function node(partial: Partial<FlowNode> = {}): FlowNode {
  return { id: 'dev', title: '开发', role: 'developer', roleContract: '', channel: claude(), outputs: output, ...partial }
}

function run(partial: Partial<FlowRun> = {}) {
  return { steps: [], ...partial } as unknown as FlowRun
}

function live(partial: Partial<LivePtyInfo> = {}): LivePtyInfo {
  return { ptyId: 'p1', key: 'k', projectId: 'proj', toolId: 'claude', sessionId: 'old', title: '开发', alive: true, ...partial } as LivePtyInfo
}

function windowIo(partial: Partial<WindowResetIo> & { rows?: () => { id: string; account?: { id: string } }[] } = {}) {
  const writes: string[] = []
  let clock = 1_000_000
  const io: WindowResetIo = {
    now: () => clock,
    sleep: async (ms) => {
      clock += ms
    },
    isCurrent: () => true,
    lastOutputAt: () => undefined,
    listSessions: async () => (partial.rows ? partial.rows() : [{ id: 'old' }, ...(writes.length ? [{ id: 'new' }] : [])]) as never,
    write: async (data) => {
      writes.push(data)
    },
    ...partial
  }
  return { io, writes }
}

test('nodes that hand over through files start clean unless told otherwise', () => {
  assert.equal(nodeSession(node()), 'fresh')
  assert.equal(nodeSession(node({ session: 'reuse' })), 'reuse')
})

test('relay nodes keep their conversation', () => {
  assert.equal(nodeSession(node({ outputs: undefined })), 'reuse')
})

test('channels that cannot clear fall back to reuse by default', () => {
  assert.equal(nodeSession(node({ channel: defaultPtyChannel('kimi') })), 'reuse')
  assert.equal(nodeSession(node({ channel: defaultCodexChannel() })), 'reuse')
})

test('only the three main agents can clear', () => {
  assert.equal(channelSupportsFresh(defaultPtyChannel('claude')), true)
  assert.equal(channelSupportsFresh(defaultPtyChannel('grokbuild')), true)
  assert.equal(channelSupportsFresh({ kind: 'ompRpc', toolId: 'omp', sessionId: '', ptyId: '' }), true)
  assert.equal(channelSupportsFresh(defaultPtyChannel('kimi')), false)
  assert.equal(channelSupportsFresh({ kind: 'human' }), false)
})

test('asking for fresh on a channel that cannot clear stops the run', () => {
  const flow = { nodes: [node({ session: 'fresh', channel: defaultPtyChannel('kimi') })] } as FlowDef
  assert.match(validateSessions(flow) || '', /不能清空上下文/)
  assert.equal(validateSessions({ nodes: [node({ session: 'fresh' })] } as FlowDef), null)
})

test('a fresh node may start in a window that has no session yet', () => {
  const channel = { ...defaultPtyChannel('claude'), ptyId: 'p1' }
  const flow = (n: Partial<FlowNode>) => ({ nodes: [node({ channel, ...n })], edges: [] }) as unknown as FlowDef
  assert.equal(validateFlowPtyBindings(flow({}), [live({ sessionId: '' })], 'proj'), null)
  assert.match(validateFlowPtyBindings(flow({ session: 'reuse' }), [live({ sessionId: '' })], 'proj') || '', /至少一轮对话/)
})

test('flow files keep a valid session and drop anything else', () => {
  const flow = (session: unknown) => normalizeFlow({ id: 'f', name: 'f', nodes: [{ id: 'a', role: 'developer', channel: claude(), session }], edges: [] }) as FlowDef
  assert.equal(flow('reuse').nodes[0].session, 'reuse')
  assert.equal(flow('sometimes').nodes[0].session, undefined)
})

test('a node is cleared on its first step of a task', () => {
  assert.equal(startsFresh(node(), run()), true)
})

test('a rework in the same task keeps the conversation', () => {
  assert.equal(startsFresh(node(), run({ freshNodes: ['dev'] })), false)
})

test('a retry after a failed reset still clears', () => {
  const failed = { nodeId: 'dev', status: 'failed' } as RunStep
  assert.equal(startsFresh(node(), run({ steps: [failed], priorAttempts: { dev: 2 } })), true)
})

test('a lesson turn never clears, it has to remember the work', () => {
  assert.equal(startsFresh(node(), run(), { nodeId: 'dev', purpose: 'lesson' } as RunStep), false)
})

test('a reuse node is never cleared', () => {
  assert.equal(startsFresh(node({ session: 'reuse' }), run()), false)
})

test('the window command names the tool and reports the session it started', async () => {
  const { io, writes } = windowIo()
  assert.deepEqual(await resetWindow(live(), io), { ok: true, sessionId: 'new' })
  assert.deepEqual(writes, ['/clear\r'])
})

test('Grok uses its own command', async () => {
  const { io, writes } = windowIo()
  await resetWindow(live({ toolId: 'grokbuild' }), io)
  assert.deepEqual(writes, ['/new\r'])
})

test('a window that stays silent is not sent the prompt', async () => {
  const { io, writes } = windowIo({ rows: () => [{ id: 'old' }] })
  const reset = await resetWindow(live(), io)
  assert.equal(reset.ok, false)
  assert.match(!reset.ok ? reset.error : '', /提示词未发送/)
  assert.equal(writes.length, 1)
})

test('a busy window is left alone', async () => {
  let at = 0
  const { io, writes } = windowIo({ lastOutputAt: () => at })
  const now = io.now
  io.now = () => {
    at = now()
    return at
  }
  const reset = await resetWindow(live(), io)
  assert.match(!reset.ok ? reset.error : '', /还在工作/)
  assert.deepEqual(writes, [])
})

test('a window that goes quiet in time is cleared', async () => {
  const { io, writes } = windowIo()
  const start = io.now()
  io.lastOutputAt = () => start + 1000
  assert.equal((await resetWindow(live(), io)).ok, true)
  assert.deepEqual(writes, ['/clear\r'])
})

test('two new sessions are ambiguous, so neither is picked', async () => {
  let calls = 0
  const { io } = windowIo({ rows: () => (++calls > 1 ? [{ id: 'a' }, { id: 'b' }] : []) })
  const reset = await resetWindow(live(), io)
  assert.match(!reset.ok ? reset.error : '', /分不清/)
})

test('a new session from another account is not this window', async () => {
  let calls = 0
  const { io } = windowIo({ rows: () => (++calls > 1 ? [{ id: 'theirs', account: { id: 'b' } }, { id: 'mine', account: { id: 'a' } }] : []) })
  const reset = await resetWindow(live({ account: { id: 'a' } as LivePtyInfo['account'] }), io)
  assert.deepEqual(reset, { ok: true, sessionId: 'mine' })
})

test('a failing session list is retried, a failing write is reported', async () => {
  let calls = 0
  const flaky = windowIo({
    listSessions: async () => {
      calls += 1
      if (calls === 2) throw new Error('busy')
      return calls > 2 ? [{ id: 'new' }] : []
    }
  })
  assert.equal((await resetWindow(live(), flaky.io)).ok, true)
  const broken = windowIo({
    write: async () => {
      throw new Error('closed')
    }
  })
  assert.match((await resetWindow(live(), broken.io) as { error: string }).error, /closed/)
})

test('cancelling stops the wait', async () => {
  let current = true
  const { io } = windowIo({ rows: () => [] })
  io.isCurrent = () => current
  const sleep = io.sleep
  io.sleep = async (ms) => {
    current = false
    await sleep(ms)
  }
  assert.deepEqual(await resetWindow(live(), io), { ok: false, error: '已取消' })
})

test('a tool without a command is refused', async () => {
  const { io, writes } = windowIo()
  assert.equal((await resetWindow(live({ toolId: 'kimi' }), io)).ok, false)
  assert.deepEqual(writes, [])
})

const omp = (rows: { settled?: boolean; messages?: unknown[]; sessionId?: string | null }[], data: { cancelled?: boolean } = { cancelled: false }) => {
  let calls = 0
  let created = 0
  return {
    io: {
      snapshot: async () => {
        const row = rows[Math.min(calls++, rows.length - 1)]
        return { settled: row.settled ?? true, messages: row.messages ?? [], info: { sessionId: row.sessionId } }
      },
      newSession: async () => {
        created += 1
        return { data }
      }
    },
    created: () => created
  }
}

test('OMP reports the new session once it has no history', async () => {
  const { io } = omp([{ sessionId: 'old', messages: [1] }, { sessionId: 'new' }])
  assert.deepEqual(await resetOmpSession(io), { ok: true, sessionId: 'new' })
})

test('a working OMP session is not aborted', async () => {
  const { io, created } = omp([{ settled: false, sessionId: 'old' }])
  assert.equal((await resetOmpSession(io)).ok, false)
  assert.equal(created(), 0)
})

test('OMP fails closed when it stays on the old session or keeps history', async () => {
  assert.equal((await resetOmpSession(omp([{ sessionId: 'old' }, { sessionId: 'old' }]).io)).ok, false)
  assert.equal((await resetOmpSession(omp([{ sessionId: 'old' }, { sessionId: 'new', messages: [1] }]).io)).ok, false)
  assert.equal((await resetOmpSession(omp([{ sessionId: 'old' }, { sessionId: 'new' }], { cancelled: true }).io)).ok, false)
})

for (const [name, fn] of tests) {
  await fn()
  console.log(`ok ${name}`)
}
