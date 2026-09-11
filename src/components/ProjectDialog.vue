<script setup lang="ts">
import { open as pickDirectory } from '@tauri-apps/plugin-dialog'
import { computed, reactive, watch } from 'vue'
import * as api from '../lib/api'
import type { Project, ProjectDraft } from '../lib/types'

const props = defineProps<{
  visible: boolean
  project: Project | null
  defaultProxyUrl: string
}>()

const emit = defineEmits<{
  close: []
  save: [draft: ProjectDraft]
}>()

const form = reactive({
  name: '',
  path: '',
  proxyEnabled: false,
  proxyUrl: '',
  pathError: '',
  proxyError: '',
  saving: false
})

const title = computed(() => (props.project ? '编辑项目' : '添加项目'))

watch(
  () => [props.visible, props.project] as const,
  () => {
    if (!props.visible) return
    form.name = props.project?.name ?? ''
    form.path = props.project?.path ?? ''
    form.proxyEnabled = props.project?.proxyEnabled ?? false
    form.proxyUrl = props.project?.proxyUrl || props.defaultProxyUrl
    form.pathError = ''
    form.proxyError = ''
    form.saving = false
  }
)

async function pickFolder() {
  if (!api.isTauri) {
    form.pathError = '请在桌面窗口里选择文件夹'
    return
  }
  const selected = await pickDirectory({ directory: true, multiple: false, title: '选择项目文件夹' })
  if (typeof selected !== 'string' || !selected) return
  form.path = selected
  form.pathError = ''
  if (!form.name.trim()) {
    form.name = await api.folderLabel(selected)
  }
}

function validateProxy() {
  if (!form.proxyEnabled) {
    form.proxyError = ''
    return true
  }
  const url = form.proxyUrl.trim()
  if (!url) {
    form.proxyError = '代理地址不能为空，请填写 http:// 或 https:// 开头的地址'
    return false
  }
  if (!/^https?:\/\//i.test(url)) {
    form.proxyError = '代理地址格式不正确，请使用 http://127.0.0.1:7890 这种形式'
    return false
  }
  form.proxyError = ''
  return true
}

function submit() {
  if (!form.path.trim()) {
    form.pathError = '请选择项目文件夹'
    return
  }
  if (!form.name.trim()) {
    form.pathError = '项目名称不能为空'
    return
  }
  if (!validateProxy()) return
  form.saving = true
  emit('save', {
    name: form.name.trim(),
    path: form.path.trim(),
    proxyEnabled: form.proxyEnabled,
    proxyUrl: form.proxyUrl.trim() || props.defaultProxyUrl
  })
}

function stopSave() {
  form.saving = false
}

defineExpose({ stopSave })
</script>

<template>
  <div v-if="visible" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" :aria-label="title">
      <h2>{{ title }}</h2>
      <label class="field">
        <span>文件夹 <i>*</i></span>
        <div class="path-row">
          <input v-model="form.path" readonly placeholder="选择代码目录" />
          <button type="button" class="btn btn-ghost" @click="pickFolder">选择文件夹</button>
        </div>
        <p v-if="form.pathError" class="field-error">{{ form.pathError }}</p>
      </label>
      <label class="field">
        <span>名称 <i>*</i></span>
        <input v-model="form.name" maxlength="60" placeholder="显示在左侧的项目名" />
      </label>
      <label class="switch">
        <input v-model="form.proxyEnabled" type="checkbox" />
        <span class="switch-ui" aria-hidden="true" />
        <span>使用代理</span>
      </label>
      <label class="field">
        <span>代理地址</span>
        <input
          v-model="form.proxyUrl"
          :disabled="!form.proxyEnabled"
          placeholder="http://127.0.0.1:7890"
          @blur="validateProxy"
        />
        <p class="help">只作用于这个项目打开的终端，不改系统环境变量。</p>
        <p v-if="form.proxyError" class="field-error">{{ form.proxyError }}</p>
      </label>
      <div class="actions">
        <button type="button" class="btn btn-ghost" :disabled="form.saving" @click="emit('close')">取消</button>
        <button type="button" class="btn btn-primary" :disabled="form.saving" @click="submit">
          {{ form.saving ? '保存中…' : '保存项目' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 664px;
  max-width: calc(100vw - 48px);
}

h2 {
  margin: 0 0 24px;
  font-size: 16px;
  line-height: 24px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.field span i {
  color: var(--ad-error);
  font-style: normal;
}

.path-row {
  display: flex;
  gap: 8px;
}

.path-row input {
  flex: 1;
}

.switch {
  margin-bottom: 16px;
}

.help {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 24px;
}
</style>
