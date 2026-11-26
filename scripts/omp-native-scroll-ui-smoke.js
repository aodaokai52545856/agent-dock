// OMP and Antigravity scrolling in real xterm with synthetic PTY traffic.
async page => {
  const errors = []
  page.on('pageerror', e => errors.push(e.message))
  await page.setViewportSize({ width: 1200, height: 760 })
  await page.route('**/omp-scroll-regression', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body style="margin:0"><div id="app" style="height:100vh;display:flex"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1423/omp-scroll-regression')
  await page.evaluate(async () => {
    const callbacks = new Map(), listeners = new Map()
    let serial = 0
    const smoke = window.__scrollRegression = { ptyId: 'early-omp', sizes: [], input: [], repaint: null }
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} }
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: fn => { callbacks.set(++serial, fn); return serial },
      unregisterCallback: id => callbacks.delete(id),
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') {
          const windowEvent = ['tauri://focus', 'tauri://blur', 'agent-dock-window-activation'].includes(args.event)
          if (!windowEvent || (args.target.kind === 'Window' && args.target.label === 'main')) {
            listeners.set(args.event, callbacks.get(args.handler))
          }
          return args.handler
        }
        if (command === 'plugin:event|unlisten') return
        if (command === 'pty_reset_output') return
        if (command === 'pty_write') { if (args.ptyId === smoke.ptyId) smoke.input.push(args.data); return }
        if (command === 'pty_resize') {
          if (args.ptyId !== smoke.ptyId) return
          smoke.sizes.push([args.cols, args.rows])
          if (smoke.repaint?.cols === args.cols) {
            const data = smoke.repaint.data; smoke.repaint = null; smoke.write(data)
          }
          return
        }
        throw new Error('Unexpected IPC ' + command)
      }
    }
    const source = await (await fetch('/src/components/TerminalPane.vue')).text()
    const { createApp, h, ref, nextTick } = await import(source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)[1])
    const { store } = await import(source.match(/from ["']([^"']*\/lib\/store\.ts[^"']*)["']/)[1])
    const { default: Pane } = await import('/src/components/TerminalPane.vue')
    await import('/src/styles/global.css')
    const pane = ref(null)
    createApp({ render: () => h('div', { style: { display: store.appMode === 'console' ? 'flex' : 'none', flex: '1', minWidth: '0', minHeight: '0' } }, [h(Pane, { ref: pane, fontSize: 14 })]) }).mount('#app')
    await nextTick()
    smoke.write = data => listeners.get('pty-data')({ payload: { ptyId: smoke.ptyId, data } })
    smoke.nativeFocus = focused => listeners.get(focused ? 'tauri://focus' : 'tauri://blur')?.({ payload: null })
    smoke.activateWindow = active => listeners.get('agent-dock-window-activation')?.({ payload: active })
    // The backend starts emitting before the pty_open response registers live metadata.
    smoke.write('EARLY_NATIVE_OUTPUT\r\n')
    smoke.host = pane.value.ensureHost('early-omp')
    store.live = [{ ptyId: 'early-omp', toolId: 'omp', kind: 'pty', projectId: 'fixture', key: 'fixture', alive: true }]
    store.activePtyId = 'early-omp'
    smoke.pane = pane.value; smoke.store = store
    smoke.screen = () => {
      const term = smoke.host.term, buffer = term.buffer.active
      return Array.from({ length: term.rows }, (_, i) => buffer.getLine(buffer.viewportY + i)?.translateToString(true) || '').join('\n')
    }
    await nextTick()
  })
  await page.waitForTimeout(700)
  if (!await page.evaluate(() => Boolean(window.__scrollRegression.host.recovery))) throw new Error('Early PTY output permanently skipped OMP recovery initialization')
  await page.evaluate(() => { window.__scrollRegression.write('idle row\r\n'.repeat(120) + 'IDLE_LAST_CONTENT\r\n') })
  await page.waitForTimeout(150)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    Object.defineProperty(document, 'hidden', { configurable: true, value: true })
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    s.write('\x1b[?1049hTEMPORARY_NATIVE_REPAINT\x1b[?1049l')
  })
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    delete document.hidden
    document.dispatchEvent(new Event('visibilitychange'))
    window.dispatchEvent(new Event('focus'))
  })
  await page.waitForTimeout(500)
  if (!await page.evaluate(() => window.__scrollRegression.host.follow.isFollowing() && window.__scrollRegression.screen().includes('IDLE_LAST_CONTENT'))) throw new Error('Idle buffer restore was mistaken for a user scroll and returned to the session beginning')
  // A TUI can displace the xterm viewport while writing, without a user scroll.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.write('history\r\n'.repeat(150))
  })
  await page.waitForTimeout(150)
  await page.evaluate(() => { const s = window.__scrollRegression; s.host.term.scrollToTop(); s.write('LIVE_STREAM_TAIL\r\n') })
  await page.waitForTimeout(150)
  if (!await page.evaluate(() => window.__scrollRegression.screen().includes('LIVE_STREAM_TAIL'))) throw new Error('OMP output stopped following the live viewport')
  // Hidden output must resume following after returning to the session.
  await page.evaluate(() => { window.__scrollRegression.store.activePtyId = '' })
  await page.waitForTimeout(60)
  await page.evaluate(() => { const s = window.__scrollRegression; s.host.term.scrollToTop(); s.write('BACKGROUND_OUTPUT_TAIL\r\n') })
  await page.waitForTimeout(60)
  await page.evaluate(() => { window.__scrollRegression.store.activePtyId = 'early-omp' })
  await page.waitForTimeout(700)
  if (!await page.evaluate(() => window.__scrollRegression.screen().includes('BACKGROUND_OUTPUT_TAIL'))) throw new Error('Returning to OMP kept a stale viewport')
  // Switching Dock tabs keeps activePtyId unchanged and hides the whole pane.
  await page.evaluate(() => { window.__scrollRegression.store.appMode = 'usage' })
  await page.waitForTimeout(60)
  await page.evaluate(() => { const s = window.__scrollRegression; s.host.term.scrollToTop(); s.write('DOCK_TAB_BACKGROUND_TAIL\r\n') })
  await page.waitForTimeout(60)
  await page.evaluate(() => { window.__scrollRegression.store.appMode = 'console' })
  await page.waitForTimeout(700)
  if (!await page.evaluate(() => window.__scrollRegression.screen().includes('DOCK_TAB_BACKGROUND_TAIL'))) throw new Error('Returning from another Dock tab kept a stale viewport')
  // Deliberately reading history must remain stable during subsequent output.
  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(100)
  await page.evaluate(() => { const s = window.__scrollRegression; s.sizes.length = 0; s.write('WHILE_READING_HISTORY\r\n') })
  await page.waitForTimeout(500)
  if (!await page.evaluate(() => { const b = window.__scrollRegression.host.term.buffer.active; return b.viewportY < b.baseY })) throw new Error('Following output interrupted manual history reading')
  if (await page.evaluate(() => window.__scrollRegression.sizes.length)) throw new Error('Recovery interrupted manual history reading')
  // A short answer and periodic cursor controls used to prevent recovery forever.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.repaint = { cols: s.host.term.cols, data: '\r\nSHORT_ANSWER_RECOVERED\r\nDRAFT_PRESERVED' }
    s.noise = setInterval(() => s.write('\x1b[?25h'), 50)
  })
  await page.mouse.wheel(0, 10000)
  await page.waitForFunction(() => window.__scrollRegression.screen().includes('SHORT_ANSWER_RECOVERED'), undefined, { timeout: 4000 })
  await page.evaluate(() => clearInterval(window.__scrollRegression.noise))
  await page.waitForTimeout(900)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== 2) throw new Error('Bottom recovery looped or changed more than one size round trip')
  if (await page.evaluate(() => window.__scrollRegression.input.length)) throw new Error('Scroll recovery sent terminal input')
  // A free-spinning wheel keeps sending events well beyond the cooldown.
  // Neither repeated bottom events nor terminal housekeeping may repaint again.
  const repairedSizes = await page.evaluate(() => {
    const s = window.__scrollRegression
    s.wheelNoise = setInterval(() => s.write('\x1b[?25h\x1b[0m\x1b]0;OMP\x07'), 40)
    return s.sizes.length
  })
  for (let i = 0; i < 100; i++) {
    await page.mouse.wheel(0, 600)
    await page.waitForTimeout(20)
  }
  await page.evaluate(() => clearInterval(window.__scrollRegression.wheelNoise))
  await page.waitForTimeout(400)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== repairedSizes) throw new Error('Free-spinning at the bottom repeatedly repainted an unchanged answer')
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(100)
  await page.mouse.wheel(0, 10000)
  await page.waitForTimeout(1700)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== repairedSizes) throw new Error('Returning to an already recovered answer repainted it again')
  // A genuinely new answer is still eligible for one recovery on return.
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.write('NEW_SHORT_ANSWER_PREFIX\r\n')
    s.repaint = { cols: s.host.term.cols, data: '\r\nNEW_SHORT_ANSWER_RECOVERED\r\n' }
  })
  await page.mouse.wheel(0, 10000)
  await page.waitForFunction(() => window.__scrollRegression.screen().includes('NEW_SHORT_ANSWER_RECOVERED'))
  await page.waitForTimeout(400)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== repairedSizes + 2) throw new Error('New answer did not recover exactly once')
  // Inactive WebViews can deliver a delayed native buffer restore on return.
  // Buffer activation clears selection even when the user selected nothing.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.write('IDLE_HISTORY_ROW\r\n'.repeat(120) + 'IDLE_LAST_CONTENT\r\n')
  })
  await page.waitForTimeout(150)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    Object.defineProperty(document, 'hidden', { configurable: true, value: true })
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    s.write('\x1b[?1049hTEMPORARY_NATIVE_REPAINT\x1b[?1049l')
  })
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    delete document.hidden
    document.dispatchEvent(new Event('visibilitychange'))
    window.dispatchEvent(new Event('focus'))
  })
  await page.waitForTimeout(500)
  const idle = await page.evaluate(() => {
    const s = window.__scrollRegression
    return { following: s.host.follow.isFollowing(), tailVisible: s.screen().includes('IDLE_LAST_CONTENT') }
  })
  if (!idle.following || !idle.tailVisible) throw new Error('Idle buffer restore was mistaken for a user scroll and returned to the session beginning')
  const bar = await page.evaluate(() => {
    const s = window.__scrollRegression, viewport = s.host.el.querySelector('.xterm-viewport')
    const rect = viewport.getBoundingClientRect()
    return { width: viewport.offsetWidth - viewport.clientWidth, scrollable: viewport.scrollHeight > viewport.clientHeight,
      style: getComputedStyle(viewport).scrollbarWidth, x: rect.right - 3, y: rect.top + 20 }
  })
  if (bar.style === 'none' || bar.width <= 0 || !bar.scrollable) throw new Error('The native terminal scrollbar is still hidden')
  // Use the real browser scrollbar, rather than directly mutating follow state.
  await page.mouse.move(bar.x, bar.y)
  await page.mouse.down()
  await page.waitForTimeout(120)
  await page.mouse.up()
  await page.waitForTimeout(150)
  const savedLine = await page.evaluate(() => {
    const s = window.__scrollRegression, buffer = s.host.term.buffer.active
    if (s.host.follow.isFollowing() || buffer.viewportY >= buffer.baseY) throw new Error('Scrollbar interaction did not pause live following')
    return buffer.viewportY
  })
  await page.evaluate(() => {
    const s = window.__scrollRegression
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    s.host.term.clearSelection()
    window.dispatchEvent(new Event('focus'))
  })
  await page.waitForTimeout(300)
  if (await page.evaluate(() => window.__scrollRegression.host.term.buffer.active.viewportY) !== savedLine) throw new Error('Returning to the window lost the saved history position')
  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, 10000)
  await page.waitForTimeout(150)
  if (!await page.evaluate(() => window.__scrollRegression.host.follow.isFollowing())) throw new Error('Returning from scrollbar history to the bottom did not resume following')

  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.ptyId = 'antigravity'
    s.sizes.length = 0
    s.input.length = 0
    s.store.live.push({ ptyId: s.ptyId, toolId: 'antigravity', kind: 'pty', projectId: 'fixture', key: 'agy', alive: true })
    s.store.activePtyId = s.ptyId
    s.host = s.pane.ensureHost(s.ptyId)
    const history = Array.from({ length: 150 }, (_, i) => `ANTIGRAVITY_HISTORY_${i}\r\n`).join('')
    s.write(history + 'ANTIGRAVITY_LATEST\r\nDRAFT_PROMPT')
  })
  await page.waitForFunction(() => {
    const s = window.__scrollRegression
    return s.sizes.length > 0 && s.screen().includes('DRAFT_PROMPT')
  })
  await page.evaluate(() => { window.__scrollRegression.sizes.length = 0 })

  async function hideAntigravity(surface) {
    await page.evaluate(surface => {
      const s = window.__scrollRegression
      if (surface === 'window' || surface === 'desktop') window.dispatchEvent(new Event('blur'))
      else if (surface === 'visibility' || surface === 'minimized') {
        s.nativeFocus(false)
        Object.defineProperty(document, 'hidden', { configurable: true, value: true })
        document.dispatchEvent(new Event('visibilitychange'))
      } else if (surface === 'tab') s.store.appMode = 'usage'
      else s.store.activePtyId = 'early-omp'
      if (surface === 'desktop') s.nativeFocus(false)
    }, surface)
    await page.evaluate(async () => {
      const s = window.__scrollRegression
      await new Promise(resolve => {
        const parsed = s.host.term.onWriteParsed(() => { parsed.dispose(); resolve() })
        s.write('\x1b[?1049hANTIGRAVITY_REPAINT\x1b[?1049l')
      })
      // Apply the displaced viewport after xterm finishes activating and
      // laying out the normal buffer. Buffer activation itself can scroll it.
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      s.host.term.scrollToTop()
    })
    await page.waitForFunction(() => {
      const buffer = window.__scrollRegression.host.term.buffer.active
      return buffer.type === 'normal' && buffer.viewportY === 0
    }, undefined, { timeout: 1500 }).catch(() => { throw new Error(`Could not park the hidden Antigravity viewport at the top for ${surface}`) })
  }

  async function showAntigravity(surface) {
    await page.evaluate(surface => {
      const s = window.__scrollRegression
      if (surface === 'window') window.dispatchEvent(new Event('focus'))
      // WebView2 can reactivate the parent window before the page gets input focus.
      else if (surface === 'desktop') s.nativeFocus(true)
      else if (surface === 'visibility' || surface === 'minimized') {
        if (surface === 'minimized') s.nativeFocus(true)
        delete document.hidden
        document.dispatchEvent(new Event('visibilitychange'))
      } else if (surface === 'tab') s.store.appMode = 'console'
      else s.store.activePtyId = s.ptyId
    }, surface)
  }

  const surfaces = ['window', 'desktop', 'visibility', 'minimized', 'tab', 'session']
  for (const surface of surfaces) {
    await hideAntigravity(surface)
    await showAntigravity(surface)
    await page.waitForFunction(() => {
      const s = window.__scrollRegression
      const buffer = s.host.term.buffer.active
      const viewport = s.host.el.querySelector('.xterm-viewport')
      return buffer.viewportY === buffer.baseY && viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop <= 1
        && s.host.el.querySelector('.xterm-rows').textContent.includes('DRAFT_PROMPT')
    }, undefined, { timeout: 1500 }).catch(() => { throw new Error(`Antigravity lost the latest output when returning to the ${surface}`) })
  }

  // A wheel callback queued before blur can run for the first time AFTER focus.
  // The last gesture was down at the bottom; a transient row 0 is not history intent.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    const viewport = s.host.el.querySelector('.xterm-viewport')
    viewport.dispatchEvent(new WheelEvent('wheel', { bubbles: true, deltaY: 1 }))
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    viewport.scrollTop = 0
    window.dispatchEvent(new Event('focus'))
  })
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  await page.waitForFunction(() => {
    const s = window.__scrollRegression
    const buffer = s.host.term.buffer.active
    return s.host.follow.isFollowing() && buffer.viewportY === buffer.baseY && s.host.el.querySelector('.xterm-rows').textContent.includes('DRAFT_PROMPT')
  }, undefined, { timeout: 1500 }).catch(() => { throw new Error('A queued wheel callback pinned Antigravity at the top after refocus') })

  // A minimized window can gain native focus before its terminal is measurable.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.nativeFocus(false)
    s.host.el.closest('.workspace').style.display = 'none'
    s.host.term.scrollToTop()
    s.nativeFocus(true)
  })
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  await page.evaluate(() => { window.__scrollRegression.host.el.closest('.workspace').style.removeProperty('display') })
  await page.waitForFunction(() => {
    const s = window.__scrollRegression
    return s.host.el.querySelector('.xterm-rows').textContent.includes('DRAFT_PROMPT')
  }, undefined, { timeout: 1500 }).catch(() => { throw new Error('An idle Antigravity session lost its position when layout returned after focus') })

  // A WebView can restore the scrollbar without a scroll event reaching xterm.
  // Its model then already reports the bottom, so scrollToBottom() is a no-op.
  await page.evaluate(async () => {
    const s = window.__scrollRegression
    s.nativeFocus(false)
    const viewport = s.host.el.querySelector('.xterm-viewport')
    const ignoreHiddenScroll = event => event.stopImmediatePropagation()
    viewport.addEventListener('scroll', ignoreHiddenScroll, true)
    viewport.scrollTop = 0
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    viewport.removeEventListener('scroll', ignoreHiddenScroll, true)
    const buffer = s.host.term.buffer.active
    if (viewport.scrollTop !== 0 || buffer.viewportY !== buffer.baseY) throw new Error('Failed to simulate a stale WebView scrollbar')
    s.nativeFocus(true)
  })
  await page.waitForFunction(() => {
    const viewport = window.__scrollRegression.host.el.querySelector('.xterm-viewport')
    return viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop <= 1
  }, undefined, { timeout: 1500 }).catch(() => { throw new Error('The WebView scrollbar stayed at the top while xterm reported the bottom') })

  await page.evaluate(async () => {
    const s = window.__scrollRegression, term = s.host.term
    term.select(0, term.buffer.active.baseY, 5)
    s.selectedText = term.getSelection()
    s.nativeFocus(false)
    term.scrollToTop()
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    s.nativeFocus(true)
  })
  await page.waitForFunction(() => {
    const s = window.__scrollRegression
    return s.selectedText && s.host.term.getSelection() === s.selectedText && s.host.term.buffer.active.viewportY === s.host.term.buffer.active.baseY
  }, undefined, { timeout: 1500 }).catch(() => { throw new Error('A text selection required a click to restore the terminal viewport') })
  await page.evaluate(() => window.__scrollRegression.host.term.clearSelection())

  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, -480)
  await page.waitForFunction(() => {
    const buffer = window.__scrollRegression.host.term.buffer.active
    return buffer.viewportY > 0 && buffer.viewportY < buffer.baseY
  })
  const historyLine = await page.evaluate(() => window.__scrollRegression.screen().split('\n')[0])
  for (const surface of surfaces) {
    await hideAntigravity(surface)
    await showAntigravity(surface)
    await page.waitForFunction(line => window.__scrollRegression.host.el.querySelector('.xterm-rows > div').textContent.trimEnd() === line, historyLine, { timeout: 1500 })
      .catch(() => { throw new Error(`Antigravity lost the history position when returning to the ${surface}`) })
  }

  if (await page.evaluate(() => { const s = window.__scrollRegression; return s.input.length || s.sizes.length })) {
    throw new Error('Antigravity scroll restoration sent terminal input or resized the PTY')
  }

  await page.evaluate(() => window.__scrollRegression.write('\x1b[?1049hANTIGRAVITY_ALT_VIEW'))
  await page.waitForFunction(() => window.__scrollRegression.host.term.buffer.active.type === 'alternate')
  await page.mouse.wheel(0, -480)
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  await page.evaluate(() => window.__scrollRegression.write('\x1b[?1049l'))
  await page.waitForFunction(() => window.__scrollRegression.host.term.buffer.active.type === 'normal')
  await hideAntigravity('window')
  await showAntigravity('window')
  await page.waitForFunction(line => window.__scrollRegression.screen().startsWith(line + '\n'), historyLine, { timeout: 1500 })
    .catch(() => { throw new Error('Antigravity alternate-screen scrolling replaced the saved history position') })

  // On Windows the HWND returns to the foreground, but WebView2 can report
  // only a late blur. The page remains visible throughout an application switch.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.activateWindow(true)
    s.nativeFocus(false)
  })
  await page.mouse.wheel(0, -200)
  await page.waitForFunction(line => {
    const s = window.__scrollRegression
    return !s.host.follow.isSuspended() && s.host.el.querySelector('.xterm-rows > div').textContent.trimEnd() !== line
  }, historyLine, { timeout: 1500 })
  const parked = await page.evaluate(() => {
    const el = window.__scrollRegression.host.el
    return { line: el.querySelector('.xterm-rows > div').textContent.trimEnd(), top: el.querySelector('.xterm-viewport').scrollTop }
  })
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.activateWindow(false)
    s.nativeFocus(false)
    s.host.el.querySelector('.xterm-viewport').scrollTop = 0
  })
  await page.waitForFunction(() => window.__scrollRegression.host.term.buffer.active.viewportY === 0)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.activateWindow(true)
    window.dispatchEvent(new Event('blur'))
    s.nativeFocus(false)
  })
  await page.waitForFunction(expected => {
    const s = window.__scrollRegression, el = s.host.el
    return !s.host.follow.isSuspended() && el.querySelector('.xterm-rows > div').textContent.trimEnd() === expected.line
      && Math.abs(el.querySelector('.xterm-viewport').scrollTop - expected.top) < 16
  }, parked, { timeout: 1500 }).catch(() => { throw new Error('Switching desktop applications lost the parked history line after a late child blur') })

  if (errors.length) throw new Error(errors.join('\n'))
  return { ok: true, cases: [
    'output-before-metadata', 'follow-streaming-output', 'resume-hidden-output', 'resume-dock-tab-output',
    'preserve-manual-history', 'short-answer-bottom-recovery', 'periodic-control-output', 'no-repaint-loop',
    'free-spinning-wheel-no-repaint', 'deduplicate-answer-recovery', 'recover-new-short-answer-once',
    'idle-buffer-restore', 'visible-scrollbar', 'scrollbar-pauses-follow', 'restore-idle-history-marker',
    'antigravity-restore-live-output', 'antigravity-restore-history', 'antigravity-native-window-focus',
    'antigravity-cancel-stale-gesture', 'antigravity-resume-after-layout', 'antigravity-restore-scrollbar',
    'antigravity-preserve-selection', 'antigravity-restore-without-repaint', 'antigravity-alternate-screen-preserves-history',
    'antigravity-top-level-window-activation'
  ] }
}
