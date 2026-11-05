// Run via playwright-cli run-code against the Vite preview. No local documents are read or written.
async page => {
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.setViewportSize({ width: 1600, height: 1000 })
  await page.route('**/document-reader-smoke', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1421/document-reader-smoke')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.evaluate(async () => {
    const source = await (await fetch('/src/components/MarkdownPreviewDialog.vue')).text()
    const vueUrl = source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    if (!vueUrl) throw new Error('Could not resolve Vue runtime')
    const text = [
      '下面按一条主线展开：**概念页内容 → 流程角色 → 是否进入提示词 → Debug 字段 → 生效阶段 → 代码位置**。', '',
      '> **先给总判断**：业务概念页由 `frontmatter`（机器配置）和正文（人读文档）组成。运行时真正生效的只有配置；进入提示词的是 **改写后的问句** 和 **被锁定的指标**。', '',
      '## 1. 一张总图', '', '```mermaid', 'flowchart TD',
      ' A["概念页 Markdown"] --> B["YAML frontmatter"]',
      ' A --> C["正文：定义 / 默认口径 / 判定规则"]',
      ' B --> D["解析、结构校验与目录校验"]',
      ' C --> E["管理台展示 / 导出"]',
      ' D --> F["生效快照 current_pages"]',
      ' F --> G["路由检验与概念命中"]',
      ' G --> H["问句改写与指标锁定"]',
      ' H --> I["逐指标规划、执行与权限切面"]',
      ' I --> J["合表"]',
      ' J --> K{"verdict"}',
      ' K -->|unmet / met / all| L["build_attainment 达标判定"]',
      ' K -->|analyze| M["build_composition 构成分析"]',
      ' L --> N["规则叙述、回显与 Debug"]',
      ' M --> N', '```', '',
      '## 2. 逐项矩阵：概念内容到底在哪里生效', '',
      '### 2.1 真正影响答案的字段', '',
      '| 概念内容 | 担任角色 | 是否进入提示词 | Debug「业务概念」中的体现 | 生效阶段 | 底层代码 |',
      '| --- | --- | --- | --- | --- | --- |',
      '| `triggers.unmet/met/all` | 入口键，决定 verdict（未达标 / 达标 / 全部） | **间接**：触发词会从问句消费掉 | `concept_wiki_route.data.trigger_verdict`、overview 节点复用 `.phrase` | 路由预检 → 概念命中 → 问句改写 | `matcher.py:274`、`:326`、`:208` |',
      '| `triggers.analyze` | 分析型入口，决定走构成分析 | **间接**：触发词被消费 | `.verdict="analyze"`、`.compositions` | 概念命中 → 构成范围选择 | `matcher.py:300–320` |',
      '| `entities` | 识别对象词（车间 / 基地 / 产线），决定分组和判定对象列 | **间接**：改写时在对象词前补名称 | `.entity`（只有 word，不含 dim） | 命中 → 问句改写 → 后处理对齐 | `matcher.py:244` |',
      '| `pairs.actual` | 本轮要取数的实际指标 | **间接**：指标 code 被锁定，名称和定义进入提示词 | `.pairs` 只有 pair id；指标名称在 summary 文案里 | 锁定指标 → 规划 → 执行 → 判定 | `concept_route.py:178` |',
      '| `pairs.reference` | 对比用的参照指标（目标 / 计划） | **间接**：同上 | 同上 | 锁定指标 → 执行 → 判定 | `validation.py:12` |',
      '| `pairs.label` | 点名识别与展示名称 | 否 | 影响 `.pairs` 和 `summary` | 点名识别、问句消费、通知文案 | `matcher.py:97–112` |',
      '| `pairs.keywords` | 识别别名，**不从问句消费** | 否 | 无 | 点名识别，避免删坏“电池车间”等词 | `matcher.py:111–112` |',
      '| `pairs.direction` | 比较方向；目录未配置时兜底 | 否 | 兜底时进入结果 `notices` | 合表后判定 | `validation.py:50` |',
      '| `pairs.value_type` | 比率 / 数量：决定刻度换算和差距单位 | 否 | 影响 `counts`、判定文案 | 合表后判定 | `attainment.py:114` |',
      '| `pairs.tolerance` | 比较容差 | 否 | 无 | 合表后判定 | `attainment.py:134` |',
      '| `overview` | 概念整体问法，识别标题 / 别名 | 否 | `concept_wiki_overview.data.phrase` | 概念命中 → 指标展开 | `matcher.py:374` |',
      '| `concept_id` | 概念身份 | 否 | `.concept_id` | 快照、合同、trace、选项 id | `concept_route.py:203` |', '',
      '### 2.2 只影响“是否可用 / 展示”的内容', '',
      '- 标题和正文供人阅读；**不要当作运行时规则**。',
      '- 指标名称来自目录，代码标识在 `pairs` 中锁定。', '',
      '## 3. Debug 节点与代码追踪', '',
      '确认 `concept_id`、命中原因与指标范围后，再检查执行结果。', '',
      '```ts', 'const verdict = route.verdict', 'const pairs = route.pairs', '```', '',
      '## 4. 数值与对齐', '', '| 指标 | 实际 | 目标 |', '| :--- | ---: | ---: |', '| 产量 | 1,234 | 1,500 |', '',
      '## 5. 另一种图表', '', '```mermaid', 'sequenceDiagram', ' participant A as 用户', ' participant B as 路由', ' A->>B: 查询产量', ' B-->>A: 锁定指标', '```', '',
      '## 6. 错误回退', '', '```mermaid', 'flowchart TD', ' A[broken', '```'
    ].join('\n')
    window.__readerFixture = { text, failed: false, empty: false, delay: 0, calls: 0 }
    window.__TAURI_INTERNALS__ = { invoke: async command => {
      const f = window.__readerFixture
      if (command === 'read_session_doc') {
        f.calls++
        if (f.delay) await new Promise(resolve => setTimeout(resolve, f.delay))
        if (f.failed) throw new Error('文档暂时无法读取')
        return { path: 'fixture.md', title: '业务概念与运行时', relPath: null, text: f.empty ? '' : f.text }
      }
      throw new Error(`Unexpected command: ${command}`)
    } }
    const { createApp, h, ref } = await import(vueUrl)
    const { modal } = await import('/src/lib/modal.ts')
    const { default: Reader } = await import('/src/components/MarkdownPreviewDialog.vue')
    await import('/src/styles/global.css')
    const open = ref(true)
    window.__readerOpen = open
    createApp({ render: () => h(Reader, { open: open.value, projectId: 'fixture', toolId: 'omp', sessionTitle: '业务概念', doc: { path: 'fixture.md', title: '第 2 轮 · 业务概念在现有流程中的生效机制与代码追踪', kind: 'summary', round: 2, updatedAt: Date.now() }, onClose: () => { open.value = false } }) }).directive('modal', modal).mount('#app')
  })
  const assert = (condition, message) => { if (!condition) throw new Error(message) }
  const dialog = page.getByRole('dialog', { name: '文档阅读', exact: true })
  const first = page.locator('.ad-mermaid').first()
  await first.locator('svg').waitFor()
  await page.locator('.ad-mermaid.is-error .ad-diagram-error').waitFor()
  assert(await page.locator('.ad-mermaid').nth(1).locator('svg').count() === 1, 'Sequence diagram did not render')
  const originalSource = await first.getAttribute('data-source')
  assert(await first.locator('.ad-diagram-viewport').evaluate(el => el.clientHeight <= 408 && el.scrollHeight > el.clientHeight), 'Long diagram is not bounded')
  assert(await first.locator('svg').evaluate(el => el.getBoundingClientRect().width > 200), 'Diagram labels are too small by default')
  await dialog.screenshot({ path: '.playwright-cli/reader-dark-overview.png' })
  await first.getByRole('button', { name: '放大图表', exact: true }).click()
  assert(await first.getByRole('button', { name: '按原始大小显示', exact: true }).textContent() === '115%', 'Zoom in did not work')
  await first.getByRole('button', { name: '完整显示图表', exact: true }).click()
  assert(await first.locator('svg').evaluate(el => el.getBoundingClientRect().height <= 361), 'Fit did not show the complete diagram')
  await first.getByRole('button', { name: '按原始大小显示', exact: true }).click()
  await first.getByRole('button', { name: '展开图表', exact: true }).click()
  assert(await first.locator('.ad-diagram-viewport').evaluate(el => el.clientHeight > 408), 'Expanded diagram is still constrained')
  await first.getByRole('button', { name: '展开图表', exact: true }).click()
  await first.getByRole('button', { name: '切换横向布局', exact: true }).click()
  await page.waitForFunction(() => !document.querySelector('.ad-mermaid').hasAttribute('aria-busy'))
  assert(await first.getAttribute('data-source') === originalSource, 'Layout toggle changed the document source')
  await dialog.screenshot({ path: '.playwright-cli/reader-dark-horizontal.png' })
  await first.getByRole('button', { name: '查看图表源码', exact: true }).click()
  assert(await first.locator('.ad-diagram-source').isVisible(), 'Source toggle did not open')
  await first.getByRole('button', { name: '查看图表源码', exact: true }).click()
  await dialog.getByRole('button', { name: '2. 逐项矩阵：概念内容到底在哪里生效', exact: true }).click()
  await page.waitForFunction(() => document.querySelector('.reading-scroll').scrollTop > 300)
  const table = page.locator('.ad-table-scroll').first()
  assert(await table.locator('table').evaluate(el => getComputedStyle(el).display === 'table'), 'Table lost native layout')
  assert(await table.evaluate(el => el.scrollWidth <= el.clientWidth + 1), 'Desktop table needs unnecessary horizontal scrolling')
  assert(await table.locator('td').first().evaluate(el => getComputedStyle(el).borderRightWidth === '0px'), 'Heavy vertical table rules remain')
  assert(await table.locator('td').first().locator('code').evaluate(el => el.getBoundingClientRect().height < 22), 'Field identifier was split across lines')
  assert(await page.locator('td[align="right"]').first().evaluate(el => getComputedStyle(el).textAlign === 'right'), 'Numeric alignment was overridden by reader styles')
  assert(await dialog.locator('.outline [aria-current="location"]').count() === 1, 'Current section is not highlighted')
  await dialog.screenshot({ path: '.playwright-cli/reader-dark-table.png' })
  const contentWidth = await dialog.locator('.md').evaluate(el => el.clientWidth)
  await dialog.getByRole('button', { name: '目录', exact: true }).click()
  assert(await dialog.locator('.md').evaluate(el => el.clientWidth) > contentWidth + 180, 'Collapsing the outline did not reclaim space')
  await dialog.getByRole('button', { name: '目录', exact: true }).click()
  await dialog.getByRole('button', { name: '紧凑', exact: true }).click()
  assert(await dialog.getAttribute('data-density') === 'comfortable', 'Density did not change')
  await dialog.getByRole('button', { name: '紧凑', exact: true }).click()
  await dialog.getByRole('button', { name: '全屏阅读', exact: true }).click()
  assert(await dialog.evaluate(el => el.clientWidth > innerWidth - 30), 'Maximized reader did not fill the screen')
  await dialog.getByRole('button', { name: '还原', exact: true }).click()

  for (const theme of ['light', 'dark']) {
    await page.evaluate(async theme => {
      const { store } = await import('/src/lib/store.ts')
      store.settings.uiTheme = theme
      document.documentElement.setAttribute('data-theme', theme)
    }, theme)
    await page.waitForFunction(theme => document.querySelector('.ad-mermaid').dataset.painted === theme, theme)
    await dialog.getByRole('tab', { name: 'Markdown 原文', exact: true }).click()
    assert((await dialog.locator('.source').textContent()) === await page.evaluate(() => window.__readerFixture.text), 'Source was rewritten')
    await dialog.getByRole('tab', { name: '阅读', exact: true }).click()
    await first.locator('svg').waitFor()
    if (theme === 'light') {
      await dialog.getByRole('button', { name: '2. 逐项矩阵：概念内容到底在哪里生效', exact: true }).click()
      await page.waitForFunction(() => document.querySelector('.reading-scroll').scrollTop > 300)
      await dialog.screenshot({ path: '.playwright-cli/reader-light-table.png' })
    }
    for (const [width, height] of [[1024, 640], [800, 520], [420, 700], [680, 360]]) {
      await page.setViewportSize({ width, height })
      assert(await dialog.evaluate(el => { const r = el.getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1 && el.scrollWidth <= el.clientWidth }), `Reader overflows at ${width}×${height}`)
      assert(await dialog.locator('.actions .btn-primary').evaluate(el => el.getBoundingClientRect().bottom <= innerHeight), 'Primary action was cropped')
      if (width === 420) {
        if (await dialog.locator('.outline').isVisible()) await dialog.getByRole('button', { name: '目录', exact: true }).click()
        await dialog.getByRole('button', { name: '目录', exact: true }).click()
        await dialog.getByRole('button', { name: '1. 一张总图', exact: true }).click()
        assert(!await dialog.locator('.outline').isVisible(), 'Narrow outline obscures the selected section')
        await dialog.locator('.reading-scroll').evaluate(el => { el.scrollTop = 0 })
        await page.screenshot({ path: `.playwright-cli/reader-${theme}-narrow.png` })
      }
    }
    await page.setViewportSize({ width: 1600, height: 1000 })
  }
  await page.evaluate(() => { window.__readerOpen.value = false; window.__readerFixture.failed = true })
  await dialog.waitFor({ state: 'hidden' })
  await page.evaluate(() => { window.__readerOpen.value = true })
  await dialog.getByText('文档暂时无法读取', { exact: true }).waitFor()
  await page.evaluate(() => { window.__readerFixture.failed = false; window.__readerFixture.empty = true })
  await dialog.getByRole('button', { name: '重试', exact: true }).click()
  await dialog.getByText('这份文档暂无内容。', { exact: true }).waitFor()
  await page.evaluate(() => { window.__readerOpen.value = false; window.__readerFixture.empty = false; window.__readerFixture.delay = 150 })
  await dialog.waitFor({ state: 'hidden' })
  await page.evaluate(() => { window.__readerOpen.value = true })
  await dialog.getByRole('status').waitFor()
  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'hidden' })
  if (errors.length) throw new Error(errors.join('\n'))
  return { ok: true, themes: ['dark', 'light'], viewportCases: 8, diagramControls: true, tableLayout: true, sourcePreserved: true, errorAndEmptyStates: true }
}
