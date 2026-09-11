import assert from 'node:assert/strict'
import { canAdvanceSlice, combineVerdicts, parseReviewVerdict } from './reviewVerdict.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('empty text is unknown', () => {
  assert.equal(parseReviewVerdict(''), 'unknown')
  assert.equal(parseReviewVerdict('   '), 'unknown')
})

test('explicit PASS/FAIL tokens win', () => {
  assert.equal(parseReviewVerdict('Looks fine.\nVERDICT: PASS'), 'pass')
  assert.equal(parseReviewVerdict('Several issues.\nVERDICT: FAIL'), 'fail')
  assert.equal(parseReviewVerdict('REVIEW: PASS\nNice work.'), 'pass')
})

test('Chinese gate words', () => {
  assert.equal(parseReviewVerdict('闸门：通过\n可以进入下一片'), 'pass')
  assert.equal(parseReviewVerdict('审查：未通过\n请补测试'), 'fail')
})

test('FAIL beats PASS if both appear', () => {
  assert.equal(parseReviewVerdict('Would pass locally.\nVERDICT: FAIL'), 'fail')
})

test('prose without a ticket stays unknown', () => {
  assert.equal(parseReviewVerdict('Looks solid overall, a few nits in the sidebar.'), 'unknown')
})

test('combine verdicts ANDs the gate', () => {
  assert.equal(combineVerdicts(['pass', 'pass']), 'pass')
  assert.equal(combineVerdicts(['pass', 'unknown']), 'unknown')
  assert.equal(combineVerdicts(['pass', 'fail']), 'fail')
  assert.equal(combineVerdicts([]), 'unknown')
})

test('next slice stays locked until passed', () => {
  assert.equal(canAdvanceSlice('passed'), true)
  assert.equal(canAdvanceSlice('failed'), false)
  assert.equal(canAdvanceSlice('unknown'), false)
  assert.equal(canAdvanceSlice('reviewing'), false)
  assert.equal(canAdvanceSlice('idle'), false)
})
