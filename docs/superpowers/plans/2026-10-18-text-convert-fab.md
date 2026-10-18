# 简繁与符号清洗浮动工具 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Agent Dock 右下角加一个常驻浮动按钮，把中文转成繁体字形并清洗成英文半角标点，结果可改再复制；未映射字符一律删除。

**Architecture:** 纯前端。`src/lib/textConvert.ts` 按码点封闭分类（汉字保留、允许的 ASCII 保留、映射表替换、其余删除）；简繁用 OpenCC `STCharacters` 字符表逐字替换，不用词组。`TextConvertFab.vue` 挂在 `App.vue` 最外层，控制台和编排共用。

**Tech Stack:** Vue 3、TypeScript、现有 `node --experimental-strip-types` 测试、OpenCC `STCharacters.txt`（生成 `stCharacters.ts`）、现有 `api.clipboardWrite` / `showToast`。

**Spec:** `docs/superpowers/specs/2026-10-18-text-convert-fab-design.md`

## Global Constraints

- 每个码点必须归类；缺映射或不允许保留的一律删除，禁止原样漏出。
- 简繁只换字形（「软件」→「軟件」），不用 STPhrases / s2twp。
- 第一版 `traditional` 与 `all` 输出相同；`symbols` 不做简繁。
- 每次转换从原文重算，不拿结果框二次加工。
- 浮动按钮：`right: 16px; bottom: 56px; z-index: 35`。不铺遮罩。不写 localStorage。不走 Rust。
- 复制成功 toast「已复制」，失败「复制失败」。空原文禁用三个转换按钮，空结果禁用复制。
- 测试命令：`npm run test:convert`。

---

## File structure

| File | Responsibility |
|------|----------------|
| Create: `src/lib/textConvert.ts` | `convertText` / `isAllowedOutputChar` / `convertReady`；封闭分类 |
| Create: `src/lib/textConvert.test.ts` | 映射、删除、扫表不漏原符、简繁字形 |
| Create: `scripts/fetch-st-characters.mjs` | 从 OpenCC 拉取 STCharacters，生成字符表 |
| Create: `src/lib/opencc/stCharacters.ts` | `ST_CHARACTERS: Record<string, string>` |
| Create: `src/components/TextConvertFab.vue` | 右下角按钮 + 浮层面板 |
| Modify: `package.json` | 增加 `test:convert` |
| Modify: `src/lib/api.ts` | `clipboardWrite` 返回 `Promise<boolean>` |
| Modify: `src/App.vue` | 挂载 `TextConvertFab` |

---

### Task 1: 封闭分类器（洗符号）

**Files:**
- Create: `src/lib/textConvert.ts`
- Create: `src/lib/textConvert.test.ts`
- Modify: `package.json`（`scripts.test:convert`）

**Interfaces:**
- Consumes: 无
- Produces:
  - `export type ConvertMode = 'traditional' | 'symbols' | 'all'`
  - `export function convertText(input: string, mode: ConvertMode): string`
  - `export function isAllowedOutputChar(ch: string): boolean`
  - 本任务 `traditional` / `all` 尚未做简繁，行为等于先原样再分类（与 `symbols` 相同）。简繁在 Task 2。

- [ ] **Step 1: 写失败测试**

创建 `src/lib/textConvert.test.ts`：

```ts
import assert from 'node:assert/strict'
import { convertText, isAllowedOutputChar, type ConvertMode } from './textConvert.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function assertAllowed(out: string) {
  for (const ch of out) {
    const cp = ch.codePointAt(0)!.toString(16)
    assert.equal(isAllowedOutputChar(ch), true, `leaked U+${cp}`)
  }
}

function assertNoBlockChar(out: string, cp: number) {
  const ch = String.fromCodePoint(cp)
  assert.ok(!out.includes(ch), `block char leaked U+${cp.toString(16)}`)
}

test('empty input stays empty', () => {
  assert.equal(convertText('', 'symbols'), '')
  assert.equal(convertText('', 'all'), '')
})

test('fullwidth letters digits parens become ASCII', () => {
  const out = convertText('ＡＢＣ（１２３）', 'symbols')
  assert.equal(out, 'ABC(123)')
  assertAllowed(out)
})

test('CJK punctuation becomes ASCII and simplified Han stays', () => {
  const out = convertText('软件（测试），。', 'symbols')
  assert.equal(out, '软件(测试),.')
  assertAllowed(out)
})

test('emoji yen zwsp and latin-1 are deleted', () => {
  const out = convertText('软件😀￥\u200Bé，', 'symbols')
  assert.equal(out, '软件,')
  assertAllowed(out)
})

test('ideographic comma ellipsis and quotes map', () => {
  const out = convertText('说\uFF1A\u300C你好\u300D\u2026\u300A书\u300B', 'symbols')
  assert.equal(out, '说:["你好"]...<<书>>')
  assertAllowed(out)
})

test('every output char of mixed sample is allowed', () => {
  const out = convertText('（Hello，世界！）', 'symbols')
  assert.equal(out, '(Hello,世界!)')
  assertAllowed(out)
})

test('fullwidth and CJK punctuation blocks never leak original chars', () => {
  const ranges: Array<[number, number]> = [
    [0xff00, 0xffef],
    [0x3000, 0x303f]
  ]
  for (const [from, to] of ranges) {
    for (let cp = from; cp <= to; cp++) {
      const ch = String.fromCodePoint(cp)
      const out = convertText(ch, 'symbols')
      assertAllowed(out)
      assertNoBlockChar(out, cp)
    }
  }
})

test('traditional and all match in v1 once classifier-only', () => {
  const input = '软件（测试），。😀'
  const modes: ConvertMode[] = ['traditional', 'all']
  const a = convertText(input, modes[0])
  const b = convertText(input, modes[1])
  assert.equal(a, b)
  assertAllowed(a)
})
```

- [ ] **Step 2: 跑测试，确认失败**

Run: `node --experimental-strip-types src/lib/textConvert.test.ts`

Expected: FAIL，`Cannot find module './textConvert.ts'`（或等价的 MODULE_NOT_FOUND）。

- [ ] **Step 3: 写最小实现**

创建 `src/lib/textConvert.ts`：

```ts
export type ConvertMode = 'traditional' | 'symbols' | 'all'

const HAN_RANGES: Array<[number, number]> = [
  [0x3400, 0x4dbf],
  [0x4e00, 0x9fff],
  [0xf900, 0xfaff],
  [0x20000, 0x2a6df],
  [0x2a700, 0x2b73f],
  [0x2b740, 0x2b81f],
  [0x2b820, 0x2ceaf],
  [0x2ceb0, 0x2ebef],
  [0x2ebf0, 0x2ee5f],
  [0x2f800, 0x2fa1f],
  [0x30000, 0x3134f],
  [0x31350, 0x323af]
]

const PUNCT: Record<number, string> = {
  0x3000: ' ',
  0x3001: ',',
  0x3002: '.',
  0x3008: '<',
  0x3009: '>',
  0x300a: '<<',
  0x300b: '>>',
  0x300c: '[',
  0x300d: ']',
  0x300e: '[',
  0x300f: ']',
  0x3010: '[',
  0x3011: ']',
  0x3014: '[',
  0x3015: ']',
  0x3016: '[',
  0x3017: ']',
  0x3018: '[',
  0x3019: ']',
  0x301a: '[',
  0x301b: ']',
  0x301d: '"',
  0x301e: '"',
  0x201c: '"',
  0x201d: '"',
  0x2018: "'",
  0x2019: "'",
  0x2026: '...',
  0x22ef: '...',
  0x2014: '-',
  0x2013: '-',
  0x2015: '-',
  0x00b7: '.',
  0x30fb: '.',
  0xff65: '.',
  0x00a0: ' ',
  0x00ab: '"',
  0x00bb: '"'
}

function isAllowedAscii(cp: number): boolean {
  return cp === 0x09 || cp === 0x0a || cp === 0x0d || (cp >= 0x20 && cp <= 0x7e)
}

function isHan(cp: number): boolean {
  for (const [lo, hi] of HAN_RANGES) {
    if (cp >= lo && cp <= hi) return true
  }
  return false
}

export function isAllowedOutputChar(ch: string): boolean {
  const cp = ch.codePointAt(0)
  if (cp === undefined) return false
  return isAllowedAscii(cp) || isHan(cp)
}

function mapCodePoint(cp: number): string | null {
  if (cp >= 0xff01 && cp <= 0xff5e) return String.fromCodePoint(cp - 0xfee0)
  return PUNCT[cp] ?? null
}

function toTraditional(input: string): string {
  return input
}

function classify(input: string): string {
  let out = ''
  for (const ch of input) {
    const cp = ch.codePointAt(0)
    if (cp === undefined) continue
    if (isAllowedAscii(cp) || isHan(cp)) {
      out += ch
      continue
    }
    const mapped = mapCodePoint(cp)
    if (mapped !== null) out += mapped
  }
  return out
}

export function convertText(input: string, mode: ConvertMode): string {
  if (!input) return ''
  const source = mode === 'symbols' ? input : toTraditional(input)
  return classify(source)
}
```

`package.json` 的 `scripts` 增加：

```json
"test:convert": "node --experimental-strip-types src/lib/textConvert.test.ts"
```

放在 `"test:cite"` 旁边。

- [ ] **Step 4: 跑测试，确认通过**

Run: `npm run test:convert`

Expected: 每条打印 `ok …`，进程退出码 0。标点用例已用 `\uFF1A` `\u300C` `\u2026` `\u300A` 写死码点，不要改分类器去迁就错误输入。

- [ ] **Step 5: Commit**

```
git add src/lib/textConvert.ts src/lib/textConvert.test.ts package.json
git commit -m "feat: 文本转换封闭分类器（洗符号）"
```

---

### Task 2: OpenCC 字形表（转繁体）

**Files:**
- Create: `scripts/fetch-st-characters.mjs`
- Create: `src/lib/opencc/stCharacters.ts`（脚本生成，不要手写整表）
- Modify: `src/lib/textConvert.ts`
- Modify: `src/lib/textConvert.test.ts`

**Interfaces:**
- Consumes: Task 1 的 `convertText` / `ConvertMode` / `isAllowedOutputChar`
- Produces:
  - `export function convertReady(): boolean`
  - `toTraditional` 改为逐字查 `ST_CHARACTERS`（多候选取第一个）
  - `convertText(..., 'traditional' | 'all')` 先字形再分类
  - `convertText(..., 'symbols')` 仍不简繁

- [ ] **Step 1: 写失败的简繁测试**

在 `src/lib/textConvert.test.ts` 追加：

```ts
test('all converts glyphs and punctuation', () => {
  const out = convertText('软件（测试），。', 'all')
  assert.equal(out, '軟件(測試),.')
  assertAllowed(out)
})

test('traditional matches all', () => {
  const input = '软件（测试），。😀￥'
  assert.equal(convertText(input, 'traditional'), convertText(input, 'all'))
})

test('symbols keeps simplified Han', () => {
  const out = convertText('软件，', 'symbols')
  assert.equal(out, '软件,')
  assertAllowed(out)
})
```

把 Task 1 里「traditional and all match in v1 once classifier-only」留着，它在接上字形表后仍应通过（两者仍然相等，只是都会变成繁体）。

- [ ] **Step 2: 跑测试，确认简繁用例失败**

Run: `npm run test:convert`

Expected: FAIL，`软件（测试），。` 的 `all` 实际是 `软件(测试),.`，期望 `軟件(測試),.`。

- [ ] **Step 3: 拉取字形表并接到 convertText**

创建 `scripts/fetch-st-characters.mjs`：

```js
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const SRC =
  'https://raw.githubusercontent.com/BYVoid/OpenCC/ver.1.1.9/data/dictionary/STCharacters.txt'
const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outFile = join(root, 'src', 'lib', 'opencc', 'stCharacters.ts')

const res = await fetch(SRC)
if (!res.ok) throw new Error(`fetch STCharacters failed: ${res.status}`)
const text = await res.text()
const entries = {}
for (const line of text.split(/\r?\n/)) {
  const trimmed = line.trim()
  if (!trimmed || trimmed.startsWith('#')) continue
  const [from, to] = trimmed.split('\t')
  if (!from || !to) continue
  entries[from] = to.split(' ')[0]
}
if (entries['软'] !== '軟' || entries['测'] !== '測' || entries['试'] !== '試') {
  throw new Error('STCharacters missing expected glyph mappings')
}
mkdirSync(dirname(outFile), { recursive: true })
const body =
  'export const ST_CHARACTERS: Record<string, string> = ' +
  JSON.stringify(entries) +
  '\n'
writeFileSync(outFile, body)
console.log(`wrote ${Object.keys(entries).length} mappings to ${outFile}`)
```

Run: `node scripts/fetch-st-characters.mjs`

Expected: 打印 wrote N mappings（N 应大于 3000），并生成 `src/lib/opencc/stCharacters.ts`。

改 `src/lib/textConvert.ts`：加上 import 和 `convertReady`，替换 `toTraditional`：

```ts
import { ST_CHARACTERS } from './opencc/stCharacters.ts'

export function convertReady(): boolean {
  return ST_CHARACTERS['软'] === '軟'
}

function toTraditional(input: string): string {
  let out = ''
  for (const ch of input) {
    out += ST_CHARACTERS[ch] ?? ch
  }
  return out
}
```

其余分类逻辑不动。`convertText` 已是 `mode === 'symbols' ? input : toTraditional(input)`。

- [ ] **Step 4: 跑测试，确认通过**

Run: `npm run test:convert`

Expected: 全部 `ok`，退出码 0。扫表测试仍须通过（简繁不得把全角块字符原样留下）。

- [ ] **Step 5: Commit**

```
git add scripts/fetch-st-characters.mjs src/lib/opencc/stCharacters.ts src/lib/textConvert.ts src/lib/textConvert.test.ts
git commit -m "feat: 接入 OpenCC 字形表做简转繁"
```

---

### Task 3: 浮动按钮和面板

**Files:**
- Create: `src/components/TextConvertFab.vue`
- Modify: `src/App.vue`（import、模板挂载）
- Modify: `src/lib/api.ts`（`clipboardWrite` 返回是否成功）

**Interfaces:**
- Consumes:
  - `convertText(input: string, mode: ConvertMode): string`
  - `convertReady(): boolean`
  - `showToast(message: string): void` from `src/lib/store.ts`
  - `api.clipboardWrite(text: string): Promise<boolean>`
- Produces: 控制台和编排都可见的右下角工具，无新 store 字段

- [ ] **Step 1: clipboardWrite 改为返回 boolean**

把 `src/lib/api.ts` 的 `clipboardWrite` 改成：

```ts
export async function clipboardWrite(text: string): Promise<boolean> {
  if (!text) return false
  if (isTauri) {
    try {
      await invoke('clipboard_write', { text })
      return true
    } catch {
      /* fall through to the web clipboard */
    }
  }
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch {
      /* copy is best-effort */
    }
  }
  return false
}
```

`TerminalPane.vue` 的 `await api.clipboardWrite(text)` 不用改。不要为此单独加测试文件。

- [ ] **Step 2: 写 TextConvertFab.vue**

创建 `src/components/TextConvertFab.vue`：

```vue
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import * as api from '../lib/api'
import { convertReady, convertText, type ConvertMode } from '../lib/textConvert'
import { showToast } from '../lib/store'

const open = ref(false)
const source = ref('')
const result = ref('')
const tableReady = convertReady()

const hasSource = computed(() => source.value.length > 0)
const hasResult = computed(() => result.value.length > 0)
const convertEnabled = computed(() => tableReady && hasSource.value)

function toggle() {
  open.value = !open.value
}

function close() {
  open.value = false
}

function run(mode: ConvertMode) {
  if (!convertEnabled.value) return
  result.value = convertText(source.value, mode)
}

async function copyResult() {
  if (!hasResult.value) return
  const ok = await api.clipboardWrite(result.value)
  showToast(ok ? '已复制' : '复制失败')
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === 'Escape' && open.value) {
    ev.preventDefault()
    close()
  }
}

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="wrap">
    <button
      type="button"
      class="fab"
      aria-label="文本转换"
      :aria-expanded="open"
      @click="toggle"
    >
      文
    </button>
    <div v-if="open" class="panel" role="dialog" aria-label="文本转换">
      <p v-if="!tableReady" class="warn">转换表不可用</p>
      <label class="field">
        <span>原文</span>
        <textarea v-model="source" class="source" spellcheck="false" />
      </label>
      <label class="field">
        <span>结果</span>
        <textarea v-model="result" class="result" spellcheck="false" />
      </label>
      <div class="row">
        <button type="button" class="btn" :disabled="!convertEnabled" @click="run('traditional')">
          转繁体
        </button>
        <button type="button" class="btn" :disabled="!convertEnabled" @click="run('symbols')">
          洗符号
        </button>
        <button type="button" class="btn btn-primary" :disabled="!convertEnabled" @click="run('all')">
          全部转换
        </button>
        <button type="button" class="btn btn-ghost" :disabled="!hasResult" @click="copyResult">
          复制
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wrap {
  position: fixed;
  right: 16px;
  bottom: 56px;
  z-index: 35;
  display: flex;
  flex-direction: column-reverse;
  align-items: flex-end;
  gap: 8px;
}

.fab {
  width: 44px;
  height: 44px;
  border-radius: var(--ad-radius-pill);
  background: var(--ad-primary);
  color: var(--ad-primary-text);
  font-size: 16px;
  font-weight: 600;
  box-shadow: var(--ad-shadow-menu);
}

.fab:hover {
  background: var(--ad-accent-hover);
}

.panel {
  width: 420px;
  max-width: calc(100vw - 32px);
  height: 480px;
  max-height: calc(100vh - 120px);
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  background: var(--ad-float);
  border: 1px solid var(--ad-border);
  border-radius: var(--ad-radius-card);
  box-shadow: var(--ad-shadow-menu);
}

.warn {
  margin: 0;
  color: var(--ad-error);
  font-size: 13px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
  color: var(--ad-muted);
  font-size: 12px;
}

.source,
.result {
  height: auto;
  min-height: 0;
  flex: 1;
  padding: 8px 10px;
  resize: none;
  line-height: 1.55;
  color: var(--ad-text);
  font-size: 13px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: flex-start;
}

.row .btn-ghost {
  margin-left: auto;
}
</style>
```

`.wrap` 使用 `column-reverse`：DOM 里按钮在前，视觉上按钮在下、面板在上。

- [ ] **Step 3: 挂到 App.vue**

在 `src/App.vue` 的 script import 区增加：

```ts
import TextConvertFab from './components/TextConvertFab.vue'
```

在模板里、`<ConfirmDialog ... />` 之后、`store.toast` 之前插入：

```vue
    <TextConvertFab />
```

不要包 `v-if` 按 `appMode` 过滤。

- [ ] **Step 4: 类型检查 + 转换测试**

Run: `npm run test:convert`

Expected: 仍全部通过。

Run: `npx vue-tsc --noEmit`

Expected: 退出码 0。若 `stCharacters.ts` 触发体积/解析问题，保持生成文件为单行 `export const ST_CHARACTERS: Record<string, string> = {...}`，不要拆成上万行。

- [ ] **Step 5: 浏览器/桌面核对（实现者本机）**

`npm run tauri:dev`（或 `npm run dev` 若只验 UI）：

1. 控制台模式右下角有「文」按钮，点开面板，Esc 或再点按钮关闭。
2. 原文贴 `软件（测试），。😀￥`，点「全部转换」，结果为 `軟件(測試),.`（无 emoji、无 ￥）。
3. 点「洗符号」，结果为 `软件(测试),.`（简体汉字还在）。
4. 改结果框几个字，点「复制」，出现 toast「已复制」。
5. 切到编排模式，按钮仍在，转换仍可用。
6. 原文清空时三个转换按钮不可用；结果清空时复制不可用。

- [ ] **Step 6: Commit**

```
git add src/components/TextConvertFab.vue src/App.vue src/lib/api.ts
git commit -m "feat: 右下角简繁与符号清洗浮动面板"
```

---

## Self-review (plan vs spec)

| Spec 条目 | Task |
|-----------|------|
| 右下角常驻 FAB，两种模式 | Task 3 `App.vue` 根上挂载，无 appMode 过滤 |
| 面板原文/结果/三按钮/复制 | Task 3 SFC |
| 从原文重算、不写 localStorage | Task 3：`run` 读 `source`；无 storage API |
| 只换字形 软件→軟件 | Task 2 `ST_CHARACTERS` 逐字 |
| 封闭分类默认删除 | Task 1 `classify` else 分支不 append |
| 全角 FF01–FF5E、CJK 标点映射 | Task 1 `mapCodePoint` + `PUNCT` |
| 扫表不漏原符 | Task 1 双 block 循环 |
| `traditional` === `all` | Task 1+2 测试 |
| OpenCC 失败时按钮不可用 | Task 2 `convertReady` + Task 3 `convertEnabled` / 「转换表不可用」 |
| 复制成功/失败 toast | Task 3 + `clipboardWrite` boolean |
| `test:convert` | Task 1 `package.json` |
| 不做拖动、贴终端、词组级繁体、Rust | 全计划未引入 |

无 TBD / 「类似 Task N」占位。`ConvertMode`、`convertText`、`convertReady`、`clipboardWrite(): Promise<boolean>` 在后续任务中的名字与 Task 1–3 一致。
