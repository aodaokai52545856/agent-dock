import assert from 'node:assert/strict'
import {
  DSH_KEY_NAME,
  DSH_PLATFORM_URL,
  DSH_WEB_CONFLICT_MARK,
  isDshWebConflict,
  maskSecret,
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

test('frontend never treats masked value as the secret', () => {
  const masked = maskSecret('sk-work-abcdefghijk')
  assert.notEqual(masked, 'sk-work-abcdefghijk')
  assert.ok(!masked.includes('work-abcdef'))
})
