import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const SRC =
  'https://raw.githubusercontent.com/BYVoid/OpenCC/ver.1.1.9/data/dictionary/STCharacters.txt'
const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outFile = join(root, 'src', 'lib', 'opencc', 'stCharacters.ts')

const res = await fetch(SRC)
if (!res.ok) throw new Error(`fetch STCharacters failed: ${res.status}`)
const text = await res.text()
const entries = {}
for (const line of text.split(/\r?\n/)) {
  const trimmed = line.trim()
  if (!trimmed || trimmed.startsWith('#')) continue
  const [from, to] = trimmed.split('\t')
  if (!from || !to) continue
  entries[from] = to.split(' ')[0]
}
if (entries['软'] !== '軟' || entries['测'] !== '測' || entries['试'] !== '試') {
  throw new Error('STCharacters missing expected glyph mappings')
}
mkdirSync(dirname(outFile), { recursive: true })
const body =
  'export const ST_CHARACTERS: Record<string, string> = ' +
  JSON.stringify(entries) +
  '\n'
writeFileSync(outFile, body)
console.log(`wrote ${Object.keys(entries).length} mappings to ${outFile}`)
