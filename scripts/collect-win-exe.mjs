import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const script = join(fileURLToPath(new URL('.', import.meta.url)), 'collect-release.mjs')
const result = spawnSync(process.execPath, [script, '--os', 'win'], { stdio: 'inherit' })
process.exit(result.status ?? 1)
