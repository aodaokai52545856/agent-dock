import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const OPENCC = 'https://raw.githubusercontent.com/BYVoid/OpenCC/ver.1.1.9/data/dictionary'
const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const dir = join(root, 'src', 'lib', 'opencc')

async function loadDict(name) {
  const res = await fetch(`${OPENCC}/${name}`)
  if (!res.ok) throw new Error(`fetch ${name} failed: ${res.status}`)
  const entries = {}
  for (const line of (await res.text()).split(/\r?\n/)) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const [from, to] = trimmed.split('\t')
    if (!from || !to) continue
    entries[from] = to.split(' ')[0]
  }
  return entries
}

function writeTs(file, exportName, entries) {
  writeFileSync(file, `export const ${exportName}: Record<string, string> = ${JSON.stringify(entries)}\n`)
}

const st = await loadDict('STCharacters.txt')
const tw = await loadDict('TWVariants.txt')
if (st['软'] !== '軟' || st['测'] !== '測' || st['试'] !== '試') {
  throw new Error('STCharacters missing expected glyph mappings')
}
if (tw['裏'] !== '裡' || tw['爲'] !== '為' || tw['着'] !== '著') {
  throw new Error('TWVariants missing expected Taiwan glyph mappings')
}
mkdirSync(dir, { recursive: true })
writeTs(join(dir, 'stCharacters.ts'), 'ST_CHARACTERS', st)
writeTs(join(dir, 'twVariants.ts'), 'TW_VARIANTS', tw)
console.log(`wrote ${Object.keys(st).length} ST + ${Object.keys(tw).length} TW mappings`)
