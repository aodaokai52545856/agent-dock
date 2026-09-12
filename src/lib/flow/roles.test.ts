import assert from 'node:assert/strict'
import { addCustomRole, allRoles, BUILTIN_ROLES, loadCustomRoles, roleLabel } from './roles.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function memory() {
  const map = new Map<string, string>()
  return {
    getItem(key: string) {
      return map.get(key) ?? null
    },
    setItem(key: string, value: string) {
      map.set(key, value)
    }
  }
}

test('builtin roles cover developer reviewer and pm', () => {
  const ids = BUILTIN_ROLES.map((item) => item.id)
  assert.deepEqual(ids, ['developer', 'reviewer', 'pm'])
  assert.equal(roleLabel('developer'), '开发者')
  assert.equal(roleLabel('reviewer'), '审查者')
})

test('custom role is appended and persisted', () => {
  const store = memory()
  const first = addCustomRole('测试', [], store)
  assert.equal(first.error, '')
  assert.equal(first.role?.label, '测试')
  const loaded = loadCustomRoles(store)
  assert.equal(loaded.length, 1)
  assert.equal(allRoles(loaded).some((item) => item.label === '测试'), true)
})

test('duplicate custom role name is rejected', () => {
  const store = memory()
  addCustomRole('测试', [], store)
  const again = addCustomRole('测试', loadCustomRoles(store), store)
  assert.equal(again.error, '已有同名角色')
})
