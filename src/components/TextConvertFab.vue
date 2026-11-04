<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import { isDshWeb } from '../lib/livePty'
import { CONVERT_CHARSETS, convertReady, convertText, type ConvertCharset } from '../lib/textConvert'
import { activeLive, showToast, store } from '../lib/store'

const open = ref(false)
const source = ref('')
const result = ref('')
const charset = ref<ConvertCharset>('tw')
const sending = ref(false)
const tableReady = convertReady()
const FAB_SIZE = 40
const DEFAULT_RIGHT = 16
const DEFAULT_BOTTOM = 56
const viewport = ref({ width: window.innerWidth, height: window.innerHeight })
// Keep the default anchored in CSS; only an intentional drag overrides it for this launch.
const draggedOffset = ref<{ right: number; bottom: number } | null>(null)
const position = computed(() => ({
  x: viewport.value.width - (draggedOffset.value?.right ?? DEFAULT_RIGHT) - FAB_SIZE,
  y: viewport.value.height - (draggedOffset.value?.bottom ?? DEFAULT_BOTTOM) - FAB_SIZE
}))
const fabPosition = computed(() => draggedOffset.value && ({
  right: `${draggedOffset.value.right}px`,
  bottom: `${draggedOffset.value.bottom}px`
}))
const dragging = ref(false)
let pointer: { id: number; x: number; y: number; originX: number; originY: number; moved: boolean } | null = null
let suppressClick = false

function moveTo(x: number, y: number) {
  const left = Math.max(8, Math.min(x, viewport.value.width - FAB_SIZE - 8))
  const top = Math.max(8, Math.min(y, viewport.value.height - FAB_SIZE - 8))
  draggedOffset.value = {
    right: viewport.value.width - left - FAB_SIZE,
    bottom: viewport.value.height - top - FAB_SIZE
  }
}

function resize() {
  viewport.value = { width: window.innerWidth, height: window.innerHeight }
  if (draggedOffset.value) moveTo(position.value.x, position.value.y)
}

const panelPosition = computed(() => {
  const width = Math.min(420, viewport.value.width - 32)
  const roomAbove = Math.max(0, position.value.y - 24)
  const roomBelow = Math.max(0, viewport.value.height - position.value.y - FAB_SIZE - 24)
  const height = Math.min(480, Math.max(roomAbove, roomBelow))
  const above = position.value.y - height - 8
  return {
    left: `${Math.max(16, Math.min(position.value.x + FAB_SIZE - width, viewport.value.width - width - 16))}px`,
    top: `${Math.max(16, Math.min(above >= 16 ? above : position.value.y + FAB_SIZE + 8, viewport.value.height - height - 16))}px`,
    height: `${height}px`
  }
})

function startDrag(event: PointerEvent) {
  if (event.button !== 0 || !event.isPrimary || pointer) return
  const target = event.currentTarget as HTMLElement
  const bounds = target.getBoundingClientRect()
  pointer = { id: event.pointerId, x: event.clientX, y: event.clientY, originX: bounds.x, originY: bounds.y, moved: false }
  suppressClick = false
  target.setPointerCapture(event.pointerId)
}

function moveDrag(event: PointerEvent) {
  if (!pointer || pointer.id !== event.pointerId) return
  const dx = event.clientX - pointer.x
  const dy = event.clientY - pointer.y
  if (!pointer.moved && Math.hypot(dx, dy) < 5) return
  pointer.moved = true
  dragging.value = true
  moveTo(pointer.originX + dx, pointer.originY + dy)
  event.preventDefault()
}

function endDrag(event: PointerEvent) {
  if (!pointer || pointer.id !== event.pointerId) return
  suppressClick = pointer.moved
  pointer = null
  dragging.value = false
  const target = event.currentTarget as HTMLElement
  if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId)
}

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

function toggle(event: MouseEvent) {
  const wasDrag = suppressClick && event.detail > 0
  suppressClick = false
  if (wasDrag) return
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
    await api.writeSessionText(store.activePtyId, result.value, true)
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

onMounted(() => {
  resize()
  window.addEventListener('keydown', onKey)
  window.addEventListener('resize', resize)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('resize', resize)
})
</script>

<template>
  <div class="wrap" :style="fabPosition ?? undefined">
    <button
      type="button"
      class="fab"
      :class="{ 'is-dragging': dragging, 'is-open': open }"
      aria-label="文本转换"
      title="文本转换（拖动可调整位置）"
      aria-haspopup="dialog"
      :aria-expanded="open"
      @click="toggle"
      @pointerdown="startDrag"
      @pointermove="moveDrag"
      @pointerup="endDrag"
      @pointercancel="endDrag"
      @lostpointercapture="endDrag"
    >
      文
    </button>
    <div v-if="open" class="panel" :style="panelPosition" role="dialog" aria-label="文本转换">
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
  width: 40px;
  height: 40px;
  z-index: 35;
}

.fab {
  display: grid;
  place-items: center;
  touch-action: none;
  user-select: none;
  cursor: grab;
  width: 100%;
  height: 100%;
  padding: 0;
  border: 1px solid var(--ad-border-strong);
  border-radius: var(--ad-radius-control);
  background: var(--ad-float-solid);
  color: var(--ad-muted);
  font-family: var(--ad-display);
  font-size: 16px;
  font-weight: 600;
  line-height: 1;
  box-shadow: 0 2px 8px color-mix(in srgb, var(--ad-mask) 30%, transparent);
  transition: background var(--ad-transition), color var(--ad-transition), border-color var(--ad-transition), box-shadow var(--ad-transition);
}

.fab:hover,
.fab.is-open,
.fab.is-dragging {
  border-color: var(--ad-muted);
  background: var(--ad-float-inset-solid);
  color: var(--ad-text);
}

.fab:focus-visible {
  outline: 2px solid var(--ad-accent);
  outline-offset: 3px;
}

.fab.is-dragging {
  cursor: grabbing;
  transition: none;
}

.panel {
  position: fixed;
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
