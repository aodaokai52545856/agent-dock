import type { SessionAccount } from './types'

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
