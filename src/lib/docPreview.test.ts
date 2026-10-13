import assert from 'node:assert/strict'
import {
  DOC_PREVIEW_DEFAULT_H,
  DOC_PREVIEW_DEFAULT_W,
  clampDocPreviewSize,
  nextDocPreviewSize,
  parseDocPreviewSize
} from './docPreview.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('default size is wider than the old 880 dialog', () => {
  assert.equal(DOC_PREVIEW_DEFAULT_W >= 1100, true)
  assert.equal(DOC_PREVIEW_DEFAULT_H >= 640, true)
  const size = clampDocPreviewSize(DOC_PREVIEW_DEFAULT_W, DOC_PREVIEW_DEFAULT_H, 1440, 900)
  assert.equal(size.w, DOC_PREVIEW_DEFAULT_W)
  assert.equal(size.h, DOC_PREVIEW_DEFAULT_H)
})

test('clamps to the viewport with a 48px gutter', () => {
  const size = clampDocPreviewSize(2000, 2000, 1000, 700)
  assert.equal(size.w, 952)
  assert.equal(size.h, 652)
})

test('tiny windows shrink below the usual minimum', () => {
  const size = clampDocPreviewSize(1120, 720, 400, 300)
  assert.equal(size.w, 352)
  assert.equal(size.h, 252)
})

test('parses stored size and ignores junk', () => {
  assert.equal(parseDocPreviewSize('{"w":980,"h":640}', 1400, 900).w, 980)
  assert.equal(parseDocPreviewSize('nope', 1400, 900).w, DOC_PREVIEW_DEFAULT_W)
  assert.equal(parseDocPreviewSize(null, 1400, 900).h, DOC_PREVIEW_DEFAULT_H)
})

test('se and w edges grow and shrink from the drag delta', () => {
  const start = { w: 1000, h: 600 }
  assert.deepEqual(nextDocPreviewSize(start, 'se', 40, 20, 1600, 1000), { w: 1040, h: 620 })
  assert.deepEqual(nextDocPreviewSize(start, 'w', 80, 0, 1600, 1000), { w: 920, h: 600 })
})
