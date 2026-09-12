import assert from 'node:assert/strict'
import { termMenuForbidden, termMenuItems } from './termMenu.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('terminal menu follows the app copy and does not include browser chrome', () => {
  const items = termMenuItems({ hasSelection: true, mac: false })
  const labels = items.map((item) => item.label)
  assert.deepEqual(labels, ['复制', '粘贴', '全选'])
  assert.equal(labels.some(termMenuForbidden), false)
  assert.equal(items.find((item) => item.id === 'copy')?.enabled, true)
  assert.equal(items.find((item) => item.id === 'copy')?.hint, 'Ctrl+C')
})

test('copy is disabled without a selection', () => {
  const items = termMenuItems({ hasSelection: false, mac: true })
  assert.equal(items.find((item) => item.id === 'copy')?.enabled, false)
  assert.equal(items.find((item) => item.id === 'copy')?.hint, '⌘+C')
  assert.equal(items.find((item) => item.id === 'paste')?.enabled, true)
  assert.equal(items.find((item) => item.id === 'selectAll')?.enabled, true)
})
