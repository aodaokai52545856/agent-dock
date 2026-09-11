export type HostOs = 'macos' | 'windows' | 'linux'

export function detectHostOs(): HostOs {
  if (typeof navigator === 'undefined') return 'windows'
  const ua = navigator.userAgent
  if (/Mac|iPhone|iPad/.test(ua) && !/Win/.test(ua)) return 'macos'
  if (/Win/.test(ua)) return 'windows'
  return 'linux'
}

export const hostOs = detectHostOs()
export const isMac = hostOs === 'macos'
export const isWindows = hostOs === 'windows'

export function defaultShellPath(): string {
  if (isWindows) return 'powershell.exe'
  return '/bin/zsh'
}

export function applyHostOsClass() {
  if (typeof document === 'undefined') return
  document.documentElement.classList.toggle('ad-mac', isMac)
  document.documentElement.classList.toggle('ad-windows', isWindows)
  document.documentElement.classList.toggle(
    'ad-desktop-glass',
    typeof window !== 'undefined' && !!(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  )
}
