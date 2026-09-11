import assert from 'node:assert/strict'
import { defaultShellPathFor } from './platform.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('default shell is native on each desktop OS', () => {
  assert.equal(defaultShellPathFor('windows'), 'powershell.exe')
  assert.equal(defaultShellPathFor('macos'), '/bin/zsh')
  assert.equal(defaultShellPathFor('linux'), '/bin/bash')
})
