import assert from 'node:assert/strict'
import { codexInstructions, emptyEnvelope, promptFromEnvelope, renderEnvelope, transformEnvelope, withVerdictTicket } from './envelope.ts'
import { ROLE_CONTRACTS } from './roles.ts'
import { FLOW_END, type FlowDef, type FlowNode } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const reviewer: FlowNode = {
  id: 'n-rev',
  title: '审查者',
  role: 'reviewer',
  roleContract: ROLE_CONTRACTS.reviewer,
  channel: {
    kind: 'codexApp',
    threadIds: [],
    targetKind: 'uncommittedChanges',
    commitSha: '',
    baseBranch: 'main',
    customInstructions: ''
  }
}

const developer: FlowNode = {
  id: 'n-dev',
  title: '开发者',
  role: 'developer',
  roleContract: ROLE_CONTRACTS.developer,
  channel: { kind: 'pty', toolId: 'grokbuild', sessionId: '', ptyId: '' }
}

const flow: FlowDef = {
  id: 'f1',
  name: 't',
  nodes: [reviewer, developer],
  edges: [
    {
      id: 'e1',
      from: 'n-rev',
      to: 'n-dev',
      mode: 'manual',
      gate: 'none'
    }
  ],
  maxRetries: 2,
  maxSteps: 12,
  slice: 1,
  draftTask: '修闸门'
}

test('roleWrap names both roles and keeps the contract', () => {
  const env = emptyEnvelope({
    runId: 'r1',
    fromNode: 'n-rev',
    toNode: 'n-dev',
    fromRole: 'reviewer',
    toRole: 'developer',
    task: '修闸门',
    done: '缺测试\nVERDICT: FAIL',
    verdict: 'fail',
    artifacts: [{ kind: 'text', body: '缺测试\nVERDICT: FAIL' }]
  })
  const text = renderEnvelope(env, { contract: ROLE_CONTRACTS.developer })
  assert.match(text, /【编排接力 · 审查者 → 开发者】/)
  assert.match(text, /本轮任务：修闸门/)
  assert.match(text, /结论：FAIL/)
  assert.match(text, /目标角色合同：必须只改当前这一片/)
  assert.equal(env.causationId, '')
})

test('verbatim is the artifact body only', () => {
  const env = emptyEnvelope({
    runId: 'r1',
    toNode: 'n-dev',
    toRole: 'developer',
    task: '修闸门',
    artifacts: [{ kind: 'text', body: '原文意见' }]
  })
  assert.equal(renderEnvelope(env, { transform: 'verbatim' }), '原文意见')
})

test('transform sets causationId and target role', () => {
  const env = emptyEnvelope({
    id: 'env-1',
    runId: 'r1',
    fromNode: 'n-rev',
    toNode: 'n-rev',
    fromRole: 'reviewer',
    toRole: 'reviewer',
    task: '修闸门',
    done: 'FAIL'
  })
  const next = transformEnvelope(env, flow, 'n-dev')
  assert.equal(next.causationId, 'env-1')
  assert.notEqual(next.id, 'env-1')
  assert.equal(next.toNode, 'n-dev')
  assert.equal(next.toRole, 'developer')
})

test('developer prompt includes the slice task and prior notes', () => {
  const env = emptyEnvelope({
    runId: 'r1',
    toNode: 'n-dev',
    toRole: 'developer',
    task: '修闸门',
    done: '缺测试'
  })
  const prompt = promptFromEnvelope(developer, env)
  assert.match(prompt, /本轮任务：/)
  assert.match(prompt, /修闸门/)
  assert.match(prompt, /缺测试/)
  assert.match(prompt, /不得开新需求/)
})

test('FLOW_END is the terminal token', () => {
  assert.equal(FLOW_END, '__end__')
})

test('codex instructions carry the contract, task and upstream notes', () => {
  const env = emptyEnvelope({ runId: 'r1', toNode: 'n-rev', toRole: 'reviewer', task: '修闸门', done: '已改 engine' })
  const custom: FlowNode = {
    ...reviewer,
    channel: { ...(reviewer.channel as Extract<FlowNode['channel'], { kind: 'codexApp' }>), customInstructions: '重点看回退' }
  }
  const text = codexInstructions(custom, env)
  assert.match(text, /VERDICT: PASS/)
  assert.match(text, /重点看回退/)
  assert.match(text, /本轮任务：修闸门/)
  assert.match(text, /上一节点做了：已改 engine/)
})

test('codex instructions skip upstream notes that only repeat the task', () => {
  const env = emptyEnvelope({ runId: 'r1', toNode: 'n-rev', toRole: 'reviewer', task: '修闸门', done: '修闸门' })
  assert.doesNotMatch(codexInstructions(reviewer, env), /上一节点做了/)
})

test('verdict ticket is added once', () => {
  assert.match(withVerdictTicket('你是测试工程师。'), /你是测试工程师。\n.*VERDICT: PASS/)
  assert.equal(withVerdictTicket(ROLE_CONTRACTS.reviewer), ROLE_CONTRACTS.reviewer)
  assert.match(withVerdictTicket(''), /^结束时/)
})
