import assert from 'node:assert/strict'
import { CCSWITCH_HOMEPAGE, CCSWITCH_RELEASES, formatBytes, formatProgress } from './ccswitch.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('official urls are https', () => {
  assert.ok(CCSWITCH_HOMEPAGE.startsWith('https://'))
  assert.ok(CCSWITCH_RELEASES.startsWith('https://'))
  assert.ok(CCSWITCH_RELEASES.includes('farion1231/cc-switch'))
})

test('formatBytes matches portable zip size label', () => {
  assert.equal(formatBytes(13_658_645), '13.0 MB')
  assert.equal(formatBytes(800), '800 B')
  assert.equal(formatBytes(-1), '—')
})

test('formatProgress shows percent when total is known', () => {
  assert.equal(formatProgress(5_242_880, 10_485_760), '5.0 MB / 10.0 MB · 50%')
  assert.equal(formatProgress(512, 0), '512 B')
})
