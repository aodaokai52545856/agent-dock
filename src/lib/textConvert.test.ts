import assert from 'node:assert/strict'
import { convertText, isAllowedOutputChar, type ConvertMode } from './textConvert.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function assertAllowed(out: string) {
  for (const ch of out) {
    const cp = ch.codePointAt(0)!.toString(16)
    assert.equal(isAllowedOutputChar(ch), true, `leaked U+${cp}`)
  }
}

function assertNoBlockChar(out: string, cp: number) {
  const ch = String.fromCodePoint(cp)
  assert.ok(!out.includes(ch), `block char leaked U+${cp.toString(16)}`)
}

test('empty input stays empty', () => {
  assert.equal(convertText('', 'symbols'), '')
  assert.equal(convertText('', 'all'), '')
})

test('fullwidth letters digits parens become ASCII', () => {
  const out = convertText('ＡＢＣ（１２３）', 'symbols')
  assert.equal(out, 'ABC(123)')
  assertAllowed(out)
})

test('CJK punctuation becomes ASCII and simplified Han stays', () => {
  const out = convertText('软件（测试），。', 'symbols')
  assert.equal(out, '软件(测试),.')
  assertAllowed(out)
})

test('emoji yen zwsp and latin-1 are deleted', () => {
  const out = convertText('软件😀￥\u200Bé，', 'symbols')
  assert.equal(out, '软件,')
  assertAllowed(out)
})

test('ideographic comma ellipsis and quotes map', () => {
  const out = convertText('说\uFF1A\u300C你好\u300D\u2026\u300A书\u300B', 'symbols')
  assert.equal(out, '说:[你好]...<<书>>')
  assertAllowed(out)
})

test('every output char of mixed sample is allowed', () => {
  const out = convertText('（Hello，世界！）', 'symbols')
  assert.equal(out, '(Hello,世界!)')
  assertAllowed(out)
})

test('fullwidth and CJK punctuation blocks never leak original chars', () => {
  const ranges: Array<[number, number]> = [
    [0xff00, 0xffef],
    [0x3000, 0x303f]
  ]
  for (const [from, to] of ranges) {
    for (let cp = from; cp <= to; cp++) {
      const ch = String.fromCodePoint(cp)
      const out = convertText(ch, 'symbols')
      assertAllowed(out)
      assertNoBlockChar(out, cp)
    }
  }
})

test('traditional and all match in v1 once classifier-only', () => {
  const input = '软件（测试），。😀'
  const modes: ConvertMode[] = ['traditional', 'all']
  const a = convertText(input, modes[0])
  const b = convertText(input, modes[1])
  assert.equal(a, b)
  assertAllowed(a)
})

test('all converts glyphs and punctuation', () => {
  const out = convertText('软件（测试），。', 'all')
  assert.equal(out, '軟件(測試),.')
  assertAllowed(out)
})

test('traditional matches all', () => {
  const input = '软件（测试），。😀￥'
  assert.equal(convertText(input, 'traditional'), convertText(input, 'all'))
})

test('symbols keeps simplified Han', () => {
  const out = convertText('软件，', 'symbols')
  assert.equal(out, '软件,')
  assertAllowed(out)
})
