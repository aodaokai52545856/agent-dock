import assert from 'node:assert/strict'
import {
  COLLAPSED_WIDTH,
  DOCRAIL_MIN,
  LAYOUT_VERSION,
  docRailPaneWidth,
  layout,
  migrateStoredLayout,
  resizeDocRail,
  sidebarPaneWidth,
  toggleDocRail
} from './layout.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const snapshot = { ...layout }

function restore() {
  Object.assign(layout, snapshot)
}

test('fresh layout keeps the console full width by collapsing the doc rail', () => {
  assert.equal(snapshot.docRailCollapsed, true)
})

test('collapsed doc rail takes no layout width so the PTY is not squeezed', () => {
  layout.docRailCollapsed = true
  layout.docRailWidth = 260
  try {
    assert.equal(docRailPaneWidth(), 0)
  } finally {
    restore()
  }
})

test('expanded doc rail reports its own width, not the sidebar collapsed strip', () => {
  layout.docRailCollapsed = false
  layout.docRailWidth = 260
  try {
    assert.equal(docRailPaneWidth(), 260)
    assert.notEqual(docRailPaneWidth(), COLLAPSED_WIDTH)
  } finally {
    restore()
  }
})

test('sidebar still uses the 40px collapsed strip', () => {
  layout.sidebarCollapsed = true
  try {
    assert.equal(sidebarPaneWidth(), COLLAPSED_WIDTH)
  } finally {
    restore()
  }
})

test('toggleDocRail only flips collapsed and does not steal a collapsed strip', () => {
  layout.docRailCollapsed = true
  try {
    toggleDocRail()
    assert.equal(layout.docRailCollapsed, false)
    assert.equal(docRailPaneWidth(), layout.docRailWidth)
    toggleDocRail()
    assert.equal(layout.docRailCollapsed, true)
    assert.equal(docRailPaneWidth(), 0)
  } finally {
    restore()
  }
})

test('old flex-column layouts collapse the doc rail once so the console gets its width back', () => {
  const next = migrateStoredLayout({
    sidebarWidth: 280,
    docRailWidth: 260,
    docRailCollapsed: false
  })
  assert.equal(next.docRailCollapsed, true)
  assert.equal(next.layoutVersion, LAYOUT_VERSION)
  assert.equal(next.sidebarWidth, 280)
})

test('layouts already on the overlay version keep an opened doc rail', () => {
  const next = migrateStoredLayout({
    docRailCollapsed: false,
    layoutVersion: LAYOUT_VERSION
  })
  assert.equal(next.docRailCollapsed, false)
})

test('dragging the doc rail below the minimum collapses it to zero width', () => {
  layout.docRailCollapsed = false
  layout.docRailWidth = 260
  try {
    resizeDocRail(DOCRAIL_MIN - 40)
    assert.equal(layout.docRailCollapsed, true)
    assert.equal(docRailPaneWidth(), 0)
  } finally {
    restore()
  }
})
