import { ST_CHARACTERS } from './opencc/stCharacters.ts'

export type ConvertMode = 'traditional' | 'symbols' | 'all'

export function convertReady(): boolean {
  return ST_CHARACTERS['软'] === '軟'
}

const HAN_RANGES: Array<[number, number]> = [
  [0x3400, 0x4dbf],
  [0x4e00, 0x9fff],
  [0xf900, 0xfaff],
  [0x20000, 0x2a6df],
  [0x2a700, 0x2b73f],
  [0x2b740, 0x2b81f],
  [0x2b820, 0x2ceaf],
  [0x2ceb0, 0x2ebef],
  [0x2ebf0, 0x2ee5f],
  [0x2f800, 0x2fa1f],
  [0x30000, 0x3134f],
  [0x31350, 0x323af]
]

const PUNCT: Record<number, string> = {
  0x3000: ' ',
  0x3001: ',',
  0x3002: '.',
  0x3008: '<',
  0x3009: '>',
  0x300a: '<<',
  0x300b: '>>',
  0x300c: '[',
  0x300d: ']',
  0x300e: '[',
  0x300f: ']',
  0x3010: '[',
  0x3011: ']',
  0x3014: '[',
  0x3015: ']',
  0x3016: '[',
  0x3017: ']',
  0x3018: '[',
  0x3019: ']',
  0x301a: '[',
  0x301b: ']',
  0x301d: '"',
  0x301e: '"',
  0x201c: '"',
  0x201d: '"',
  0x2018: "'",
  0x2019: "'",
  0x2026: '...',
  0x22ef: '...',
  0x2014: '-',
  0x2013: '-',
  0x2015: '-',
  0x00b7: '.',
  0x30fb: '.',
  0xff65: '.',
  0x00a0: ' ',
  0x00ab: '"',
  0x00bb: '"'
}

function isAllowedAscii(cp: number): boolean {
  return cp === 0x09 || cp === 0x0a || cp === 0x0d || (cp >= 0x20 && cp <= 0x7e)
}

function isHan(cp: number): boolean {
  for (const [lo, hi] of HAN_RANGES) {
    if (cp >= lo && cp <= hi) return true
  }
  return false
}

export function isAllowedOutputChar(ch: string): boolean {
  const cp = ch.codePointAt(0)
  if (cp === undefined) return false
  return isAllowedAscii(cp) || isHan(cp)
}

function mapCodePoint(cp: number): string | null {
  if (cp >= 0xff01 && cp <= 0xff5e) return String.fromCodePoint(cp - 0xfee0)
  return PUNCT[cp] ?? null
}

function toTraditional(input: string): string {
  let out = ''
  for (const ch of input) {
    out += ST_CHARACTERS[ch] ?? ch
  }
  return out
}

function classify(input: string): string {
  let out = ''
  for (const ch of input) {
    const cp = ch.codePointAt(0)
    if (cp === undefined) continue
    if (isAllowedAscii(cp) || isHan(cp)) {
      out += ch
      continue
    }
    const mapped = mapCodePoint(cp)
    if (mapped !== null) out += mapped
  }
  return out
}

export function convertText(input: string, mode: ConvertMode): string {
  if (!input) return ''
  const source = mode === 'symbols' ? input : toTraditional(input)
  return classify(source)
}
