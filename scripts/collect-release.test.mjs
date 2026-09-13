import assert from 'node:assert/strict'
import { join } from 'node:path'
import {
  appVersion,
  detectPackOs,
  plannedCopies,
  releaseFileName,
  requiredHost,
  tauriBundles
} from './collect-release.mjs'

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

test('pack version comes from package.json', () => {
  assert.equal(appVersion({ version: '0.1.0' }), '0.1.0')
  assert.equal(appVersion({ version: '1.2.3' }), '1.2.3')
  assert.throws(() => appVersion({ version: '' }), /版本号/)
})

test('release file names include the app version', () => {
  assert.equal(releaseFileName('win-exe', '0.1.0'), 'AgentDock-0.1.0.exe')
  assert.equal(releaseFileName('win-setup', '0.1.0'), 'AgentDock-0.1.0-Setup.exe')
  assert.equal(releaseFileName('mac-app', '1.2.3'), 'Agent Dock.app')
  assert.equal(releaseFileName('mac-dmg', '1.2.3'), 'AgentDock-1.2.3.dmg')
  assert.equal(releaseFileName('linux-appimage', '0.2.0'), 'AgentDock-0.2.0.AppImage')
  assert.equal(releaseFileName('linux-deb', '0.2.0'), 'AgentDock-0.2.0.deb')
})

test('windows plan copies portable exe and nsis setup', () => {
  const plan = plannedCopies('win', join('target', 'release'), '0.1.0')
  assert.equal(plan[0].to, 'AgentDock-0.1.0.exe')
  assert.equal(plan[0].required, true)
  assert.equal(plan[1].to, 'AgentDock-0.1.0-Setup.exe')
})

test('mac plan copies app bundle and dmg', () => {
  const plan = plannedCopies('mac', join('target', 'release'), '1.4.0')
  assert.equal(plan[0].to, 'Agent Dock.app')
  assert.equal(plan[1].to, 'AgentDock-1.4.0.dmg')
})

test('linux plan copies AppImage and deb', () => {
  const plan = plannedCopies('linux', join('target', 'release'), '0.3.1')
  assert.equal(plan[0].to, 'AgentDock-0.3.1.AppImage')
  assert.equal(plan[1].to, 'AgentDock-0.3.1.deb')
})
