import assert from 'node:assert/strict'
import {
  accountSessionGroups,
  compareSessionRecency,
  groupBodyHidden,
  parseCollapsedGroups,
  serializeCollapsedGroups,
  sessionLiveFilterLabel,
  shouldShowSessionGroupHead,
  toggleCollapsedGroup
} from './sessionGroups.ts'
import type { SessionRow } from './types.ts'

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

const row = (id: string, accountId: string, updatedAt: number): SessionRow => ({
  id, toolId: 'claude', title: id, cwd: '', renameKind: 'native', updatedAt,
  account: { id: accountId, name: `账号 ${accountId}` }
})

test('Claude accounts form independent groups in a stable order', () => {
  const sessions = [row('b-new', 'b', 300), row('a-old', 'a', 100), row('a-new', 'a', 200)]
  const groups = accountSessionGroups('claude', sessions)
  assert.deepEqual(groups.map((g) => [g.id, g.sessions.map((s) => s.id)]), [
    ['claude:a', ['a-new', 'a-old']], ['claude:b', ['b-new']]
  ])
  assert.equal(groups[0].label, 'Claude · 账号 a')
  sessions[1].updatedAt = 400
  assert.deepEqual(accountSessionGroups('claude', sessions).map((g) => g.id), groups.map((g) => g.id))
  assert.deepEqual(sessions.map((s) => s.id), ['b-new', 'a-old', 'a-new'])
})

test('account collapse survives reload and search temporarily reveals the results', () => {
  const collapsed = toggleCollapsedGroup(new Set(), 'claude:a')
  assert.ok(parseCollapsedGroups(serializeCollapsedGroups(collapsed)).has('claude:a'))
  assert.ok(!collapsed.has('claude:b'))
  assert.equal(groupBodyHidden(collapsed.has('claude:a'), 'find'), false)
  assert.equal(groupBodyHidden(collapsed.has('claude:a'), ''), true)
})

test('unknown ownership stays separate and other tools keep their group', () => {
  const unknown = { ...row('old', 'a', 0), account: null }
  assert.equal(accountSessionGroups('claude', [unknown])[0].label, 'Claude · 归属未知')
  const grok = { ...unknown, toolId: 'grokbuild' as const }
  assert.equal(accountSessionGroups('grokbuild', [grok])[0].id, 'grokbuild')
})

test('equal conversation timestamps keep the same order across scans', () => {
  const a = row('a', 'one', 100)
  const b = row('b', 'one', 100)
  const recent = row('recent', 'one', 200)
  assert.deepEqual([b, a, recent].sort(compareSessionRecency).map((s) => s.id), ['recent', 'a', 'b'])
  assert.deepEqual([recent, a, b].sort(compareSessionRecency).map((s) => s.id), ['recent', 'a', 'b'])
})
