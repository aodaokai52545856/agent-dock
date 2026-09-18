<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { describeChannel, inspectPty } from '../lib/flow/channels.ts'
import {
  END_H,
  NODE_H,
  NODE_W,
  START_H,
  backPath,
  bottomAnchor,
  canvasSize,
  forwardPath,
  hitNode,
  nodePoint,
  pathMid,
  rightAnchor,
  snap,
  terminals,
  topAnchor,
  withNodePoints
} from '../lib/flow/chartLayout.ts'
import { roleLabel } from '../lib/flow/roles.ts'
import {
  addRoleNodeAt,
  connectFlowNodes,
  disconnectEdge,
  moveFlowNode,
  moveTerminal,
  removeRoleNode,
  setEdgeMode,
  clearSelection,
  setSelectedEdge,
  setSelectedNode
} from '../lib/flow/runtime.ts'
import { FLOW_END, FLOW_START, type FlowDef, type FlowEdge, type FlowNode } from '../lib/flow/types.ts'
import { currentRun, store } from '../lib/store'

const props = defineProps<{
  flow: FlowDef
  readonly?: boolean
}>()

type NodeDrag = { id: string; x: number; y: number; grabX: number; grabY: number }
type TermDrag = { which: 'start' | 'end'; x: number; y: number; grabX: number; grabY: number }

const chartRef = ref<HTMLElement | null>(null)
const surfaceRef = ref<HTMLElement | null>(null)
const livePos = ref<NodeDrag | null>(null)
const liveTerm = ref<TermDrag | null>(null)
const link = ref<{ fromId: string; x: number; y: number } | null>(null)
const dragging = ref(false)
const startPt = ref({ x: 0, y: 0 })

const placed = computed(() => withNodePoints(props.flow))
const size = computed(() => {
  const box = canvasSize(placed.value)
  return {
    width: Math.max(box.width, 520),
    height: Math.max(box.height, 360)
  }
})
const selectedId = computed(() => store.bridgeSelectedNodeId)
const selectedEdgeId = computed(() => store.bridgeSelectedEdgeId)
const currentId = computed(() => currentRun.value?.currentNodeId || '')

const docks = computed(() => {
  const box = terminals(placed.value)
  if (liveTerm.value?.which === 'start') return { ...box, start: { x: liveTerm.value.x, y: liveTerm.value.y } }
  if (liveTerm.value?.which === 'end') return { ...box, end: { x: liveTerm.value.x, y: liveTerm.value.y } }
  return box
})

function xy(node: FlowNode, index: number) {
  if (livePos.value?.id === node.id) return livePos.value
  return nodePoint(node, index)
}

function channelText(node: FlowNode) {
  return describeChannel(node.channel, store.live)
}

function nodeWarn(node: FlowNode) {
  if (node.channel.kind !== 'pty') return false
  return !inspectPty(node.channel, store.live, store.selectedProjectId).ok
}

function nodeState(node: FlowNode) {
  const run = currentRun.value
  if (!run || run.flowId !== props.flow.id) return 'idle'
  if (run.currentNodeId === node.id && (run.status === 'running' || run.status === 'waiting')) return 'active'
  if (run.steps.some((step) => step.nodeId === node.id && step.status === 'completed')) return 'done'
  if (run.steps.some((step) => step.nodeId === node.id && step.status === 'failed')) return 'failed'
  return 'idle'
}

function modeLabel(edge: FlowEdge) {
  return edge.mode === 'auto' ? '自动' : '手动'
}

function findNode(id: string) {
  return placed.value.nodes.find((item) => item.id === id) ?? null
}

function findIndex(id: string) {
  return placed.value.nodes.findIndex((item) => item.id === id)
}

type DrawnEdge = {
  edge: FlowEdge
  from: { x: number; y: number }
  to: { x: number; y: number }
  d: string
  mid: { x: number; y: number }
  back?: { d: string; mid: { x: number; y: number }; title: string }
}

function sourceAnchor(fromId: string) {
  if (fromId === FLOW_START) {
    const box = docks.value
    return { x: box.start.x + box.w / 2, y: box.start.y + START_H }
  }
  const fromNode = findNode(fromId)
  const fromIndex = findIndex(fromId)
  if (!fromNode) return { x: 0, y: 0 }
  return bottomAnchor({ ...fromNode, ...xy(fromNode, fromIndex) })
}

function targetAnchor(toId: string) {
  if (!toId || toId === FLOW_END) {
    const box = docks.value
    return { x: box.end.x + box.w / 2, y: box.end.y }
  }
  const toNode = findNode(toId)
  const toIndex = findIndex(toId)
  if (!toNode) return { x: 0, y: 0 }
  return topAnchor({ ...toNode, ...xy(toNode, toIndex) })
}

const drawn = computed((): DrawnEdge[] => {
  return placed.value.edges.map((edge) => {
    const fromNode = findNode(edge.from)
    const fromIndex = findIndex(edge.from)
    const toNode = findNode(edge.to)
    const toIndex = findIndex(edge.to)
    if (edge.gate === 'loop' && fromNode && toNode) {
      const origin = rightAnchor({ ...fromNode, ...xy(fromNode, fromIndex) })
      const target = rightAnchor({ ...toNode, ...xy(toNode, toIndex) })
      return {
        edge,
        from: origin,
        to: target,
        d: backPath(origin, target),
        mid: { x: Math.max(origin.x, target.x) + 56, y: (origin.y + target.y) / 2 }
      }
    }
    const start = sourceAnchor(edge.from)
    const end = targetAnchor(edge.to)
    const row: DrawnEdge = {
      edge,
      from: start,
      to: end,
      d: forwardPath(start, end),
      mid: pathMid(start, end)
    }
    if (edge.gate === 'passFail' && edge.backTo) {
      const backNode = findNode(edge.backTo)
      const backIndex = findIndex(edge.backTo)
      if (fromNode && backNode) {
        const origin = rightAnchor({ ...fromNode, ...xy(fromNode, fromIndex) })
        const target = rightAnchor({ ...backNode, ...xy(backNode, backIndex) })
        row.back = {
          d: backPath(origin, target),
          mid: { x: Math.max(origin.x, target.x) + 56, y: (origin.y + target.y) / 2 },
          title: backNode.title || roleLabel(backNode.role)
        }
      }
    }
    return row
  })
})

function surfacePoint(event: PointerEvent | DragEvent) {
  const el = surfaceRef.value
  if (!el) return { x: 0, y: 0 }
  const rect = el.getBoundingClientRect()
  return { x: event.clientX - rect.left, y: event.clientY - rect.top }
}

function inDock(which: 'start' | 'end', x: number, y: number) {
  const box = docks.value
  const point = which === 'start' ? box.start : box.end
  const height = which === 'start' ? START_H : END_H
  return x >= point.x && x <= point.x + box.w && y >= point.y && y <= point.y + height
}

function onTermDown(event: PointerEvent, which: 'start' | 'end') {
  if (props.readonly) return
  const handle = (event.target as HTMLElement).closest('[data-handle]')
  const point = surfacePoint(event)
  startPt.value = point
  dragging.value = false
  if (which === 'start' && handle) {
    link.value = { fromId: FLOW_START, x: point.x, y: point.y }
  } else {
    const origin = which === 'start' ? docks.value.start : docks.value.end
    liveTerm.value = {
      which,
      x: origin.x,
      y: origin.y,
      grabX: point.x - origin.x,
      grabY: point.y - origin.y
    }
  }
  surfaceRef.value?.setPointerCapture(event.pointerId)
}

function onDeleteNode(event: Event, nodeId: string) {
  event.preventDefault()
  event.stopPropagation()
  if (props.readonly) return
  removeRoleNode(nodeId)
}

function onKey(event: KeyboardEvent) {
  if (props.readonly) return
  if (event.key !== 'Delete' && event.key !== 'Backspace') return
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, [contenteditable="true"]')) return
  event.preventDefault()
  if (store.bridgeSelectedEdgeId) {
    disconnectEdge(store.bridgeSelectedEdgeId)
    return
  }
  if (store.bridgeSelectedNodeId) removeRoleNode(store.bridgeSelectedNodeId)
}

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))

function onNodeDown(event: PointerEvent, node: FlowNode, index: number) {
  if ((event.target as HTMLElement).closest('[data-delete]')) return
  setSelectedNode(node.id)
  if (props.readonly) return
  const handle = (event.target as HTMLElement).closest('[data-handle]')
  const point = surfacePoint(event)
  startPt.value = point
  dragging.value = false
  if (handle) {
    link.value = { fromId: node.id, x: point.x, y: point.y }
  } else {
    const origin = xy(node, index)
    livePos.value = {
      id: node.id,
      x: origin.x,
      y: origin.y,
      grabX: point.x - origin.x,
      grabY: point.y - origin.y
    }
  }
  surfaceRef.value?.setPointerCapture(event.pointerId)
}

function onPointerMove(event: PointerEvent) {
  if (props.readonly) return
  const point = surfacePoint(event)
  if (Math.hypot(point.x - startPt.value.x, point.y - startPt.value.y) > 4) dragging.value = true
  if (link.value) {
    link.value = { ...link.value, x: point.x, y: point.y }
    return
  }
  if (liveTerm.value) {
    liveTerm.value = {
      ...liveTerm.value,
      x: point.x - liveTerm.value.grabX,
      y: point.y - liveTerm.value.grabY
    }
    return
  }
  const pos = livePos.value
  if (!pos) return
  livePos.value = {
    id: pos.id,
    x: point.x - pos.grabX,
    y: point.y - pos.grabY,
    grabX: pos.grabX,
    grabY: pos.grabY
  }
}

function onPointerUp(event: PointerEvent) {
  const point = surfacePoint(event)
  const wasDragging = dragging.value
  const linking = Boolean(link.value)
  if (link.value) {
    if (dragging.value) {
      if (inDock('end', point.x, point.y)) {
        connectFlowNodes(link.value.fromId, FLOW_END)
      } else if (!inDock('start', point.x, point.y)) {
        const hit = hitNode(placed.value, point.x, point.y)
        if (hit && hit.id !== link.value.fromId) connectFlowNodes(link.value.fromId, hit.id)
      }
    }
    link.value = null
  } else if (liveTerm.value) {
    if (dragging.value) moveTerminal(liveTerm.value.which, liveTerm.value.x, liveTerm.value.y)
    liveTerm.value = null
  } else if (livePos.value) {
    if (dragging.value) moveFlowNode(livePos.value.id, livePos.value.x, livePos.value.y)
    livePos.value = null
  }
  dragging.value = false
  if (props.readonly || wasDragging || linking) return
  const target = event.target as HTMLElement
  if (target.closest('.box, .edge-tag, .dock, .handle, .node-del')) return
  if (hitNode(placed.value, point.x, point.y)) return
  if (inDock('start', point.x, point.y) || inDock('end', point.x, point.y)) return
  clearSelection()
}

function onEdgeClick(edge: FlowEdge, event: MouseEvent) {
  event.stopPropagation()
  setSelectedEdge(edge.id)
}

function toggleMode(edge: FlowEdge) {
  if (props.readonly) return
  setEdgeMode(edge.id, edge.mode === 'auto' ? 'manual' : 'auto')
}

function onDragOver(event: DragEvent) {
  if (props.readonly) return
  if (!event.dataTransfer?.types.includes('application/x-agent-dock-role')) return
  event.preventDefault()
  event.dataTransfer.dropEffect = 'copy'
}

function onDrop(event: DragEvent) {
  if (props.readonly) return
  const role = event.dataTransfer?.getData('application/x-agent-dock-role')
  if (!role) return
  event.preventDefault()
  const point = surfacePoint(event)
  addRoleNodeAt(role, { x: snap(point.x - NODE_W / 2), y: snap(point.y - NODE_H / 2) })
}

const linkLine = computed(() => {
  if (!link.value) return null
  const start = sourceAnchor(link.value.fromId)
  return { d: forwardPath(start, { x: link.value.x, y: link.value.y }) }
})
</script>

<template>
  <div
    ref="chartRef"
    class="chart"
    :class="{ 'is-readonly': readonly, 'is-linking': Boolean(link) }"
    aria-label="流程图"
  >
    <div
      ref="surfaceRef"
      class="surface"
      :style="{ width: size.width + 'px', height: size.height + 'px' }"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
      @dragover="onDragOver"
      @drop="onDrop"
    >
      <svg class="wires" :width="size.width" :height="size.height" aria-hidden="true">
        <path
          v-for="item in drawn"
          :key="item.edge.id"
          class="wire"
          :class="{ 'is-on': selectedEdgeId === item.edge.id, 'wire-back': item.edge.gate === 'loop' }"
          :d="item.d"
        />
        <path
          v-for="item in drawn.filter((row) => row.back)"
          :key="item.edge.id + '-back'"
          class="wire wire-back"
          :d="item.back!.d"
        />
        <path v-if="linkLine" class="wire wire-draft" :d="linkLine.d" />
      </svg>

      <button
        v-for="item in drawn"
        :key="item.edge.id + '-tag'"
        type="button"
        class="edge-tag"
        :class="{ 'is-on': selectedEdgeId === item.edge.id, 'is-loop': item.edge.gate === 'loop' }"
        :style="{ left: item.mid.x + 'px', top: item.mid.y + 'px' }"
        :disabled="readonly"
        @click="onEdgeClick(item.edge, $event)"
        @dblclick.stop="toggleMode(item.edge)"
      >
        <template v-if="item.edge.gate === 'loop'">循环 · {{ item.edge.maxLoops || 3 }} 次</template>
        <template v-else-if="item.edge.to === FLOW_END">
          {{ item.edge.gate === 'passFail' ? '通过 · 结束' : '结束' }}
        </template>
        <template v-else>完成后 · {{ modeLabel(item.edge) }}</template>
      </button>

      <span
        v-for="item in drawn.filter((row) => row.back)"
        :key="item.edge.id + '-back-tag'"
        class="back-tag"
        :style="{ left: item.back!.mid.x + 'px', top: item.back!.mid.y + 'px' }"
      >
        未通过 · {{ item.back!.title }}
      </span>

      <div
        class="dock dock-start"
        :style="{ left: docks.start.x + 'px', top: docks.start.y + 'px', width: docks.w + 'px', height: START_H + 'px' }"
        @pointerdown="onTermDown($event, 'start')"
      >
        <strong>开始</strong>
        <span v-if="!readonly" class="handle" data-handle title="拖到第一个角色" />
      </div>

      <div
        class="dock dock-end"
        :style="{ left: docks.end.x + 'px', top: docks.end.y + 'px', width: docks.w + 'px', height: END_H + 'px' }"
        @pointerdown="onTermDown($event, 'end')"
      >
        <strong>结束</strong>
      </div>

      <div
        v-for="(node, index) in placed.nodes"
        :key="node.id"
        class="box"
        :class="{
          'is-on': selectedId === node.id,
          'is-live': currentId === node.id,
          [`is-${nodeState(node)}`]: true
        }"
        :style="{ left: xy(node, index).x + 'px', top: xy(node, index).y + 'px', width: NODE_W + 'px', height: NODE_H + 'px' }"
        @pointerdown="onNodeDown($event, node, index)"
      >
        <span class="idx">{{ index + 1 }}</span>
        <span class="box-body">
          <strong>{{ node.title || roleLabel(node.role) }}</strong>
          <span>{{ channelText(node) }}</span>
        </span>
        <span v-if="nodeWarn(node)" class="warn" title="未绑定或窗口已关闭">!</span>
        <button
          v-if="!readonly"
          type="button"
          class="node-del"
          data-delete
          title="删除节点"
          @pointerdown.stop
          @click.stop="onDeleteNode($event, node.id)"
        >
          ×
        </button>
        <span v-if="!readonly" class="handle" data-handle title="拖到下一节点，或拖到「结束」" />
      </div>

      <p v-if="!placed.nodes.length" class="empty">把角色拖进画布，再从「开始」拉线。</p>
      <p v-else-if="!readonly" class="hint">审查拉回开发会变成循环边。点那条线可改最大次数。用尽后走接到「结束」的线。</p>
    </div>
  </div>
</template>

<style scoped>
.chart {
  flex: 1;
  min-height: 240px;
  overflow: auto;
  position: relative;
}

.chart.is-linking {
  cursor: crosshair;
}

.surface {
  position: relative;
  min-width: 100%;
  min-height: 100%;
  background-image:
    linear-gradient(to right, rgba(236, 236, 236, 0.04) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(236, 236, 236, 0.04) 1px, transparent 1px);
  background-size: 16px 16px;
  background-position: 0 0;
}

.wires {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.wire {
  fill: none;
  stroke: var(--ad-border-strong);
  stroke-width: 1.6;
}

.wire.is-on {
  stroke: var(--ad-text);
}

.wire-back {
  stroke: var(--ad-warning);
  stroke-dasharray: 5 4;
}

.edge-tag.is-loop {
  color: var(--ad-warning);
  border-color: var(--ad-warning);
}

.wire-draft {
  stroke: var(--ad-text);
  stroke-dasharray: 4 3;
}

.box {
  position: absolute;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 28px 10px 12px;
  text-align: left;
  border: 1px solid var(--ad-border);
  border-radius: 10px;
  background: var(--ad-harbor);
  color: var(--ad-text);
  box-sizing: border-box;
  user-select: none;
  touch-action: none;
  cursor: grab;
}

.box.is-on {
  border-color: var(--ad-border-strong);
  background: var(--ad-selected);
  z-index: 3;
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
  flex: 1;
}

.box-body strong {
  font-size: 13px;
  font-weight: 560;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.box-body span {
  font-size: 12px;
  color: var(--ad-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.warn {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 999px;
  background: var(--ad-error);
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  display: grid;
  place-items: center;
}

.node-del {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--ad-muted);
  font-size: 16px;
  line-height: 20px;
  cursor: pointer;
}

.node-del:hover {
  color: var(--ad-error);
  background: rgba(226, 75, 74, 0.16);
}

.handle {
  position: absolute;
  left: 50%;
  bottom: -7px;
  width: 12px;
  height: 12px;
  margin-left: -6px;
  border-radius: 999px;
  border: 2px solid var(--ad-text);
  background: var(--ad-harbor);
  cursor: crosshair;
  z-index: 3;
}

.dock {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--ad-border-strong);
  border-radius: 10px;
  background: var(--ad-raised);
  color: var(--ad-text);
  box-sizing: border-box;
  user-select: none;
  touch-action: none;
  cursor: grab;
  z-index: 2;
}

.dock strong {
  font-size: 13px;
  font-weight: 560;
}

.dock-end {
  border-style: solid;
}

.edge-tag,
.back-tag {
  position: absolute;
  transform: translate(-50%, -50%);
  height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--ad-border);
  background: var(--ad-raised);
  color: var(--ad-muted);
  font-size: 11px;
  line-height: 20px;
  white-space: nowrap;
  z-index: 1;
}

.edge-tag.is-on {
  border-color: var(--ad-border-strong);
  color: var(--ad-text);
}

.back-tag {
  color: var(--ad-warning);
  border-style: dashed;
  border-color: var(--ad-warning);
  pointer-events: none;
}

.empty,
.hint {
  position: absolute;
  left: 48px;
  color: var(--ad-muted);
  font-size: 12px;
  pointer-events: none;
}

.empty {
  top: 96px;
  font-size: 13px;
}

.hint {
  bottom: 12px;
  left: 16px;
}
</style>
