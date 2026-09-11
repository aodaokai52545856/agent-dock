<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { formatCiteBlock, wrapBracketedPaste } from '../lib/sessionCite'
import { showToast, store } from '../lib/store'
import { toolLabel, type SessionTurn, type ToolId } from '../lib/types'

const props = defineProps<{
  open: boolean
  projectId: string
  toolId: ToolId
  sessionId: string
  sessionTitle: string
}>()

const emit = defineEmits<{
  close: []
}>()

const turns = ref<SessionTurn[]>([])
const selectedId = ref('')
const targetPtyId = ref('')
const status = ref<'idle' | 'loading' | 'ready' | 'empty' | 'error'>('idle')
const error = ref('')
const citing = ref(false)
let loadSeq = 0

const selected = computed(() => turns.value.find((item) => item.id === selectedId.value) ?? null)
const targets = computed(() => store.live.filter((item) => item.alive && item.projectId === props.projectId))

watch(
  () => [props.open, props.projectId, props.toolId, props.sessionId],
  () => {
    if (!props.open) return
    selectedId.value = ''
    targetPtyId.value = store.activePtyId
    void load()
  }
)

async function load() {
  const seq = ++loadSeq
  status.value = 'loading'
  error.value = ''
  try {
    const rows = await api.listSessionTurns(props.projectId, props.toolId, props.sessionId)
    if (seq !== loadSeq) return
    turns.value = rows
    selectedId.value = rows[0]?.id ?? ''
    status.value = rows.length ? 'ready' : 'empty'
  } catch (err) {
    if (seq !== loadSeq) return
    error.value = err instanceof Error ? err.message : String(err)
    turns.value = []
    status.value = 'error'
  }
}

async function cite() {
  const turn = selected.value
  if (!turn) {
    showToast('请先选一条片段')
    return
  }
  if (!targetPtyId.value) {
    showToast('先打开目标会话')
    return
  }
  citing.value = true
  try {
    await api.ptyWrite(
      targetPtyId.value,
      wrapBracketedPaste(
        formatCiteBlock({
          toolId: props.toolId,
          sessionTitle: props.sessionTitle,
          body: turn.text
        })
      )
    )
    showToast('已写入目标终端，未发送')
    emit('close')
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  } finally {
    citing.value = false
  }
}

function roleLabel(role: string) {
  return role === 'user' ? '用户' : '助手'
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="引用片段">
      <header>
        <div>
          <h2>引用片段</h2>
          <p class="hint">{{ toolLabel(toolId) }} · {{ sessionTitle }}。写入提示行，不会回车发送。</p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <div v-if="status === 'loading'" class="state muted">正在读取对话</div>
      <div v-else-if="status === 'error'" class="state">
        <p>{{ error }}</p>
        <button type="button" class="btn btn-ghost btn-small" @click="load">重试</button>
      </div>
      <div v-else-if="status === 'empty'" class="state muted">这个会话还没有可引用的用户或助手回合。</div>
      <ul v-else class="turns">
        <li v-for="turn in turns" :key="turn.id">
          <button
            type="button"
            class="turn"
            :class="{ 'turn--on': selectedId === turn.id }"
            @click="selectedId = turn.id"
          >
            <span class="turn-role">{{ roleLabel(turn.role) }}</span>
            <span class="turn-text">{{ turn.excerpt }}</span>
          </button>
        </li>
      </ul>

      <label class="field">
        <span>目标终端</span>
        <select v-model="targetPtyId" :disabled="!targets.length">
          <option disabled value="">选择已打开的终端</option>
          <option v-for="item in targets" :key="item.ptyId" :value="item.ptyId">
            {{ toolLabel(item.toolId) }} · {{ item.title }}
          </option>
        </select>
      </label>
      <p v-if="!targets.length" class="hint">先打开目标会话，才能把片段贴进提示行。</p>

      <div class="actions">
        <button type="button" class="btn btn-ghost" @click="emit('close')">取消</button>
        <button type="button" class="btn btn-primary" :disabled="citing || !selected || !targetPtyId" @click="cite">
          {{ citing ? '写入中…' : '引用到终端' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 640px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
}

header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
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

.state {
  padding: 16px 0;
}

.state p {
  margin: 0 0 12px;
}

.turns {
  flex: 1;
  min-height: 160px;
  max-height: 360px;
  overflow: auto;
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
}

.turn {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 100%;
  padding: 10px 12px;
  margin-bottom: 6px;
  border-radius: 10px;
  text-align: left;
}

.turn:hover,
.turn--on {
  background: var(--ad-hover);
}

.turn--on {
  background: var(--ad-selected);
}

.turn-role {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.turn-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 8px;
  font-size: 13px;
}

.field span {
  color: var(--ad-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}
</style>
