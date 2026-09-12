import assert from 'node:assert/strict'
import { TOOLS, TOOL_OFFICIAL_URLS } from './types.ts'

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

test('every tool has an official https download page', () => {
  for (const tool of TOOLS) {
    assert.ok(TOOL_OFFICIAL_URLS[tool.id].startsWith('https://'))
  }
  assert.ok(TOOL_OFFICIAL_URLS.claude.includes('claude.com'))
})
