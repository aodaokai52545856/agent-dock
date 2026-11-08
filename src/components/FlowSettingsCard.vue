<script setup lang="ts">
import { computed, ref } from 'vue'
import { validateProfiles } from '../lib/flow/preflight.ts'
import { routesByExits } from '../lib/flow/routing.ts'
import { convertCurrentFlow, currentFlow, patchCurrentFlow } from '../lib/flow/runtime.ts'
import type { FlowProfile } from '../lib/flow/types.ts'

const flow = computed(() => currentFlow())
const profiles = computed(() => Object.entries(flow.value?.profiles ?? {}))
const onExits = computed(() => Boolean(flow.value && routesByExits(flow.value)))
const problem = computed(() => (flow.value ? validateProfiles(flow.value) : null))
const newName = ref('')
const conversion = ref<{ notes: string[]; problems: string[] }>({ notes: [], problems: [] })

function saveProfiles(rows: [string, FlowProfile][]) {
  patchCurrentFlow({ profiles: rows.length ? Object.fromEntries(rows) : undefined })
}

function addProfile() {
  const name = newName.value.trim()
  if (!name || profiles.value.some(([key]) => key === name)) return
  saveProfiles([...profiles.value, [name, { skip: [] }]])
  newName.value = ''
}

function toggleSkip(name: string, nodeId: string, skipped: boolean) {
  saveProfiles(profiles.value.map(([key, row]) => (
    key === name
      ? [key, { skip: skipped ? [...row.skip, nodeId] : row.skip.filter((id) => id !== nodeId) }]
      : [key, row]
  )))
}

function onLimit(field: 'maxRetries' | 'maxSteps', event: Event) {
  const value = Math.floor(Number((event.target as HTMLInputElement).value))
  if (value > 0) patchCurrentFlow({ [field]: value })
}

function convert() {
  const result = convertCurrentFlow()
  conversion.value = !result ? { notes: [], problems: [] } : 'problems' in result ? { notes: [], problems: result.problems } : { notes: result.notes, problems: [] }
}
</script>

<template>
  <article v-if="flow" class="card">
    <section>
      <h3>路由</h3>
      <template v-if="onExits">
        <p class="hint">按出口走：每个节点写结论，Dock 按“出口”标签页里的规则走下一步。</p>
        <ul v-if="conversion.notes.length" class="notes">
          <li v-for="item in conversion.notes" :key="item">{{ item }}</li>
        </ul>
      </template>
      <template v-else>
        <p class="hint">这条流程用旧的连线，照常能跑。转换为出口后才能用中段闸门、按责任方退回和 profile。</p>
        <button type="button" class="soft-btn" @click="convert">转换为出口</button>
        <ul v-if="conversion.problems.length" class="warn-list">
          <li v-for="item in conversion.problems" :key="item">{{ item }}，没有转换</li>
        </ul>
      </template>
    </section>

    <section>
      <h3>次数上限</h3>
      <div class="row">
        <label class="field">
          <span>返工次数（节点默认尝试上限 = 这个数 + 1）</span>
          <input type="number" min="1" :value="flow.maxRetries" @change="onLimit('maxRetries', $event)" />
        </label>
        <label class="field">
          <span>整条流程最多步数</span>
          <input type="number" min="1" :value="flow.maxSteps" @change="onLimit('maxSteps', $event)" />
        </label>
      </div>
    </section>

    <section>
      <h3>Profile</h3>
      <p class="hint">同一条流程按场景跳过节点。第一个是默认；被跳过的节点沿它的 PASS 出口接到下一个。</p>
      <p v-if="!onExits" class="warn">先转换为出口，才能用 profile。</p>
      <div v-for="[name, row] in profiles" :key="name" class="profile">
        <div class="line">
          <strong>{{ name }}</strong>
          <button type="button" class="text-btn" @click="saveProfiles(profiles.filter(([key]) => key !== name))">删除</button>
        </div>
        <div class="checks">
          <label v-for="node in flow.nodes" :key="node.id">
            <input type="checkbox" :checked="row.skip.includes(node.id)" @change="toggleSkip(name, node.id, ($event.target as HTMLInputElement).checked)" />
            跳过{{ node.title || node.id }}
          </label>
        </div>
      </div>
      <div class="line">
        <input v-model="newName" placeholder="profile 名称，比如 quick" aria-label="新 profile 名称" @keydown.enter="addProfile" />
        <button type="button" class="soft-btn" :disabled="!newName.trim()" @click="addProfile">添加</button>
      </div>
      <p v-if="problem" class="warn">{{ problem }}</p>
    </section>
  </article>
</template>

<style scoped>
.card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 12px 14px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

h3 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.hint {
  margin: 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
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

.profile {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
}

.line {
  display: flex;
  gap: 6px;
  align-items: center;
  justify-content: space-between;
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
}

.checks label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.warn,
.warn-list,
.notes {
  margin: 0;
  font-size: 12px;
}

.warn,
.warn-list {
  color: var(--ad-warning);
}

.notes {
  color: var(--ad-muted);
}

.warn-list,
.notes {
  padding-left: 16px;
}

.text-btn {
  height: 28px;
  padding: 0 6px;
  color: var(--ad-muted);
  font-size: 12px;
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
  .row {
    grid-template-columns: 1fr;
  }
}
</style>
