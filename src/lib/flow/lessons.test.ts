import assert from 'node:assert/strict'
import test from 'node:test'
import {
  addIndexLine,
  entryStem,
  entryText,
  indexLine,
  lessonRequest,
  parseLessons,
  preventionPaths,
  validateLessonText
} from './lessons.ts'
import type { LessonDraft } from './types.ts'

const FULL = `## 可复用经验

### PTY 写入后要等回合
范围：src/lib/flow

#### 症状
节点提前结束
#### 根因
没有等回合
#### 修复
轮询会话
#### 防复发措施
在 \`src/lib/flow/adapters.ts\` 里统一等待
#### 复发检测
跑 \`src/lib/flow/ptyWait.test.ts\`
`

function draft(patch: Partial<LessonDraft> = {}): LessonDraft {
  return {
    id: 'dev-2.md#0', file: 'dev-2.md', title: '标题', scope: '流程', body: '### 标题\n内容',
    missingSections: [], missingPaths: [], ...patch
  }
}

test('an entry keeps its text verbatim and reads its scope', () => {
  const { entries, notApplicable } = parseLessons(`前言\n\n${FULL}\n## 结论\nPASS\n`)
  assert.equal(notApplicable, false)
  assert.equal(entries.length, 1)
  assert.equal(entries[0].title, 'PTY 写入后要等回合')
  assert.equal(entries[0].scope, 'src/lib/flow')
  assert.deepEqual(entries[0].missingSections, [])
  assert.ok(entries[0].body.startsWith('### PTY 写入后要等回合\n范围：src/lib/flow'))
  assert.doesNotMatch(entries[0].body, /结论/)
})

test('several entries split on their headings', () => {
  const { entries } = parseLessons('## 可复用经验\n### 一\n正文\n### 二\nscope: docs\n')
  assert.deepEqual(entries.map((item) => [item.title, item.scope]), [['一', '通用'], ['二', 'docs']])
})

test('missing and empty sections are listed, not rejected', () => {
  const { entries } = parseLessons('## 可复用经验\n### 一\n#### 症状\n有\n#### 根因\n\n#### 修复\n修了\n')
  assert.deepEqual(entries[0].missingSections, ['根因', '防复发措施', '复发检测'])
})

test('N/A needs a reason', () => {
  assert.equal(parseLessons('## 可复用经验\nN/A：一次性配置问题\n').notApplicable, true)
  assert.equal(parseLessons('## 可复用经验\nn/a: one off\n').notApplicable, true)
  assert.equal(parseLessons('## 可复用经验\nN/A\n').notApplicable, false)
  assert.equal(parseLessons('## 可复用经验\n### 一\nN/A：不算\n').notApplicable, false)
})

test('the rule decides whether N/A is enough', () => {
  const skip = '## 可复用经验\nN/A：没有机制问题\n'
  assert.equal(validateLessonText('a.md', skip, 'entries-or-na'), null)
  assert.match(validateLessonText('a.md', skip, 'entries') || '', /写成条目/)
  assert.match(validateLessonText('a.md', '## 可复用经验\n随便写点\n', 'entries-or-na') || '', /或写 N\/A/)
  assert.equal(validateLessonText('a.md', FULL, 'entries'), null)
})

test('the judge asks for a lesson on its own line', () => {
  assert.equal(lessonRequest('看完了\nLESSON: NEEDED 同类问题会再犯\nVERDICT: PASS'), '同类问题会再犯')
  assert.equal(lessonRequest('lesson：needed'), '判定节点认为需要')
  assert.equal(lessonRequest('这里提到 LESSON: NEEDED 只是举例'), '')
  assert.equal(lessonRequest('VERDICT: PASS'), '')
})

test('prevention paths come from code spans in that section only', () => {
  const body = [
    '#### 症状', '`src/skip.ts`',
    '#### 防复发措施',
    '改 `./src/a.ts:12` 和 `docs/guide.md`，跑 `npm run test`，看 `src/*.ts`、`foo()`、`https://x.dev/a`、`C:\\x.ts`、`README.md`',
    '#### 复发检测', '`src/also-skip.ts`'
  ].join('\n')
  assert.deepEqual(preventionPaths(body), ['src/a.ts', 'docs/guide.md', 'README.md'])
})

test('entry file names are safe and stable', () => {
  assert.equal(entryStem('2026-11-06', { scope: 'src/lib/flow', title: 'PTY: 写入?要等*回合' }), '2026-11-06__src-lib-flow__PTY-写入-要等-回合')
  assert.equal(entryStem('2026-11-06', { scope: '../..', title: '???' }), '2026-11-06__general__lesson')
  assert.ok(entryStem('2026-11-06', { scope: 'a', title: '长'.repeat(200) }).length < 80)
})

test('the entry header marks gaps and the body stays verbatim', () => {
  const clean = entryText(draft(), '2026-11-06', 't1')
  assert.doesNotMatch(clean, /missing_/)
  assert.ok(clean.endsWith('---\n\n### 标题\n内容\n'))
  assert.match(clean, /source: experience\/dev-2\.md/)
  const gaps = entryText(draft({ missingSections: ['根因'], missingPaths: ['src/a.ts'] }), '2026-11-06', 't1')
  assert.match(gaps, /missing_sections: 根因/)
  assert.match(gaps, /missing_paths: src\/a\.ts/)
})

test('the index gathers lines under their scope', () => {
  const one = addIndexLine('# 经验索引\n', '流程', '- 一')
  assert.equal(one, '# 经验索引\n\n## 流程\n- 一\n')
  const two = addIndexLine(one, '文档', '- 二')
  const three = addIndexLine(two, '流程', '- 三')
  assert.equal(three, '# 经验索引\n\n## 流程\n- 一\n- 三\n\n## 文档\n- 二\n')
})

test('adding a line twice changes nothing', () => {
  const index = '# 经验索引\n\n## 流程\n- 一\n'
  assert.equal(addIndexLine(index, '流程', '- 一'), index)
})

test('index lines point at a path an agent can open', () => {
  assert.equal(indexLine('标题', 'memory/entries/a.md'), '- 标题 — .agentDock/memory/entries/a.md')
})
