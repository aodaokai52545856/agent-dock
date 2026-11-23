import assert from 'node:assert/strict'
import { antigravityBannerAccount } from './antigravityBanner.ts'

function accountFrom(...lines: string[]) {
  return antigravityBannerAccount(row => lines[row] ?? '', lines.length)
}

assert.deepEqual(
  accountFrom(
    '',
    '  ▄█▄           Antigravity CLI 1.5.14',
    ' ▄█▀█▄          Work.User+agy@example.com (Google AI Pro)',
    '                Gemini 4 Argon (High)'
  ),
  { id: 'work.user+agy@example.com', name: 'Work.User+agy@example.com' }
)

assert.deepEqual(
  accountFrom('Antigravity CLI v1.5.14-preview.1', 'Gemini API key'),
  { id: 'gemini-api-key', name: 'Gemini API key' }
)

assert.equal(accountFrom('Login required', 'someone@example.com'), null)
assert.equal(accountFrom('Antigravity CLI 1.5.14', '', 'someone@example.com'), null)
assert.equal(accountFrom('Antigravity CLI 1.5.14', 'someone@example.com (Google AI'), null)
