import {
  addIndexLine,
  entryStem,
  entryText,
  indexLine,
  LESSON_HEADING,
  LESSON_SECTIONS,
  lessonRequest,
  parseLessons,
  preventionPaths
} from './lessons.ts'
import { attemptOf, taskOf, type DockFs } from './taskRun.ts'
import type {
  AdapterOutcome,
  FlowDef,
  FlowExperience,
  FlowNode,
  FlowRun,
  LessonDraft,
  LessonRecord,
  LessonRule,
  NodeOutput,
  RunStep
} from './types.ts'

const DRAFT_DIR = 'experience'
const DRAFT_PREFIX = `task:${DRAFT_DIR}/`
const MEMORY_REF = 'project:memory/index.md'
const MEMORY_INDEX = 'memory/index.md'
const NEW_INDEX = '# 经验索引\n'
const MIN_ATTEMPTS_DEFAULT = 2

const CRITERIA = '值得写的经验：问题源于可复用的机制、未来会再次触发、能写出可执行的防复发措施。需求或方案本身的错误、一次性细节不写。'
const LESSON_FORMAT = [
  `在 ${LESSON_HEADING} 段里，每条经验这样写：`,
  '### 一句话标题',
  '范围：受影响的模块或目录',
  ...LESSON_SECTIONS.map((name) => `#### ${name}`),
  '防复发措施要可执行，涉及的文件路径用反引号标出。'
].join('\n')
const SKIP_RULE = '这一段必填。没有值得写的经验，就写一行“N/A：理由”。'
const LESSON_TURN = '这一轮只补写经验，不改代码，也不要重做任务。'
const JUDGE_NOTE = [
  '做完本职工作后，再判断这次任务暴露的问题是否值得沉淀成经验。',
  CRITERIA,
  '如果作者写了 N/A 而你认为需要，单独一行写 LESSON: NEEDED 理由（理由里不要出现 PASS 或 FAIL）；否则不要写这一行。'
].join('\n')

export type ExperienceFs = DockFs & {
  list(dir: string): Promise<string[]>
  missing(paths: string[]): Promise<string[]>
}

export type StepPlan = { node: FlowNode; note: string }

export function validateExperience(flow: FlowDef): string | null {
  const experience = flow.experience
  if (!experience) return null
  if (!experience.authors.length) return '经验配置没有指定作者节点（authors）'
  const worker = (id: string) => {
    const node = flow.nodes.find((item) => item.id === id)
    return node && node.channel.kind !== 'human' && node.channel.kind !== 'script' ? node : null
  }
  for (const id of experience.authors) {
    const author = worker(id)
    if (!author) return `经验作者要是流程里的非人工节点：${id}`
    // Review mode writes its own summary and drops format requests.
    if (author.channel.kind === 'codexApp') return `Codex 审查节点不能当经验作者：${author.title}`
  }
  const judge = experience.judge
  if (judge && !worker(judge)) return `经验判定节点要是流程里的非人工节点：${judge}`
  if (judge && experience.authors.includes(judge)) return `经验判定节点不能同时是作者：${judge}`
  return null
}

function draftOutput(nodeId: string, attempt: number, lesson: LessonRule): NodeOutput {
  return { path: `${DRAFT_PREFIX}${nodeId}-${attempt}.md`, sections: [LESSON_HEADING], lesson }
}

function draftDue(experience: FlowExperience, attempt: number) {
  if (experience.policy !== 'attempts') return true
  return attempt >= (experience.minAttempts ?? MIN_ATTEMPTS_DEFAULT)
}

function withMemory(node: FlowNode): FlowNode {
  const inputs = node.inputs ?? []
  if (inputs.some((input) => input.ref === MEMORY_REF)) return node
  return { ...node, inputs: [...inputs, { ref: MEMORY_REF, read: 'index-first' }] }
}

function draftRefOf(step: RunStep) {
  return step.outputs?.find((file) => file.ref.startsWith(DRAFT_PREFIX))?.ref
}

function latestAuthorStep(experience: FlowExperience, run: FlowRun) {
  return [...run.steps].reverse().find((step) => (
    experience.authors.includes(step.nodeId) && step.status === 'completed' && step.purpose !== 'lesson'
  ))
}

function authorPlan(experience: FlowExperience, node: FlowNode, run: FlowRun): StepPlan {
  const reader = withMemory(node)
  const attempt = attemptOf(run, node.id)
  if (!draftDue(experience, attempt)) return { node: reader, note: '' }
  const rule: LessonRule = experience.policy === 'model' ? 'entries-or-na' : 'entries'
  const notes = rule === 'entries-or-na' ? [CRITERIA, LESSON_FORMAT, SKIP_RULE] : [CRITERIA, LESSON_FORMAT]
  return {
    node: { ...reader, outputs: [...(reader.outputs ?? []), draftOutput(node.id, attempt, rule)] },
    note: notes.join('\n')
  }
}

function judgePlan(experience: FlowExperience, node: FlowNode, run: FlowRun): StepPlan {
  const author = latestAuthorStep(experience, run)
  const draft = author && draftRefOf(author)
  return {
    node: draft ? { ...node, inputs: [...(node.inputs ?? []), { ref: draft }] } : node,
    note: JUDGE_NOTE
  }
}

function lessonTurnPlan(node: FlowNode, run: FlowRun): StepPlan {
  return {
    node: { ...node, inputs: [], outputs: [draftOutput(node.id, attemptOf(run, node.id), 'entries')] },
    note: [LESSON_TURN, CRITERIA, LESSON_FORMAT].join('\n')
  }
}

// What this step owes the experience loop on top of the node's own contract.
export function planStep(flow: FlowDef, node: FlowNode, run: FlowRun, step?: RunStep): StepPlan {
  const experience = flow.experience
  if (!experience || !run.taskDir) return { node, note: '' }
  if (step?.purpose === 'lesson') return lessonTurnPlan(node, run)
  if (experience.authors.includes(node.id)) return authorPlan(experience, node, run)
  if (experience.policy === 'model' && experience.judge === node.id) return judgePlan(experience, node, run)
  return { node, note: '' }
}

export function noteLessonRequest(flow: FlowDef, node: FlowNode, run: FlowRun, outcome: AdapterOutcome) {
  const experience = flow.experience
  if (experience?.policy !== 'model' || experience.judge !== node.id || !outcome.ok) return
  const reason = lessonRequest(outcome.text)
  if (reason) run.lessonWanted = reason
}

// The judge asked for a lesson and the latest author draft has none.
export async function lessonTurnAuthor(fs: DockFs, flow: FlowDef, run: FlowRun) {
  const experience = flow.experience
  if (experience?.policy !== 'model' || !run.lessonWanted) return ''
  const step = latestAuthorStep(experience, run)
  const ref = step && draftRefOf(step)
  if (!step || !ref) return ''
  const file = await fs.read(`tasks/${taskOf(run)}/${ref.slice('task:'.length)}`)
  return file && parseLessons(file.text).entries.length ? '' : step.nodeId
}

async function readDrafts(fs: ExperienceFs, run: FlowRun) {
  const dir = `tasks/${taskOf(run)}/${DRAFT_DIR}`
  const names = (await fs.list(dir))
    .filter((name) => name.endsWith('.md'))
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
  // A later attempt replaces its own earlier wording of the same lesson.
  const latest = new Map<string, LessonDraft>()
  for (const name of names) {
    const file = await fs.read(`${dir}/${name}`)
    parseLessons(file?.text ?? '').entries.forEach((entry, index) => {
      const key = `${entry.scope}\n${entry.title}`
      latest.delete(key)
      latest.set(key, { ...entry, id: `${name}#${index}`, file: name, missingPaths: [] })
    })
  }
  return [...latest.values()]
}

export async function collectLessons(fs: ExperienceFs, run: FlowRun): Promise<LessonDraft[]> {
  const drafts = await readDrafts(fs, run)
  const missing = new Set(await fs.missing([...new Set(drafts.flatMap((draft) => preventionPaths(draft.body)))]))
  return drafts.map((draft) => ({
    ...draft,
    missingPaths: preventionPaths(draft.body).filter((path) => missing.has(path))
  }))
}

// Rerunning after a partial failure lands on the same file instead of a copy.
async function writeEntry(fs: DockFs, lesson: LessonDraft, date: string, run: FlowRun) {
  const stem = entryStem(date, lesson)
  const text = entryText(lesson, date, taskOf(run))
  for (let suffix = 1; ; suffix += 1) {
    const rel = `memory/entries/${stem}${suffix > 1 ? `-${suffix}` : ''}.md`
    const existing = await fs.read(rel)
    if (existing?.text === text) return rel
    if (!existing) {
      await fs.write(rel, text)
      return rel
    }
  }
}

export async function harvestLessons(fs: DockFs, run: FlowRun, approved: string[], date: string): Promise<LessonRecord[]> {
  const records: LessonRecord[] = []
  const lines: { scope: string; line: string }[] = []
  for (const lesson of run.pendingLessons ?? []) {
    const record: LessonRecord = { title: lesson.title, scope: lesson.scope }
    if (approved.includes(lesson.id)) {
      const entry = await writeEntry(fs, lesson, date, run)
      record.entry = entry
      lines.push({ scope: lesson.scope, line: indexLine(lesson.title, entry) })
    }
    records.push(record)
  }
  if (lines.length) {
    const current = (await fs.read(MEMORY_INDEX))?.text ?? NEW_INDEX
    await fs.write(MEMORY_INDEX, lines.reduce((index, row) => addIndexLine(index, row.scope, row.line), current))
  }
  return records
}
