import { spawnSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { detectPackOs, readAppVersion, requiredHost, tauriBundles } from './collect-release.mjs'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..')
const argv = process.argv.slice(2)
const osIdx = argv.indexOf('--os')
const os = osIdx >= 0 ? argv[osIdx + 1] : detectPackOs()

if (!['win', 'mac', 'linux'].includes(os)) {
  console.error(`未知系统：${os}。请用 npm run pack:win|pack:mac|pack:linux`)
  process.exit(1)
}

if (process.platform !== requiredHost(os)) {
  const where = { win: 'Windows', mac: 'macOS', linux: 'Linux' }
  console.error(`${where[os]} 安装包必须在 ${where[os]} 上打包。当前是 ${process.platform}。`)
  console.error('在对应系统执行同一条 npm run pack:*，或用 GitHub Actions 的 release workflow。')
  process.exit(1)
}

const version = readAppVersion(root)
console.log(`打包 Agent Dock v${version}（${os}）`)
const overlayDir = mkdtempSync(join(tmpdir(), 'agent-dock-pack-'))
const overlay = join(overlayDir, 'version.json')
writeFileSync(overlay, JSON.stringify({ version }))
const tauri = spawnSync(
  'npx',
  ['tauri', 'build', '--bundles', tauriBundles(os), '--config', overlay],
  { cwd: root, stdio: 'inherit', shell: true }
)
if (tauri.status) process.exit(tauri.status)

const collect = spawnSync(process.execPath, [join(root, 'scripts', 'collect-release.mjs'), '--os', os], {
  cwd: root,
  stdio: 'inherit'
})
process.exit(collect.status ?? 1)
