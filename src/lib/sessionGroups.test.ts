import assert from 'node:assert/strict'
import {
  groupBodyHidden,
  parseCollapsedGroups,
  serializeCollapsedGroups,
  sessionLiveFilterLabel,
  shouldShowSessionGroupHead,
  toggleCollapsedGroup
} from './sessionGroups.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('parses only known tool ids', () => {
  assert.deepEqual([...parseCollapsedGroups(null)], [])
  assert.deepEqual([...parseCollapsedGroups('["grokbuild","kimi"]')].sort(), ['grokbuild', 'kimi'])
  assert.deepEqual([...parseCollapsedGroups('["nope",123,"dsh"]')], ['dsh'])
  assert.deepEqual([...parseCollapsedGroups('{')], [])
})

test('toggle remembers collapsed groups', () => {
  const once = toggleCollapsedGroup(new Set(), 'grokbuild')
  assert.equal(once.has('grokbuild'), true)
  const twice = toggleCollapsedGroup(once, 'grokbuild')
  assert.equal(twice.has('grokbuild'), false)
  assert.equal(serializeCollapsedGroups(once), '["grokbuild"]')
})

test('search keeps collapsed groups visible', () => {
  assert.equal(groupBodyHidden(true), true)
  assert.equal(groupBodyHidden(true, '  样式  '), false)
  assert.equal(groupBodyHidden(false, '样式'), false)
})

test('group heads only show in the all-tools list', () => {
  assert.equal(shouldShowSessionGroupHead(1), false)
  assert.equal(shouldShowSessionGroupHead(2), true)
  assert.equal(shouldShowSessionGroupHead(1, true), true)
})

test('live filter switch labels all sessions when off', () => {
  assert.equal(sessionLiveFilterLabel(false), '全部会话')
  assert.equal(sessionLiveFilterLabel(true), '已开对话')
})
