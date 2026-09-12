<script setup lang="ts">
import { computed } from 'vue'
import { describeChannel } from '../lib/flow/channels.ts'
import { roleLabel } from '../lib/flow/roles.ts'
import { FLOW_END, type FlowDef, type FlowEdge, type FlowNode } from '../lib/flow/types.ts'
import { setEdgeMode, setSelectedNode } from '../lib/flow/runtime.ts'
import { currentRun, store } from '../lib/store'

const props = defineProps<{
  flow: FlowDef
  readonly?: boolean
}>()

const selectedId = computed(() => store.bridgeSelectedNodeId)
const currentId = computed(() => currentRun.value?.currentNodeId || '')

const pair = computed(() => {
  const nodes = props.flow.nodes
  if (nodes.length !== 2) return null
  const back = props.flow.edges.find((edge) => edge.backTo === nodes[0].id && edge.from === nodes[1].id)
  if (!back) return null
  const forward = props.flow.edges.find((edge) => edge.from === nodes[0].id && edge.to === nodes[1].id)
  return { first: nodes[0], second: nodes[1], forward, back }
})

function outgoing(nodeId: string) {
  return props.flow.edges.find((edge) => edge.from === nodeId) ?? null
}

function nodeState(node: FlowNode) {
  const run = currentRun.value
  if (!run || run.flowId !== props.flow.id) return 'idle'
  if (run.currentNodeId === node.id && (run.status === 'running' || run.status === 'waiting')) return 'active'
  if (run.steps.some((step) => step.nodeId === node.id && step.status === 'completed')) return 'done'
  if (run.steps.some((step) => step.nodeId === node.id && step.status === 'failed')) return 'failed'
  return 'idle'
}

function modeLabel(edge: FlowEdge | null | undefined) {
  if (!edge) return ''
  return edge.mode === 'auto' ? '自动' : '手动'
}

function onBox(nodeId: string) {
  if (props.readonly && store.bridgePaneMode === 'run') {
    setSelectedNode(nodeId)
    return
  }
  setSelectedNode(nodeId)
}

function toggleMode(edge: FlowEdge | undefined) {
  if (!edge || props.readonly) return
  setEdgeMode(edge.id, edge.mode === 'auto' ? 'manual' : 'auto')
}
</script>

<template>
  <div class="chart" :class="{ 'is-readonly': readonly }" aria-label="流程图">
    <div v-if="pair" class="pair">
      <div class="spine">
        <button
          type="button"
          class="box"
          :class="{
            'is-on': selectedId === pair.first.id,
            'is-live': currentId === pair.first.id,
            [`is-${nodeState(pair.first)}`]: true
          }"
          @click="onBox(pair.first.id)"
        >
          <span class="idx">1</span>
          <span class="box-body">
            <strong>{{ pair.first.title || roleLabel(pair.first.role) }}</strong>
            <span>{{ describeChannel(pair.first.channel) }}</span>
          </span>
        </button>

        <div class="link">
          <span class="v-line" />
          <button type="button" class="link-tag" :disabled="readonly" @click="toggleMode(pair.forward)">
            完成后 · {{ modeLabel(pair.forward) }}
          </button>
          <span class="v-line" />
        </div>

        <button
          type="button"
          class="box"
          :class="{
            'is-on': selectedId === pair.second.id,
            'is-live': currentId === pair.second.id,
            [`is-${nodeState(pair.second)}`]: true
          }"
          @click="onBox(pair.second.id)"
        >
          <span class="idx">2</span>
          <span class="box-body">
            <strong>{{ pair.second.title || roleLabel(pair.second.role) }}</strong>
            <span>{{ describeChannel(pair.second.channel) }}</span>
          </span>
        </button>

        <div class="link">
          <span class="v-line" />
          <span class="link-tag is-static">通过 · 结束</span>
        </div>
      </div>

      <div class="return" aria-hidden="true">
        <span class="return-arm" />
        <span class="return-label">未通过 · 回到{{ pair.first.title || '开发者' }}</span>
      </div>
    </div>

    <div v-else class="stack">
      <template v-for="(node, index) in flow.nodes" :key="node.id">
        <button
          type="button"
          class="box"
          :class="{
            'is-on': selectedId === node.id,
            'is-live': currentId === node.id,
            [`is-${nodeState(node)}`]: true
          }"
          @click="onBox(node.id)"
        >
          <span class="idx">{{ index + 1 }}</span>
          <span class="box-body">
            <strong>{{ node.title || roleLabel(node.role) }}</strong>
            <span>{{ describeChannel(node.channel) }}</span>
          </span>
        </button>
        <div v-if="outgoing(node.id)" class="link">
          <span class="v-line" />
          <button type="button" class="link-tag" :disabled="readonly" @click="toggleMode(outgoing(node.id)!)">
            <template v-if="outgoing(node.id)!.to === FLOW_END">
              {{ outgoing(node.id)!.gate === 'passFail' ? '通过 · 结束' : '结束' }}
            </template>
            <template v-else>
              完成后 · {{ modeLabel(outgoing(node.id)) }}
            </template>
          </button>
          <p v-if="outgoing(node.id)!.backTo" class="back-note">
            未通过回到 {{ flow.nodes.find((item) => item.id === outgoing(node.id)!.backTo)?.title || '上一角色' }}
          </p>
          <span class="v-line" />
        </div>
      </template>
      <p v-if="!flow.nodes.length" class="empty">从下方角色库点一个角色，放到图上。</p>
    </div>
  </div>
</template>

<style scoped>
.chart {
  min-height: 280px;
  padding: 16px 12px 8px;
}

.pair {
  position: relative;
  max-width: 420px;
  margin: 0 auto;
  padding-right: 108px;
}

.spine {
  display: flex;
  flex-direction: column;
  align-items: stretch;
}

.box {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 56px;
  padding: 10px 12px;
  text-align: left;
  border: 1px solid var(--ad-border);
  border-radius: 10px;
  background: var(--ad-harbor);
  color: var(--ad-text);
}

.box.is-on {
  border-color: var(--ad-border-strong);
  background: var(--ad-selected);
}

.box.is-live,
.box.is-active {
  box-shadow: inset 3px 0 0 var(--ad-warning);
}

.box.is-done {
  box-shadow: inset 3px 0 0 var(--ad-success);
}

.box.is-failed {
  box-shadow: inset 3px 0 0 var(--ad-error);
}

.idx {
  width: 22px;
  height: 22px;
  border-radius: 999px;
  background: var(--ad-raised);
  border: 1px solid var(--ad-border);
  display: grid;
  place-items: center;
  font-size: 11px;
  color: var(--ad-muted);
  flex-shrink: 0;
}

.box-body {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.box-body strong {
  font-size: 13px;
  font-weight: 560;
}

.box-body span {
  font-size: 12px;
  color: var(--ad-muted);
}

.link {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 2px 0;
}

.v-line {
  width: 1px;
  height: 10px;
  background: var(--ad-border-strong);
}

.link-tag {
  height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--ad-border);
  background: var(--ad-raised);
  color: var(--ad-muted);
  font-size: 11px;
}

.link-tag:hover:not(:disabled) {
  color: var(--ad-text);
}

.link-tag.is-static {
  cursor: default;
}

.return {
  position: absolute;
  top: 28px;
  right: 8px;
  bottom: 72px;
  width: 92px;
  pointer-events: none;
}

.return-arm {
  position: absolute;
  inset: 0 18px 0 0;
  border: 1.5px solid var(--ad-warning);
  border-left: none;
  border-radius: 0 10px 10px 0;
}

.return-label {
  position: absolute;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 72px;
  font-size: 11px;
  line-height: 16px;
  color: var(--ad-warning);
}

.back-note {
  margin: 4px 0 0;
  font-size: 11px;
  color: var(--ad-warning);
}

.stack {
  max-width: 360px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  align-items: stretch;
}

.empty {
  margin: 48px 0;
  text-align: center;
  color: var(--ad-muted);
  font-size: 13px;
}
</style>
