import { cpSync, existsSync, mkdirSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

export function detectPackOs(platform = process.platform) {
  if (platform === 'win32') return 'win'
  if (platform === 'darwin') return 'mac'
  return 'linux'
}

export function tauriBundles(os) {
  if (os === 'win') return 'nsis'
  if (os === 'mac') return 'app,dmg'
  return 'appimage,deb'
}

export function requiredHost(os) {
  if (os === 'win') return 'win32'
  if (os === 'mac') return 'darwin'
  return 'linux'
}

export function cargoReleaseDir(root, env = process.env) {
  return env.CARGO_TARGET_DIR
    ? join(env.CARGO_TARGET_DIR, 'release')
    : join(root, 'src-tauri', 'target', 'release')
}

function firstMatch(dir, test) {
  if (!existsSync(dir)) return null
  const hit = readdirSync(dir).find((name) => test(name))
  return hit ? join(dir, hit) : null
}

export function plannedCopies(os, cargoTarget) {
  const bundle = join(cargoTarget, 'bundle')
  if (os === 'win') {
    return [
      { from: join(cargoTarget, 'agent-dock.exe'), to: 'AgentDock.exe', required: true },
      {
        from: firstMatch(join(bundle, 'nsis'), (name) => name.toLowerCase().endsWith('-setup.exe')),
        to: 'AgentDock-Setup.exe',
        required: false
      }
    ]
  }
  if (os === 'mac') {
    return [
      {
        from: firstMatch(join(bundle, 'macos'), (name) => name.endsWith('.app')),
        to: 'Agent Dock.app',
        required: true
      },
      {
        from: firstMatch(join(bundle, 'dmg'), (name) => name.toLowerCase().endsWith('.dmg')),
        to: 'AgentDock.dmg',
        required: false
      }
    ]
  }
  return [
    {
      from: firstMatch(join(bundle, 'appimage'), (name) => name.toLowerCase().endsWith('.appimage')),
      to: 'AgentDock.AppImage',
      required: true
    },
    {
      from: firstMatch(join(bundle, 'deb'), (name) => name.toLowerCase().endsWith('.deb')),
      to: 'AgentDock.deb',
      required: false
    }
  ]
}

export function copyReleaseFiles(os, cargoTarget, releaseDir) {
  mkdirSync(releaseDir, { recursive: true })
  const copied = []
  for (const item of plannedCopies(os, cargoTarget)) {
    if (!item.from || !existsSync(item.from)) {
      if (item.required) {
        throw new Error(`没有找到 ${item.to}，请先在对应系统上执行 npm run pack:${os}。`)
      }
      continue
    }
    const dest = join(releaseDir, item.to)
    cpSync(item.from, dest, { recursive: true })
    copied.push(item.to)
  }
  return copied
}

function parseOs(argv) {
  const idx = argv.indexOf('--os')
  if (idx >= 0 && argv[idx + 1]) return argv[idx + 1]
  return detectPackOs()
}

const isMain = Boolean(process.argv[1]) && import.meta.url === pathToFileURL(process.argv[1]).href
if (isMain) {
  const os = parseOs(process.argv.slice(2))
  if (!['win', 'mac', 'linux'].includes(os)) {
    console.error(`未知系统：${os}。请用 --os win|mac|linux`)
    process.exit(1)
  }
  const root = join(fileURLToPath(new URL('.', import.meta.url)), '..')
  const cargoTarget = cargoReleaseDir(root)
  const releaseDir = join(root, 'release')
  try {
    const copied = copyReleaseFiles(os, cargoTarget, releaseDir)
    console.log('已输出到 release/')
    for (const name of copied) console.log(`  ${name}`)
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err))
    process.exit(1)
  }
}
