<script setup lang="ts">
import { open as pickDirectory } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, reactive, ref, watch } from 'vue'
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
  nameError: '',
  proxyError: '',
  saving: false
})

const title = computed(() => (props.project ? '编辑项目' : '添加项目'))
const formEl = ref<HTMLFormElement>()

watch(
  () => [props.visible, props.project] as const,
  () => {
    if (!props.visible) return
    form.name = props.project?.name ?? ''
    form.path = props.project?.path ?? ''
    form.proxyEnabled = props.project?.proxyEnabled ?? false
    form.proxyUrl = props.project?.proxyUrl || props.defaultProxyUrl
    form.pathError = ''
    form.nameError = ''
    form.proxyError = ''
    form.saving = false
  }
)

async function pickFolder() {
  if (!api.isTauri) {
    form.pathError = '请在桌面窗口里选择文件夹'
    return
  }
  try {
    const selected = await pickDirectory({ directory: true, multiple: false, title: '选择项目文件夹' })
    if (typeof selected !== 'string' || !selected) return
    form.path = selected
    form.pathError = ''
    if (!form.name.trim()) {
      form.name = await api.folderLabel(selected)
      form.nameError = ''
    }
  } catch (err) {
    form.pathError = err instanceof Error ? err.message : '无法打开文件夹选择器，请重试或粘贴路径'
  }
}

function validateName() {
  form.nameError = form.name.trim() ? '' : '请填写项目名称'
  return !form.nameError
}

function validatePath() {
  form.pathError = form.path.trim() ? '' : '请选择文件夹或粘贴项目路径'
  return !form.pathError
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
  if (form.saving) return
  const pathValid = validatePath()
  const nameValid = validateName()
  const proxyValid = validateProxy()
  if (!pathValid || !nameValid || !proxyValid) {
    void nextTick(() => formEl.value?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus())
    return
  }
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

function close() {
  if (!form.saving) emit('close')
}

defineExpose({ stopSave })
</script>

<template>
  <div v-if="visible" class="ad-mask" @click.self="close">
    <form ref="formEl" v-modal="close" class="ad-dialog dialog" role="dialog" aria-modal="true" :aria-label="title" @submit.prevent="submit">
      <h2>{{ title }}</h2>
      <label class="field">
        <span>文件夹 <i>*</i></span>
        <div class="path-row">
          <input v-model="form.path" data-modal-initial aria-label="项目文件夹" :aria-invalid="!!form.pathError" :disabled="form.saving" placeholder="粘贴项目路径或选择文件夹" @blur="validatePath" @input="form.pathError = ''" />
          <button type="button" class="btn btn-ghost" :disabled="form.saving" @click="pickFolder">选择文件夹</button>
        </div>
        <p :class="form.pathError ? 'field-error' : 'help'">{{ form.pathError || '支持直接粘贴本机文件夹的完整路径。' }}</p>
      </label>
      <label class="field">
        <span>名称 <i>*</i></span>
        <input v-model="form.name" maxlength="60" :aria-invalid="!!form.nameError" :disabled="form.saving" placeholder="显示在左侧的项目名" @blur="validateName" @input="form.nameError = ''" />
        <p :class="form.nameError ? 'field-error' : 'help'">{{ form.nameError || '最多 60 个字符，用于左侧项目列表。' }}</p>
      </label>
      <label class="switch">
        <input v-model="form.proxyEnabled" type="checkbox" :disabled="form.saving" />
        <span class="switch-ui" aria-hidden="true" />
        <span>使用代理</span>
      </label>
      <label class="field">
        <span>代理地址</span>
        <input
          v-model="form.proxyUrl"
          :disabled="!form.proxyEnabled || form.saving"
          :aria-invalid="!!form.proxyError"
          placeholder="http://127.0.0.1:7890"
          @blur="validateProxy"
        />
        <p :class="form.proxyError ? 'field-error' : 'help'">{{ form.proxyError || '只作用于这个项目打开的终端，不改系统环境变量。' }}</p>
      </label>
      <div class="actions">
        <button type="button" class="btn btn-ghost" :disabled="form.saving" @click="close">取消</button>
        <button type="submit" class="btn btn-primary" :disabled="form.saving">
          {{ form.saving ? '保存中…' : '保存项目' }}
        </button>
      </div>
    </form>
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
  min-width: 0;
}

[aria-invalid='true'] { border-color: var(--ad-error); }

.switch {
  margin-bottom: 16px;
}

.help,
.field-error {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.field-error { color: var(--ad-error); }

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 24px;
}
</style>
