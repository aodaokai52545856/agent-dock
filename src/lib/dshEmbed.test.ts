import assert from 'node:assert/strict'
import {
  DSH_EMBED_EDGE,
  DSH_EMBED_GUTTER,
  DSH_STATUS_BAR,
  clampDshEmbedBounds,
  dshEmbedBlocked,
  toastOverlayTop
} from './dshEmbed.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const pane = { left: 280, top: 80, width: 1000, height: 700 }

test('keeps a window-edge strip so frameless resize still works', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false
  })
  assert.ok(bounds)
  assert.equal(bounds!.x, 280 + DSH_EMBED_GUTTER)
  assert.equal(bounds!.x + bounds!.width, 1280 - DSH_EMBED_EDGE)
  assert.equal(bounds!.y + bounds!.height, 780)
  assert.ok(bounds!.y + bounds!.height <= 840 - DSH_EMBED_EDGE)
})

test('leaves the sidebar gutter uncovered', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false
  })
  assert.ok(bounds)
  assert.ok(bounds!.x > pane.left)
})

test('leaves the doc rail gutter uncovered', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 260,
    maximized: false
  })
  assert.ok(bounds)
  assert.equal(bounds!.width, 1000 - DSH_EMBED_GUTTER - 260 - DSH_EMBED_GUTTER)
})

test('maximized windows do not keep the outer resize strip', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: true
  })
  assert.ok(bounds)
  assert.equal(bounds!.x + bounds!.width, 1280)
})

test('embed stops above the status bar so DeepSeek cannot cover it', () => {
  const tall = { left: 280, top: 40, width: 1000, height: 800 }
  const bounds = clampDshEmbedBounds(tall, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false
  })
  assert.ok(bounds)
  assert.equal(bounds!.y + bounds!.height, 840 - DSH_STATUS_BAR)
  assert.ok(bounds!.y + bounds!.height <= 840 - DSH_STATUS_BAR)
})

test('title-bar menus and dialogs block the native embed', () => {
  function fakeRoot(present: string[]) {
    return {
      querySelector: (selector: string) => {
        const parts = selector.split(',').map((item) => item.trim().replace(/^\./, ''))
        return parts.some((name) => present.includes(name)) ? {} : null
      }
    }
  }
  assert.equal(dshEmbedBlocked(fakeRoot([])), false)
  assert.equal(dshEmbedBlocked(fakeRoot(['ad-menu'])), true)
  assert.equal(dshEmbedBlocked(fakeRoot(['ad-mask'])), true)
  assert.equal(
    dshEmbedBlocked(fakeRoot(['toast'])),
    false,
    'toast should inset the embed, not hide the whole DeepSeek page'
  )
})

test('toast band insets the embed so the native page cannot cover it', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false,
    overlayTop: 92
  })
  assert.ok(bounds)
  assert.equal(bounds!.y, 92)
  assert.equal(bounds!.height, 780 - 92)
  assert.equal(bounds!.y + bounds!.height, 780)
})

test('toast overlay top includes a gutter below the pill', () => {
  assert.equal(toastOverlayTop(null), 0)
  assert.equal(
    toastOverlayTop({ getBoundingClientRect: () => ({ bottom: 86 }) }),
    86 + DSH_EMBED_GUTTER
  )
})
