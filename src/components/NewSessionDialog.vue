<script setup lang="ts">
import { computed, onUnmounted, reactive, ref, watch } from 'vue'
import ToolMark from './ToolMark.vue'
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

const pickerOpen = ref(false)
const pickerFocus = ref(0)

const canCreate = computed(() => Boolean(form.projectId && form.toolId && store.projects.length))
const projectLabel = computed(
  () => store.projects.find((item) => item.id === form.projectId)?.name ?? '选择项目'
)

watch(
  () => props.open,
  (open) => {
    pickerOpen.value = false
    window.removeEventListener('keydown', onKey)
    if (!open) return
    form.projectId = store.selectedProjectId || store.projects[0]?.id || ''
    form.toolId = ''
    form.error = ''
    window.addEventListener('keydown', onKey)
  }
)

onUnmounted(() => {
  window.removeEventListener('keydown', onKey)
})

function togglePicker(event: MouseEvent) {
  event.stopPropagation()
  pickerOpen.value = !pickerOpen.value
  if (pickerOpen.value) {
    pickerFocus.value = Math.max(
      0,
      store.projects.findIndex((item) => item.id === form.projectId)
    )
  }
}

function pickProject(id: string) {
  form.projectId = id
  form.error = ''
  pickerOpen.value = false
}

function pickTool(id: ToolId) {
  form.toolId = id
  form.error = ''
  pickerOpen.value = false
}

function closePicker() {
  pickerOpen.value = false
}

function onKey(event: KeyboardEvent) {
  if (!props.open) return
  if (event.key === 'Escape' && pickerOpen.value) {
    event.preventDefault()
    event.stopPropagation()
    closePicker()
    return
  }
  if (!pickerOpen.value) return
  const options = store.projects
  if (!options.length) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const step = event.key === 'ArrowDown' ? 1 : options.length - 1
    pickerFocus.value = (pickerFocus.value + step) % options.length
    return
  }
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    const option = options[pickerFocus.value]
    if (option) pickProject(option.id)
  }
}

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
    <div
      class="ad-dialog dialog"
      role="dialog"
      aria-modal="true"
      aria-label="新建会话"
      @click="closePicker"
    >
      <h2>新建会话</h2>
      <p class="lead">打开后挂在所选项目下。</p>

      <div v-if="!store.projects.length" class="empty">
        <p>还没有项目。先添加一个代码目录。</p>
        <div class="actions">
          <button type="button" class="btn btn-ghost" @click="emit('close')">取消</button>
          <button type="button" class="btn btn-primary" @click="emit('addProject')">添加项目</button>
        </div>
      </div>

      <template v-else>
        <div class="field">
          <span class="label">项目</span>
          <div class="picker" @click.stop>
            <button
              type="button"
              class="picker-btn"
              :class="{ 'is-open': pickerOpen }"
              aria-haspopup="listbox"
              :aria-expanded="pickerOpen"
              aria-label="选择项目"
              @click="togglePicker"
            >
              <span class="picker-value">{{ projectLabel }}</span>
              <svg class="picker-chevron" viewBox="0 0 12 12" aria-hidden="true">
                <path d="M2.4 4.2L6 7.8l3.6-3.6" />
              </svg>
            </button>
            <div v-if="pickerOpen" class="ad-menu picker-menu" role="listbox" aria-label="项目">
              <button
                v-for="(project, index) in store.projects"
                :key="project.id"
                type="button"
                class="ad-menu-item"
                :class="{
                  'is-active': form.projectId === project.id,
                  'is-focus': pickerFocus === index
                }"
                role="option"
                :aria-selected="form.projectId === project.id"
                :title="project.path"
                @mouseenter="pickerFocus = index"
                @click="pickProject(project.id)"
              >
                <span>{{ project.name }}</span>
                <span v-if="form.projectId === project.id" class="picker-check" aria-hidden="true">✓</span>
              </button>
            </div>
          </div>
        </div>

        <div class="field">
          <span class="label">工具</span>
          <div class="tools" role="radiogroup" aria-label="工具">
            <button
              v-for="tool in TOOLS"
              :key="tool.id"
              type="button"
              role="radio"
              class="tool"
              :class="{ 'tool--active': form.toolId === tool.id }"
              :aria-checked="form.toolId === tool.id"
              @click="pickTool(tool.id)"
            >
              <span class="tool-mark" aria-hidden="true">
                <ToolMark :id="tool.id" />
              </span>
              <span class="tool-name">{{ tool.label }}</span>
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
  width: 420px;
  max-width: calc(100vw - 48px);
}

h2 {
  margin: 0 0 4px;
  font-size: 16px;
  line-height: 24px;
}

.lead,
.empty p {
  margin: 0 0 20px;
  color: var(--ad-muted);
  font-size: 12px;
  line-height: 18px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 16px;
}

.label {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.picker {
  position: relative;
}

.picker-btn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  height: 32px;
  padding: 0 8px 0 10px;
  border: 1px solid transparent;
  border-radius: var(--ad-radius-control);
  color: var(--ad-text);
  font-size: 12px;
  line-height: 20px;
  text-align: left;
}

.picker-btn:hover,
.picker-btn.is-open {
  background: var(--ad-hover);
}

.picker-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-chevron {
  width: 10px;
  height: 10px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--ad-muted);
  stroke-width: 1.4;
  stroke-linecap: square;
  stroke-linejoin: miter;
  transition: transform var(--ad-transition);
}

.picker-btn.is-open .picker-chevron {
  transform: rotate(180deg);
}

.picker-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  right: 0;
  z-index: 6;
  max-height: 220px;
  overflow: auto;
  animation: ad-rise 140ms ease;
}

.picker-check {
  color: var(--ad-muted);
  font-size: 12px;
}

@media (prefers-reduced-motion: reduce) {
  .picker-menu {
    animation: none;
  }
}

.tools {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
}

.tool {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: var(--ad-radius-control);
  color: var(--ad-muted);
  text-align: left;
}

.tool:hover {
  background: var(--ad-hover);
  color: var(--ad-text);
}

.tool--active {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.tool-mark {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}

.tool-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  line-height: 20px;
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
  margin-top: 8px;
}

.empty .actions {
  margin-top: 24px;
}
</style>
