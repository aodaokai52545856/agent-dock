import assert from 'node:assert/strict'
import test from 'node:test'
import { completedNodes, finishStep, prepareStep, runRecord, type DockFs, type PreparedStep } from './taskRun.ts'
import type { AdapterOutcome, FlowNode, FlowRun, RunStep } from './types.ts'

function memoryFs(seed: Record<string, string> = {}) {
  const files = new Map(Object.entries(seed))
  const writes: string[] = []
  const fs: DockFs = {
    async read(rel) {
      const text = files.get(rel)
      return text === undefined ? null : { text, sha256: `sha:${text}` }
    },
    async write(rel, text) {
      files.set(rel, text)
      writes.push(rel)
      return `sha:${text}`
    }
  }
  return { fs, files, writes }
}

const pty: FlowNode['channel'] = { kind: 'pty', toolId: 'claude', sessionId: 's', ptyId: 'p' }
const codex: FlowNode['channel'] = { kind: 'codexApp', threadIds: [], targetKind: 'uncommittedChanges', commitSha: '', baseBranch: 'main', customInstructions: '' }
const TASK = 'tasks/t1'
const ok: AdapterOutcome = { ok: true, text: '完成' }

function node(patch: Partial<FlowNode>): FlowNode {
  return { id: 'dev', title: '开发', role: 'developer', roleContract: '', channel: pty, ...patch }
}

function step(nodeId: string, status: RunStep['status'] = 'working'): RunStep {
  return { id: `s-${nodeId}`, nodeId, status, startedAt: 1 }
}

function run(steps: RunStep[] = [step('dev')], extra: Partial<FlowRun> = {}): FlowRun {
  return {
    id: 'r1', projectId: 'p', flowId: 'f', status: 'running', currentNodeId: 'dev', task: '任务', steps,
    lastEnvelope: null, pendingHandoff: null, retryCount: 0, loopCounts: {}, stepCount: 1, lastError: '',
    cursorAgentId: '', git: null, createdAt: 1, taskDir: 't1', ...extra
  }
}

async function prepared(fs: DockFs, target: FlowNode, current = run()) {
  const result = await prepareStep(fs, target, current)
  assert.ok(!('error' in result), JSON.stringify(result))
  return result as PreparedStep
}

test('required inputs must exist before the node runs', async () => {
  const { fs } = memoryFs()
  const result = await prepareStep(fs, node({ inputs: [{ ref: 'task:design.md', required: true }] }), run())
  assert.deepEqual(result, { error: '缺少必读输入：task:design.md' })
})

test('inputs are listed by path and recorded with their hash', async () => {
  const { fs } = memoryFs({ [`${TASK}/design.md`]: '设计' })
  const result = await prepared(fs, node({ inputs: [{ ref: 'task:design.md', required: true }, { ref: 'project:memory/index.md' }] }))
  assert.match(result.block, /\.agentDock\/tasks\/t1\/design\.md/)
  assert.doesNotMatch(result.block, /设计/)
  assert.doesNotMatch(result.block, /index\.md/)
  assert.deepEqual(result.inputs, [
    { ref: 'task:design.md', sha256: 'sha:设计' },
    { ref: 'project:memory/index.md', sha256: null }
  ])
})

test('channels that cannot read files get the content inline', async () => {
  const { fs } = memoryFs({ [`${TASK}/design.md`]: '设计正文' })
  const result = await prepared(fs, node({ channel: { kind: 'cursorSdk', agentId: '' }, inputs: [{ ref: 'task:design.md' }] }))
  assert.match(result.block, /设计正文/)
})

test('rework-only inputs appear from the second attempt', async () => {
  const { fs } = memoryFs({ [`${TASK}/review.md`]: '意见' })
  const target = node({ inputs: [{ ref: 'task:review.md', when: 'reworked', required: true }] })
  assert.deepEqual((await prepared(fs, target)).inputs, [])
  const second = await prepared(fs, target, run([step('dev', 'completed'), step('cr', 'completed'), step('dev')]))
  assert.deepEqual(second.inputs, [{ ref: 'task:review.md', sha256: 'sha:意见' }])
})

test('a retry after a failed first try is not a rework', async () => {
  const { fs } = memoryFs()
  const target = node({ inputs: [{ ref: 'task:review.md', when: 'reworked', required: true }] })
  const retried = run([step('dev')], { priorAttempts: { dev: 1 } })
  assert.deepEqual((await prepared(fs, target, retried)).inputs, [])
})

test('a rerun of a node that finished earlier is still a rework', async () => {
  const { fs } = memoryFs()
  const target = node({ inputs: [{ ref: 'task:review.md', when: 'reworked', required: true }] })
  const rerun = run([step('dev')], { priorCompleted: ['dev'] })
  assert.deepEqual(await prepareStep(fs, target, rerun), { error: '缺少必读输入：task:review.md' })
  assert.deepEqual(completedNodes(run([step('dev', 'completed'), step('cr', 'failed')], { priorCompleted: ['sa'] })), ['sa', 'dev'])
})

test('a template seeds the output and an untouched copy fails', async () => {
  const { fs, files } = memoryFs({ 'roles/developer/log.md': '## 结论\n{{结论}}\n' })
  const target = node({ outputs: [{ path: 'task:log.md', template: 'log.md' }] })
  const ready = await prepared(fs, target)
  assert.equal(files.get(`${TASK}/log.md`), '## 结论\n{{结论}}\n')
  assert.match(ready.block, /按模板 \.agentDock\/roles\/developer\/log\.md/)
  const { outcome } = await finishStep(fs, target, run(), ready, ok, false)
  assert.match(outcome.error || '', /没有更新产物/)
})

test('a template placeholder left in the output fails, whatever else the text says', async () => {
  const { fs, files } = memoryFs({ 'roles/developer/log.md': '## 结论\n{{结论}}\n## 证据\n{{证据}}\n' })
  const target = node({ outputs: [{ path: 'task:log.md', template: 'log.md' }] })
  const ready = await prepared(fs, target)
  files.set(`${TASK}/log.md`, '## 结论\nPASS\n## 证据\n{{证据}}\n')
  const { outcome } = await finishStep(fs, target, run(), ready, ok, false)
  assert.match(outcome.error || '', /占位符：\{\{证据\}\}/)
})

test('a report that mentions placeholders is not a leftover', async () => {
  const { fs, files } = memoryFs({ 'roles/developer/log.md': '## 结论\n{{结论}}\n' })
  const target = node({ outputs: [{ path: 'task:log.md', template: 'log.md' }] })
  const ready = await prepared(fs, target)
  files.set(`${TASK}/log.md`, '## 结论\nPASS，全文不含 `{{}}` 这类写法\n')
  assert.equal((await finishStep(fs, target, run(), ready, ok, false)).outcome.ok, true)
})

test('an output without a template is never checked for placeholders', async () => {
  const { fs, files } = memoryFs()
  const target = node({ outputs: [{ path: 'task:log.md' }] })
  const ready = await prepared(fs, target)
  files.set(`${TASK}/log.md`, '核查：dev-log 不含 `{{}}`、TODO 等占位符\n')
  assert.equal((await finishStep(fs, target, run(), ready, ok, false)).outcome.ok, true)
  assert.doesNotMatch(ready.block, /占位符/)
})

test('a missing template stops the node', async () => {
  const result = await prepareStep(memoryFs().fs, node({ outputs: [{ path: 'task:log.md', template: 'log.md' }] }), run())
  assert.deepEqual(result, { error: '找不到模板：roles/developer/log.md' })
})

test('file outputs are validated after the node ran', async () => {
  const { fs, files } = memoryFs()
  const target = node({ outputs: [{ path: 'task:log.md', sections: ['## 结论'] }] })
  const ready = await prepared(fs, target)

  const missing = await finishStep(fs, target, run(), ready, ok, false)
  assert.match(missing.outcome.error || '', /没有写出产物/)

  files.set(`${TASK}/log.md`, '只写了正文')
  const incomplete = await finishStep(fs, target, run(), ready, ok, false)
  assert.match(incomplete.outcome.error || '', /缺少段落/)
  assert.equal(incomplete.outcome.ok, false)

  files.set(`${TASK}/log.md`, '## 结论\n完成\n')
  const done = await finishStep(fs, target, run(), ready, ok, false)
  assert.equal(done.outcome.ok, true)
  assert.deepEqual(done.outputs, [{ ref: 'task:log.md', sha256: 'sha:## 结论\n完成\n' }])
})

test('a stale file from an earlier attempt is not accepted', async () => {
  const { fs } = memoryFs({ [`${TASK}/log.md`]: '## 结论\n完成\n' })
  const target = node({ outputs: [{ path: 'task:log.md' }] })
  const ready = await prepared(fs, target)
  const { outcome } = await finishStep(fs, target, run(), ready, ok, false)
  assert.match(outcome.error || '', /没有更新产物/)
})

test('read-only channels get their reply captured', async () => {
  const { fs, files } = memoryFs()
  const target = node({ channel: codex, outputs: [{ path: 'task:review.md', sections: ['## 结论'] }] })
  const ready = await prepared(fs, target)
  assert.match(ready.block, /在回复里给出完整产物/)
  const reply: AdapterOutcome = { ok: true, text: '## 结论\nVERDICT: PASS\n' }
  const { outcome, outputs } = await finishStep(fs, target, run(), ready, reply, true)
  assert.equal(outcome.ok, true)
  assert.equal(outcome.verdict, 'pass')
  assert.equal(files.get(`${TASK}/review.md`), reply.text)
  assert.equal(outputs.length, 1)
})

test('a captured reply is kept even when it fails validation', async () => {
  const { fs, files } = memoryFs()
  const target = node({ channel: codex, outputs: [{ path: 'task:review.md', sections: ['## 结论'] }] })
  const { outcome } = await finishStep(fs, target, run(), await prepared(fs, target), { ok: true, text: '没有结论' }, true)
  assert.equal(outcome.ok, false)
  assert.equal(files.get(`${TASK}/review.md`), '没有结论')
})

test('the document conclusion outranks the reply on a gated node', async () => {
  const { fs, files } = memoryFs()
  const target = node({ outputs: [{ path: 'task:log.md' }] })
  const ready = await prepared(fs, target)
  files.set(`${TASK}/log.md`, '## 结论\n未通过\n')
  const { outcome } = await finishStep(fs, target, run(), ready, { ok: true, text: 'done', verdict: 'pass' }, true)
  assert.equal(outcome.verdict, 'fail')
})

test('an ungated node takes no verdict from its document', async () => {
  const { fs, files } = memoryFs()
  const target = node({ outputs: [{ path: 'task:log.md' }] })
  const ready = await prepared(fs, target)
  files.set(`${TASK}/log.md`, '## 结论\n未通过\n')
  const { outcome } = await finishStep(fs, target, run(), ready, ok, false)
  assert.equal(outcome.verdict, undefined)
})

test('a failed node and an unconfirmed send skip or refuse the checks', async () => {
  const { fs, writes } = memoryFs()
  const target = node({ channel: codex, outputs: [{ path: 'task:review.md' }] })
  const ready = await prepared(fs, target)
  const failed: AdapterOutcome = { ok: false, text: '', error: '通道断开' }
  assert.equal((await finishStep(fs, target, run(), ready, failed, false)).outcome.error, '通道断开')
  assert.deepEqual(writes, [])
  const unconfirmed = await finishStep(fs, target, run(), ready, { ok: true, text: '已写入并发送', unconfirmed: true }, false)
  assert.match(unconfirmed.outcome.error || '', /没有确认完成/)
  assert.deepEqual(writes, [])
})

test('file errors become a node failure', async () => {
  const fs: DockFs = {
    async read() { throw new Error('磁盘坏了') },
    async write() { throw new Error('磁盘坏了') }
  }
  assert.deepEqual(await prepareStep(fs, node({ inputs: [{ ref: 'task:a.md' }] }), run()), { error: '磁盘坏了' })
  const target = node({ channel: codex, outputs: [{ path: 'task:a.md' }] })
  const { outcome } = await finishStep(fs, target, run(), { block: '', inputs: [], before: {}, placeholders: {} }, ok, false)
  assert.equal(outcome.error, '磁盘坏了')
})

test('the run record numbers attempts per node and keeps the trace', () => {
  const first = { ...step('dev', 'completed'), inputs: [{ ref: 'task:a.md', sha256: 'x' }] }
  const review = {
    ...step('cr', 'completed'),
    envelope: { id: 'e', runId: 'r1', causationId: '', fromNode: 'cr', toNode: 'cr', fromRole: 'reviewer', toRole: 'reviewer', task: '', done: '', remaining: '', blockers: [], artifacts: [], verdict: 'fail' as const }
  }
  const record = runRecord(run([first, review, step('dev')]))
  assert.deepEqual(record.steps.map((row) => [row.nodeId, row.attempt, row.verdict]), [
    ['dev', 1, undefined],
    ['cr', 1, 'fail'],
    ['dev', 2, undefined]
  ])
  assert.deepEqual(record.steps[0].inputs, [{ ref: 'task:a.md', sha256: 'x' }])
  assert.equal(JSON.stringify(record).includes('envelope'), false)
})
