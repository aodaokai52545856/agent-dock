<script setup lang="ts">
import { reactive, watch } from 'vue'
import type { AppSettings } from '../lib/types'
import {
  UI_OPACITY_MAX,
  UI_OPACITY_MIN,
  applyUiOpacity,
  clampUiOpacity
} from '../lib/store'

const props = defineProps<{
  open: boolean
  settings: AppSettings
}>()

const emit = defineEmits<{
  close: []
  save: [settings: AppSettings]
}>()

const form = reactive({
  terminalFontSize: props.settings.terminalFontSize,
  uiOpacity: clampUiOpacity(props.settings.uiOpacity),
  cursorApiKey: props.settings.cursorApiKey ?? '',
  codexPath: props.settings.codexPath ?? '',
  saving: false
})

watch(
  () => [props.open, props.settings] as const,
  ([open]) => {
    form.terminalFontSize = props.settings.terminalFontSize
    form.uiOpacity = clampUiOpacity(props.settings.uiOpacity)
    form.cursorApiKey = props.settings.cursorApiKey ?? ''
    form.codexPath = props.settings.codexPath ?? ''
    form.saving = false
    if (!open) applyUiOpacity(props.settings.uiOpacity)
  }
)

function previewOpacity() {
  applyUiOpacity(form.uiOpacity)
}

function submit() {
  const size = Number(form.terminalFontSize) || 13
  form.saving = true
  emit('save', {
    ...props.settings,
    terminalFontSize: Math.min(22, Math.max(10, size)),
    uiOpacity: clampUiOpacity(form.uiOpacity),
    cursorApiKey: form.cursorApiKey.trim(),
    codexPath: form.codexPath.trim()
  })
}

function stopSave() {
  form.saving = false
}

defineExpose({ stopSave })
</script>

<template>
  <div v-if="open" class="ad-mask ad-mask--flush" @click.self="emit('close')">
    <aside class="ad-drawer drawer" aria-label="设置">
      <header>
        <h2>设置</h2>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>
      <label class="field">
        <span>终端字号</span>
        <input v-model.number="form.terminalFontSize" type="number" min="10" max="22" />
      </label>
      <p class="hint">只影响右侧命令行窗口的字号，已打开的终端会立刻套用。</p>
      <label class="field">
        <span>界面透明度 <em>{{ form.uiOpacity }}%</em></span>
        <input
          v-model.number="form.uiOpacity"
          type="range"
          :min="UI_OPACITY_MIN"
          :max="UI_OPACITY_MAX"
          step="1"
          @input="previewOpacity"
        />
      </label>
      <p class="hint">数字越大，越能透过窗口看到桌面。终端里的字仍保持不透明。拖动即可预览，点保存后写入本机。</p>
      <label class="field">
        <span>Codex 路径</span>
        <input v-model="form.codexPath" type="text" placeholder="留空则从 PATH 查找 codex" />
      </label>
      <p class="hint">编排模式用 `codex app-server` 列线程并做 detached 审查，不走内嵌终端。</p>
      <label class="field">
        <span>Cursor API Key</span>
        <input v-model="form.cursorApiKey" type="password" autocomplete="off" placeholder="只用于自动开发，可留空" />
      </label>
      <p class="hint">写在本机配置里。Cursor SDK 与 IDE 订阅分开计费。半自动审查不需要这项。</p>
      <div class="actions">
        <button type="button" class="btn btn-primary" :disabled="form.saving" @click="submit">
          {{ form.saving ? '保存中…' : '保存' }}
        </button>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.drawer {
  width: 400px;
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 24px;
}

h2 {
  margin: 0;
  font-size: 16px;
  line-height: 24px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}

.field em {
  margin-left: 8px;
  font-style: normal;
  color: var(--ad-muted);
  font-variant-numeric: tabular-nums;
}

.hint {
  margin: 0 0 24px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.actions {
  margin-top: 8px;
}
</style>
