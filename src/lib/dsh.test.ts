import assert from 'node:assert/strict'
import {
  DSH_FIXED_SESSION_ID,
  DSH_FIXED_SESSION_TITLE,
  DSH_KEY_NAME,
  DSH_LIVE_KEY_ID,
  DSH_PLATFORM_URL,
  DSH_WEB_CONFLICT_MARK,
  displayDshKeys,
  isDshWebConflict,
  isFixedDshSession,
  isManagedDshKey,
  maskSecret,
  showDshKeyEmpty,
  sourceLabel
} from './dsh.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('key name is DEEPSEEK_API_KEY', () => {
  assert.equal(DSH_KEY_NAME, 'DEEPSEEK_API_KEY')
  assert.ok(DSH_PLATFORM_URL.startsWith('https://'))
})

test('source labels match harness precedence', () => {
  assert.equal(sourceLabel('env'), '启动环境变量')
  assert.equal(sourceLabel('file'), 'Harness 凭据文件')
  assert.equal(sourceLabel('none'), '未配置')
})

test('maskSecret matches rust head-tail form', () => {
  assert.equal(maskSecret('sk-abcdefghijklmnop'), 'sk-a…mnop')
  assert.equal(maskSecret('short'), '••••')
  assert.equal(maskSecret('  '), '')
})

test('conflict toast is recognized and does not look like a missing key', () => {
  const foreign = `${DSH_WEB_CONFLICT_MARK}：系统里已有你自己启动的 dsh web（PID 20）。请先在那个窗口或终端里关掉它，再从 Agent Dock 打开。Agent Dock 不会结束你自己开的进程。`
  assert.equal(isDshWebConflict(foreign), true)
  assert.equal(isDshWebConflict('dsh web 启动超时，没有给出页面地址。'), false)
  assert.ok(!foreign.includes('API Key'))
  assert.ok(foreign.includes('不会结束你自己开的进程'))
})

test('each project has one fixed DeepSeek session that cannot be created or deleted', () => {
  assert.equal(DSH_FIXED_SESSION_ID, 'deepseek')
  assert.equal(DSH_FIXED_SESSION_TITLE, 'deepseek')
  assert.equal(isFixedDshSession('dsh', 'deepseek'), true)
  assert.equal(isFixedDshSession('dsh', 'session-abc'), false)
  assert.equal(isFixedDshSession('grokbuild', 'deepseek'), false)
})

test('frontend never treats masked value as the secret', () => {
  const masked = maskSecret('sk-work-abcdefghijk')
  assert.notEqual(masked, 'sk-work-abcdefghijk')
  assert.ok(!masked.includes('work-abcdef'))
})

const idleStatus = {
  configured: false,
  writable: true,
  source: 'none',
  masked: '',
  dshHome: '~/.dsh',
  credentialsPath: '~/.dsh/.credentials.yaml',
  envBlocks: false
}

test('display keys fall back to live harness key when vault list is empty but configured', () => {
  const rows = displayDshKeys({
    status: { ...idleStatus, configured: true, source: 'file', masked: 'sk-5…6c44' },
    keys: [],
    vaultError: '无法用 Windows 用户凭据解密 Key。请确认是同一台电脑、同一个 Windows 用户。'
  })
  assert.equal(rows.length, 1)
  assert.equal(rows[0].id, DSH_LIVE_KEY_ID)
  assert.equal(rows[0].name, '当前正在使用')
  assert.equal(rows[0].masked, 'sk-5…6c44')
  assert.equal(rows[0].active, true)
  assert.equal(rows[0].managed, false)
  assert.equal(isManagedDshKey(rows[0]), false)
})

test('empty copy is hidden when a live key or vault error exists', () => {
  const live = displayDshKeys({
    status: { ...idleStatus, configured: true, source: 'file', masked: 'sk-5…6c44' },
    keys: []
  })
  assert.equal(showDshKeyEmpty(live, ''), false)
  assert.equal(showDshKeyEmpty([], '无法用 Windows 用户凭据解密 Key。'), false)
  assert.equal(showDshKeyEmpty([], ''), true)
})

test('vault keys stay as-is and remain managed', () => {
  const work = {
    id: 'a',
    name: '工作号',
    masked: 'sk-ab…wxyz',
    updatedAt: '',
    active: true,
    managed: true
  }
  const rows = displayDshKeys({
    status: { ...idleStatus, configured: true, source: 'file', masked: 'sk-ab…wxyz' },
    keys: [work]
  })
  assert.deepEqual(rows, [work])
  assert.equal(isManagedDshKey(work), true)
})
