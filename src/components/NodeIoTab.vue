<script setup lang="ts">
import { computed } from 'vue'
import { resolveRef } from '../lib/flow/contract.ts'
import { updateNode } from '../lib/flow/runtime.ts'
import type { FlowNode, NodeInput, NodeOutput } from '../lib/flow/types.ts'

const props = defineProps<{
  node: FlowNode
  kind: 'inputs' | 'outputs'
}>()

const inputs = computed(() => props.node.inputs ?? [])
const outputs = computed(() => props.node.outputs ?? [])
const windowed = computed(() => props.node.channel.kind === 'pty' || props.node.channel.kind === 'ompRpc')

function saveInputs(rows: NodeInput[]) {
  updateNode(props.node.id, { inputs: rows.length ? rows : undefined })
}

function saveOutputs(rows: NodeOutput[]) {
  updateNode(props.node.id, { outputs: rows.length ? rows : undefined })
}

function patchInput(index: number, patch: Partial<NodeInput>) {
  saveInputs(inputs.value.map((row, at) => (at === index ? { ...row, ...patch } : row)))
}

function patchOutput(index: number, patch: Partial<NodeOutput>) {
  saveOutputs(outputs.value.map((row, at) => (at === index ? { ...row, ...patch } : row)))
}

function onSections(index: number, event: Event) {
  const text = (event.target as HTMLInputElement).value
  const sections = text.split(/[,，]/).map((item) => item.trim()).filter(Boolean)
  patchOutput(index, { sections: sections.length ? sections : undefined })
}

function refProblem(ref: string, outputOnly = false) {
  if (!ref.trim()) return '还没填'
  if (outputOnly && !ref.startsWith('task:')) return '产物只能写到 task: 下'
  return resolveRef(ref, 'task') ? '' : '写法是 task:文件名 或 project:路径'
}

function value(event: Event) {
  return (event.target as HTMLInputElement).value.trim()
}
</script>

<template>
  <div class="io">
    <template v-if="kind === 'inputs'">
      <p class="hint">节点开工前要读的文件。task: 是本任务目录，project: 是 .agentDock 下的路径。必读的文件缺了，运行前就会拦下。</p>
      <div v-for="(row, index) in inputs" :key="index" class="row-card">
        <div class="line">
          <input :value="row.ref" placeholder="task:design.md" aria-label="输入路径" @change="patchInput(index, { ref: value($event) })" />
          <button type="button" class="text-btn" @click="saveInputs(inputs.filter((_, at) => at !== index))">删除</button>
        </div>
        <p v-if="refProblem(row.ref)" class="warn">{{ refProblem(row.ref) }}</p>
        <div class="checks">
          <label><input type="checkbox" :checked="row.required" @change="patchInput(index, { required: ($event.target as HTMLInputElement).checked || undefined })" />必读</label>
          <label><input type="checkbox" :checked="row.when === 'reworked'" @change="patchInput(index, { when: ($event.target as HTMLInputElement).checked ? 'reworked' : undefined })" />只在返工时读</label>
          <label><input type="checkbox" :checked="row.read === 'index-first'" @change="patchInput(index, { read: ($event.target as HTMLInputElement).checked ? 'index-first' : undefined })" />是索引，只读命中条目</label>
        </div>
      </div>
      <button type="button" class="soft-btn" @click="saveInputs([...inputs, { ref: 'task:', required: true }])">添加输入</button>
    </template>

    <template v-else>
      <p class="hint">节点必须交出的文件，只能写到 task: 下。节点跑完后 Dock 会检查文件存在、段落齐全。</p>
      <div v-for="(row, index) in outputs" :key="index" class="row-card">
        <div class="line">
          <input :value="row.path" placeholder="task:dev-log.md" aria-label="产物路径" @change="patchOutput(index, { path: value($event) })" />
          <button type="button" class="text-btn" @click="saveOutputs(outputs.filter((_, at) => at !== index))">删除</button>
        </div>
        <p v-if="refProblem(row.path, true)" class="warn">{{ refProblem(row.path, true) }}</p>
        <label class="field">
          <span>必须包含的段落（逗号分隔）</span>
          <input :value="(row.sections ?? []).join(', ')" placeholder="## 摘要, ## 结论" @change="onSections(index, $event)" />
        </label>
        <label class="field">
          <span>模板（roles/{{ node.role }}/ 下的文件名，可不填）</span>
          <input :value="row.template ?? ''" placeholder="dev-log.md" @change="patchOutput(index, { template: value($event) || undefined })" />
        </label>
        <label v-if="windowed" class="field">
          <span>谁来写</span>
          <select :value="row.capture ?? 'file'" @change="patchOutput(index, { capture: ($event.target as HTMLSelectElement).value === 'reply' ? 'reply' : undefined })">
            <option value="file">agent 自己写文件</option>
            <option value="reply">Dock 保存它的回复</option>
          </select>
        </label>
      </div>
      <button type="button" class="soft-btn" @click="saveOutputs([...outputs, { path: 'task:' }])">添加产物</button>
    </template>
  </div>
</template>

<style scoped>
.io {
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

.row-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
}

.line {
  display: flex;
  gap: 6px;
  align-items: center;
}

.line input {
  flex: 1;
  min-width: 0;
}

.checks {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  font-size: 12px;
  color: var(--ad-text);
}

.checks label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
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

.warn {
  margin: 0;
  font-size: 12px;
  color: var(--ad-warning);
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
</style>
