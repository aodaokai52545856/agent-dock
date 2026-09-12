import assert from 'node:assert/strict'
import {
  VERSION_CHECKING,
  VERSION_IDLE,
  idleVersionRow,
  isVersionChecking,
  isVersionIdle,
  markVersionChecking,
  mergeVersionRows,
  patchVersionRow,
  seedVersionRows
} from './toolVersions.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('opening the panel starts idle, not checking', () => {
  const rows = seedVersionRows()
  assert.ok(rows.length >= 6)
  assert.ok(rows.every((row) => row.compare === VERSION_IDLE))
  assert.ok(rows.every((row) => row.localVersion === '-'))
  assert.equal(isVersionIdle(rows[0]!.compare), true)
  assert.equal(isVersionChecking(rows[0]!.compare), false)
})

test('markVersionChecking only flips that row to 检测中', () => {
  const idle = idleVersionRow('dsh', 'DeepSeek')
  const next = markVersionChecking(idle)
  assert.equal(next.compare, VERSION_CHECKING)
  assert.equal(idle.compare, VERSION_IDLE)
})

test('patchVersionRow replaces one tool and keeps the others', () => {
  const rows = seedVersionRows()
  const patched = patchVersionRow(rows, {
    toolId: 'dsh',
    name: 'DeepSeek Harness',
    found: true,
    localVersion: '0.1.5-rc.1',
    latestVersion: '0.1.5-rc.1',
    compare: '已是最新'
  })
  assert.equal(patched.find((row) => row.toolId === 'dsh')?.compare, '已是最新')
  assert.equal(patched.find((row) => row.toolId === 'pi')?.compare, VERSION_IDLE)
})

test('mergeVersionRows fills missing tools as 查询失败', () => {
  const merged = mergeVersionRows([
    {
      toolId: 'claude',
      name: 'Claude Code',
      found: true,
      localVersion: '2.1.269',
      latestVersion: '2.1.269',
      compare: '已是最新'
    }
  ])
  assert.equal(merged.find((row) => row.toolId === 'claude')?.compare, '已是最新')
  assert.equal(merged.find((row) => row.toolId === 'dsh')?.compare, '无法对比')
})
