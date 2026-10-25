<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import { isDshWeb } from '../lib/livePty'
import { wrapBracketedPaste } from '../lib/sessionCite'
import { CONVERT_CHARSETS, convertReady, convertText, type ConvertCharset } from '../lib/textConvert'
import { activeLive, showToast, store } from '../lib/store'

const open = ref(false)
const source = ref('')
const result = ref('')
const charset = ref<ConvertCharset>('tw')
const sending = ref(false)
const tableReady = convertReady()

const hasSource = computed(() => source.value.length > 0)
const hasResult = computed(() => result.value.length > 0)
const convertEnabled = computed(() => tableReady && hasSource.value)
const canSend = computed(() => {
  if (!hasResult.value || sending.value) return false
  if (!store.activePtyId) return false
  return !isDshWeb(activeLive.value)
})
const sendTitle = computed(() => {
  if (!hasResult.value) return '先转换出结果'
  if (!store.activePtyId) return '先打开目标会话'
  if (isDshWeb(activeLive.value)) return '当前会话不支持写入'
  return '发送到当前聊天框'
})

function toggle() {
  open.value = !open.value
}

function close() {
  open.value = false
}

function run() {
  if (!convertEnabled.value) return
  result.value = convertText(source.value, charset.value)
}

watch(source, () => {
  result.value = ''
})

watch(charset, () => {
  if (convertEnabled.value && hasResult.value) run()
})

async function copyText(text: string) {
  if (!text) return
  const ok = await api.clipboardWrite(text)
  showToast(ok ? '已复制' : '复制失败')
}

async function sendResult() {
  if (!hasResult.value) return
  if (!store.activePtyId) {
    showToast('先打开目标会话')
    return
  }
  if (isDshWeb(activeLive.value)) {
    showToast('当前会话不支持写入')
    return
  }
  sending.value = true
  try {
    await api.ptyWrite(store.activePtyId, wrapBracketedPaste(result.value) + '\r')
    result.value = ''
    showToast('已发送到当前聊天')
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  } finally {
    sending.value = false
  }
}

function onKey(ev: KeyboardEvent) {
  if (store.appMode !== 'console') return
  if (ev.target instanceof Element && ev.target.closest('[aria-modal="true"]')) return
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
      <div class="field">
        <div class="field-head">
          <span>原文</span>
          <button
            type="button"
            class="btn btn-ghost field-copy"
            :disabled="!hasSource"
            @click="copyText(source)"
          >
            复制
          </button>
        </div>
        <textarea v-model="source" class="source" aria-label="原文" spellcheck="false" />
      </div>
      <div class="field">
        <div class="field-head">
          <span>结果</span>
          <button
            type="button"
            class="btn btn-ghost field-copy"
            :disabled="!hasResult"
            @click="copyText(result)"
          >
            复制
          </button>
        </div>
        <textarea v-model="result" class="result" aria-label="结果" spellcheck="false" />
      </div>
      <div class="row">
        <select
          v-model="charset"
          class="charset"
          aria-label="繁体字符集"
          :disabled="!tableReady"
        >
          <option v-for="item in CONVERT_CHARSETS" :key="item.id" :value="item.id">
            {{ item.label }}
          </option>
        </select>
        <button type="button" class="btn btn-primary" :disabled="!convertEnabled" @click="run">
          全部转换
        </button>
        <button
          type="button"
          class="btn btn-ghost send"
          :disabled="!canSend"
          :aria-label="sendTitle"
          :title="sendTitle"
          @click="sendResult"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M22 2 11 13M22 2 15 22 11 13 2 9 22 2Z"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
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

.field-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.field-copy {
  min-width: 0;
  height: 24px;
  padding: 0 8px;
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
  align-items: center;
  gap: 8px;
}

.charset {
  width: 132px;
  flex: 0 0 132px;
}

.send {
  margin-left: auto;
  min-width: 36px;
  width: 36px;
  padding: 0;
}

.send svg {
  width: 16px;
  height: 16px;
  display: block;
}
</style>
