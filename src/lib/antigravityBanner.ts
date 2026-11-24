import type { SessionAccount } from './types'
import type { SessionViewStorage } from './sessionViewMemory'

const versionLabel = /\bAntigravity CLI v?\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/
const emailLabel = /^([\w.+%'-]+@[\w-]+(?:\.[\w-]+)+)(?:\s+\([^()]+\))?$/i

export function antigravityBannerAccount(readLine: (row: number) => string, rows: number): SessionAccount | null {
  for (let row = 0; row < rows - 1; row++) {
    const banner = versionLabel.exec(readLine(row).trimEnd())
    if (!banner) continue

    const label = readLine(row + 1).slice(banner.index).trim()
    if (label === 'Gemini API key') return { id: 'gemini-api-key', name: label }
    const email = emailLabel.exec(label)?.[1]
    if (email) return { id: email.toLowerCase(), name: email }
  }
  return null
}

// agy's history doesn't identify the account. Keep only identities actually seen in its banner.
export function rememberConversationAccount(storage: SessionViewStorage | null, id: string, account: SessionAccount) {
  try { storage?.setItem(`ad-agy-account:${id}`, JSON.stringify(account)) } catch { /* storage unavailable */ }
}

export function rememberedConversationAccount(storage: SessionViewStorage | null, id: string): SessionAccount | null {
  try {
    const value = JSON.parse(storage?.getItem(`ad-agy-account:${id}`) ?? 'null')
    return typeof value?.id === 'string' && typeof value?.name === 'string' ? value : null
  } catch { return null }
}
