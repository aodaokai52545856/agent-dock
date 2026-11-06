import { DOCK_DIR, sectionBody } from './contract.ts'
import type { LessonDraft, LessonRule } from './types.ts'

export const LESSON_HEADING = '## 可复用经验'
export const LESSON_SECTIONS = ['症状', '根因', '修复', '防复发措施', '复发检测']

const PREVENTION = '#### 防复发措施'
const ENTRY_HEADING = /^###\s+(.+?)\s*$/
const SCOPE_LINE = /^(?:范围|scope)\s*[:：]\s*(.+?)\s*$/im
const NOT_APPLICABLE = /^N\/A\s*[:：]\s*\S/i
const LESSON_REQUEST = /^\s*LESSON\s*[:：]\s*NEEDED\b[ \t]*(.*)$/im
const DEFAULT_SCOPE = '通用'
const DEFAULT_REASON = '判定节点认为需要'
const SLUG_LENGTH = 40

export type ParsedLesson = Pick<LessonDraft, 'title' | 'scope' | 'body' | 'missingSections'>

function parseEntry(lines: string[]): ParsedLesson {
  const body = lines.join('\n').trim()
  const title = ENTRY_HEADING.exec(lines[0])?.[1] ?? ''
  return {
    title,
    scope: SCOPE_LINE.exec(body)?.[1] ?? DEFAULT_SCOPE,
    body,
    missingSections: LESSON_SECTIONS.filter((name) => !sectionBody(body, `#### ${name}`))
  }
}

export function parseLessons(text: string) {
  const body = sectionBody(text, LESSON_HEADING) ?? ''
  const blocks: string[][] = []
  for (const line of body.split(/\r?\n/)) {
    if (ENTRY_HEADING.test(line)) blocks.push([line])
    else blocks[blocks.length - 1]?.push(line)
  }
  return {
    entries: blocks.map(parseEntry),
    notApplicable: !blocks.length && NOT_APPLICABLE.test(body)
  }
}

export function validateLessonText(path: string, text: string, rule: LessonRule): string | null {
  const { entries, notApplicable } = parseLessons(text)
  if (entries.length || (notApplicable && rule === 'entries-or-na')) return null
  const skip = rule === 'entries-or-na' ? '，或写 N/A：理由' : ''
  return `产物 ${path} 的经验段要用 ### 标题 写成条目${skip}`
}

export function lessonRequest(text: string) {
  const match = LESSON_REQUEST.exec(text)
  if (!match) return ''
  return match[1].trim() || DEFAULT_REASON
}

export function preventionPaths(body: string) {
  const section = sectionBody(body, PREVENTION) ?? ''
  const found = new Set<string>()
  for (const [, raw] of section.matchAll(/`([^`\s]+)`/g)) {
    const token = raw.replace(/^\.\//, '').replace(/:\d+(?::\d+)?$/, '')
    if (/[*?<>{}()]|^[a-z]+:/i.test(token)) continue
    if (token.includes('/') || /\.[a-z0-9]{1,8}$/i.test(token)) found.add(token)
  }
  return [...found]
}

function slug(text: string, fallback: string) {
  const cleaned = text.replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-+/, '')
  return [...cleaned].slice(0, SLUG_LENGTH).join('').replace(/-+$/, '') || fallback
}

export function entryStem(date: string, lesson: Pick<LessonDraft, 'scope' | 'title'>) {
  return `${date}__${slug(lesson.scope, 'general')}__${slug(lesson.title, 'lesson')}`
}

export function entryText(lesson: LessonDraft, date: string, taskDir: string) {
  const header = [
    '---',
    `scope: ${lesson.scope}`,
    `date: ${date}`,
    `task: ${taskDir}`,
    `source: experience/${lesson.file}`,
    lesson.missingSections.length ? `missing_sections: ${lesson.missingSections.join('、')}` : '',
    lesson.missingPaths.length ? `missing_paths: ${lesson.missingPaths.join('、')}` : '',
    '---'
  ].filter(Boolean)
  return `${header.join('\n')}\n\n${lesson.body}\n`
}

export function indexLine(title: string, rel: string) {
  return `- ${title} — ${DOCK_DIR}/${rel}`
}

export function addIndexLine(index: string, scope: string, line: string) {
  const lines = index.replace(/\r\n/g, '\n').trimEnd().split('\n')
  if (lines.includes(line)) return index
  const heading = `## ${scope}`
  const at = lines.indexOf(heading)
  if (at < 0) return `${[...lines, '', heading, line].join('\n')}\n`
  let end = at + 1
  while (end < lines.length && !/^#{1,2}\s/.test(lines[end])) end += 1
  while (end > at + 1 && !lines[end - 1].trim()) end -= 1
  lines.splice(end, 0, line)
  return `${lines.join('\n')}\n`
}
