import assert from 'node:assert/strict'
import { detectNewAssistantTurn, extendDeadline, turnIds } from './ptyWait.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('no new assistant turn while ids stay the same', () => {
  const turns = [
    { id: 't-0', role: 'user', excerpt: 'hi', text: 'hi' },
    { id: 't-1', role: 'assistant', excerpt: 'old', text: 'old' }
  ]
  assert.equal(detectNewAssistantTurn(turnIds(turns), turns), null)
})

test('new assistant id completes the wait', () => {
  const before = [
    { id: 't-0', role: 'user', excerpt: 'hi', text: 'hi' },
    { id: 't-1', role: 'assistant', excerpt: 'old', text: 'old' }
  ]
  const after = [
    ...before,
    { id: 't-2', role: 'user', excerpt: 'go', text: 'go' },
    { id: 't-3', role: 'assistant', excerpt: 'done', text: 'done' }
  ]
  const found = detectNewAssistantTurn(turnIds(before), after)
  assert.equal(found?.id, 't-3')
  assert.equal(found?.text, 'done')
})

test('an opening remark before tool calls does not end the wait', () => {
  const before = [{ id: 't-0', role: 'user', excerpt: 'go', text: 'go' }]
  const opening = { id: 't-1', role: 'assistant', excerpt: '我先看看', text: '我先看看', working: true }
  assert.equal(detectNewAssistantTurn(turnIds(before), [...before, opening]), null)
  const final = { id: 't-2', role: 'assistant', excerpt: '写好了', text: '写好了' }
  assert.equal(detectNewAssistantTurn(turnIds(before), [...before, opening, final])?.text, '写好了')
})

test('the latest reply is the one handed on', () => {
  const first = { id: 't-1', role: 'assistant', excerpt: '一', text: '一' }
  const second = { id: 't-2', role: 'assistant', excerpt: '二', text: '二' }
  assert.equal(detectNewAssistantTurn([], [first, second])?.text, '二')
})

test('window output pushes the deadline, silence does not', () => {
  assert.equal(extendDeadline(100, 90, 50), 140)
  assert.equal(extendDeadline(100, 10, 50), 100)
  assert.equal(extendDeadline(100, undefined, 50), 100)
})
