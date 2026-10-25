// Run with playwright-cli run-code against the Vite browser preview on port 1421.
// Settings and project writes are mocked in the page; no desktop data is changed.
async page => {
  const errors = []
  const warnings = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', message => {
    if (message.type() === 'warning' && message.text().includes('[Vue warn]')) warnings.push(message.text())
  })
  await page.goto('http://127.0.0.1:1421')
  await page.locator('.start-btn').waitFor()
  await page.evaluate(async () => {
    const { store } = await import('/src/lib/store.ts')
    const state = window.__uiSmoke = { store, writes: [] }
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        state.writes.push(command)
        if (command === 'save_settings') return JSON.parse(JSON.stringify({ projects: store.projects, settings: args.settings }))
        if (command === 'add_project') return JSON.parse(JSON.stringify({ projects: [{ ...args.draft, id: 'smoke-project', createdAt: new Date().toISOString() }, ...store.projects], settings: store.settings }))
        throw new Error(`Unexpected preview command: ${command}`)
      }
    }
  })

  const assert = (condition, message) => { if (!condition) throw new Error(message) }
  const fits = async locator => locator.evaluate(el => {
    const r = el.getBoundingClientRect()
    return r.left >= -1 && r.top >= -1 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1 && el.scrollWidth <= el.clientWidth + 1
  })
  const focusInside = async dialog => dialog.evaluate(el => el.contains(document.activeElement))
  const screenshots = []
  async function capture(name) {
    const path = `.playwright-cli/ui-${name}.png`
    await page.screenshot({ path })
    screenshots.push(path)
  }

  for (const theme of ['light', 'dark']) {
    await page.evaluate(async theme => {
      const { store } = await import('/src/lib/store.ts')
      const { applyAppearance, appearanceFromSettings } = await import('/src/lib/appearance.ts')
      store.settings.uiTheme = theme
      applyAppearance(appearanceFromSettings(store.settings))
    }, theme)
    for (const [width, height] of [[1440, 900], [1024, 640], [800, 520], [680, 480]]) {
      await page.setViewportSize({ width, height })
      assert(await fits(page.locator('.titlebar')), `Title bar overflows at ${width}`)
      const footer = page.getByRole('contentinfo')
      assert(await fits(footer), `Status bar overflows at ${width}`)
      assert(await footer.getByRole('button', { name: '设置', exact: true }).evaluate(el => el.getBoundingClientRect().right <= innerWidth), 'Settings action was clipped')
      const head = page.locator('.session-head')
      assert(await head.getByText('会话', { exact: true }).evaluate(el => el.scrollWidth <= el.clientWidth), 'Session label is clipped')
      assert(await head.getByRole('button', { name: /筛选会话工具/ }).evaluate(el => el.clientWidth >= 60), 'Tool filter lost its label')
      if (width === 800) await capture(`${theme}-compact`)

      await footer.getByRole('button', { name: '设置', exact: true }).click()
      const settings = page.getByRole('dialog', { name: '设置', exact: true })
      await settings.waitFor()
      assert(await fits(settings), 'Settings drawer overflows')
      assert(await focusInside(settings), 'Settings did not receive focus')
      const save = settings.getByRole('button', { name: '保存设置', exact: true })
      assert(await save.evaluate(el => el.getBoundingClientRect().bottom <= innerHeight), 'Settings save is below the viewport')
      await settings.getByRole('button', { name: '终端与工具', exact: true }).click()
      assert(await settings.getByLabel('Claude Code 路径', { exact: true }).isVisible(), 'Tool paths are not discoverable')
      await settings.getByRole('button', { name: '自动化', exact: true }).click()
      assert(await settings.getByLabel('Cursor API Key', { exact: true }).isVisible(), 'Automation settings are missing')
      await settings.getByRole('button', { name: '外观', exact: true }).click()
      if (width === 1024) await capture(`${theme}-settings`)
      for (let i = 0; i < 26; i++) {
        await page.keyboard.press('Tab')
        assert(await focusInside(settings), 'Tab escaped the settings drawer')
      }
      await page.keyboard.press('Escape')
      await settings.waitFor({ state: 'hidden' })
      assert(await footer.getByRole('button', { name: '设置', exact: true }).evaluate(el => el === document.activeElement), 'Closing settings did not restore focus')

      await page.locator('.start-btn').click()
      const create = page.getByRole('dialog', { name: '新建会话' })
      assert(await fits(create), 'New session dialog overflows')
      assert(await create.getByRole('radio', { checked: true }).count() === 1, 'Last used tool is not preselected')
      await create.getByRole('button', { name: '选择项目' }).click()
      await page.keyboard.press('Escape')
      assert(await create.isVisible(), 'Escape closed the dialog instead of its project picker')
      assert(await create.getByRole('listbox').count() === 0, 'Project picker stayed open')
      await page.keyboard.press('Escape')
      await create.waitFor({ state: 'hidden' })

      await page.getByRole('button', { name: '版本', exact: true }).click()
      const versions = page.getByRole('dialog', { name: '版本', exact: true })
      assert(await fits(versions), 'Version dialog overflows')
      await page.keyboard.press('Escape')
      await versions.waitFor({ state: 'hidden' })

      for (const [entry, name] of [['Grok 账号', 'Grok 账号'], ['Claude 账号', 'Claude 账号'], ['DeepSeek', 'DeepSeek Key']]) {
        await page.getByRole('button', { name: '系统', exact: true }).click()
        await page.getByRole('menuitem', { name: entry, exact: true }).click()
        const account = page.getByRole('dialog', { name, exact: true })
        assert(await fits(account), `${name} overflows at ${width}`)
        await page.keyboard.press('Tab')
        assert(await focusInside(account), `${name} did not retain focus`)
        await page.keyboard.press('Escape')
        await account.waitFor({ state: 'hidden' })
      }
    }
  }

  // Maximum font size and the narrowest supported sidebar must keep controls reachable.
  for (const width of [1024, 800, 680]) {
    await page.setViewportSize({ width, height: 480 })
    await page.evaluate(async () => {
      const { store } = await import('/src/lib/store.ts')
      const { layout } = await import('/src/lib/layout.ts')
      const { applyAppearance, appearanceFromSettings } = await import('/src/lib/appearance.ts')
      layout.sidebarWidth = 240
      store.settings.uiFontSize = 18
      applyAppearance(appearanceFromSettings(store.settings))
    })
    assert(await fits(page.locator('.titlebar')), `Title bar overflows with large text at ${width}`)
    assert(await fits(page.getByRole('contentinfo')), 'Status bar overflows with large text')
    await page.getByRole('button', { name: /^时钟，/ }).click()
    const clock = page.getByRole('dialog', { name: '世界时钟', exact: true })
    assert(await fits(clock), 'World clock is cropped in a short window')
    await page.keyboard.press('Escape')
    await clock.waitFor({ state: 'hidden' })
  }
  await page.evaluate(async () => {
    const { store } = await import('/src/lib/store.ts')
    const { layout } = await import('/src/lib/layout.ts')
    const { applyAppearance, appearanceFromSettings } = await import('/src/lib/appearance.ts')
    layout.sidebarWidth = 280
    store.settings.uiFontSize = 13
    applyAppearance(appearanceFromSettings(store.settings))
  })

  // Search feedback and clearing recover the original list.
  const search = page.getByRole('searchbox', { name: '搜索会话' })
  await search.fill('this-session-does-not-exist')
  await page.getByText('未找到匹配的会话', { exact: true }).waitFor()
  await page.getByRole('button', { name: '清除搜索', exact: true }).click()
  assert(await search.inputValue() === '', 'Search did not clear')
  await page.getByRole('button', { name: '会话操作：修窗口圆角和毛玻璃', exact: true }).click()
  await page.getByRole('menuitem', { name: /^重命名/ }).waitFor()
  await page.keyboard.press('Escape')
  await page.getByRole('menuitem', { name: /^重命名/ }).waitFor({ state: 'hidden' })

  // Unsaved paths survive dismissing the warning, and save correctly through the mocked API.
  await page.getByRole('contentinfo').getByRole('button', { name: '设置', exact: true }).click()
  const settings = page.getByRole('dialog', { name: '设置', exact: true })
  await settings.getByRole('button', { name: '终端与工具', exact: true }).click()
  await settings.getByLabel('Claude Code 路径', { exact: true }).fill('C:\\Tools\\claude.exe')
  await page.keyboard.press('Escape')
  await settings.getByRole('alert').waitFor()
  await settings.getByRole('button', { name: '继续编辑', exact: true }).click()
  assert(await settings.getByLabel('Claude Code 路径', { exact: true }).inputValue() === 'C:\\Tools\\claude.exe', 'Unsaved path was lost')
  await settings.getByRole('button', { name: '保存设置', exact: true }).click()
  await settings.waitFor({ state: 'hidden' })
  assert(await page.evaluate(() => window.__uiSmoke.store.settings.claudePath) === 'C:\\Tools\\claude.exe', 'Settings were not saved')

  // Project validation is attached to the correct field; pasted paths work.
  await page.getByRole('button', { name: '添加', exact: true }).click()
  const project = page.getByRole('dialog', { name: '添加项目', exact: true })
  await project.getByRole('button', { name: '保存项目' }).click()
  await project.getByText('请填写项目名称', { exact: true }).waitFor()
  assert(await project.getByPlaceholder('显示在左侧的项目名').getAttribute('aria-invalid') === 'true', 'Name error is attached to the wrong field')
  await project.getByRole('textbox', { name: '项目文件夹', exact: true }).fill('D:\\Projects\\a-very-long-project-folder-for-layout-validation')
  await project.getByPlaceholder('显示在左侧的项目名').fill('布局验证项目')
  await page.keyboard.press('Enter')
  await project.waitFor({ state: 'hidden' })

  // Escape in another dialog must not clear canvas selection or delete a node.
  await page.getByRole('tab', { name: '编排', exact: true }).click()
  assert(!await page.getByRole('button', { name: '文本转换', exact: true }).isVisible(), 'Text conversion obscures the role palette')
  await page.locator('.chart .box').first().click()
  const selectedNode = await page.evaluate(() => window.__uiSmoke.store.bridgeSelectedNodeId)
  assert(selectedNode, 'A canvas node could not be selected')
  const nodeCount = await page.locator('.chart .box').count()
  await page.getByRole('contentinfo').getByRole('button', { name: '设置', exact: true }).click()
  const flowSettings = page.getByRole('dialog', { name: '设置', exact: true })
  await page.keyboard.press('Delete')
  assert(await page.locator('.chart .box').count() === nodeCount, 'Delete in a modal deleted a canvas node')
  await page.keyboard.press('Escape')
  await flowSettings.waitFor({ state: 'hidden' })
  assert(await page.evaluate(() => window.__uiSmoke.store.bridgeSelectedNodeId) === selectedNode, 'Escape in a modal cleared canvas selection')
  await page.locator('.inspector').getByRole('button', { name: '关闭', exact: true }).click()
  await capture('flow')
  await page.getByRole('tab', { name: '控制台', exact: true }).click()

  await page.evaluate(() => { window.__uiSmoke.store.activePtyId = ''; window.__uiSmoke.store.focusedSession = null })
  await page.setViewportSize({ width: 1024, height: 640 })
  await capture('empty-workspace')
  for (const size of [[800, 400], [680, 360]]) {
    await page.setViewportSize({ width: size[0], height: size[1] })
    await page.locator('.start-btn').click()
    const create = page.getByRole('dialog', { name: '新建会话' })
    assert(await fits(create), 'Dialog is cropped in a short window')
    await create.getByRole('button', { name: '取消' }).scrollIntoViewIfNeeded()
    await page.keyboard.press('Escape')
    await create.waitFor({ state: 'hidden' })
  }

  // Real confirmation components cover stacked dialogs and safe initial focus.
  await page.route('**/ui-modal-smoke', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1421/ui-modal-smoke')
  await page.evaluate(async () => {
    const source = await (await fetch('/src/components/ConfirmDialog.vue')).text()
    const vueUrl = source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    if (!vueUrl) throw new Error('Could not resolve Vue runtime')
    const { createApp, h, ref, withDirectives } = await import(vueUrl)
    const { modal } = await import('/src/lib/modal.ts')
    const { default: Confirm } = await import('/src/components/ConfirmDialog.vue')
    await import('/src/styles/global.css')
    const parent = ref(false), child = ref(false)
    createApp({ render: () => h('div', [
      h('button', { onClick: () => { parent.value = true } }, '打开测试弹窗'),
      parent.value ? h('div', { class: 'ad-mask' }, [withDirectives(h('div', { class: 'ad-dialog', role: 'dialog', 'aria-modal': 'true', 'aria-label': '父弹窗' }, [
        h('button', { onClick: () => { child.value = true } }, '打开确认'),
        h(Confirm, { open: child.value, title: '子确认', body: '测试确认', action: '删除', danger: true, onClose: () => { child.value = false } })
      ]), [[modal, () => { parent.value = false }]])]) : null
    ]) }).directive('modal', modal).mount('#app')
  })
  await page.getByRole('button', { name: '打开测试弹窗', exact: true }).click()
  await page.getByRole('button', { name: '打开确认', exact: true }).click()
  const child = page.getByRole('dialog', { name: '子确认', exact: true })
  assert(await child.getByRole('button', { name: '取消', exact: true }).evaluate(el => el === document.activeElement), 'Destructive dialog did not focus the safe action')
  for (const key of ['Tab', 'Tab', 'Shift+Tab', 'Shift+Tab']) {
    await page.keyboard.press(key)
    assert(await focusInside(child), 'Focus escaped the topmost confirmation')
  }
  await page.keyboard.press('Escape')
  await child.waitFor({ state: 'hidden' })
  assert(await page.getByRole('dialog', { name: '父弹窗', exact: true }).isVisible(), 'Escape closed both dialogs')
  assert(await page.getByRole('button', { name: '打开确认', exact: true }).evaluate(el => el === document.activeElement), 'Child dialog did not restore focus to its parent')
  await page.keyboard.press('Escape')
  await page.getByRole('dialog', { name: '父弹窗', exact: true }).waitFor({ state: 'hidden' })
  assert(await page.getByRole('button', { name: '打开测试弹窗', exact: true }).evaluate(el => el === document.activeElement), 'Parent dialog did not restore its opener')
  if (errors.length || warnings.length) throw new Error([...errors, ...warnings].join('\n'))
  return { ok: true, viewportCases: 13, themes: ['light', 'dark'], screenshots, nestedDialogs: true }
}
