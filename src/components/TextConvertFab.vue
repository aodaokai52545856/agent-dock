<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import * as api from '../lib/api'
import { convertReady, convertText, type ConvertMode } from '../lib/textConvert'
import { showToast } from '../lib/store'

const open = ref(false)
const source = ref('')
const result = ref('')
const tableReady = convertReady()

const hasSource = computed(() => source.value.length > 0)
const hasResult = computed(() => result.value.length > 0)
const convertEnabled = computed(() => tableReady && hasSource.value)

function toggle() {
  open.value = !open.value
}

function close() {
  open.value = false
}

function run(mode: ConvertMode) {
  if (!convertEnabled.value) return
  result.value = convertText(source.value, mode)
}

async function copyResult() {
  if (!hasResult.value) return
  const ok = await api.clipboardWrite(result.value)
  showToast(ok ? '已复制' : '复制失败')
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === 'Escape' && open.value) {
    ev.preventDefault()
    close()
  }
}

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="wrap">
    <button
      type="button"
      class="fab"
      aria-label="文本转换"
      :aria-expanded="open"
      @click="toggle"
    >
      文
    </button>
    <div v-if="open" class="panel" role="dialog" aria-label="文本转换">
      <p v-if="!tableReady" class="warn">转换表不可用</p>
      <label class="field">
        <span>原文</span>
        <textarea v-model="source" class="source" spellcheck="false" />
      </label>
      <label class="field">
        <span>结果</span>
        <textarea v-model="result" class="result" spellcheck="false" />
      </label>
      <div class="row">
        <button type="button" class="btn" :disabled="!convertEnabled" @click="run('traditional')">
          转繁体
        </button>
        <button type="button" class="btn" :disabled="!convertEnabled" @click="run('symbols')">
          洗符号
        </button>
        <button type="button" class="btn btn-primary" :disabled="!convertEnabled" @click="run('all')">
          全部转换
        </button>
        <button type="button" class="btn btn-ghost" :disabled="!hasResult" @click="copyResult">
          复制
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wrap {
  position: fixed;
  right: 16px;
  bottom: 56px;
  z-index: 35;
  display: flex;
  flex-direction: column-reverse;
  align-items: flex-end;
  gap: 8px;
}

.fab {
  width: 44px;
  height: 44px;
  border-radius: var(--ad-radius-pill);
  background: var(--ad-primary);
  color: var(--ad-primary-text);
  font-size: 16px;
  font-weight: 600;
  box-shadow: var(--ad-shadow-menu);
}

.fab:hover {
  background: var(--ad-accent-hover);
}

.panel {
  width: 420px;
  max-width: calc(100vw - 32px);
  height: 480px;
  max-height: calc(100vh - 120px);
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  background: var(--ad-float);
  border: 1px solid var(--ad-border);
  border-radius: var(--ad-radius-card);
  box-shadow: var(--ad-shadow-menu);
}

.warn {
  margin: 0;
  color: var(--ad-error);
  font-size: 13px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
  color: var(--ad-muted);
  font-size: 12px;
}

.source,
.result {
  height: auto;
  min-height: 0;
  flex: 1;
  padding: 8px 10px;
  resize: none;
  line-height: 1.55;
  color: var(--ad-text);
  font-size: 13px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: flex-start;
}

.row .btn-ghost {
  margin-left: auto;
}
</style>
