import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  forgetProjectSessionView,
  readProjectSessionView,
  writeProjectSessionView,
  type SessionViewStorage
} from './sessionViewMemory.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function memory(): SessionViewStorage & { dump: Record<string, string> } {
  const dump: Record<string, string> = {}
  return {
    dump,
    getItem(key) {
      return Object.prototype.hasOwnProperty.call(dump, key) ? dump[key] : null
    },
    setItem(key, value) {
      dump[key] = value
    },
    removeItem(key) {
      delete dump[key]
    }
  }
}

const fallback = { toolFilter: 'all' as const, liveOnly: false }

test('a project with no memory keeps the global fallback', () => {
  const view = readProjectSessionView(memory(), 'aiquery', { toolFilter: 'grokbuild', liveOnly: true })
  assert.deepEqual(view, { toolFilter: 'grokbuild', liveOnly: true })
})

test('each project remembers its own tool and all-sessions flag', () => {
  const store = memory()
  writeProjectSessionView(store, 'agent-dock', { toolFilter: 'grokbuild', liveOnly: true })
  writeProjectSessionView(store, 'aiquery', { toolFilter: 'claude', liveOnly: false })

  assert.deepEqual(readProjectSessionView(store, 'agent-dock', fallback), {
    toolFilter: 'grokbuild',
    liveOnly: true
  })
  assert.deepEqual(readProjectSessionView(store, 'aiquery', fallback), {
    toolFilter: 'claude',
    liveOnly: false
  })
  assert.deepEqual(readProjectSessionView(store, 'other', fallback), fallback)
})

test('a stored all-sessions flag stays off even when the fallback is on', () => {
  const store = memory()
  writeProjectSessionView(store, 'aiquery', { toolFilter: 'claude', liveOnly: false })
  assert.equal(readProjectSessionView(store, 'aiquery', { toolFilter: 'grokbuild', liveOnly: true }).liveOnly, false)
})

test('invalid stored tool falls back to all for that project only', () => {
  const store = memory()
  store.setItem('ad-session-tool-filter:agent-dock', 'nope')
  store.setItem('ad-session-live-only:agent-dock', '1')
  assert.deepEqual(readProjectSessionView(store, 'agent-dock', { toolFilter: 'claude', liveOnly: false }), {
    toolFilter: 'all',
    liveOnly: true
  })
})

test('forgetting a project drops only that project', () => {
  const store = memory()
  writeProjectSessionView(store, 'agent-dock', { toolFilter: 'grokbuild', liveOnly: true })
  writeProjectSessionView(store, 'aiquery', { toolFilter: 'claude', liveOnly: false })
  forgetProjectSessionView(store, 'agent-dock')
  assert.deepEqual(readProjectSessionView(store, 'agent-dock', fallback), fallback)
  assert.equal(readProjectSessionView(store, 'aiquery', fallback).toolFilter, 'claude')
})

test('empty project id does not write', () => {
  const store = memory()
  writeProjectSessionView(store, '', { toolFilter: 'grokbuild', liveOnly: true })
  assert.deepEqual(store.dump, {})
  assert.deepEqual(readProjectSessionView(store, '', { toolFilter: 'claude', liveOnly: true }), {
    toolFilter: 'claude',
    liveOnly: true
  })
})

test('switching projects restores the leaving view before the next id', () => {
  const root = dirname(fileURLToPath(import.meta.url))
  const source = readFileSync(join(root, 'store.ts'), 'utf8')
  const select = source.slice(
    source.indexOf('export async function selectProject'),
    source.indexOf('export async function jumpToLive')
  )
  const rememberAt = select.indexOf('rememberCurrentSessionView()')
  const assignAt = select.indexOf('store.selectedProjectId = id')
  assert.ok(rememberAt >= 0 && assignAt > rememberAt)
  assert.match(select, /applyRememberedSessionView\(id\)/)
  assert.doesNotMatch(source, /store\.sessionToolFilter = store\.settings\.sessionToolFilter/)
  assert.match(source, /forgetProjectSessionView/)
})
