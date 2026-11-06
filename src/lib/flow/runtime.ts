import * as api from '../api.ts'
import { appearanceFromSettings, resolvedTheme, systemPrefersLight } from '../appearance.ts'
import { jumpToLive, rememberOpened, showToast, store, watchPendingSession } from '../store.ts'
import { runChannel } from './adapters.ts'
import { defaultChannel, firstCodexNode, validateFlowPtyBindings } from './channels.ts'
import { connectForward, deleteEdge, placeNode, placeTerminal, withNodePoints } from './chartLayout.ts'
import { flowHasContract, nodeHasContract, taskDirName, validateContract } from './contract.ts'
import { dockFs } from './dockFs.ts'
import {
  afterOutcome,
  applyManualVerdict,
  awaitHuman,
  awaitLessons,
  cancelRun,
  expectsVerdict,
  finishLessons,
  isAwaitingLessons,
  isRunBusy,
  resumeManual,
  startLessonTurn,
  startRun
} from './engine.ts'
import { renderEnvelope, withVerdictTicket } from './envelope.ts'
import { collectLessons, harvestLessons, lessonTurnAuthor, noteLessonRequest, planStep, validateExperience } from './experience.ts'
import { FLOWS_DIR, flowFilePath, flowFileText, mergeRepoFlows, parseFlowFile } from './flowFiles.ts'
import {
  appendNode,
  createCustomNode,
  persistAllFlows,
  persistAllRuns,
  removeNode,
  replaceFlow,
  selectedFlow,
  ensureProjectFlows as ensureBundle
} from './model.ts'
import { addCustomRole as pushCustomRole, roleContract, roleLabel } from './roles.ts'
import { attemptCounts, finishStep, prepareStep, runRecord, type PreparedStep } from './taskRun.ts'
import { flowFromTemplate } from './templates.ts'
import type {
  EngineEvent,
  EngineResult,
  Envelope,
  FlowDef,
  FlowNode,
  FlowRun,
  FlowTemplateId,
  RoleId
} from './types.ts'

const FLOW_WRITE_DELAY_MS = 400

let runSeq = 0
let harvesting = false
let traceQueue: Promise<void> = Promise.resolve()
// Canonical text of each flow file as last read or written.
const flowTexts = new Map<string, string>()
const pendingFlowWrites = new Map<string, { timer: ReturnType<typeof setTimeout>; write: () => Promise<void> }>()

function bundle() {
  const id = store.selectedProjectId
  if (!id) return null
  return ensureBundle(store.projectFlows, id)
}

export function currentFlow(): FlowDef | null {
  return selectedFlow(bundle())
}

export function currentRun(): FlowRun | null {
  const id = store.selectedProjectId
  if (!id) return null
  return store.runs[id] ?? null
}

export function persistFlows() {
  persistAllFlows(store.projectFlows)
}

export function persistRuns() {
  persistAllRuns(store.runs)
}

export function persistCurrent() {
  persistFlows()
  persistRuns()
}

function traceRun(run: FlowRun) {
  if (!run.taskDir) return
  const path = `tasks/${run.taskDir}/run.json`
  const text = `${JSON.stringify(runRecord(run), null, 2)}\n`
  traceQueue = traceQueue
    .then(async () => {
      await dockFs(run.projectId).write(path, text)
    })
    .catch((err) => showToast(`运行记录写入失败：${err instanceof Error ? err.message : String(err)}`))
}

// A run keeps going after the user switches project, so it files under its own.
function setRun(run: FlowRun | null) {
  const id = run?.projectId || store.selectedProjectId
  if (!id) return
  store.runs[id] = run
  persistRuns()
  if (run) traceRun(run)
}

function flowKey(projectId: string, flowId: string) {
  return `${projectId}/${flowId}`
}

async function writeFlowFile(projectId: string, flow: FlowDef) {
  const key = flowKey(projectId, flow.id)
  const text = flowFileText(flow)
  if (flowTexts.get(key) === text) return
  try {
    await dockFs(projectId).write(flowFilePath(flow.id), text)
    flowTexts.set(key, text)
  } catch (err) {
    showToast(`流程文件写入失败：${err instanceof Error ? err.message : String(err)}`)
  }
}

function scheduleFlowWrite(projectId: string, flow: FlowDef) {
  const key = flowKey(projectId, flow.id)
  const pending = pendingFlowWrites.get(key)
  if (pending) clearTimeout(pending.timer)
  const write = () => {
    pendingFlowWrites.delete(key)
    return writeFlowFile(projectId, flow)
  }
  pendingFlowWrites.set(key, { timer: setTimeout(write, FLOW_WRITE_DELAY_MS), write })
}

async function flushFlowWrites() {
  for (const pending of [...pendingFlowWrites.values()]) {
    clearTimeout(pending.timer)
    await pending.write()
  }
}

async function removeFlowFile(projectId: string, flowId: string) {
  const key = flowKey(projectId, flowId)
  const pending = pendingFlowWrites.get(key)
  if (pending) clearTimeout(pending.timer)
  pendingFlowWrites.delete(key)
  flowTexts.delete(key)
  try {
    await dockFs(projectId).remove(flowFilePath(flowId))
  } catch (err) {
    showToast(`流程文件删除失败：${err instanceof Error ? err.message : String(err)}`)
  }
}

async function readRepoFlows(projectId: string) {
  const fs = dockFs(projectId)
  const flows: FlowDef[] = []
  for (const name of await fs.list(FLOWS_DIR)) {
    if (!name.endsWith('.json')) continue
    const file = await fs.read(`${FLOWS_DIR}/${name}`)
    const flow = file ? parseFlowFile(name, file.text) : null
    if (!flow) {
      showToast(`流程文件格式不对：${name}`)
      continue
    }
    flowTexts.set(flowKey(projectId, flow.id), flowFileText(flow))
    flows.push(flow)
  }
  return flows
}

export async function syncProjectFlows(projectId = store.selectedProjectId) {
  if (!projectId || isRunBusy(store.runs[projectId])) return
  try {
    await flushFlowWrites()
    const repo = await readRepoFlows(projectId)
    if (!repo.length || isRunBusy(store.runs[projectId])) return
    const row = ensureBundle(store.projectFlows, projectId)
    store.projectFlows[projectId] = { ...row, flows: mergeRepoFlows(row.flows, repo) }
    persistFlows()
  } catch (err) {
    showToast(`读取流程文件失败：${err instanceof Error ? err.message : String(err)}`)
  }
}

// Binding changes stay local, so running a flow never touches the repository.
function patchFlow(flow: FlowDef) {
  const id = store.selectedProjectId
  if (!id) return
  const row = ensureBundle(store.projectFlows, id)
  const before = row.flows.find((item) => item.id === flow.id)
  store.projectFlows[id] = replaceFlow(row, flow)
  persistFlows()
  if (!before || flowFileText(before) !== flowFileText(flow)) scheduleFlowWrite(id, flow)
}

function lessonsToast() {
  if (!isAwaitingLessons(currentRun())) return false
  showToast('先审批这一片的经验，或者全部跳过')
  return true
}

function localDate(at: number) {
  const date = new Date(at)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

function busyToast() {
  const run = currentRun()
  if (isRunBusy(run)) {
    showToast('这一片还在跑')
    return true
  }
  return false
}

async function settleExperience(flow: FlowDef, run: FlowRun): Promise<EngineResult | null> {
  if (!flow.experience || !run.taskDir) return null
  try {
    const fs = dockFs(run.projectId)
    const author = await lessonTurnAuthor(fs, flow, run)
    if (author) return startLessonTurn(flow, run, author)
    const lessons = await collectLessons(fs, run)
    return lessons.length ? awaitLessons(run, lessons) : null
  } catch (err) {
    showToast(`读取经验草稿失败：${err instanceof Error ? err.message : String(err)}`)
    return null
  }
}

async function drive(flow: FlowDef, result: EngineResult) {
  const seq = ++runSeq
  let run = result.run
  let event: EngineEvent = result.event
  setRun(run)

  while (event.type === 'execute') {
    const execute = event
    const node = flow.nodes.find((item) => item.id === execute.nodeId)
    if (!node) {
      run.lastError = '找不到节点'
      run.status = 'failed'
      setRun(run)
      showToast(run.lastError)
      return
    }
    if (node.channel.kind === 'cursorSdk' && node.channel.agentId !== run.cursorAgentId && run.cursorAgentId) {
      node.channel.agentId = run.cursorAgentId
      patchFlow({ ...flow })
    }
    if (node.channel.kind === 'human') {
      const paused = awaitHuman(run, node.id)
      run = paused.run
      event = paused.event
      setRun(run)
      break
    }
    const step = run.steps[run.steps.length - 1]
    const gated = step?.purpose !== 'lesson' && expectsVerdict(flow, node.id)
    const plan = planStep(flow, node, run, step)
    let prepared: PreparedStep | null = null
    if (run.taskDir && nodeHasContract(plan.node)) {
      const result = await prepareStep(dockFs(run.projectId), plan.node, run)
      if (seq !== runSeq) return
      if ('error' in result) {
        const failed = afterOutcome(flow, run, { ok: false, text: '', error: result.error })
        run = failed.run
        event = failed.event
        setRun(run)
        continue
      }
      prepared = result
      if (step) step.inputs = result.inputs
      setRun(run)
    }
    const contract = [node.roleContract, prepared?.block, plan.note].filter(Boolean).join('\n\n')
    const contracted = { ...plan.node, roleContract: gated ? withVerdictTicket(contract) : contract }
    let outcome = await runChannel(contracted, execute.envelope, {
      projectId: run.projectId,
      send: execute.send,
      // Verbatim text would drop the input list and the output requirements.
      verbatim: execute.verbatim && !prepared,
      live: store.live,
      ptyDataAt: store.ptyDataAt,
      cursorApiKey: store.settings.cursorApiKey,
      isCurrent: () => seq === runSeq,
      onInputRequired(required) {
        if(seq!==runSeq)return
        run.status=required?'waiting':'running'
        const step=[...run.steps].reverse().find(s=>s.nodeId===node.id && (s.status==='working'||s.status==='input-required'))
        if(step)step.status=required?'input-required':'working'
        setRun(run)
      }
    })
    if (seq !== runSeq) return
    if (outcome.cursorAgentId && node.channel.kind === 'cursorSdk') {
      node.channel.agentId = outcome.cursorAgentId
      patchFlow({ ...flow, nodes: flow.nodes.map((item) => (item.id === node.id ? node : item)) })
    }
    if (prepared) {
      const finished = await finishStep(dockFs(run.projectId), plan.node, run, prepared, outcome, gated)
      if (seq !== runSeq) return
      outcome = finished.outcome
      if (step) step.outputs = finished.outputs
    }
    noteLessonRequest(flow, node, run, outcome)
    const next = afterOutcome(flow, run, outcome)
    run = next.run
    event = next.event
    setRun(run)
  }

  if (event.type === 'waitManual') {
    showToast('等待手动桥接：可改信封后再写入下一窗口')
    return
  }
  if (event.type === 'waitHuman') {
    showToast('等待人工审批：看完后标记通过或未通过')
    return
  }
  if (event.type === 'complete') {
    const settled = await settleExperience(flow, run)
    if (seq !== runSeq) return
    if (settled?.event.type === 'execute') {
      await drive(flow, settled)
      return
    }
    if (settled) {
      setRun(settled.run)
      showToast('流程已通过：审批经验后本片才算走完')
      return
    }
    showToast('本片流程已走完')
    return
  }
  if (event.type === 'fail') {
    showToast(event.error)
  }
}

export async function startSelectedFlow(nodeId?: string) {
  const projectId = store.selectedProjectId
  if (!currentFlow() || !projectId) {
    showToast('先选一个项目')
    return
  }
  if (busyToast()) return
  if (lessonsToast()) return
  await syncProjectFlows(projectId)
  const flow = currentFlow()
  if (!flow) return
  const bindError = validateFlowPtyBindings(flow, store.live, projectId) || validateContract(flow) || validateExperience(flow)
  if (bindError) {
    showToast(bindError)
    return
  }
  const previous = currentRun()
  store.bridgePaneMode = 'run'
  await refreshGit()
  const result = startRun(flow, projectId, flow.draftTask, nodeId)
  if (flowHasContract(flow)) {
    // Rerunning from a failed node continues the same task's documents.
    const resumed = nodeId && previous?.flowId === flow.id ? previous.taskDir : ''
    result.run.taskDir = resumed || taskDirName(result.run.task, result.run.createdAt, result.run.id)
    if (resumed && previous) result.run.priorAttempts = attemptCounts(previous)
  }
  await drive(flow, result)
}

export async function startReviewOnly() {
  const flow = currentFlow()
  const reviewer = flow?.nodes.find((node) => node.role === 'reviewer')
  if (!reviewer) {
    showToast('这条流程没有审查者节点')
    return
  }
  await startSelectedFlow(reviewer.id)
}

export async function sendHandoff(send: boolean, envelope?: Envelope) {
  const flow = currentFlow()
  const run = currentRun()
  if (!flow || !run?.pendingHandoff) {
    showToast('现在没有待桥接的消息')
    return
  }
  if (busyToast()) return
  const payload = envelope || run.pendingHandoff
  await drive(flow, resumeManual(flow, run, payload, send))
}

export function editHandoff(text: string) {
  const run = currentRun()
  if (!run?.pendingHandoff) return
  run.pendingHandoff = {
    ...run.pendingHandoff,
    artifacts: [{ kind: 'text', body: text }],
    done: text
  }
  setRun(run)
}

export function markVerdict(verdict: 'pass' | 'fail') {
  const flow = currentFlow()
  const run = currentRun()
  if (!flow || !run) return
  if (busyToast() || lessonsToast()) return
  void drive(flow, applyManualVerdict(flow, run, verdict))
}

export async function approveLessons(ids: string[]) {
  const run = currentRun()
  if (!run?.pendingLessons?.length || harvesting) return
  harvesting = true
  try {
    const records = await harvestLessons(dockFs(run.projectId), run, ids, localDate(Date.now()))
    setRun(finishLessons(run, records))
    showToast(ids.length ? `已写入 ${ids.length} 条经验` : '已跳过经验，本片走完')
  } catch (err) {
    showToast(`经验写入失败：${err instanceof Error ? err.message : String(err)}`)
  } finally {
    harvesting = false
  }
}

export function advanceSlice() {
  const flow = currentFlow()
  const run = currentRun()
  if (!flow) return
  if (run && run.status !== 'completed') {
    showToast('审查未通过，不能进入下一片')
    return
  }
  flow.slice += 1
  flow.draftTask = ''
  patchFlow(flow)
  setRun(null)
  showToast(`已解锁第 ${flow.slice} 片`)
}

export function selectFlow(flowId: string) {
  const row = bundle()
  if (!row) return
  const flow = row.flows.find((item) => item.id === flowId)
  if (!flow) return
  if (isRunBusy(currentRun())) {
    showToast('这一片还在跑，先等它结束再换流程')
    return
  }
  if (lessonsToast()) return
  row.selectedFlowId = flowId
  store.bridgeSelectedNodeId = flow.nodes[0]?.id || ''
  store.bridgeSelectedEdgeId = ''
  persistFlows()
  setRun(null)
}

export function setPaneMode(mode: 'edit' | 'run') {
  if (mode === 'edit' && isRunBusy(currentRun())) {
    showToast('这一片还在跑，结束后再改图')
    return
  }
  store.bridgePaneMode = mode
}

export function setSelectedNode(nodeId: string) {
  store.bridgeSelectedNodeId = nodeId
  store.bridgeSelectedEdgeId = ''
}

export function setSelectedEdge(edgeId: string) {
  store.bridgeSelectedEdgeId = edgeId
  store.bridgeSelectedNodeId = ''
}

export function clearSelection() {
  store.bridgeSelectedNodeId = ''
  store.bridgeSelectedEdgeId = ''
}

export function createPairFlow() {
  createFlowFromTemplate('dev-review')
}

export function createFlowFromTemplate(templateId: FlowTemplateId) {
  const row = bundle()
  if (!row) return
  if (isRunBusy(currentRun())) {
    showToast('这一片还在跑，先等它结束再新建')
    return
  }
  if (lessonsToast()) return
  const flow = flowFromTemplate(templateId)
  const count = row.flows.filter((item) => item.templateId === templateId).length
  if (count) flow.name = `${flow.name} ${count + 1}`
  row.flows.push(flow)
  row.selectedFlowId = flow.id
  store.bridgePaneMode = 'edit'
  store.bridgeSelectedNodeId = flow.nodes[0]?.id || ''
  store.bridgeSelectedEdgeId = ''
  persistFlows()
  scheduleFlowWrite(store.selectedProjectId, flow)
  setRun(null)
}

export function stopSelectedFlow() {
  const run = currentRun()
  if (!run || (run.status !== 'running' && run.status !== 'waiting')) {
    showToast('现在没有在跑的流程')
    return
  }
  runSeq += 1
  setRun(cancelRun(run))
  showToast('已停止本片')
}

export function renameFlow(flowId: string, name: string) {
  const row = bundle()
  const flow = row?.flows.find((item) => item.id === flowId)
  if (!flow) return
  const next = name.trim()
  if (!next) return
  flow.name = next
  persistFlows()
  scheduleFlowWrite(store.selectedProjectId, flow)
}

export function deleteFlow(flowId: string) {
  const row = bundle()
  if (!row) return
  if (isRunBusy(currentRun())) {
    showToast('这一片还在跑，不能删流程')
    return
  }
  if (lessonsToast()) return
  if (row.flows.length <= 1) {
    showToast('至少留一条流程')
    return
  }
  row.flows = row.flows.filter((item) => item.id !== flowId)
  if (row.selectedFlowId === flowId) {
    row.selectedFlowId = row.flows[0].id
    store.bridgeSelectedNodeId = row.flows[0].nodes[0]?.id || ''
  }
  persistFlows()
  void removeFlowFile(store.selectedProjectId, flowId)
  setRun(null)
}

export function registerCustomRole(label: string) {
  const result = pushCustomRole(label, store.customRoles)
  if (result.error) {
    showToast(result.error)
    return null
  }
  store.customRoles = result.roles
  showToast(`已添加角色「${result.role?.label}」`)
  return result.role
}

export function addRoleNode(role: RoleId) {
  addRoleNodeAt(role)
}

export function addRoleNodeAt(role: RoleId, point?: { x: number; y: number }) {
  const flow = currentFlow()
  if (!flow) return
  const next = appendNode(flow, createCustomNode(role, defaultChannel(role), point))
  patchFlow(next)
  const id = next.nodes[next.nodes.length - 1]?.id || ''
  store.bridgeSelectedNodeId = id
  store.bridgeSelectedEdgeId = ''
}

export function moveFlowNode(nodeId: string, x: number, y: number) {
  const flow = currentFlow()
  if (!flow) return
  patchFlow(placeNode(withNodePoints(flow), nodeId, x, y))
}

export function connectFlowNodes(fromId: string, toId: string) {
  const flow = currentFlow()
  if (!flow) return
  patchFlow(connectForward(flow, fromId, toId))
}

export function disconnectEdge(edgeId: string) {
  const flow = currentFlow()
  if (!flow) return
  patchFlow(deleteEdge(flow, edgeId))
  if (store.bridgeSelectedEdgeId === edgeId) store.bridgeSelectedEdgeId = ''
}

export function moveTerminal(which: 'start' | 'end', x: number, y: number) {
  const flow = currentFlow()
  if (!flow) return
  patchFlow(placeTerminal(flow, which, x, y))
}

export function setEdgeGate(edgeId: string, gate: 'none' | 'passFail' | 'loop', backTo?: string) {
  const flow = currentFlow()
  if (!flow) return
  patchFlow({
    ...flow,
    edges: flow.edges.map((edge) => (
      edge.id === edgeId
        ? {
            ...edge,
            gate,
            backTo: gate === 'passFail' ? backTo || edge.backTo : undefined,
            maxLoops: gate === 'loop' ? edge.maxLoops || 3 : undefined
          }
        : edge
    ))
  })
}

export function setEdgeMaxLoops(edgeId: string, maxLoops: number) {
  const flow = currentFlow()
  if (!flow) return
  const n = Math.min(20, Math.max(1, Math.round(maxLoops) || 3))
  patchFlow({
    ...flow,
    edges: flow.edges.map((edge) => (edge.id === edgeId ? { ...edge, gate: 'loop', maxLoops: n } : edge))
  })
}

export function setEdgeBackTo(edgeId: string, backTo: string) {
  const flow = currentFlow()
  if (!flow) return
  patchFlow({
    ...flow,
    edges: flow.edges.map((edge) => (
      edge.id === edgeId
        ? { ...edge, gate: 'passFail', backTo: backTo || undefined }
        : edge
    ))
  })
}

export function removeRoleNode(nodeId: string) {
  const flow = currentFlow()
  if (!flow) return
  const next = removeNode(flow, nodeId)
  patchFlow(next)
  if (store.bridgeSelectedNodeId === nodeId) {
    store.bridgeSelectedNodeId = next.nodes[0]?.id || ''
    store.bridgeSelectedEdgeId = ''
  }
}

export async function jumpBoundWindow(ptyId: string) {
  await jumpToLive(ptyId, { keepMode: true })
}

export async function openWindowForNode(nodeId: string) {
  const flow = currentFlow()
  const node = flow?.nodes.find((item) => item.id === nodeId)
  if (!node || (node.channel.kind !== 'pty' && node.channel.kind !== 'ompRpc')) return
  const project = store.projects.find((item) => item.id === store.selectedProjectId)
  if (!project) {
    showToast('先选一个项目')
    return
  }
  if (!api.isTauri) {
    showToast('请在桌面端打开窗口')
    return
  }
  try {
    const opened = await api.ptyOpen({
      projectId: project.id,
      toolId: node.channel.toolId,
      ompMode:node.channel.toolId==='omp'?'wrapped':undefined,
      profileId:node.channel.profileId,
      title: node.title || '新会话',
      cols: 120,
      rows: 32,
      uiTheme: resolvedTheme(appearanceFromSettings(store.settings).theme, systemPrefersLight())
    })
    rememberOpened({
      ptyId: opened.ptyId,
      key: opened.key,
      projectId: project.id,
      toolId: node.channel.toolId,
      sessionId: opened.sessionId ?? '',
      title: opened.title,
      alive: true,
      openedAt: opened.openedAt ?? Date.now(),
      kind: opened.kind ?? 'pty',
      url: opened.url ?? null,
      account: opened.account
    })
    if (!opened.sessionId) watchPendingSession(opened.ptyId)
    updateNode(nodeId, {
      channel: {
        kind: node.channel.kind,
        toolId: node.channel.toolId,
        ptyId: opened.ptyId,
        sessionId: opened.sessionId || ''
      }
    })
    showToast('已打开并绑定窗口')
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  }
}

export function updateNode(nodeId: string, patch: Partial<FlowNode>) {
  const flow = currentFlow()
  if (!flow) return
  const nodes = flow.nodes.map((node) => {
    if (node.id !== nodeId) return node
    const next = { ...node, ...patch }
    if (patch.role && patch.role !== node.role && !patch.roleContract) {
      next.roleContract = roleContract(patch.role)
      next.title = roleLabel(patch.role)
    }
    return next
  })
  patchFlow({ ...flow, nodes })
}

export function setEdgeMode(edgeId: string, mode: 'auto' | 'manual') {
  const flow = currentFlow()
  if (!flow) return
  patchFlow({
    ...flow,
    edges: flow.edges.map((edge) => (edge.id === edgeId ? { ...edge, mode } : edge))
  })
}

export function toggleCodexThread(threadId: string) {
  const flow = currentFlow()
  const node = flow ? firstCodexNode(flow) : null
  if (!flow || !node || node.channel.kind !== 'codexApp') return
  const ids = node.channel.threadIds
  const idx = ids.indexOf(threadId)
  if (idx >= 0) ids.splice(idx, 1)
  else ids.push(threadId)
  patchFlow({ ...flow })
}

export function handoffPreview() {
  const run = currentRun()
  const flow = currentFlow()
  if (!run?.pendingHandoff) return ''
  const to = flow?.nodes.find((item) => item.id === run.pendingHandoff?.toNode)
  return renderEnvelope(run.pendingHandoff, {
    transform: 'roleWrap',
    contract: to?.roleContract
  })
}

export async function refreshGit() {
  const row = bundle()
  const project = store.selectedProjectId
  if (!row || !project) return
  if (!api.isTauri) {
    row.git = {
      branch: 'main',
      head: 'preview000',
      dirty: true,
      summary: ' M agent-dock/src/App.vue'
    }
    const run = currentRun()
    if (run) {
      run.git = row.git
      setRun(run)
    }
    persistFlows()
    return
  }
  try {
    row.git = await api.gitSnapshot(project)
    const run = currentRun()
    if (run) {
      run.git = row.git
      setRun(run)
    }
    persistFlows()
  } catch (err) {
    const run = currentRun()
    if (run) {
      run.lastError = err instanceof Error ? err.message : String(err)
      setRun(run)
    }
  }
}
