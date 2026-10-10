import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { DSH_FIXED_SESSION_ID } from './dsh.ts'
import { pendingSessionId } from './liveBind.ts'
import {
  allKeysSelected,
  canPhysicallyDeleteSession,
  deletableSessionKeys,
  pruneSessionKeys,
  sessionSelectKey,
  toggleSessionKey,
  toggleVisibleSelection
} from './sessionBatch.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('only on-disk sessions can be batch deleted', () => {
  assert.equal(canPhysicallyDeleteSession('grokbuild', 's1'), true)
  assert.equal(canPhysicallyDeleteSession('claude', 's1'), true)
  assert.equal(canPhysicallyDeleteSession('kimi', 's1'), true)
  assert.equal(canPhysicallyDeleteSession('pi', 's1'), true)
  assert.equal(canPhysicallyDeleteSession('opencode', 's1'), true)
  assert.equal(canPhysicallyDeleteSession('dsh', DSH_FIXED_SESSION_ID), false)
  assert.equal(canPhysicallyDeleteSession('grokbuild', pendingSessionId('pty-1')), false)
})

test('select all covers the visible deletable rows and can clear them', () => {
  const rows = [
    { toolId: 'dsh', id: DSH_FIXED_SESSION_ID },
    { toolId: 'grokbuild', id: 's1' },
    { toolId: 'opencode', id: 's3' }
  ]
  const visible = deletableSessionKeys(rows)
  assert.deepEqual(visible, ['grokbuild:s1', 'opencode:s3'])
  const picked = toggleVisibleSelection([], visible)
  assert.equal(allKeysSelected(picked, visible), true)
  assert.deepEqual(toggleVisibleSelection(picked, visible), [])
  assert.deepEqual(toggleSessionKey(picked, 'grokbuild:s1'), ['opencode:s3'])
})

test('a session that left the list drops out of the selection', () => {
  const keys = ['grokbuild:s1', 'kimi:gone']
  assert.deepEqual(pruneSessionKeys(keys, ['grokbuild:s1']), ['grokbuild:s1'])
  assert.equal(sessionSelectKey('kimi', 'gone'), 'kimi:gone')
})

test('batch delete uses the physical delete command and refreshes the list once', () => {
  const root = dirname(fileURLToPath(import.meta.url))
  const store = readFileSync(join(root, 'store.ts'), 'utf8')
  const batch = store.slice(store.indexOf('export async function deleteSessions'))
  assert.match(batch, /api\.deleteSession/)
  assert.match(batch, /refreshSessions\(\{ silent: true \}\)/)
  assert.doesNotMatch(batch.slice(0, batch.indexOf('export async function deleteCurrentSession')), /sessionRefreshBusy = true/)
  const app = readFileSync(join(root, '../App.vue'), 'utf8')
  assert.match(app, /delete-many/)
  assert.match(app, /deleteSessions/)
  const sidebar = readFileSync(join(root, '../components/SideBar.vue'), 'utf8')
  assert.match(sidebar, /批量/)
  assert.match(sidebar, /delete-many/)
  assert.match(sidebar, /canPhysicallyDeleteSession/)
})
