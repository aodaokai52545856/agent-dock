import assert from 'node:assert/strict'
import { claudeStatus, canUseClaudeAccount, proxyLabel } from './claudeAccounts.ts'

assert.equal(canUseClaudeAccount({ status: 'ready' }), true)
assert.equal(canUseClaudeAccount({ status: 'refreshable' }), true)
for (const status of ['signedOut', 'legacy', 'broken', 'conflict', 'unverified'] as const) {
  assert.equal(canUseClaudeAccount({ status }), false, `${status} must not become the default account`)
  assert.ok(claudeStatus(status))
}
assert.equal(proxyLabel('off'), '直连')
assert.equal(proxyLabel('inherit'), '跟随项目 / 应用代理')
assert.equal(proxyLabel('http://127.0.0.1:7901'), 'http://127.0.0.1:7901')
console.log('ok Claude login states and per-account proxy labels')
