export type ReviewVerdict = 'pass' | 'fail' | 'unknown'

function windowText(text: string) {
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
  const head = lines.slice(0, 10)
  const tail = lines.slice(-10)
  return [...head, ...tail].join('\n')
}

export function parseReviewVerdict(text: string): ReviewVerdict {
  const raw = text.trim()
  if (!raw) return 'unknown'
  const sample = windowText(raw)

  const fail =
    /\b(FAIL|FAILED|VERDICT\s*[:：]?\s*FAIL|REVIEW\s*[:：]?\s*FAIL)\b/i.test(sample) ||
    /闸门?\s*[:：]?\s*未通过/.test(sample) ||
    /审查\s*[:：]?\s*未通过/.test(sample) ||
    /(^|\n)未通过(\n|$)/.test(sample)
  const pass =
    /\b(PASS|PASSED|VERDICT\s*[:：]?\s*PASS|REVIEW\s*[:：]?\s*PASS)\b/i.test(sample) ||
    /闸门?\s*[:：]?\s*通过/.test(sample) ||
    /审查\s*[:：]?\s*通过/.test(sample) ||
    /(^|\n)通过(\n|$)/.test(sample)

  if (fail) return 'fail'
  if (pass) return 'pass'
  return 'unknown'
}

export function combineVerdicts(verdicts: ReviewVerdict[]): ReviewVerdict {
  if (!verdicts.length) return 'unknown'
  if (verdicts.some((item) => item === 'fail')) return 'fail'
  if (verdicts.every((item) => item === 'pass')) return 'pass'
  return 'unknown'
}

export function canAdvanceSlice(gate: string) {
  return gate === 'passed'
}
