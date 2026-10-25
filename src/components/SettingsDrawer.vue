<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import {
  CODE_FONTS,
  CONTENT_FONTS,
  UI_FONTS,
  UI_FONT_MAX,
  UI_FONT_MIN,
  UI_THEMES,
  lookPatchFromDraft,
  parseHex,
  resolvedTheme,
  sanitizeLookSettings,
  systemPrefersLight,
  themeDefaults,
  type UiTheme
} from '../lib/appearance'
import type { AppSettings } from '../lib/types'
import {
  UI_FROST_MAX,
  UI_FROST_MIN,
  UI_OPACITY_MAX,
  UI_OPACITY_MIN,
  clampUiFrost,
  clampUiOpacity,
  commitLookSettings,
  flushLookSettings
} from '../lib/store'

const props = defineProps<{
  open: boolean
  settings: AppSettings
}>()

const emit = defineEmits<{
  close: []
  save: [settings: AppSettings]
}>()

const section = ref('appearance')
const discardOpen = ref(false)
const sections = [
  { id: 'appearance', label: '外观' },
  { id: 'tools', label: '终端与工具' },
  { id: 'automation', label: '自动化' }
]

function syncForm() {
  const look = sanitizeLookSettings(props.settings)
  form.terminalFontSize = props.settings.terminalFontSize
  form.uiFontSize = look.uiFontSize
  form.uiTheme = look.uiTheme
  form.uiAccent = look.uiAccent
  form.uiBackground = look.uiBackground
  form.uiForeground = look.uiForeground
  form.uiAccentLight = look.uiAccentLight
  form.uiBackgroundLight = look.uiBackgroundLight
  form.uiForegroundLight = look.uiForegroundLight
  form.uiFontFamily = look.uiFontFamily
  form.contentFontFamily = look.contentFontFamily
  form.codeFontFamily = look.codeFontFamily
  form.uiContrast = look.uiContrast
  form.translucentSidebar = look.translucentSidebar
  form.uiOpacity = clampUiOpacity(props.settings.uiOpacity)
  form.uiFrost = clampUiFrost(props.settings.uiFrost)
  form.grokFollowGlass = Boolean(props.settings.grokFollowGlass)
  form.cursorApiKey = props.settings.cursorApiKey ?? ''
  form.codexPath = props.settings.codexPath ?? ''
  form.claudePath = props.settings.claudePath ?? ''
  form.piPath = props.settings.piPath ?? ''
  form.dshPath = props.settings.dshPath ?? ''
  form.saving = false
}

const form = reactive({
  terminalFontSize: props.settings.terminalFontSize,
  uiFontSize: 13,
  uiTheme: 'system' as UiTheme,
  uiAccent: '',
  uiBackground: '',
  uiForeground: '',
  uiAccentLight: '',
  uiBackgroundLight: '',
  uiForegroundLight: '',
  uiFontFamily: '',
  contentFontFamily: '',
  codeFontFamily: '',
  uiContrast: 60,
  translucentSidebar: true,
  uiOpacity: clampUiOpacity(props.settings.uiOpacity),
  uiFrost: clampUiFrost(props.settings.uiFrost),
  grokFollowGlass: Boolean(props.settings.grokFollowGlass),
  cursorApiKey: props.settings.cursorApiKey ?? '',
  codexPath: props.settings.codexPath ?? '',
  claudePath: props.settings.claudePath ?? '',
  piPath: props.settings.piPath ?? '',
  dshPath: props.settings.dshPath ?? '',
  saving: false
})
syncForm()

const configDirty = computed(() =>
  (['cursorApiKey', 'codexPath', 'claudePath', 'piPath', 'dshPath'] as const)
    .some(key => form[key] !== (props.settings[key] ?? ''))
)

function requestClose() {
  if (form.saving) return
  if (configDirty.value) discardOpen.value = true
  else emit('close')
}

const resolved = computed(() => resolvedTheme(form.uiTheme, systemPrefersLight()))
const defaults = computed(() => themeDefaults(resolved.value))
const themeTitle = computed(() => (resolved.value === 'light' ? '浅色主题' : '深色主题'))
const paintAccent = computed(() =>
  resolved.value === 'light' ? form.uiAccentLight : form.uiAccent
)
const paintBackground = computed(() =>
  resolved.value === 'light' ? form.uiBackgroundLight : form.uiBackground
)
const paintForeground = computed(() =>
  resolved.value === 'light' ? form.uiForegroundLight : form.uiForeground
)

let hydrating = false

watch(
  () => props.open,
  (open) => {
    if (open) {
      discardOpen.value = false
      hydrating = true
      try {
        syncForm()
      } finally {
        hydrating = false
      }
      return
    }
    flushLookSettings()
  }
)

function lookPatch() {
  return {
    ...lookPatchFromDraft(form),
    uiOpacity: clampUiOpacity(form.uiOpacity),
    uiFrost: clampUiFrost(form.uiFrost)
  }
}

watch(
  () => [
    form.uiTheme,
    form.uiFontSize,
    form.uiAccent,
    form.uiBackground,
    form.uiForeground,
    form.uiAccentLight,
    form.uiBackgroundLight,
    form.uiForegroundLight,
    form.uiFontFamily,
    form.contentFontFamily,
    form.codeFontFamily,
    form.uiContrast,
    form.translucentSidebar,
    form.terminalFontSize,
    form.uiOpacity,
    form.uiFrost,
    form.grokFollowGlass
  ],
  () => {
    if (!props.open || hydrating) return
    commitLookSettings(lookPatch())
  },
  { flush: 'sync' }
)

function pickTheme(id: UiTheme) {
  form.uiTheme = id
}

function setColor(key: 'accent' | 'background' | 'foreground', value: string) {
  const hex = parseHex(value)
  if (resolved.value === 'light') {
    if (key === 'accent') form.uiAccentLight = hex
    else if (key === 'background') form.uiBackgroundLight = hex
    else form.uiForegroundLight = hex
  } else if (key === 'accent') form.uiAccent = hex
  else if (key === 'background') form.uiBackground = hex
  else form.uiForeground = hex
}

function submit() {
  if (form.saving) return
  const size = Number(form.terminalFontSize) || 14
  form.saving = true
  emit('save', {
    ...props.settings,
    ...lookPatch(),
    terminalFontSize: Math.min(22, Math.max(10, size)),
    cursorApiKey: form.cursorApiKey.trim(),
    codexPath: form.codexPath.trim(),
    claudePath: form.claudePath.trim(),
    piPath: form.piPath.trim(),
    dshPath: form.dshPath.trim()
  })
}

function stopSave() {
  form.saving = false
}

defineExpose({ stopSave })
</script>

<template>
  <div v-if="open" class="ad-mask ad-mask--flush" @click.self="requestClose">
    <aside v-modal="requestClose" class="ad-drawer drawer" role="dialog" aria-modal="true" aria-label="设置">
      <header>
        <h2>设置</h2>
        <button type="button" class="btn btn-ghost btn-small" :disabled="form.saving" @click="requestClose">关闭</button>
      </header>

      <div class="settings-nav" role="group" aria-label="设置分类">
        <button v-for="item in sections" :key="item.id" type="button" :aria-pressed="section === item.id" :class="{ active: section === item.id }" @click="section = item.id">{{ item.label }}</button>
      </div>

      <div class="settings-body">
      <section v-show="section === 'appearance'" aria-label="外观设置">
      <p class="section">主题与界面</p>
      <div class="themes" role="radiogroup" aria-label="主题">
        <button
          v-for="item in UI_THEMES"
          :key="item.id"
          type="button"
          class="theme"
          :class="{ on: form.uiTheme === item.id }"
          role="radio"
          :aria-checked="form.uiTheme === item.id"
          @click="pickTheme(item.id)"
        >
          <span class="theme-preview" :data-theme="item.id" aria-hidden="true">
            <i />
            <b />
            <em />
          </span>
          {{ item.label }}
        </button>
      </div>

      <div class="look">
        <div class="look-head">{{ themeTitle }}</div>
        <label class="look-row">
          <span>强调色</span>
          <span class="swatch">
            <input
              type="color"
              :value="paintAccent || defaults.accent"
              @input="setColor('accent', ($event.target as HTMLInputElement).value)"
            />
            <code>{{ (paintAccent || defaults.accent).toLowerCase() }}</code>
          </span>
        </label>
        <label class="look-row">
          <span>背景</span>
          <span class="swatch">
            <input
              type="color"
              :value="paintBackground || defaults.background"
              @input="setColor('background', ($event.target as HTMLInputElement).value)"
            />
            <code>{{ (paintBackground || defaults.background).toLowerCase() }}</code>
          </span>
        </label>
        <label class="look-row">
          <span>前景</span>
          <span class="swatch">
            <input
              type="color"
              :value="paintForeground || defaults.foreground"
              @input="setColor('foreground', ($event.target as HTMLInputElement).value)"
            />
            <code>{{ (paintForeground || defaults.foreground).toLowerCase() }}</code>
          </span>
        </label>
        <label class="look-row">
          <span>UI 字体</span>
          <select v-model="form.uiFontFamily">
            <option v-for="item in UI_FONTS" :key="item.id || 'ui-default'" :value="item.id">
              {{ item.label }}
            </option>
          </select>
        </label>
        <label class="look-row">
          <span>内容字体</span>
          <select v-model="form.contentFontFamily">
            <option v-for="item in CONTENT_FONTS" :key="item.id || 'content-default'" :value="item.id">
              {{ item.label }}
            </option>
          </select>
        </label>
        <label class="look-row">
          <span>代码字体</span>
          <select v-model="form.codeFontFamily">
            <option v-for="item in CODE_FONTS" :key="item.id || 'code-default'" :value="item.id">
              {{ item.label }}
            </option>
          </select>
        </label>
        <label class="look-row">
          <span>半透明侧栏</span>
          <button
            type="button"
            class="switch"
            :class="{ on: form.translucentSidebar }"
            role="switch"
            aria-label="半透明侧栏"
            :aria-checked="form.translucentSidebar"
            @click="form.translucentSidebar = !form.translucentSidebar"
          />
        </label>
        <label class="look-row">
          <span>对比度</span>
          <span class="contrast">
            <input
              v-model.number="form.uiContrast"
              type="range"
              min="0"
              max="100"
              step="1"
            />
            <strong>{{ form.uiContrast }}</strong>
          </span>
        </label>
        <label class="look-row">
          <span>界面字号</span>
          <span class="contrast">
            <input
              v-model.number="form.uiFontSize"
              type="range"
              :min="UI_FONT_MIN"
              :max="UI_FONT_MAX"
              step="1"
            />
            <strong>{{ form.uiFontSize }}</strong>
          </span>
        </label>
      </div>

      <label class="field">
        <span>界面透明度 <em>{{ form.uiOpacity }}%</em></span>
        <input
          v-model.number="form.uiOpacity"
          type="range"
          :min="UI_OPACITY_MIN"
          :max="UI_OPACITY_MAX"
          step="1"
        />
      </label>
      <p class="hint">数字越大，窗体底色越淡，越能看见后面的桌面。终端里的字仍保持不透明。</p>
      <label class="field">
        <span>毛玻璃 <em>{{ form.uiFrost }}%</em></span>
        <input
          v-model.number="form.uiFrost"
          type="range"
          :min="UI_FROST_MIN"
          :max="UI_FROST_MAX"
          step="1"
        />
      </label>
      <p class="hint">数值越大，背景越模糊；设为 0 关闭模糊。外观调整立即生效并自动保存。</p>
      </section>

      <section v-show="section === 'tools'" aria-label="终端与工具设置">
      <p class="section">终端显示</p>
      <label class="field">
        <span>终端字号</span>
        <input v-model.number="form.terminalFontSize" type="number" min="10" max="22" />
      </label>
      <p class="hint">调整后立即生效并自动保存。代码字体可在「外观」中设置。</p>
      <label class="look-row glass-row">
        <span>Grok 跟随窗口玻璃</span>
        <button
          type="button"
          class="switch"
          :class="{ on: form.grokFollowGlass }"
          role="switch"
          aria-label="Grok 跟随窗口玻璃"
          :aria-checked="form.grokFollowGlass"
          @click="form.grokFollowGlass = !form.grokFollowGlass"
        />
      </label>
      <p class="hint">新建 Grok 会话时使用透明终端主题。此项自动保存，已打开的会话需重开后生效。</p>
      <p class="section">工具路径</p>
      <p class="hint">留空时自动查找已安装的工具。修改路径后点击下方「保存设置」。</p>
      <label class="field">
        <span>Claude Code 路径</span>
        <input v-model="form.claudePath" type="text" placeholder="留空则从 PATH 查找 claude" />
      </label>
      <label class="field">
        <span>Pi 路径</span>
        <input v-model="form.piPath" type="text" placeholder="留空则从 PATH 查找 pi" />
      </label>
      <label class="field">
        <span>DeepSeek Harness 路径</span>
        <input v-model="form.dshPath" type="text" placeholder="留空则从 PATH 查找 dsh（含 dsh web）" />
      </label>
      <label class="field">
        <span>Codex 路径</span>
        <input v-model="form.codexPath" type="text" placeholder="留空则从 PATH 查找 codex" />
      </label>
      <p class="hint">Codex 用于编排中的线程读取和自动审查。</p>
      </section>

      <section v-show="section === 'automation'" aria-label="自动化设置">
      <p class="section">Cursor 自动开发</p>
      <label class="field">
        <span>Cursor API Key</span>
        <input v-model="form.cursorApiKey" type="password" autocomplete="off" placeholder="只用于自动开发，可留空" />
      </label>
      <p class="hint">写在本机配置里。Cursor SDK 与 IDE 订阅分开计费。只有通道选 Cursor SDK 的开发者节点才需要。</p>
      </section>
      </div>

      <footer class="settings-footer">
        <template v-if="discardOpen">
          <p role="alert">工具配置尚未保存，要放弃修改并关闭吗？</p>
          <div class="footer-actions"><button type="button" class="btn btn-ghost btn-small" @click="discardOpen = false">继续编辑</button><button type="button" class="btn btn-danger btn-small" @click="emit('close')">放弃修改</button></div>
        </template>
        <template v-else>
          <p>{{ configDirty ? '工具配置有未保存的修改' : section === 'appearance' ? '外观调整会自动保存' : '配置已保存' }}</p>
          <button type="button" class="btn btn-primary btn-small" :disabled="form.saving || !configDirty" @click="submit">{{ form.saving ? '保存中…' : '保存设置' }}</button>
        </template>
      </footer>
    </aside>
  </div>
</template>

<style scoped>
.drawer {
  width: 480px;
  display: flex;
  flex-direction: column;
  padding: 0;
  overflow: hidden;
}

.settings-nav { display: flex; gap: 4px; padding: 0 20px 12px; flex-shrink: 0; border-bottom: 1px solid var(--ad-border); }
.settings-nav button { flex: 1; height: 32px; border-radius: var(--ad-radius-control); color: var(--ad-muted); white-space: nowrap; }
.settings-nav button:hover { background: var(--ad-hover); color: var(--ad-text); }
.settings-nav button.active { background: var(--ad-selected); color: var(--ad-text); font-weight: 600; }
.settings-body { flex: 1; min-height: 0; overflow: auto; padding: 20px 24px; overscroll-behavior: contain; }
.settings-footer { flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 12px; padding: 16px 24px; background: var(--ad-float-solid); border-top: 1px solid var(--ad-border); }
.settings-footer p { margin: 0; font-size: 12px; color: var(--ad-muted); }
.footer-actions { display: flex; gap: 8px; margin-left: auto; }

.look {
  margin: 8px 0 20px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  overflow: hidden;
  background: color-mix(in srgb, var(--ad-harbor) 80%, transparent);
}

.look-head {
  padding: 12px 14px;
  font-weight: 600;
  border-bottom: 1px solid var(--ad-border);
}

.look-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 46px;
  margin: 0;
  padding: 0 14px;
  border-bottom: 1px solid var(--ad-border);
}

.look-row:last-child {
  border-bottom: 0;
}

.glass-row {
  margin: 8px 0 0;
  padding: 0;
  border: 0;
  min-height: 36px;
}

.look-row > span:first-child {
  color: var(--ad-text);
}

.look-row select {
  width: auto;
  min-width: min(168px, 55%);
  max-width: 65%;
  height: 30px;
  padding: 0 28px 0 10px;
}

.swatch {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 10px 0 6px;
  border-radius: 8px;
  background: var(--ad-raised);
  border: 1px solid var(--ad-border);
}

.swatch input[type='color'] {
  width: 18px;
  height: 18px;
  padding: 0;
  border: 0;
  background: none;
}

.swatch code {
  font-size: 12px;
  color: var(--ad-muted);
}

.switch {
  width: 36px;
  height: 20px;
  border-radius: 99px;
  background: var(--ad-selected);
  position: relative;
}

.switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  transition: left var(--ad-transition);
}

.switch.on {
  background: #3b82f6;
}

.switch.on::after {
  left: 18px;
}

.contrast {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 55%;
  min-width: 0;
}

.contrast input {
  min-width: 0;
  width: 100%;
  flex: 1;
}

.contrast strong {
  min-width: 28px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.section {
  margin: 0 0 10px;
  font-size: 13px;
  color: var(--ad-text);
}

.themes {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
  margin-bottom: 8px;
}

.theme {
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: center;
  padding: 0;
  color: var(--ad-muted);
  font-size: 12px;
}

.theme.on {
  color: var(--ad-text);
}

.theme-preview {
  position: relative;
  display: block;
  width: 100%;
  height: 72px;
  border-radius: 10px;
  border: 2px solid var(--ad-border);
  overflow: hidden;
}

.theme.on .theme-preview {
  border-color: var(--ad-text);
}

.theme-preview[data-theme='dark'] {
  background: linear-gradient(180deg, #1f2327 0 18px, #0b0f13 18px);
}

.theme-preview[data-theme='light'] {
  background: linear-gradient(180deg, #ececec 0 18px, #ffffff 18px);
}

.theme-preview[data-theme='system'] {
  background: linear-gradient(90deg, #f3f3f3 50%, #0b0f13 50%);
}

.theme-preview i,
.theme-preview b,
.theme-preview em {
  position: absolute;
  border-radius: 4px;
}

.theme-preview i {
  left: 8px;
  right: 8px;
  top: 26px;
  height: 8px;
  background: currentColor;
  opacity: 0.28;
}

.theme-preview b {
  left: 8px;
  width: 46%;
  top: 40px;
  height: 8px;
  background: currentColor;
  opacity: 0.18;
}

.theme-preview em {
  left: 8px;
  width: 32%;
  top: 54px;
  height: 8px;
  background: currentColor;
  opacity: 0.12;
}

.theme-preview[data-theme='dark'] {
  color: #fff;
}

.theme-preview[data-theme='light'] {
  color: #111;
}

.theme-preview[data-theme='system'] {
  color: #888;
}

.field-row {
  flex-direction: row;
  align-items: center;
}

.field-row span {
  min-width: 72px;
}

.field-row input[type='range'] {
  flex: 1;
}

.field-row strong {
  min-width: 48px;
  text-align: right;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px 16px;
  flex-shrink: 0;
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
