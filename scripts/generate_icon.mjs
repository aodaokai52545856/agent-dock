import { spawnSync } from 'node:child_process'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..')
const script = join(root, 'scripts', 'generate_icon.py')
const cmds = process.platform === 'win32' ? ['py', 'python', 'python3'] : ['python3', 'python']

for (const cmd of cmds) {
  const args = cmd === 'py' ? ['-3', script] : [script]
  const result = spawnSync(cmd, args, { cwd: root, stdio: 'inherit' })
  if (result.error?.code === 'ENOENT') continue
  process.exit(result.status ?? 1)
}

console.error('未找到 Python。请安装 Python 3 和 Pillow（pip install pillow）。macOS / Linux 用 python3。')
process.exit(1)
