import assert from 'node:assert/strict'
import test from 'node:test'
import {
  channelReadsFiles,
  flowHasContract,
  outputCapture,
  renderContractBlock,
  resolveRef,
  taskDirName,
  validateContract,
  validateOutputText,
  verdictFromDocument
} from './contract.ts'
import type { FlowDef, FlowNode } from './types.ts'

const pty: FlowNode['channel'] = { kind: 'pty', toolId: 'claude', sessionId: 's', ptyId: 'p' }
const codex: FlowNode['channel'] = { kind: 'codexApp', threadIds: [], targetKind: 'uncommittedChanges', commitSha: '', baseBranch: 'main', customInstructions: '' }

function flowWith(node: Partial<FlowNode>): FlowDef {
  return {
    id: 'f',
    name: 'f',
    nodes: [{ id: 'n', title: '开发', role: 'developer', roleContract: '', channel: pty, ...node }],
    edges: [],
    maxRetries: 2,
    maxSteps: 12,
    slice: 1,
    draftTask: ''
  }
}

test('refs resolve under the task and the project scope', () => {
  assert.deepEqual(resolveRef('task:design.md', 't1'), { rel: 'tasks/t1/design.md', path: '.agentDock/tasks/t1/design.md' })
  assert.deepEqual(resolveRef('project:memory/index.md', 't1'), { rel: 'memory/index.md', path: '.agentDock/memory/index.md' })
})

test('malformed or escaping refs are refused', () => {
  for (const ref of ['design.md', 'repo:a.md', 'task:', 'task:../a.md', 'project:memory/../../a', 'task:a//b', 'task:a\\..\\b']) {
    assert.equal(resolveRef(ref, 't1'), null, ref)
  }
})

test('flows without inputs or outputs have no contract', () => {
  assert.equal(flowHasContract(flowWith({})), false)
  assert.equal(flowHasContract(flowWith({ inputs: [{ ref: 'task:a.md' }] })), true)
  assert.equal(flowHasContract(flowWith({ outputs: [{ path: 'task:a.md' }] })), true)
})

test('capture follows what the channel can do', () => {
  assert.equal(outputCapture(pty, { path: 'task:a.md' }), 'file')
  assert.equal(outputCapture(pty, { path: 'task:a.md', capture: 'reply' }), 'reply')
  assert.equal(outputCapture(codex, { path: 'task:a.md', capture: 'file' }), 'reply')
  assert.equal(outputCapture({ kind: 'cursorSdk', agentId: '' }, { path: 'task:a.md' }), 'reply')
  assert.equal(channelReadsFiles({ kind: 'cursorSdk', agentId: '' }), false)
  assert.equal(channelReadsFiles(codex), true)
})

test('contract validation rejects bad declarations', () => {
  assert.equal(validateContract(flowWith({ inputs: [{ ref: 'task:a.md' }], outputs: [{ path: 'task:b.md' }] })), null)
  assert.match(validateContract(flowWith({ inputs: [{ ref: 'a.md' }] })) || '', /输入写法不对/)
  assert.match(validateContract(flowWith({ outputs: [{ path: 'project:memory/a.md' }] })) || '', /只能写到 task:/)
  assert.match(validateContract(flowWith({ channel: { kind: 'human' }, outputs: [{ path: 'task:a.md' }] })) || '', /人工节点/)
  assert.match(validateContract(flowWith({ channel: { ...pty, toolId: 'pi' }, outputs: [{ path: 'task:a.md' }] })) || '', /不能确认一轮是否结束/)
})

test('task directory names are readable and safe', () => {
  const at = new Date(2026, 10, 6, 9, 5).getTime()
  assert.equal(taskDirName('修复 闸门/回退: bug?', at, 'run-abc123'), '20261106-0905-修复-闸门-回退-bug-c123')
  assert.equal(taskDirName('   ', at, 'run-abc123'), '20261106-0905-c123')
  assert.ok(taskDirName('长'.repeat(100), at, 'r1').length < 50)
})

test('the block lists paths by default and inlines only when asked', () => {
  const listed = renderContractBlock([{ path: '.agentDock/a.md' }, { path: '.agentDock/memory/index.md', read: 'index-first' }], [])
  assert.match(listed, /- \.agentDock\/a\.md\n/)
  assert.match(listed, /index\.md（这是索引/)
  const inlined = renderContractBlock([{ path: '.agentDock/a.md', text: '正文' }], [])
  assert.match(inlined, /<file>\n正文\n<\/file>/)
  assert.doesNotMatch(inlined, /先读完/)
})

test('the block states how each output is delivered', () => {
  const file = renderContractBlock([], [{ path: '.agentDock/x.md', capture: 'file', sections: ['## 结论'], template: '.agentDock/roles/dev/x.md' }])
  assert.match(file, /把产物写到 \.agentDock\/x\.md/)
  assert.match(file, /按模板 \.agentDock\/roles\/dev\/x\.md/)
  assert.match(file, /必须包含：## 结论/)
  assert.match(renderContractBlock([], [{ path: '.agentDock/x.md', capture: 'reply', sections: [] }]), /在回复里给出完整产物/)
})

test('output validation catches empty, unfilled and incomplete documents', () => {
  const sections = ['## 验证证据', '## 结论']
  assert.equal(validateOutputText('a.md', '## 验证证据\n跑过了\n\n## 结论\nPASS\n', sections), null)
  assert.match(validateOutputText('a.md', '  \n', []) || '', /是空的/)
  assert.match(validateOutputText('a.md', '## 结论\n{{结论}}', [], ['{{结论}}']) || '', /占位符：\{\{结论\}\}/)
  assert.match(validateOutputText('a.md', '## 结论\nPASS', sections) || '', /缺少段落：## 验证证据/)
  assert.match(validateOutputText('a.md', '## 验证证据\n\n## 结论\nPASS', sections) || '', /段落是空的：## 验证证据/)
})

test('a section keeps its deeper headings', () => {
  assert.equal(validateOutputText('a.md', '## 验证证据\n### 单测\n通过\n## 结论\nPASS', ['## 验证证据']), null)
})

test('a heading inside a word does not count as a section', () => {
  assert.match(validateOutputText('a.md', '见 ## 结论 一节', ['## 结论']) || '', /缺少段落/)
})

test('the verdict comes from the conclusion section only', () => {
  assert.equal(verdictFromDocument('## 结论\nVERDICT: PASS\n'), 'pass')
  assert.equal(verdictFromDocument('## 结论\n未通过\n缺测试\n'), 'fail')
  assert.equal(verdictFromDocument('## 结论\n未通过：缺测试\n'), undefined)
  assert.equal(verdictFromDocument('正文写着 PASS，但没有结论段'), undefined)
  assert.equal(verdictFromDocument('## 结论\n还在看\n'), undefined)
})
