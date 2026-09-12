import assert from 'node:assert/strict'
import { isTitlebarDoubleClick } from './titleDrag.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('second left click on the caption toggles maximize', () => {
  assert.equal(isTitlebarDoubleClick({ button: 0, detail: 2 }), true)
  assert.equal(isTitlebarDoubleClick({ button: 0, detail: 3 }), true)
})

test('first click and other buttons keep dragging', () => {
  assert.equal(isTitlebarDoubleClick({ button: 0, detail: 1 }), false)
  assert.equal(isTitlebarDoubleClick({ button: 0, detail: 0 }), false)
  assert.equal(isTitlebarDoubleClick({ button: 2, detail: 2 }), false)
})
