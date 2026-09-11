import { copyFileSync, existsSync, mkdirSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..')
const releaseDir = join(root, 'release')
const cargoTarget = process.env.CARGO_TARGET_DIR
  ? join(process.env.CARGO_TARGET_DIR, 'release')
  : join(root, 'src-tauri', 'target', 'release')
const exe = join(cargoTarget, 'agent-dock.exe')
const nsisDir = join(cargoTarget, 'bundle', 'nsis')

if (!existsSync(exe)) {
  console.error(`没有找到 ${exe}，请先在 Windows 上执行 npm run pack:exe。`)
  process.exit(1)
}

mkdirSync(releaseDir, { recursive: true })
copyFileSync(exe, join(releaseDir, 'AgentDock.exe'))

const setups = existsSync(nsisDir)
  ? readdirSync(nsisDir).filter((name) => name.toLowerCase().endsWith('-setup.exe'))
  : []
if (setups.length) {
  copyFileSync(join(nsisDir, setups[0]), join(releaseDir, 'AgentDock-Setup.exe'))
}

console.log('已输出到 release/')
console.log('  AgentDock.exe        可直接双击（Win10/11 需已有 WebView2）')
if (setups.length) {
  console.log('  AgentDock-Setup.exe  安装包，当前用户安装，不需管理员')
}
console.log('打包后的 exe 不占用 1421 端口；1421 只给 npm run tauri:dev 用。')
