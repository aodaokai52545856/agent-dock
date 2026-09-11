<script setup lang="ts">
import { ref, watch } from 'vue'
import * as api from '../lib/api'
import type { GrokAccount, GrokAccountList } from '../lib/types'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
  switched: []
}>()

const data = ref<GrokAccountList>({
  loggedIn: false,
  currentEmail: '',
  currentName: '',
  accounts: []
})
const nameDraft = ref('')
const busy = ref('')
const error = ref('')
const removing = ref<GrokAccount | null>(null)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    error.value = ''
    removing.value = null
    void reload()
  }
)

async function reload() {
  busy.value = 'load'
  error.value = ''
  try {
    data.value = await api.listGrokAccounts()
    if (!nameDraft.value && data.value.currentEmail) {
      nameDraft.value = data.value.currentName || data.value.currentEmail
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function saveCurrent() {
  busy.value = 'save'
  error.value = ''
  try {
    data.value = await api.saveGrokAccount(nameDraft.value.trim())
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function loginNew() {
  busy.value = 'login'
  error.value = ''
  try {
    data.value = await api.loginGrokAccount()
    nameDraft.value = data.value.currentName || data.value.currentEmail
    emit('switched')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function switchTo(account: GrokAccount) {
  if (account.active || busy.value) return
  busy.value = 'switch'
  error.value = ''
  try {
    data.value = await api.switchGrokAccount(account.id)
    emit('switched')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

async function removeAccount() {
  const account = removing.value
  if (!account) return
  busy.value = 'delete'
  error.value = ''
  try {
    data.value = await api.deleteGrokAccount(account.id)
    removing.value = null
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="Grok 账号">
      <header>
        <div>
          <h2>Grok 账号</h2>
          <p class="hint">快照存在本机。切换后会关掉已打开的 Grok 终端，请再新建会话。</p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <section class="current">
        <p class="kicker">当前登录</p>
        <p v-if="data.loggedIn" class="current-id">
          <strong>{{ data.currentEmail || data.currentName || '已登录' }}</strong>
        </p>
        <p v-else class="muted">还没有登录</p>
        <div class="row">
          <input v-model="nameDraft" maxlength="40" placeholder="保存时的显示名，例如 工作号" :disabled="!!busy" />
          <button type="button" class="btn btn-ghost" :disabled="!!busy || !data.loggedIn" @click="saveCurrent">
            {{ busy === 'save' ? '保存中…' : '保存当前' }}
          </button>
          <button type="button" class="btn btn-primary" :disabled="!!busy" @click="loginNew">
            {{ busy === 'login' ? '登录中…' : '登录新号' }}
          </button>
        </div>
        <p v-if="busy === 'login'" class="hint">会弹出窗口和浏览器，请用新号登录，完成后再回到这里。</p>
      </section>

      <p v-if="error" class="field-error">{{ error }}</p>
      <p v-if="!api.isTauri" class="hint">浏览器预览无法管理本机账号，请在桌面端使用。</p>

      <section>
        <p class="kicker">已保存</p>
        <p v-if="busy === 'load'" class="muted">正在读取账号…</p>
        <p v-else-if="!data.accounts.length" class="muted">还没有保存的账号。登录后点「保存当前」，再登录新号即可来回切。</p>
        <ul v-else class="list">
          <li v-for="account in data.accounts" :key="account.id" class="item" :class="{ 'item--active': account.active }">
            <div>
              <div class="item-name">
                {{ account.name }}
                <span v-if="account.active" class="ad-tag ad-tag--ok">当前</span>
              </div>
              <div class="item-email">{{ account.email || '-' }}</div>
            </div>
            <div class="item-ops">
              <button
                type="button"
                class="link"
                :disabled="!!busy || account.active"
                @click="switchTo(account)"
              >
                {{ busy === 'switch' ? '切换中…' : '切换' }}
              </button>
              <button type="button" class="link link-danger" :disabled="!!busy" @click="removing = account">
                删除
              </button>
            </div>
          </li>
        </ul>
      </section>

      <div v-if="removing" class="confirm">
        <p>删除后只是拿掉本机快照，不会退出当前 Grok。确认删除 {{ removing.name }}？</p>
        <div class="row">
          <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="removing = null">取消</button>
          <button type="button" class="btn btn-danger btn-small" :disabled="!!busy" @click="removeAccount">
            {{ busy === 'delete' ? '删除中…' : '删除快照' }}
          </button>
        </div>
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

.current {
  margin-bottom: 24px;
}

.kicker {
  margin: 0 0 8px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.current-id {
  margin: 0 0 12px;
}

.row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.row input {
  flex: 1;
  min-width: 0;
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
  margin-top: 2px;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.item-ops {
  display: flex;
  gap: 12px;
  flex-shrink: 0;
}

.link {
  padding: 0;
  color: var(--ad-muted);
  font-size: 13px;
  line-height: 20px;
}

.link:hover:not(:disabled) {
  color: var(--ad-text);
}

.link-danger {
  color: var(--ad-error);
}

.confirm {
  margin-top: 16px;
  padding: 16px;
  border: 1px solid color-mix(in srgb, var(--ad-error) 28%, var(--ad-glass-border));
  border-radius: var(--ad-radius-card);
  background: color-mix(in srgb, var(--ad-error) 8%, transparent);
}

.confirm p {
  margin: 0 0 12px;
  color: var(--ad-text);
}
</style>
