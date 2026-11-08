<script setup lang="ts">
import { computed, ref } from 'vue'
import { EXIT_END, EXIT_HUMAN, validateExits } from '../lib/flow/exits.ts'
import { CONCLUSION_OPTIONS, routesByExits } from '../lib/flow/routing.ts'
import { convertCurrentFlow, currentFlow, updateNode } from '../lib/flow/runtime.ts'
import type { ConclusionOn, FlowNode, NodeExit } from '../lib/flow/types.ts'

const props = defineProps<{
  node: FlowNode
}>()

const exits = computed(() => props.node.exits ?? [])
const flow = computed(() => currentFlow())
const onEdges = computed(() => Boolean(flow.value && !routesByExits(flow.value)))
const targets = computed(() => [
  ...(flow.value?.nodes ?? []).map((node) => ({ id: node.id, label: node.id === props.node.id ? `${node.title}（自己，重做）` : node.title || node.id })),
  { id: EXIT_END, label: '结束' },
  { id: EXIT_HUMAN, label: '人工决定' }
])
// The run is refused for the same reason, so say it here, next to the rows.
const problem = computed(() => {
  const message = flow.value ? validateExits(flow.value) : null
  return message?.includes(`「${props.node.title || props.node.id}」`) ? message : null
})
const conversion = ref<string[]>([])
const defaultLimit = computed(() => (flow.value?.maxRetries || 0) + 1)

function save(rows: NodeExit[]) {
  updateNode(props.node.id, { exits: rows.length ? rows : undefined })
}

function patch(index: number, change: Partial<NodeExit>) {
  save(exits.value.map((row, at) => {
    if (at !== index) return row
    const next = { ...row, ...change }
    // PASS takes no owner: anything after PASS is read as doubt, not a pass.
    if (next.on === 'PASS') delete next.owner
    return next
  }))
}

function add() {
  const used = new Set(exits.value.filter((row) => !row.owner).map((row) => row.on))
  const on = CONCLUSION_OPTIONS.find((item) => !used.has(item)) ?? 'REJECT'
  save([...exits.value, { on, to: on === 'PASS' ? EXIT_END : EXIT_HUMAN }])
}

function onOwner(index: number, event: Event) {
  const owner = (event.target as HTMLInputElement).value.trim()
  patch(index, { owner: owner || undefined })
}

function onLimit(event: Event) {
  const value = Math.floor(Number((event.target as HTMLInputElement).value))
  updateNode(props.node.id, { maxAttempts: value > 0 ? value : undefined })
}

function convert() {
  const result = convertCurrentFlow()
  conversion.value = result && 'problems' in result ? result.problems : []
}
</script>

<template>
  <div class="exits">
    <template v-if="onEdges">
      <p class="hint">这条流程还在用旧的连线。转换后每个节点按结论走出口，可以设中段闸门和按责任方退回。</p>
      <button type="button" class="soft-btn" @click="convert">把整条流程转换为出口</button>
      <ul v-if="conversion.length" class="warn-list">
        <li v-for="item in conversion" :key="item">{{ item }}，没有转换</li>
      </ul>
    </template>

    <template v-else>
      <p class="hint">节点最后一行写 VERDICT: 结论，Dock 按下面的出口走。读不出结论或没有对应出口，会暂停等人工。只有 PASS 能走到结束。</p>
      <div v-for="(row, index) in exits" :key="index" class="exit-row">
        <select :value="row.on" aria-label="结论" @change="patch(index, { on: ($event.target as HTMLSelectElement).value as ConclusionOn })">
          <option v-for="item in CONCLUSION_OPTIONS" :key="item" :value="item">{{ item }}</option>
        </select>
        <input
          :value="row.owner ?? ''"
          :disabled="row.on === 'PASS'"
          :placeholder="row.on === 'PASS' ? '无责任方' : '责任方（可不填）'"
          aria-label="责任方"
          @change="onOwner(index, $event)"
        />
        <select :value="row.to" aria-label="去向" @change="patch(index, { to: ($event.target as HTMLSelectElement).value })">
          <option v-for="item in targets" :key="item.id" :value="item.id">{{ item.label }}</option>
        </select>
        <button type="button" class="text-btn" @click="save(exits.filter((_, at) => at !== index))">删除</button>
      </div>
      <button type="button" class="soft-btn" @click="add">添加出口</button>
      <p v-if="problem" class="warn">{{ problem }}</p>

      <label class="field">
        <span>最多尝试次数</span>
        <input type="number" min="1" :value="node.maxAttempts ?? ''" :placeholder="`默认 ${defaultLimit}`" @change="onLimit" />
      </label>
      <p class="hint">被退回的次数用完就暂停等人工，不会无限循环。</p>
    </template>
  </div>
</template>

<style scoped>
.exits {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.exit-row {
  display: grid;
  grid-template-columns: 92px 1fr 1.3fr auto;
  gap: 6px;
  align-items: center;
}

.exit-row input,
.exit-row select {
  min-width: 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field > span {
  font-size: 12px;
  color: var(--ad-muted);
}

.warn,
.warn-list {
  margin: 0;
  font-size: 12px;
  color: var(--ad-warning);
}

.warn-list {
  padding-left: 16px;
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

.soft-btn {
  align-self: flex-start;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
  font-size: 12px;
  color: var(--ad-text);
}

@media (max-width: 640px) {
  .exit-row {
    grid-template-columns: 1fr 1fr;
  }
}
</style>
