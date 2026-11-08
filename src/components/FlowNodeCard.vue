<script setup lang="ts">
import { computed } from 'vue'
import NodeExitsTab from './NodeExitsTab.vue'
import NodeIoTab from './NodeIoTab.vue'
import {
  channelKindLabel,
  channelSupportsFresh,
  nodeSession,
  describeChannel,
  defaultChannel,
  inspectPty,
  ptyOccupant,
  ptyWindowLabel
} from '../lib/flow/channels.ts'
import {
  currentFlow,
  jumpBoundWindow,
  openWindowForNode,
  removeRoleNode,
  toggleCodexThread,
  updateNode
} from '../lib/flow/runtime.ts'
import { allRoles, roleHint, roleLabel } from '../lib/flow/roles.ts'
import { SCRIPT_TIMEOUT_DEFAULT_SEC } from '../lib/flow/scripts.ts'
import type { ChannelKind, CodexChannel, FlowNode, NodeSession, PtyChannel, RoleId } from '../lib/flow/types.ts'
import { liveOfProject } from '../lib/livePty'
import { refreshCodexThreads } from '../lib/pipeline'
import { store } from '../lib/store'
import { relativeTime } from '../lib/format'
import { toolLabel, type ReviewTargetKind, type ToolId } from '../lib/types'

const props = defineProps<{
  node: FlowNode
}>()

const roles = computed(() => allRoles(store.customRoles))
const channels: ChannelKind[] = ['codexApp', 'pty', 'ompRpc', 'cursorSdk', 'human', 'script']
const tabs = [
  { id: 'about', label: '说明' },
  { id: 'inputs', label: '输入' },
  { id: 'outputs', label: '输出' },
  { id: 'exits', label: '出口' }
] as const
const tab = computed(() => store.bridgeNodeTab)
const canClear = computed(() => channelSupportsFresh(props.node.channel))
const resolvedSession = computed(() => nodeSession(props.node))
const counts = computed(() => ({
  about: 0,
  inputs: props.node.inputs?.length ?? 0,
  outputs: props.node.outputs?.length ?? 0,
  exits: props.node.exits?.length ?? 0
}))

function setSession(value: string) {
  updateNode(props.node.id, { session: value === 'fresh' || value === 'reuse' ? value as NodeSession : undefined })
}

function patchScript(patch: { command?: string; timeoutSec?: number }) {
  if (props.node.channel.kind !== 'script') return
  updateNode(props.node.id, { channel: { ...props.node.channel, ...patch } })
}

function onTimeout(event: Event) {
  const value = Math.floor(Number((event.target as HTMLInputElement).value))
  patchScript({ timeoutSec: value > 0 ? value : undefined })
}
const tools: ToolId[] = ['grokbuild', 'kimi', 'claude', 'opencode', 'pi', 'omp', 'dsh']
const targets: { id: ReviewTargetKind; label: string }[] = [
  { id: 'uncommittedChanges', label: '未提交改动' },
  { id: 'commit', label: '指定 commit' },
  { id: 'baseBranch', label: '相对分支' },
  { id: 'custom', label: '自定义说明' }
]

const liveWindows = computed(() =>
  liveOfProject(store.live, store.selectedProjectId).filter((item) => {
    if ((props.node.channel.kind !== 'pty' && props.node.channel.kind !== 'ompRpc')) return true
    return item.toolId === props.node.channel.toolId && (item.toolId!=='omp'||item.kind==='ompRpc')
  })
)

const channelHint = computed(() => describeChannel(props.node.channel, store.live))
const boundLive = computed(() => {
  if ((props.node.channel.kind !== 'pty' && props.node.channel.kind !== 'ompRpc')) return null
  const found = inspectPty(props.node.channel, store.live, store.selectedProjectId)
  return found.ok ? found.live : null
})

function occupantName(ptyId: string) {
  const other = ptyOccupant(currentFlow(), ptyId, props.node.id)
  return other ? other.title || roleLabel(other.role) : ''
}

function setRole(role: RoleId) {
  updateNode(props.node.id, { role, title: roleLabel(role), channel: defaultChannel(role) })
}

function isThreadOn(id: string) {
  return props.node.channel.kind === 'codexApp' && props.node.channel.threadIds.includes(id)
}

function setChannelKind(kind: ChannelKind) {
  if (kind === 'script') updateNode(props.node.id, { channel: { kind: 'script', command: '' } })
  else if (kind === 'codexApp') updateNode(props.node.id, { channel: defaultChannel('reviewer') })
  else if (kind === 'pty') updateNode(props.node.id, { channel: { kind: 'pty', toolId: 'grokbuild', sessionId: '', ptyId: '' } })
  else if (kind === 'ompRpc') updateNode(props.node.id, {channel:{kind:'ompRpc',toolId:'omp',sessionId:'',ptyId:''}})
  else if (kind === 'cursorSdk') updateNode(props.node.id, { channel: { kind: 'cursorSdk', agentId: '' } })
  else updateNode(props.node.id, { channel: { kind: 'human' } })
}

function onTitle(event: Event) {
  updateNode(props.node.id, { title: (event.target as HTMLInputElement).value })
}

function onContract(event: Event) {
  updateNode(props.node.id, { roleContract: (event.target as HTMLTextAreaElement).value })
}

function setToolId(toolId: ToolId) {
  if ((props.node.channel.kind !== 'pty' && props.node.channel.kind !== 'ompRpc')) return
  updateNode(props.node.id, { channel: { kind: toolId==='omp'?'ompRpc':'pty', toolId, sessionId: '', ptyId: '' } })
}

function bindPty(ptyId: string) {
  if ((props.node.channel.kind !== 'pty' && props.node.channel.kind !== 'ompRpc')) return
  if (!ptyId) {
    updateNode(props.node.id, { channel: { ...props.node.channel, ptyId: '', sessionId: '' } })
    return
  }
  const live = store.live.find((item) => item.ptyId === ptyId)
  updateNode(props.node.id, {
    channel: {
      kind: live?.toolId==='omp'?'ompRpc':props.node.channel.kind,
      toolId: live?.toolId || props.node.channel.toolId,
      ptyId,
      profileId: live?.profileId || live?.account?.id,
      sessionId: live?.sessionId || ''
    }
  })
}

function patchCodex(patch: Partial<CodexChannel>) {
  if (props.node.channel.kind !== 'codexApp') return
  updateNode(props.node.id, { channel: { ...props.node.channel, ...patch } })
}

function onTarget(kind: ReviewTargetKind) {
  patchCodex({ targetKind: kind })
}

function onCodexField(field: 'commitSha' | 'baseBranch' | 'customInstructions', event: Event) {
  const value = (event.target as HTMLInputElement | HTMLTextAreaElement).value
  patchCodex({ [field]: value })
}
</script>

<template>
  <article class="card">
    <header class="head">
      <div class="titles">
        <input :value="node.title" class="title-input" @change="onTitle" />
        <p class="hint">{{ roleHint(node.role) }} · {{ channelHint }}</p>
      </div>
      <button type="button" class="text-btn" @click="removeRoleNode(node.id)">删除节点</button>
    </header>

    <nav class="tabs" role="tablist" aria-label="节点设置">
      <button
        v-for="item in tabs"
        :key="item.id"
        type="button"
        role="tab"
        class="tab"
        :class="{ 'is-on': tab === item.id }"
        :aria-selected="tab === item.id"
        @click="store.bridgeNodeTab = item.id"
      >
        {{ item.label }}<span v-if="counts[item.id]" class="count">{{ counts[item.id] }}</span>
      </button>
    </nav>

    <NodeIoTab v-if="tab === 'inputs' || tab === 'outputs'" :node="node" :kind="tab" />
    <NodeExitsTab v-else-if="tab === 'exits'" :node="node" />
    <template v-else>
    <div class="row">
      <label class="field tight">
        <span>角色</span>
        <select :value="node.role" @change="setRole(($event.target as HTMLSelectElement).value as RoleId)">
          <option v-for="role in roles" :key="role.id" :value="role.id">{{ role.label }}</option>
        </select>
      </label>
      <label class="field tight">
        <span>通道</span>
        <select
          :value="node.channel.kind"
          @change="setChannelKind(($event.target as HTMLSelectElement).value as ChannelKind)"
        >
          <option v-for="kind in channels" :key="kind" :value="kind">{{ channelKindLabel(kind) }}</option>
        </select>
      </label>
    </div>

    <label class="field tight">
      <span>角色合同</span>
      <textarea :value="node.roleContract" rows="2" @change="onContract" />
    </label>

    <label v-if="canClear" class="field tight">
      <span>会话</span>
      <select :value="node.session ?? ''" @change="setSession(($event.target as HTMLSelectElement).value)">
        <option value="">默认（{{ resolvedSession === 'fresh' ? '每个任务清空' : '沿用' }}）</option>
        <option value="fresh">每个任务清空</option>
        <option value="reuse">沿用窗口里的对话</option>
      </select>
    </label>
    <p v-if="canClear && resolvedSession === 'fresh'" class="hint">
      任务第一次跑到这里时，会在绑定的窗口里开新会话（Claude 用 /clear，Grok 用 /new，OMP 新建会话）。旧对话保留，可以恢复。
    </p>

    <div v-if="(node.channel.kind === 'pty' || node.channel.kind === 'ompRpc')" class="pty-bind">
      <div class="row">
        <label v-if="node.channel.kind !== 'ompRpc'" class="field tight">
          <span>窗口类型</span>
          <select
            :value="node.channel.toolId"
            @change="setToolId(($event.target as HTMLSelectElement).value as ToolId)"
          >
            <option v-for="tool in tools" :key="tool" :value="tool">{{ toolLabel(tool) }}</option>
          </select>
        </label>
        <label class="field tight">
          <span>已打开的窗口</span>
          <select :value="(node.channel as PtyChannel).ptyId" @change="bindPty(($event.target as HTMLSelectElement).value)">
            <option value="">先选择一扇窗口</option>
            <option v-for="item in liveWindows" :key="item.ptyId" :value="item.ptyId">
              {{ ptyWindowLabel(item, occupantName(item.ptyId) || undefined) }}
            </option>
          </select>
        </label>
      </div>
      <div class="pty-actions">
        <button
          v-if="boundLive"
          type="button"
          class="text-btn"
          @click="jumpBoundWindow(boundLive.ptyId)"
        >
          跳到该窗
        </button>
        <button type="button" class="text-btn" @click="openWindowForNode(node.id)">
          打开新{{ toolLabel((node.channel as PtyChannel).toolId) }}并绑定
        </button>
      </div>
      <p v-if="!liveWindows.length" class="hint">控制台里先打开该类型窗口，或点上面的按钮新开一扇。</p>
      <p v-if="node.channel.toolId==='omp'" class="hint">Dock 自动编排使用封装模式；原生终端里的多智能体由 OMP 自己管理。</p>
    </div>

    <div v-else-if="node.channel.kind === 'codexApp'" class="targets">
      <button
        v-for="item in targets"
        :key="item.id"
        type="button"
        class="target"
        :class="{ 'is-on': node.channel.targetKind === item.id }"
        @click="onTarget(item.id)"
      >
        {{ item.label }}
      </button>
      <label v-if="node.channel.targetKind === 'commit'" class="field tight grow">
        <span>Commit SHA</span>
        <input :value="node.channel.commitSha" placeholder="完整或短 sha" @change="onCodexField('commitSha', $event)" />
      </label>
      <label v-if="node.channel.targetKind === 'baseBranch'" class="field tight grow">
        <span>基线分支</span>
        <input :value="node.channel.baseBranch" placeholder="main" @change="onCodexField('baseBranch', $event)" />
      </label>
      <label v-if="node.channel.targetKind === 'custom'" class="field tight grow">
        <span>审查说明</span>
        <textarea :value="node.channel.customInstructions" rows="2" @change="onCodexField('customInstructions', $event)" />
      </label>
      <div class="threads grow">
        <div class="thread-head">
          <span>Codex 线程</span>
          <button type="button" class="text-btn" :disabled="store.codexStatus === 'loading'" @click="refreshCodexThreads">
            {{ store.codexStatus === 'loading' ? '读取中' : '刷新' }}
          </button>
        </div>
        <p class="hint">勾选作为审查上下文。不会往桌面正在聊的 turn 里塞字。</p>
        <label v-for="thread in store.codexThreads" :key="thread.id" class="thread">
          <input type="checkbox" :checked="isThreadOn(thread.id)" @change="toggleCodexThread(thread.id)" />
          <span>
            <strong>{{ thread.name }}</strong>
            <em>{{ relativeTime(thread.updatedAt) }}</em>
          </span>
        </label>
        <p v-if="!store.codexThreads.length" class="hint">没有可桥接的线程时，提交审查会新开专用线程。</p>
      </div>
    </div>
    <p v-else-if="node.channel.kind === 'cursorSdk'" class="hint">走 Cursor SDK，额度与 IDE 分开。同一工作区不要同时开两个写者。</p>
    <div v-else-if="node.channel.kind === 'script'" class="script">
      <label class="field tight">
        <span>命令（在项目目录下执行）</span>
        <textarea :value="node.channel.command" rows="2" placeholder="npm test" @change="patchScript({ command: ($event.target as HTMLTextAreaElement).value.trim() })" />
      </label>
      <label class="field tight">
        <span>超时（秒）</span>
        <input type="number" min="1" :value="node.channel.timeoutSec ?? ''" :placeholder="String(SCRIPT_TIMEOUT_DEFAULT_SEC)" @change="onTimeout" />
      </label>
      <p class="hint">退出码 0 走 PASS 出口，其余走 FAIL 出口。第一次运行和每次改命令后，都要在运行面板里确认命令全文。</p>
    </div>
    <p v-else class="hint">人工节点：运行到这里会暂停，标记通过或未通过后继续。</p>
    </template>
  </article>
</template>

<style scoped>
.card {
  padding: 12px 14px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.titles {
  min-width: 0;
  flex: 1;
}

.title-input {
  width: 100%;
  height: 28px;
  padding: 0 8px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--ad-text);
  font-size: 13px;
  font-weight: 560;
}

.title-input:hover,
.title-input:focus {
  border-color: var(--ad-border);
  background: var(--ad-raised);
}

.hint {
  margin: 2px 0 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.tabs {
  display: flex;
  gap: 4px;
  margin: 4px 0 2px;
  border-bottom: 1px solid var(--ad-border);
}

.tab {
  height: 30px;
  padding: 0 10px;
  border-bottom: 2px solid transparent;
  color: var(--ad-muted);
  font-size: 12px;
}

.tab.is-on {
  border-bottom-color: var(--ad-text);
  color: var(--ad-text);
}

.count {
  margin-left: 4px;
  padding: 0 5px;
  border-radius: 8px;
  background: var(--ad-selected);
  font-size: 11px;
}

.script {
  display: flex;
  flex-direction: column;
}

.pty-bind {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.pty-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
}

.row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.field.tight {
  margin: 8px 0 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field.tight > span {
  font-size: 12px;
  color: var(--ad-muted);
}

.grow {
  grid-column: 1 / -1;
}

.targets {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 8px;
}

.target {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  color: var(--ad-muted);
  font-size: 12px;
}

.target.is-on {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.threads {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 4px;
}

.thread-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  color: var(--ad-muted);
}

.thread {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12px;
  color: var(--ad-text);
}

.thread strong {
  font-weight: 560;
}

.thread em {
  margin-left: 8px;
  font-style: normal;
  color: var(--ad-muted);
}

.text-btn {
  height: 28px;
  padding: 0 6px;
  color: var(--ad-muted);
  font-size: 12px;
}

.text-btn:hover {
  color: var(--ad-text);
}

textarea {
  min-height: 56px;
  resize: vertical;
}

@media (max-width: 640px) {
  .row {
    grid-template-columns: 1fr;
  }
}
</style>
