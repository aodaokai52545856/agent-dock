<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { TOOLS, type ToolId } from '../lib/types'
import { store } from '../lib/store'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
  create: [payload: { projectId: string; toolId: ToolId }]
  addProject: []
}>()

const form = reactive({
  projectId: '',
  toolId: '' as ToolId | '',
  error: ''
})

const canCreate = computed(() => Boolean(form.projectId && form.toolId && store.projects.length))

watch(
  () => props.open,
  (open) => {
    if (!open) return
    form.projectId = store.selectedProjectId || store.projects[0]?.id || ''
    form.toolId = ''
    form.error = ''
  }
)

function submit() {
  if (!store.projects.length) {
    form.error = '请先添加项目'
    return
  }
  if (!form.projectId) {
    form.error = '请选择项目'
    return
  }
  if (!form.toolId) {
    form.error = '请选择工具'
    return
  }
  form.error = ''
  emit('create', { projectId: form.projectId, toolId: form.toolId })
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="新建会话">
      <h2>新建会话</h2>
      <p class="lead">必须先选项目和工具，再打开命令行。</p>

      <div v-if="!store.projects.length" class="empty">
        <p>还没有项目。先添加一个代码目录。</p>
        <div class="actions">
          <button type="button" class="btn btn-ghost" @click="emit('close')">取消</button>
          <button type="button" class="btn btn-primary" @click="emit('addProject')">添加项目</button>
        </div>
      </div>

      <template v-else>
        <label class="field">
          <span>项目 <i>*</i></span>
          <select v-model="form.projectId">
            <option disabled value="">选择项目</option>
            <option v-for="project in store.projects" :key="project.id" :value="project.id">
              {{ project.name }}
            </option>
          </select>
        </label>

        <div class="field">
          <span>工具 <i>*</i></span>
          <div class="tools" role="radiogroup" aria-label="工具">
            <button
              v-for="tool in TOOLS"
              :key="tool.id"
              type="button"
              role="radio"
              class="tool"
              :class="{ 'tool--active': form.toolId === tool.id }"
              :aria-checked="form.toolId === tool.id"
              @click="form.toolId = tool.id"
            >
              <span class="tool-dot" :style="{ background: tool.tint }" aria-hidden="true" />
              {{ tool.label }}
            </button>
          </div>
        </div>

        <p v-if="form.error" class="err">{{ form.error }}</p>
        <div class="actions">
          <button type="button" class="btn btn-ghost" @click="emit('close')">取消</button>
          <button type="button" class="btn btn-primary" :disabled="!canCreate" @click="submit">新建会话</button>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 480px;
  max-width: calc(100vw - 48px);
}

h2 {
  margin: 0 0 8px;
  font-size: 16px;
  line-height: 24px;
}

.lead,
.empty p {
  margin: 0 0 24px;
  color: var(--ad-muted);
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

select {
  width: 100%;
}

.tools {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.tool {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  height: 64px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  color: var(--ad-muted);
  background: var(--ad-hover);
}

.tool:hover {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.tool--active {
  background: var(--ad-selected);
  border-color: rgba(255, 255, 255, 0.12);
  color: var(--ad-text);
}

.tool-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.err {
  margin: 0 0 8px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-error);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 24px;
}
</style>
