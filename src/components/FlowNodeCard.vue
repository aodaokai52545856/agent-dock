<script setup lang="ts">
import { computed } from 'vue'
import {
  channelKindLabel,
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
import type { ChannelKind, CodexChannel, FlowNode, PtyChannel, RoleId } from '../lib/flow/types.ts'
import { liveOfProject } from '../lib/livePty'
import { refreshCodexThreads } from '../lib/pipeline'
import { store } from '../lib/store'
import { relativeTime } from '../lib/format'
import { toolLabel, type ReviewTargetKind, type ToolId } from '../lib/types'

const props = defineProps<{
  node: FlowNode
}>()

const roles = computed(() => allRoles(store.customRoles))
const channels: ChannelKind[] = ['codexApp', 'pty', 'cursorSdk', 'human']
const tools: ToolId[] = ['grokbuild', 'kimi', 'claude', 'opencode', 'pi', 'dsh']
const targets: { id: ReviewTargetKind; label: string }[] = [
  { id: 'uncommittedChanges', label: '未提交改动' },
  { id: 'commit', label: '指定 commit' },
  { id: 'baseBranch', label: '相对分支' },
  { id: 'custom', label: '自定义说明' }
]

const liveWindows = computed(() =>
  liveOfProject(store.live, store.selectedProjectId).filter((item) => {
    if (props.node.channel.kind !== 'pty') return true
    return item.toolId === props.node.channel.toolId
  })
)

const channelHint = computed(() => describeChannel(props.node.channel, store.live))
const boundLive = computed(() => {
  if (props.node.channel.kind !== 'pty') return null
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
  if (kind === 'codexApp') updateNode(props.node.id, { channel: defaultChannel('reviewer') })
  else if (kind === 'pty') updateNode(props.node.id, { channel: { kind: 'pty', toolId: 'grokbuild', sessionId: '', ptyId: '' } })
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
  if (props.node.channel.kind !== 'pty') return
  updateNode(props.node.id, { channel: { kind: 'pty', toolId, sessionId: '', ptyId: '' } })
}

function bindPty(ptyId: string) {
  if (props.node.channel.kind !== 'pty') return
  if (!ptyId) {
    updateNode(props.node.id, { channel: { ...props.node.channel, ptyId: '', sessionId: '' } })
    return
  }
  const live = store.live.find((item) => item.ptyId === ptyId)
  updateNode(props.node.id, {
    channel: {
      kind: 'pty',
      toolId: live?.toolId || props.node.channel.toolId,
      ptyId,
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

    <div v-if="node.channel.kind === 'pty'" class="pty-bind">
      <div class="row">
        <label class="field tight">
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
    <p v-else class="hint">人工节点：运行到这里会把当前任务当作已确认，然后沿边走下去。</p>
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
