import assert from 'node:assert/strict'
import test from 'node:test'
import { afterOutcome, awaitLessons, cancelRun, finishLessons, isAwaitingLessons, startLessonTurn, startRun } from './engine.ts'
import { collectLessons, harvestLessons, lessonTurnAuthor, noteLessonRequest, planStep, validateExperience, type ExperienceFs } from './experience.ts'
import { flowFileText, parseFlowFile } from './flowFiles.ts'
import { attemptCounts, attemptOf, finishStep, prepareStep, runRecord, type PreparedStep } from './taskRun.ts'
import { FLOW_END, FLOW_START, type FlowDef, type FlowExperience, type FlowNode, type FlowRun, type LessonDraft, type RunStep } from './types.ts'

const LESSON = '## 可复用经验\n### 一条经验\n范围：flow\n#### 症状\n有\n#### 防复发措施\n改 `src/a.ts` 和 `src/gone.ts`\n'
const SKIP = '## 可复用经验\nN/A：一次性问题\n'
const DRAFTS = 'tasks/t1/experience'

function memoryFs(seed: Record<string, string> = {}) {
  const files = new Map(Object.entries(seed))
  const fs: ExperienceFs = {
    async read(rel) {
      const text = files.get(rel)
      return text === undefined ? null : { text, sha256: `sha:${text}` }
    },
    async write(rel, text) {
      files.set(rel, text)
      return `sha:${text}`
    },
    async list(dir) {
      return [...files.keys()].filter((key) => key.startsWith(`${dir}/`)).map((key) => key.slice(dir.length + 1)).sort()
    },
    async missing(paths) {
      return paths.filter((path) => path.includes('gone'))
    }
  }
  return { fs, files }
}

const pty: FlowNode['channel'] = { kind: 'pty', toolId: 'claude', sessionId: 's', ptyId: 'p' }
const codex: FlowNode['channel'] = { kind: 'codexApp', threadIds: [], targetKind: 'uncommittedChanges', commitSha: '', baseBranch: 'main', customInstructions: '' }

function flow(experience?: FlowExperience): FlowDef {
  return {
    id: 'f',
    name: 'f',
    nodes: [
      { id: 'dev', title: '开发', role: 'developer', roleContract: '写代码', channel: pty },
      { id: 'cr', title: '审查', role: 'reviewer', roleContract: '审代码', channel: codex },
      { id: 'pm', title: '人工', role: 'pm', roleContract: '确认', channel: { kind: 'human' } }
    ],
    edges: [
      { id: 'e0', from: FLOW_START, to: 'dev', mode: 'auto', transform: 'roleWrap', gate: 'none' },
      { id: 'e1', from: 'dev', to: 'cr', mode: 'auto', transform: 'roleWrap', gate: 'none' },
      { id: 'e2', from: 'cr', to: FLOW_END, mode: 'auto', transform: 'roleWrap', gate: 'passFail', backTo: 'dev' }
    ],
    experience,
    maxRetries: 2,
    maxSteps: 12,
    slice: 1,
    draftTask: ''
  }
}

function step(nodeId: string, patch: Partial<RunStep> = {}): RunStep {
  return { id: `s-${nodeId}`, nodeId, status: 'completed', startedAt: 1, ...patch }
}

function run(steps: RunStep[], patch: Partial<FlowRun> = {}): FlowRun {
  return {
    id: 'r1', projectId: 'p', flowId: 'f', status: 'running', currentNodeId: 'dev', task: '修闸门', steps,
    lastEnvelope: null, pendingHandoff: null, retryCount: 0, loopCounts: {}, stepCount: steps.length, lastError: '',
    cursorAgentId: '', git: null, createdAt: 1, taskDir: 't1', ...patch
  }
}

const dev = flow().nodes[0]
const reviewer = flow().nodes[1]
const model: FlowExperience = { policy: 'model', authors: ['dev'], judge: 'cr' }

function draftOf(plan: { node: FlowNode }) {
  return plan.node.outputs?.find((output) => output.lesson)
}

test('flows without experience are left alone', () => {
  const plan = planStep(flow(), dev, run([step('dev')]))
  assert.equal(plan.node, dev)
  assert.equal(plan.note, '')
})

test('attempts policy asks from the configured attempt on', () => {
  const config: FlowExperience = { policy: 'attempts', minAttempts: 2, authors: ['dev'] }
  const first = planStep(flow(config), dev, run([step('dev')]))
  assert.equal(draftOf(first), undefined)
  assert.equal(first.note, '')
  const second = planStep(flow(config), dev, run([step('dev'), step('dev')]))
  assert.equal(draftOf(second)?.path, 'task:experience/dev-2.md')
  assert.equal(draftOf(second)?.lesson, 'entries')
  assert.match(second.note, /#### 防复发措施/)
  assert.doesNotMatch(second.note, /N\/A/)
})

test('always policy asks on the first attempt', () => {
  const plan = planStep(flow({ policy: 'always', authors: ['dev'] }), dev, run([step('dev')]))
  assert.equal(draftOf(plan)?.path, 'task:experience/dev-1.md')
})

test('model policy accepts a reasoned skip and states the criteria', () => {
  const plan = planStep(flow(model), dev, run([step('dev')]))
  assert.equal(draftOf(plan)?.lesson, 'entries-or-na')
  assert.match(plan.note, /N\/A：理由/)
  assert.match(plan.note, /一次性细节不写/)
})

test('authors read the memory index on every attempt, without requiring it', () => {
  const config: FlowExperience = { policy: 'attempts', authors: ['dev'] }
  const plan = planStep(flow(config), dev, run([step('dev')]))
  assert.deepEqual(plan.node.inputs, [{ ref: 'project:memory/index.md', read: 'index-first' }])
  const declared = { ...dev, inputs: [{ ref: 'project:memory/index.md', required: true }] }
  assert.deepEqual(planStep(flow(config), declared, run([step('dev')])).node.inputs, declared.inputs)
})

test('the judge sees the author draft and how to ask for a lesson', () => {
  const steps = [step('dev', { outputs: [{ ref: 'task:experience/dev-1.md', sha256: 'x' }] }), step('cr', { status: 'working' })]
  const plan = planStep(flow(model), reviewer, run(steps))
  assert.deepEqual(plan.node.inputs, [{ ref: 'task:experience/dev-1.md' }])
  assert.match(plan.note, /LESSON: NEEDED/)
})

test('only the model policy involves a judge', () => {
  const config: FlowExperience = { policy: 'always', authors: ['dev'], judge: 'cr' }
  assert.equal(planStep(flow(config), reviewer, run([step('dev')])).note, '')
})

test('a lesson turn only owes the lesson', () => {
  const declared = { ...dev, inputs: [{ ref: 'task:design.md', required: true }], outputs: [{ path: 'task:dev-log.md' }] }
  const steps = [step('dev'), step('cr'), step('dev', { purpose: 'lesson', status: 'working' })]
  const plan = planStep(flow(model), declared, run(steps), steps[2])
  assert.deepEqual(plan.node.inputs, [])
  assert.equal(plan.node.outputs?.length, 1)
  assert.equal(plan.node.outputs?.[0].path, 'task:experience/dev-1.md')
  assert.equal(plan.node.outputs?.[0].lesson, 'entries')
  assert.match(plan.note, /不改代码/)
})

test('lesson turns do not count as attempts, earlier runs of the task do', () => {
  const current = run([step('dev'), step('dev', { purpose: 'lesson' }), step('cr')], { priorAttempts: { dev: 2 } })
  assert.equal(attemptOf(current, 'dev'), 3)
  assert.deepEqual(attemptCounts(current), { dev: 3, cr: 1 })
})

test('the run record keeps attempt numbers across reruns and marks lesson turns', () => {
  const record = runRecord(run([step('dev'), step('dev', { purpose: 'lesson' })], { priorAttempts: { dev: 2 } }))
  assert.deepEqual(record.steps.map((row) => [row.attempt, row.purpose]), [[3, undefined], [3, 'lesson']])
})

test('a draft without entries fails the step unless the rule allows a skip', async () => {
  const { fs, files } = memoryFs()
  const steps = [step('dev', { status: 'working' })]
  const finish = async (config: FlowExperience, text: string) => {
    const plan = planStep(flow(config), dev, run(steps))
    const prepared = await prepareStep(fs, plan.node, run(steps)) as PreparedStep
    files.set(`${DRAFTS}/dev-1.md`, text)
    return (await finishStep(fs, plan.node, run(steps), prepared, { ok: true, text: '完成' }, false)).outcome
  }
  assert.equal((await finish(model, SKIP)).ok, true)
  files.clear()
  const refused = await finish({ policy: 'always', authors: ['dev'] }, SKIP)
  assert.equal(refused.ok, false)
  assert.match(refused.error || '', /写成条目/)
  files.clear()
  assert.equal((await finish({ policy: 'always', authors: ['dev'] }, LESSON)).ok, true)
})

test('the judge request is remembered only from the judge', () => {
  const current = run([step('cr')])
  noteLessonRequest(flow(model), dev, current, { ok: true, text: 'LESSON: NEEDED 自荐' })
  assert.equal(current.lessonWanted, undefined)
  noteLessonRequest(flow({ ...model, policy: 'always' }), reviewer, current, { ok: true, text: 'LESSON: NEEDED 不算' })
  assert.equal(current.lessonWanted, undefined)
  noteLessonRequest(flow(model), reviewer, current, { ok: true, text: 'LESSON: NEEDED 会再犯\nVERDICT: FAIL' })
  assert.equal(current.lessonWanted, '会再犯')
  noteLessonRequest(flow(model), reviewer, current, { ok: true, text: 'VERDICT: PASS' })
  assert.equal(current.lessonWanted, '会再犯')
})

test('a lesson turn goes to the author who skipped, only when asked', async () => {
  const author = step('dev', { outputs: [{ ref: 'task:experience/dev-1.md', sha256: 'x' }] })
  const skipped = memoryFs({ [`${DRAFTS}/dev-1.md`]: SKIP }).fs
  const wrote = memoryFs({ [`${DRAFTS}/dev-1.md`]: LESSON }).fs
  assert.equal(await lessonTurnAuthor(skipped, flow(model), run([author], { lessonWanted: '会再犯' })), 'dev')
  assert.equal(await lessonTurnAuthor(wrote, flow(model), run([author], { lessonWanted: '会再犯' })), '')
  assert.equal(await lessonTurnAuthor(skipped, flow(model), run([author])), '')
  assert.equal(await lessonTurnAuthor(skipped, flow({ ...model, policy: 'always' }), run([author], { lessonWanted: '会再犯' })), '')
})

test('a lesson turn spends no retries or steps and ends the run', () => {
  const definition = flow(model)
  const passed = run([step('dev'), step('cr')], { status: 'completed', currentNodeId: 'cr', stepCount: 2, retryCount: 1, lessonWanted: '会再犯' })
  const started = startLessonTurn(definition, passed, 'dev')
  assert.equal(started.event.type, 'execute')
  assert.equal(started.run.lessonWanted, undefined)
  assert.equal(started.run.stepCount, 2)
  const turn = started.run.steps[started.run.steps.length - 1]
  assert.equal(turn.purpose, 'lesson')
  assert.match(turn.envelope?.task || '', /不改代码/)
  assert.match(turn.envelope?.done || '', /会再犯/)
  const done = afterOutcome(definition, started.run, { ok: true, text: '已补写', verdict: 'fail' })
  assert.equal(done.event.type, 'complete')
  assert.equal(done.run.status, 'completed')
  assert.equal(done.run.retryCount, 1)
  assert.equal(done.run.steps[done.run.steps.length - 1].envelope?.done, '已补写')
})

test('a failed lesson turn is recorded without failing work that passed', () => {
  const definition = flow(model)
  const started = startLessonTurn(definition, run([step('dev'), step('cr')], { status: 'completed', currentNodeId: 'cr', lessonWanted: 'x' }), 'dev')
  const done = afterOutcome(definition, started.run, { ok: false, text: '', error: '经验段不对' })
  assert.equal(done.run.status, 'completed')
  const turn = done.run.steps[done.run.steps.length - 1]
  assert.equal(turn.status, 'failed')
  assert.equal(turn.error, '经验段不对')
})

test('drafts are read in attempt order and the newest wording of a lesson wins', async () => {
  const { fs } = memoryFs({
    [`${DRAFTS}/dev-10.md`]: LESSON.replace('#### 症状\n有', '#### 症状\n第十次'),
    [`${DRAFTS}/dev-2.md`]: LESSON,
    [`${DRAFTS}/dev-3.md`]: SKIP,
    [`${DRAFTS}/dev-4.md`]: '## 可复用经验\n### 另一条\n正文\n',
    [`${DRAFTS}/notes.txt`]: LESSON
  })
  const lessons = await collectLessons(fs, run([]))
  assert.deepEqual(lessons.map((row) => [row.file, row.title]), [['dev-4.md', '另一条'], ['dev-10.md', '一条经验']])
  assert.match(lessons[1].body, /第十次/)
  assert.deepEqual(lessons[1].missingPaths, ['src/gone.ts'])
  assert.deepEqual(lessons[0].missingPaths, [])
})

test('no drafts means nothing to approve', async () => {
  assert.deepEqual(await collectLessons(memoryFs().fs, run([])), [])
})

function pending(): LessonDraft[] {
  const row = { file: 'dev-2.md', scope: 'flow', missingSections: [], missingPaths: [] }
  return [
    { ...row, id: 'a', title: '留下', body: '### 留下\n正文' },
    { ...row, id: 'b', title: '不要', body: '### 不要\n正文' }
  ]
}

test('harvest writes approved lessons verbatim and keeps the rest out', async () => {
  const { fs, files } = memoryFs()
  const records = await harvestLessons(fs, run([], { pendingLessons: pending() }), ['a'], '2026-11-06')
  const rel = 'memory/entries/2026-11-06__flow__留下.md'
  assert.deepEqual(records, [{ title: '留下', scope: 'flow', entry: rel }, { title: '不要', scope: 'flow' }])
  assert.ok(files.get(rel)?.endsWith('### 留下\n正文\n'))
  assert.equal(files.get('memory/index.md'), `# 经验索引\n\n## flow\n- 留下 — .agentDock/${rel}\n`)
  assert.equal([...files.keys()].filter((key) => key.includes('不要')).length, 0)
})

test('skipping everything leaves memory untouched', async () => {
  const { fs, files } = memoryFs()
  const records = await harvestLessons(fs, run([], { pendingLessons: pending() }), [], '2026-11-06')
  assert.equal(records.length, 2)
  assert.equal(files.size, 0)
})

test('retrying a harvest does not duplicate, a different lesson gets its own file', async () => {
  const { fs, files } = memoryFs()
  const current = run([], { pendingLessons: pending() })
  await harvestLessons(fs, current, ['a'], '2026-11-06')
  await harvestLessons(fs, current, ['a'], '2026-11-06')
  assert.equal([...files.keys()].filter((key) => key.startsWith('memory/entries/')).length, 1)
  assert.equal(files.get('memory/index.md')?.split('\n').filter((line) => line.startsWith('- ')).length, 1)
  const changed = run([], { pendingLessons: [{ ...pending()[0], body: '### 留下\n新内容' }] })
  await harvestLessons(fs, changed, ['a'], '2026-11-06')
  assert.ok(files.has('memory/entries/2026-11-06__flow__留下-2.md'))
})

test('harvest failure keeps the approval open', async () => {
  const { fs } = memoryFs()
  const broken: ExperienceFs = { ...fs, write: async () => { throw new Error('磁盘满') } }
  const current = run([], { status: 'waiting', pendingLessons: pending() })
  await assert.rejects(harvestLessons(broken, current, ['a'], '2026-11-06'))
  assert.equal(isAwaitingLessons(current), true)
})

test('the run waits for approval, then completes with its decisions', () => {
  const waiting = awaitLessons(run([], { status: 'completed' }), pending())
  assert.equal(waiting.event.type, 'waitLessons')
  assert.equal(isAwaitingLessons(waiting.run), true)
  const done = finishLessons(waiting.run, [{ title: '留下', scope: 'flow', entry: 'memory/entries/a.md' }])
  assert.equal(done.status, 'completed')
  assert.equal(isAwaitingLessons(done), false)
  assert.equal(done.pendingLessons, undefined)
  assert.equal(runRecord(done).lessons?.length, 1)
})

test('stopping a run drops its pending approval', () => {
  const stopped = cancelRun(awaitLessons(run([]), pending()).run)
  assert.equal(stopped.status, 'canceled')
  assert.equal(isAwaitingLessons(stopped), false)
})

test('the configuration must point at real, independent working nodes', () => {
  assert.equal(validateExperience(flow()), null)
  assert.equal(validateExperience(flow(model)), null)
  assert.match(validateExperience(flow({ policy: 'always', authors: [] })) || '', /authors/)
  assert.match(validateExperience(flow({ policy: 'always', authors: ['nope'] })) || '', /作者/)
  assert.match(validateExperience(flow({ policy: 'always', authors: ['pm'] })) || '', /作者/)
  assert.match(validateExperience(flow({ policy: 'always', authors: ['cr'] })) || '', /Codex 审查节点不能当经验作者/)
  assert.match(validateExperience(flow({ policy: 'model', authors: ['dev'], judge: 'pm' })) || '', /判定/)
  assert.match(validateExperience(flow({ policy: 'model', authors: ['dev'], judge: 'dev' })) || '', /不能同时是作者/)
})

test('experience survives the flow file and stays out of bad shapes', () => {
  const parsed = parseFlowFile('f.json', flowFileText(flow({ policy: 'attempts', minAttempts: 3, authors: ['dev'] })))
  assert.deepEqual({ ...parsed?.experience }, { policy: 'attempts', minAttempts: 3, authors: ['dev'], judge: undefined })
  const defaults = parseFlowFile('f.json', JSON.stringify({ nodes: [], edges: [], experience: { policy: 'attempts' } }))
  assert.equal(defaults?.experience?.minAttempts, 2)
  assert.equal(parseFlowFile('f.json', JSON.stringify({ nodes: [], edges: [], experience: { policy: 'often' } }))?.experience, undefined)
})

test('a flow file cannot smuggle in the lesson rule', () => {
  const text = JSON.stringify({ nodes: [{ id: 'a', channel: pty, outputs: [{ path: 'task:x.md', lesson: 'entries' }] }], edges: [] })
  assert.equal(parseFlowFile('f.json', text)?.nodes[0].outputs?.[0].lesson, undefined)
})

test('a run started from a flow with experience passes through the planner', () => {
  const started = startRun(flow(model), 'p', '修闸门')
  started.run.taskDir = 't1'
  assert.equal(draftOf(planStep(flow(model), dev, started.run, started.run.steps[0]))?.path, 'task:experience/dev-1.md')
})
