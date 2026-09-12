<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import {
  CODE_FONTS,
  CONTENT_FONTS,
  UI_FONTS,
  UI_FONT_MAX,
  UI_FONT_MIN,
  UI_THEMES,
  appearanceFromSettings,
  appearanceToSettings,
  applyAppearance,
  parseHex,
  resolvedTheme,
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
  applyUiGlass,
  clampUiFrost,
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

function syncForm() {
  const look = appearanceFromSettings(props.settings)
  form.terminalFontSize = props.settings.terminalFontSize
  form.uiFontSize = look.uiFontSize
  form.uiTheme = look.theme
  form.uiAccent = look.accent
  form.uiBackground = look.background
  form.uiForeground = look.foreground
  form.uiFontFamily = look.uiFontFamily
  form.contentFontFamily = look.contentFontFamily
  form.codeFontFamily = look.codeFontFamily
  form.uiContrast = look.contrast
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

const resolved = computed(() => resolvedTheme(form.uiTheme, systemPrefersLight()))
const defaults = computed(() => themeDefaults(resolved.value))
const themeTitle = computed(() => (resolved.value === 'light' ? '浅色主题' : '深色主题'))

watch(
  () => [props.open, props.settings] as const,
  ([open]) => {
    syncForm()
    if (!open) {
      applyUiGlass(props.settings.uiOpacity, props.settings.uiFrost)
      applyAppearance(appearanceFromSettings(props.settings))
    }
  }
)

function previewGlass() {
  applyUiGlass(form.uiOpacity, form.uiFrost)
}

function previewAppearance() {
  applyAppearance({
    theme: form.uiTheme,
    uiFontSize: form.uiFontSize,
    accent: form.uiAccent,
    background: form.uiBackground,
    foreground: form.uiForeground,
    uiFontFamily: form.uiFontFamily,
    contentFontFamily: form.contentFontFamily,
    codeFontFamily: form.codeFontFamily,
    contrast: form.uiContrast,
    translucentSidebar: form.translucentSidebar
  })
}

function pickTheme(id: UiTheme) {
  form.uiTheme = id
  previewAppearance()
}

function setColor(key: 'uiAccent' | 'uiBackground' | 'uiForeground', value: string) {
  form[key] = parseHex(value)
  previewAppearance()
}

function submit() {
  const size = Number(form.terminalFontSize) || 14
  form.saving = true
  emit('save', {
    ...props.settings,
    ...appearanceToSettings({
      theme: form.uiTheme,
      uiFontSize: form.uiFontSize,
      accent: form.uiAccent,
      background: form.uiBackground,
      foreground: form.uiForeground,
      uiFontFamily: form.uiFontFamily,
      contentFontFamily: form.contentFontFamily,
      codeFontFamily: form.codeFontFamily,
      contrast: form.uiContrast,
      translucentSidebar: form.translucentSidebar
    }),
    terminalFontSize: Math.min(22, Math.max(10, size)),
    uiOpacity: clampUiOpacity(form.uiOpacity),
    uiFrost: clampUiFrost(form.uiFrost),
    grokFollowGlass: Boolean(form.grokFollowGlass),
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
  <div v-if="open" class="ad-mask ad-mask--flush" @click.self="emit('close')">
    <aside class="ad-drawer drawer" aria-label="设置">
      <header>
        <h2>设置</h2>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <p class="section">外观</p>
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
              :value="form.uiAccent || defaults.accent"
              @input="setColor('uiAccent', ($event.target as HTMLInputElement).value)"
            />
            <code>{{ (form.uiAccent || defaults.accent).toLowerCase() }}</code>
          </span>
        </label>
        <label class="look-row">
          <span>背景</span>
          <span class="swatch">
            <input
              type="color"
              :value="form.uiBackground || defaults.background"
              @input="setColor('uiBackground', ($event.target as HTMLInputElement).value)"
            />
            <code>{{ (form.uiBackground || defaults.background).toLowerCase() }}</code>
          </span>
        </label>
        <label class="look-row">
          <span>前景</span>
          <span class="swatch">
            <input
              type="color"
              :value="form.uiForeground || defaults.foreground"
              @input="setColor('uiForeground', ($event.target as HTMLInputElement).value)"
            />
            <code>{{ (form.uiForeground || defaults.foreground).toLowerCase() }}</code>
          </span>
        </label>
        <label class="look-row">
          <span>UI 字体</span>
          <select v-model="form.uiFontFamily" @change="previewAppearance">
            <option v-for="item in UI_FONTS" :key="item.id || 'ui-default'" :value="item.id">
              {{ item.label }}
            </option>
          </select>
        </label>
        <label class="look-row">
          <span>内容字体</span>
          <select v-model="form.contentFontFamily" @change="previewAppearance">
            <option v-for="item in CONTENT_FONTS" :key="item.id || 'content-default'" :value="item.id">
              {{ item.label }}
            </option>
          </select>
        </label>
        <label class="look-row">
          <span>代码字体</span>
          <select v-model="form.codeFontFamily" @change="previewAppearance">
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
            :aria-checked="form.translucentSidebar"
            @click="form.translucentSidebar = !form.translucentSidebar; previewAppearance()"
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
              @input="previewAppearance"
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
              @input="previewAppearance"
            />
            <strong>{{ form.uiFontSize }}</strong>
          </span>
        </label>
      </div>

      <label class="field">
        <span>终端字号</span>
        <input v-model.number="form.terminalFontSize" type="number" min="10" max="22" />
      </label>
      <p class="hint">只影响右侧命令行窗口的字号。代码字体在上面外观里选。</p>
      <label class="field">
        <span>界面透明度 <em>{{ form.uiOpacity }}%</em></span>
        <input
          v-model.number="form.uiOpacity"
          type="range"
          :min="UI_OPACITY_MIN"
          :max="UI_OPACITY_MAX"
          step="1"
          @input="previewGlass"
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
          @input="previewGlass"
        />
      </label>
      <p class="hint">数字越大，背后内容越糊。拉到 0 就是清透，不再在桌面上再套一层霜。拖动即可预览，点保存后写入本机。</p>
      <label class="look-row glass-row">
        <span>Grok 跟随窗口玻璃</span>
        <button
          type="button"
          class="switch"
          :class="{ on: form.grokFollowGlass }"
          role="switch"
          :aria-checked="form.grokFollowGlass"
          @click="form.grokFollowGlass = !form.grokFollowGlass"
        />
      </label>
      <p class="hint">打开后，Dock 里新开的 Grok 使用官方 terminal 主题（不涂实色底）。不改 ~/.grok/config.toml，外面单独跑 grok 不受影响。已开的会话要关掉重开。</p>
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
      <p class="hint">编排里的 Codex 节点用 `codex app-server` 列线程并做 detached 审查，不点桌面窗口，也不走内嵌终端。</p>
      <label class="field">
        <span>Cursor API Key</span>
        <input v-model="form.cursorApiKey" type="password" autocomplete="off" placeholder="只用于自动开发，可留空" />
      </label>
      <p class="hint">写在本机配置里。Cursor SDK 与 IDE 订阅分开计费。只有通道选 Cursor SDK 的开发者节点才需要。</p>
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
  width: 480px;
}

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
  min-width: 168px;
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
  min-width: 168px;
}

.contrast input {
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
