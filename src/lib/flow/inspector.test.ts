import assert from 'node:assert/strict'
import { flowInspectorKind, flowInspectorTitle } from './inspector.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('edit mode only floats inspector when a node or edge is selected', () => {
  assert.equal(flowInspectorKind('edit', '', ''), '')
  assert.equal(flowInspectorKind('edit', 'n1', ''), 'node')
  assert.equal(flowInspectorKind('edit', '', 'e1'), 'edge')
  assert.equal(flowInspectorKind('edit', 'n1', 'e1'), 'edge')
})

test('run mode always shows the run inspector overlay', () => {
  assert.equal(flowInspectorKind('run', '', ''), 'run')
  assert.equal(flowInspectorKind('run', 'n1', ''), 'run')
})

test('inspector titles match the overlay kind', () => {
  assert.equal(flowInspectorTitle('node'), '节点')
  assert.equal(flowInspectorTitle('edge'), '连线')
  assert.equal(flowInspectorTitle('run'), '运行')
  assert.equal(flowInspectorTitle(''), '')
})
