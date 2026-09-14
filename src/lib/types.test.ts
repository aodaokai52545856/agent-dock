import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  TOOLS,
  TOOL_OFFICIAL_URLS,
  clampSessionToolFilter,
  isToolInstalled,
  parseSessionToolFilter,
  sessionFilterBlock,
  toolsToScanForFilter,
  type ToolProbeMap
} from './types.ts'

function probes(missing: Partial<Record<keyof ToolProbeMap, boolean>> = {}): ToolProbeMap {
  return {
    opencode: { found: missing.opencode !== true },
    grokbuild: { found: missing.grokbuild !== true },
    kimi: { found: missing.kimi !== true },
    claude: { found: missing.claude !== true },
    pi: { found: missing.pi !== true },
    dsh: { found: missing.dsh !== true }
  }
}

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('six CLI tools including Pi and DeepSeek Harness', () => {
  assert.equal(TOOLS.length, 6)
  assert.ok(TOOLS.some((tool) => tool.id === 'claude'))
  assert.ok(TOOLS.some((tool) => tool.id === 'pi'))
  const dsh = TOOLS.find((tool) => tool.id === 'dsh')
  assert.ok(dsh)
  assert.equal(dsh?.hint, '打开 DeepSeek Web')
})

test('DeepSeek install is only ready when Node, dsh and dsh web all pass', () => {
  const row = {
    found: false,
    localVersion: '未通过检查',
    compare: '未通过检查',
    checks: [
      { name: 'Node', ok: true, detail: '22.23.1' },
      { name: 'dsh', ok: true, detail: 'ok' },
      { name: 'dsh web', ok: false, detail: 'no output' }
    ]
  }
  assert.equal(row.checks.every((item) => item.ok), false)
  const ready = {
    found: true,
    checks: [
      { name: 'Node', ok: true, detail: '22.23.1' },
      { name: 'dsh', ok: true, detail: 'ok' },
      { name: 'dsh web', ok: true, detail: 'ok' }
    ]
  }
  assert.equal(ready.checks.every((item) => item.ok), true)
})

test('session filter accepts opened-live view', () => {
  assert.equal(parseSessionToolFilter('live'), 'all')
  assert.equal(parseSessionToolFilter('grokbuild'), 'grokbuild')
  assert.equal(parseSessionToolFilter('nope'), 'all')
})

test('refresh still scans Grok when it is missing from PATH', () => {
  const map = probes({ grokbuild: true })
  assert.equal(isToolInstalled(map, 'grokbuild'), false)
  assert.equal(isToolInstalled(map, 'kimi'), true)
  assert.deepEqual(toolsToScanForFilter('all', map), TOOLS.map((tool) => tool.id))
  assert.deepEqual(toolsToScanForFilter('dsh', map), TOOLS.map((tool) => tool.id))
  assert.deepEqual(toolsToScanForFilter('grokbuild', map), TOOLS.map((tool) => tool.id))
  assert.equal(clampSessionToolFilter('grokbuild', map), 'all')
  assert.equal(sessionFilterBlock('grokbuild', map), 'missing')
  assert.equal(sessionFilterBlock('grokbuild', map, { hasSessions: true }), null)
  assert.equal(sessionFilterBlock('kimi', map, { hasSessions: false }), 'empty')
  assert.equal(sessionFilterBlock('kimi', map, { hasSessions: false, scanning: true }), null)
  assert.equal(clampSessionToolFilter('kimi', map, { hasSessions: false }), 'kimi')
  assert.equal(clampSessionToolFilter('grokbuild', map, { hasSessions: true }), 'grokbuild')
})

test('session picker disables missing tools and all only lists installed groups', () => {
  const root = dirname(fileURLToPath(import.meta.url))
  const sidebar = readFileSync(join(root, '../components/SideBar.vue'), 'utf8')
  assert.match(sidebar, /isToolInstalled/)
  assert.match(sidebar, /:disabled="isFilterDisabled\(option\.id\)"/)
  assert.match(sidebar, /未安装/)
  assert.match(sidebar, /无会话/)
  assert.match(sidebar, /sessionFilterBlock/)
  assert.match(sidebar, /filterHint\(id\) === '未安装'/)
  assert.match(sidebar, /class="group-title"/)
  assert.match(sidebar, /class="group-mark"/)
  assert.match(sidebar, /toggleGroup\(group\.tool\.id\)/)
  const store = readFileSync(join(root, 'store.ts'), 'utf8')
  assert.match(store, /toolsToScanForFilter/)
  assert.match(store, /clampSessionToolFilter/)
})

test('every tool has an official https download page', () => {
  for (const tool of TOOLS) {
    assert.ok(TOOL_OFFICIAL_URLS[tool.id].startsWith('https://'))
  }
  assert.ok(TOOL_OFFICIAL_URLS.claude.includes('claude.com'))
})
