import assert from 'node:assert/strict'
import { detectNewAssistantTurn, turnIds } from './ptyWait.ts'

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
