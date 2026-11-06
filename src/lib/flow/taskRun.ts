import {
  channelReadsFiles,
  outputCapture,
  renderContractBlock,
  resolveRef,
  templateRel,
  validateOutputText,
  verdictFromDocument,
  type InputBlock,
  type OutputBlock
} from './contract.ts'
import { validateLessonText } from './lessons.ts'
import type { AdapterOutcome, FlowNode, FlowRun, StepFile } from './types.ts'

export type DockFs = {
  read(rel: string): Promise<{ text: string; sha256: string } | null>
  write(rel: string, text: string): Promise<string>
}

export type PreparedStep = {
  block: string
  inputs: StepFile[]
  // Output hashes before the node ran, to tell a rewrite from a stale file.
  before: Record<string, string | null>
}

export function attemptOf(run: FlowRun, nodeId: string) {
  const own = run.steps.filter((step) => step.nodeId === nodeId && step.purpose !== 'lesson').length
  return (run.priorAttempts?.[nodeId] ?? 0) + own
}

export function attemptCounts(run: FlowRun) {
  const counts: Record<string, number> = {}
  for (const nodeId of new Set([...Object.keys(run.priorAttempts ?? {}), ...run.steps.map((step) => step.nodeId)])) {
    counts[nodeId] = attemptOf(run, nodeId)
  }
  return counts
}

export function taskOf(run: FlowRun) {
  return run.taskDir || ''
}

async function collectInputs(fs: DockFs, node: FlowNode, run: FlowRun) {
  const rows: InputBlock[] = []
  const inputs: StepFile[] = []
  const reworked = attemptOf(run, node.id) > 1
  const inline = !channelReadsFiles(node.channel)
  for (const input of node.inputs ?? []) {
    if (input.when === 'reworked' && !reworked) continue
    const target = resolveRef(input.ref, taskOf(run))
    if (!target) return { error: `输入写法不对：${input.ref}` }
    const file = await fs.read(target.rel)
    if (!file && input.required) return { error: `缺少必读输入：${input.ref}` }
    inputs.push({ ref: input.ref, sha256: file?.sha256 ?? null })
    if (file) rows.push({ path: target.path, read: input.read, text: inline ? file.text : undefined })
  }
  return { rows, inputs }
}

async function prepareOutputs(fs: DockFs, node: FlowNode, run: FlowRun) {
  const rows: OutputBlock[] = []
  const before: Record<string, string | null> = {}
  for (const output of node.outputs ?? []) {
    const target = resolveRef(output.path, taskOf(run))
    if (!target) return { error: `输出写法不对：${output.path}` }
    const capture = outputCapture(node.channel, output)
    const templatePath = templateRel(node, output)
    const template = templatePath ? await fs.read(templatePath) : null
    if (templatePath && !template) return { error: `找不到模板：${templatePath}` }
    let current = (await fs.read(target.rel))?.sha256 ?? null
    if (template && capture === 'file' && !current) {
      current = await fs.write(target.rel, template.text)
    }
    before[output.path] = current
    rows.push({
      path: target.path,
      capture,
      sections: output.sections ?? [],
      template: templatePath ? resolveRef(`project:${templatePath}`, '')?.path : undefined
    })
  }
  return { rows, before }
}

export async function prepareStep(fs: DockFs, node: FlowNode, run: FlowRun): Promise<PreparedStep | { error: string }> {
  try {
    const inputs = await collectInputs(fs, node, run)
    if (inputs.error !== undefined) return { error: inputs.error }
    const outputs = await prepareOutputs(fs, node, run)
    if (outputs.error !== undefined) return { error: outputs.error }
    return {
      block: renderContractBlock(inputs.rows, outputs.rows),
      inputs: inputs.inputs,
      before: outputs.before
    }
  } catch (err) {
    return { error: err instanceof Error ? err.message : String(err) }
  }
}

async function collectOutput(fs: DockFs, node: FlowNode, run: FlowRun, prepared: PreparedStep, outcome: AdapterOutcome, index: number) {
  const output = (node.outputs ?? [])[index]
  const target = resolveRef(output.path, taskOf(run))
  if (!target) return { error: `输出写法不对：${output.path}` }
  if (outputCapture(node.channel, output) === 'reply') {
    const sha256 = await fs.write(target.rel, outcome.text)
    return { text: outcome.text, sha256, path: target.path, sections: output.sections ?? [] }
  }
  const file = await fs.read(target.rel)
  if (!file) return { error: `节点没有写出产物：${target.path}` }
  if (file.sha256 === prepared.before[output.path]) return { error: `节点没有更新产物：${target.path}` }
  return { text: file.text, sha256: file.sha256, path: target.path, sections: output.sections ?? [] }
}

export async function finishStep(
  fs: DockFs,
  node: FlowNode,
  run: FlowRun,
  prepared: PreparedStep,
  outcome: AdapterOutcome,
  gated: boolean
): Promise<{ outcome: AdapterOutcome; outputs: StepFile[] }> {
  const outputs: StepFile[] = []
  const total = node.outputs?.length ?? 0
  if (!outcome.ok || !total) return { outcome, outputs }
  const failed = (error: string) => ({ outcome: { ...outcome, ok: false, error }, outputs })
  if (outcome.unconfirmed) return failed('这一步没有确认完成，无法校验产物')
  let verdict = outcome.verdict
  try {
    for (let index = 0; index < total; index += 1) {
      const row = await collectOutput(fs, node, run, prepared, outcome, index)
      if ('error' in row) return failed(row.error ?? '')
      outputs.push({ ref: (node.outputs ?? [])[index].path, sha256: row.sha256 })
      const output = (node.outputs ?? [])[index]
      const problem = validateOutputText(row.path, row.text, row.sections)
        || (output.lesson ? validateLessonText(row.path, row.text, output.lesson) : null)
      if (problem) return failed(problem)
      if (gated) verdict = verdictFromDocument(row.text) ?? verdict
    }
  } catch (err) {
    return failed(err instanceof Error ? err.message : String(err))
  }
  return { outcome: { ...outcome, verdict }, outputs }
}

export function runRecord(run: FlowRun) {
  const attempts = new Map(Object.entries(run.priorAttempts ?? {}))
  return {
    id: run.id,
    flowId: run.flowId,
    task: run.task,
    status: run.status,
    error: run.lastError || undefined,
    createdAt: run.createdAt,
    lessons: run.lessons,
    steps: run.steps.map((step) => {
      const attempt = (attempts.get(step.nodeId) ?? 0) + (step.purpose === 'lesson' ? 0 : 1)
      attempts.set(step.nodeId, attempt)
      return {
        nodeId: step.nodeId,
        attempt,
        purpose: step.purpose,
        status: step.status,
        startedAt: step.startedAt,
        endedAt: step.endedAt,
        error: step.error,
        verdict: step.status === 'completed' ? step.envelope?.verdict : undefined,
        inputs: step.inputs,
        outputs: step.outputs
      }
    })
  }
}
