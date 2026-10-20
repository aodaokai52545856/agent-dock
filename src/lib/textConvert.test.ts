import assert from 'node:assert/strict'
import { CONVERT_CHARSETS, convertReady, convertText, isAllowedOutputChar } from './textConvert.ts'

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

test('glyph tables are ready', () => {
  assert.equal(convertReady(), true)
})

test('charset list defaults to Taiwan', () => {
  assert.equal(CONVERT_CHARSETS[0]?.id, 'tw')
  assert.deepEqual(
    CONVERT_CHARSETS.map((item) => item.id),
    ['tw', 'hk', 'std']
  )
})

test('empty input stays empty', () => {
  assert.equal(convertText(''), '')
  assert.equal(convertText('', 'hk'), '')
})

test('fullwidth letters digits parens become ASCII', () => {
  const out = convertText('ＡＢＣ（１２３）')
  assert.equal(out, 'ABC(123)')
  assertAllowed(out)
})

test('CJK punctuation becomes ASCII and Han becomes Taiwan glyphs', () => {
  const out = convertText('软件（测试），。')
  assert.equal(out, '軟件(測試),.')
  assertAllowed(out)
})

test('emoji yen zwsp and latin-1 are deleted', () => {
  const out = convertText('软件😀￥\u200Bé，')
  assert.equal(out, '軟件,')
  assertAllowed(out)
})

test('ideographic comma ellipsis and quotes map', () => {
  const out = convertText('说\uFF1A\u300C你好\u300D\u2026\u300A书\u300B')
  assert.equal(out, '說:[你好]...<<書>>')
  assertAllowed(out)
})

test('every output char of mixed sample is allowed', () => {
  const out = convertText('（Hello，世界！）')
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
      const out = convertText(ch)
      assertAllowed(out)
      assertNoBlockChar(out, cp)
    }
  }
})

test('default charset is Taiwan', () => {
  const input = '软件（测试），。'
  assert.equal(convertText(input), convertText(input, 'tw'))
  assert.equal(convertText(input, 'tw'), '軟件(測試),.')
  assertAllowed(convertText(input, 'tw'))
})

test('Taiwan glyphs not Hong Kong orthodox', () => {
  const out = convertText('里面为伪启群床着', 'tw')
  assert.equal(out, '裡面為偽啟群床著')
  assertAllowed(out)
})

test('already traditional HK forms become Taiwan glyphs', () => {
  const out = convertText('裏面爲僞啓羣牀', 'tw')
  assert.equal(out, '裡面為偽啟群床')
  assertAllowed(out)
})

test('standard charset keeps OpenCC orthodox glyphs', () => {
  const out = convertText('里面为伪启群床着', 'std')
  assert.equal(out, '裏面爲僞啓羣牀着')
  assertAllowed(out)
})

test('Hong Kong charset uses HK variants', () => {
  const out = convertText('卫生说里面为伪启', 'hk')
  assert.equal(out, '衞生説裏面為偽啓')
  assertAllowed(out)
})

test('Taiwan phrases are not applied', () => {
  const out = convertText('软件信息', 'tw')
  assert.equal(out, '軟件信息')
  assert.notEqual(out, '軟體資訊')
  assertAllowed(out)
})
