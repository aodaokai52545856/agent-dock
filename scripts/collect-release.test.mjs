import assert from 'node:assert/strict'
import { join } from 'node:path'
import { detectPackOs, plannedCopies, requiredHost, tauriBundles } from './collect-release.mjs'

function test(name, fn) {
  fn()
  console.log(`ok ${name}`)
}

test('detects pack os from process.platform', () => {
  assert.equal(detectPackOs('win32'), 'win')
  assert.equal(detectPackOs('darwin'), 'mac')
  assert.equal(detectPackOs('linux'), 'linux')
})

test('tauri bundle ids match each OS installer', () => {
  assert.equal(tauriBundles('win'), 'nsis')
  assert.equal(tauriBundles('mac'), 'app,dmg')
  assert.equal(tauriBundles('linux'), 'appimage,deb')
})

test('pack scripts refuse to cross-compile from the wrong host', () => {
  assert.equal(requiredHost('win'), 'win32')
  assert.equal(requiredHost('mac'), 'darwin')
  assert.equal(requiredHost('linux'), 'linux')
})

test('windows plan copies portable exe and nsis setup', () => {
  const plan = plannedCopies('win', join('target', 'release'))
  assert.equal(plan[0].to, 'AgentDock.exe')
  assert.equal(plan[0].required, true)
  assert.equal(plan[1].to, 'AgentDock-Setup.exe')
})

test('mac plan copies app bundle and dmg', () => {
  const plan = plannedCopies('mac', join('target', 'release'))
  assert.equal(plan[0].to, 'Agent Dock.app')
  assert.equal(plan[1].to, 'AgentDock.dmg')
})

test('linux plan copies AppImage and deb', () => {
  const plan = plannedCopies('linux', join('target', 'release'))
  assert.equal(plan[0].to, 'AgentDock.AppImage')
  assert.equal(plan[1].to, 'AgentDock.deb')
})
