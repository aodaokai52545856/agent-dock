<script setup lang="ts">
import { computed } from 'vue'
import { channelKindLabel, describeChannel, defaultChannel } from '../lib/flow/channels.ts'
import { persistCurrent, removeRoleNode, toggleCodexThread, updateNode } from '../lib/flow/runtime.ts'
import { allRoles, roleHint, roleLabel } from '../lib/flow/roles.ts'
import type { ChannelKind, FlowNode, RoleId } from '../lib/flow/types.ts'
import { liveOfProject } from '../lib/livePty'
import { refreshCodexThreads } from '../lib/pipeline'
import { store } from '../lib/store'
import { relativeTime } from '../lib/format'
import { toolLabel, type ReviewTargetKind, type ToolId } from '../lib/types'

const props = defineProps<{
  node: FlowNode
  canRemove: boolean
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

function bindPty(ptyId: string) {
  if (props.node.channel.kind !== 'pty') return
  const live = store.live.find((item) => item.ptyId === ptyId)
  props.node.channel.ptyId = ptyId
  props.node.channel.sessionId = live?.sessionId || ''
  if (live) props.node.channel.toolId = live.toolId
  persistCurrent()
}

function onTarget(kind: ReviewTargetKind) {
  if (props.node.channel.kind !== 'codexApp') return
  props.node.channel.targetKind = kind
  persistCurrent()
}
</script>

<template>
  <article class="card">
    <header class="head">
      <div class="titles">
        <input v-model="node.title" class="title-input" @change="persistCurrent" />
        <p class="hint">{{ roleHint(node.role) }} · {{ describeChannel(node.channel) }}</p>
      </div>
      <button v-if="canRemove" type="button" class="text-btn" @click="removeRoleNode(node.id)">删除</button>
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
      <textarea v-model="node.roleContract" rows="2" @change="persistCurrent" />
    </label>

    <div v-if="node.channel.kind === 'pty'" class="row">
      <label class="field tight">
        <span>窗口类型</span>
        <select v-model="node.channel.toolId" @change="persistCurrent">
          <option v-for="tool in tools" :key="tool" :value="tool">{{ toolLabel(tool) }}</option>
        </select>
      </label>
      <label class="field tight">
        <span>已打开的窗口</span>
        <select :value="node.channel.ptyId" :disabled="!liveWindows.length" @change="bindPty(($event.target as HTMLSelectElement).value)">
          <option value="">先打开目标窗口</option>
          <option v-for="item in liveWindows" :key="item.ptyId" :value="item.ptyId">
            {{ toolLabel(item.toolId) }} · {{ item.title }}
          </option>
        </select>
      </label>
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
        <input v-model="node.channel.commitSha" placeholder="完整或短 sha" @change="persistCurrent" />
      </label>
      <label v-if="node.channel.targetKind === 'baseBranch'" class="field tight grow">
        <span>基线分支</span>
        <input v-model="node.channel.baseBranch" placeholder="main" @change="persistCurrent" />
      </label>
      <label v-if="node.channel.targetKind === 'custom'" class="field tight grow">
        <span>审查说明</span>
        <textarea v-model="node.channel.customInstructions" rows="2" @change="persistCurrent" />
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
