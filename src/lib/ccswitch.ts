export const CCSWITCH_HOMEPAGE = 'https://ccswitch.io'
export const CCSWITCH_RELEASES = 'https://github.com/farion1231/cc-switch/releases'

export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—'
  const kb = 1024
  const mb = kb * 1024
  const gb = mb * 1024
  if (value >= gb) return `${(value / gb).toFixed(1)} GB`
  if (value >= mb) return `${(value / mb).toFixed(1)} MB`
  if (value >= kb) return `${Math.round(value / kb)} KB`
  return `${Math.round(value)} B`
}

export function formatProgress(received: number, total: number): string {
  if (total > 0) {
    const pct = Math.min(100, Math.round((received / total) * 100))
    return `${formatBytes(received)} / ${formatBytes(total)} · ${pct}%`
  }
  return received > 0 ? formatBytes(received) : ''
}
