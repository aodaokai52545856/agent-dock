<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { canUseClaudeAccount, claudeStatus } from '../lib/claudeAccounts'
import { modal as vModal } from '../lib/modal'
import type { ClaudeContinuation, ClaudeSessionAccounts } from '../lib/types'

const props = defineProps<{ open: boolean; projectId: string; sessionId: string; title: string }>()
const emit = defineEmits<{ close: []; accounts: []; continued: [result: ClaudeContinuation & { projectId: string }] }>()
const data = ref<ClaudeSessionAccounts | null>(null)
const selectedId = ref('')
const busy = ref('')
const error = ref('')
const accounts = computed(() => data.value?.accounts.filter(a => a.id !== data.value?.sourceAccountId && a.source !== 'legacy') || [])
const selected = computed(() => accounts.value.find(a => a.id === selectedId.value))
const canContinue = computed(() => selected.value && canUseClaudeAccount(selected.value))

watch(() => [props.open, props.projectId, props.sessionId] as const, ([open]) => {
  if (open) void load()
})

async function load() {
  if (busy.value) return
  busy.value = 'load'; error.value = ''; data.value = null; selectedId.value = ''
  try {
    data.value = await api.claudeSessionAccounts(props.projectId, props.sessionId)
    selectedId.value = accounts.value.find(a => a.active && canUseClaudeAccount(a))?.id
      || accounts.value.find(canUseClaudeAccount)?.id || ''
  } catch (err) { error.value = String(err instanceof Error ? err.message : err) }
  finally { busy.value = '' }
}

async function continueSession() {
  if (busy.value || !canContinue.value) return
  busy.value = 'copy'; error.value = ''
  const projectId = props.projectId
  try {
    const result = await api.continueClaudeSession(projectId, props.sessionId, selectedId.value, props.title)
    emit('continued', { ...result, projectId })
  } catch (err) { error.value = String(err instanceof Error ? err.message : err) }
  finally { busy.value = '' }
}

function close() { if (!busy.value) emit('close') }
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="close">
    <section v-modal="close" class="ad-dialog continue-dialog" role="dialog" aria-modal="true" aria-labelledby="claude-continue-title" :aria-busy="!!busy" tabindex="-1">
      <header><div><h2 id="claude-continue-title">用其他 Claude 账号继续</h2><p>复制会话记录，并使用所选账号打开新会话。</p></div><button class="btn btn-ghost btn-small" :disabled="!!busy" @click="close">关闭</button></header>
      <div class="body">
        <div class="source"><span>原会话</span><strong>{{ title || 'Claude 会话' }}</strong><p v-if="data">所属账号：{{ data.sourceAccountName }}</p></div>
        <p v-if="busy === 'load'" class="feedback" role="status">正在读取会话所属账号…</p>
        <div v-if="error" class="feedback error" role="alert">{{ error }}<button v-if="!data" class="text-button" :disabled="!!busy" @click="load">重试</button></div>
        <template v-if="data">
          <label for="claude-continue-account">目标账号</label>
          <select id="claude-continue-account" v-model="selectedId" :disabled="!!busy">
            <option value="" disabled>请选择已登录的账号</option>
            <option v-for="account in accounts" :key="account.id" :value="account.id" :disabled="!canUseClaudeAccount(account)">{{ account.name }}{{ account.email ? ` · ${account.email}` : '' }} · {{ claudeStatus(account.status) }}</option>
          </select>
          <p v-if="!accounts.some(canUseClaudeAccount)" class="feedback">暂无其他可用账号，请先添加账号并完成登录。</p>
          <p v-else-if="selected" class="account-note">{{ selected.message }}</p>
          <div class="copy-note">原会话和文件修改仍会保留。新会话共享当前项目文件，将以目标账号的权限、额度和可用模型继续。</div>
        </template>
      </div>
      <footer><button class="text-button" :disabled="!!busy" @click="emit('accounts')">管理 Claude 账号</button><div class="actions"><button class="btn btn-ghost" :disabled="!!busy" @click="close">取消</button><button class="btn btn-primary" :disabled="!!busy || !canContinue" @click="continueSession">{{ busy === 'copy' ? '复制中…' : '复制并继续' }}</button></div></footer>
    </section>
  </div>
</template>

<style scoped>
.continue-dialog { width: 560px; max-width: calc(100vw - 32px); max-height: calc(100vh - 32px); padding: 0; overflow: auto; }
header { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; padding: 24px; border-bottom: 1px solid var(--ad-border); }
h2, p { margin: 0; }
h2 { font-size: 18px; line-height: 28px; font-weight: 600; }
header p, .source p, .account-note, .copy-note { color: var(--ad-muted); font-size: 12px; line-height: 20px; margin-top: 8px; }
.body { padding: 24px; }
.source { padding: 16px; background: var(--ad-hover); border: 1px solid var(--ad-border); border-radius: var(--ad-radius); margin-bottom: 24px; overflow-wrap: anywhere; }
.source span { display: block; color: var(--ad-muted); font-size: 12px; margin-bottom: 8px; }
.source strong { font-size: 14px; line-height: 22px; }
label { display: block; font-size: 13px; margin-bottom: 8px; }
select { width: 100%; min-width: 0; height: 36px; font-size: 13px; }
.feedback { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; font-size: 13px; line-height: 22px; padding: 12px; margin-bottom: 16px; background: var(--ad-hover); border-radius: var(--ad-radius); overflow-wrap: anywhere; }
.error { color: var(--ad-error); background: color-mix(in srgb, var(--ad-error) 8%, transparent); }
.copy-note { margin-top: 24px; }
footer { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 16px; padding: 16px 24px; border-top: 1px solid var(--ad-border); }
.actions { display: flex; gap: 8px; margin-left: auto; }
.text-button { color: var(--ad-muted); padding: 0; font-size: 12px; }
.text-button:hover:not(:disabled) { text-decoration: underline; color: var(--ad-text); }
.btn { height: 32px; font-size: 13px; }
.btn-small { height: 28px; font-size: 12px; }
</style>
