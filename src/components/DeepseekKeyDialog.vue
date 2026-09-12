<script setup lang="ts">
import { ref, watch } from 'vue'
import * as api from '../lib/api'
import { DSH_PLATFORM_URL } from '../lib/dsh'
import { selectedProject } from '../lib/store'
import type { DshKeyBundle, DshKeyMeta } from '../lib/types'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
}>()

const data = ref<DshKeyBundle>({
  status: {
    configured: false,
    writable: true,
    source: 'none',
    masked: '',
    dshHome: '',
    credentialsPath: '',
    envBlocks: false
  },
  keys: []
})
const nameDraft = ref('')
const keyDraft = ref('')
const busy = ref('')
const error = ref('')
const removing = ref<DshKeyMeta | null>(null)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    nameDraft.value = ''
    keyDraft.value = ''
    error.value = ''
    removing.value = null
    void reload()
  }
)

function projectPath() {
  return selectedProject.value?.path
}

async function reload() {
  busy.value = 'load'
  error.value = ''
  try {
    data.value = await api.dshListKeys(projectPath())
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function addKey() {
  const name = nameDraft.value.trim()
  const key = keyDraft.value.trim()
  if (!name) {
    error.value = '请填写名称，例如 工作号'
    return
  }
  if (!key) {
    error.value = '请粘贴 DeepSeek API Key'
    return
  }
  busy.value = 'save'
  error.value = ''
  try {
    data.value = await api.dshAddKey(name, key, projectPath())
    nameDraft.value = ''
    keyDraft.value = ''
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function switchTo(item: DshKeyMeta) {
  if (item.active || busy.value) return
  busy.value = 'switch'
  error.value = ''
  try {
    data.value = await api.dshSwitchKey(item.id, projectPath())
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function removeKey() {
  const item = removing.value
  if (!item) return
  busy.value = 'delete'
  error.value = ''
  try {
    data.value = await api.dshDeleteKey(item.id, projectPath())
    removing.value = null
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

function openPlatform() {
  void api.openExternal(DSH_PLATFORM_URL)
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="DeepSeek Key">
      <header>
        <div>
          <h2>DeepSeek Key</h2>
          <p class="hint">可保存多条，随时切换。</p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <section class="add">
        <div class="row">
          <input v-model="nameDraft" maxlength="40" placeholder="名称，例如 工作号" :disabled="!!busy" />
          <input
            v-model="keyDraft"
            type="password"
            autocomplete="off"
            spellcheck="false"
            placeholder="sk-…"
            :disabled="!!busy"
            @keydown.enter.prevent="addKey"
          />
          <button type="button" class="btn btn-primary" :disabled="!!busy || !nameDraft.trim() || !keyDraft.trim()" @click="addKey">
            {{ busy === 'save' ? '保存中…' : '添加' }}
          </button>
        </div>
      </section>

      <p v-if="error" class="field-error">{{ error }}</p>
      <p v-if="data.status.envBlocks" class="field-error">系统环境变量里已有 Key，会盖过这里的切换。</p>

      <section>
        <p class="kicker">已保存 {{ data.keys.length ? `(${data.keys.length})` : '' }}</p>
        <p v-if="busy === 'load'" class="muted">正在读取…</p>
        <p v-else-if="!data.keys.length" class="muted">还没有 Key。添加后会出现在这份列表里，可切换、删除。</p>
        <ul v-else class="list">
          <li v-for="item in data.keys" :key="item.id" class="item" :class="{ 'item--active': item.active }">
            <div>
              <div class="item-name">
                {{ item.name }}
                <span v-if="item.active" class="ad-tag ad-tag--ok">当前</span>
              </div>
              <div class="item-email">{{ item.masked }}</div>
            </div>
            <div class="item-ops">
              <button type="button" class="link" :disabled="!!busy || item.active" @click="switchTo(item)">
                {{ busy === 'switch' ? '切换中…' : '切换' }}
              </button>
              <button type="button" class="link link-danger" :disabled="!!busy" @click="removing = item">删除</button>
            </div>
          </li>
        </ul>
      </section>

      <div v-if="removing" class="confirm">
        <p>确认删除 {{ removing.name }}？</p>
        <div class="row end">
          <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="removing = null">取消</button>
          <button type="button" class="btn btn-danger btn-small" :disabled="!!busy" @click="removeKey">
            {{ busy === 'delete' ? '删除中…' : '删除' }}
          </button>
        </div>
      </div>

      <div class="actions">
        <button type="button" class="btn btn-ghost" :disabled="!!busy" @click="openPlatform">获取 API Key</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 664px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  overflow: auto;
}

header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
}

h2 {
  margin: 0;
  font-size: 16px;
  line-height: 24px;
}

.hint,
.muted {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.add {
  margin-bottom: 16px;
}

.kicker {
  margin: 0 0 8px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 8px;
}

.row input {
  flex: 1;
  min-width: 0;
  height: 32px;
  padding: 0 10px;
}

.row.end {
  justify-content: flex-end;
  margin-bottom: 0;
}

.list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  margin-bottom: 8px;
  border: 1px solid var(--ad-border);
  border-radius: var(--ad-radius-card);
  background: var(--ad-hover);
}

.item--active {
  border-color: rgba(255, 255, 255, 0.1);
  background: var(--ad-selected);
}

.item-name {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
}

.item-email {
  margin-top: 4px;
  font-size: 12px;
  color: var(--ad-muted);
  font-variant-numeric: tabular-nums;
}

.item-ops {
  display: flex;
  gap: 12px;
}

.link {
  padding: 0;
  color: var(--ad-text);
  font-size: 13px;
}

.link:hover:not(:disabled) {
  text-decoration: underline;
}

.link-danger {
  color: var(--ad-error);
}

.confirm {
  margin: 12px 0 16px;
  padding: 12px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
}

.confirm p {
  margin: 0 0 12px;
  font-size: 13px;
  line-height: 20px;
}

.actions {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}
</style>
