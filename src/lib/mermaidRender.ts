type MermaidTheme = 'light' | 'dark'

let mermaidMod: Promise<typeof import('mermaid')> | null = null
let appliedTheme = ''
let paintSeq = 0
const cleanups = new WeakMap<HTMLElement, () => void>()

function loadMermaid() {
  mermaidMod ??= import('mermaid')
  return mermaidMod
}

function dropOrphans(id: string) {
  document.getElementById(id)?.remove()
  document.getElementById(`d${id}`)?.remove()
}

async function ensureTheme(theme: MermaidTheme, fontFamily: string) {
  const { default: mermaid } = await loadMermaid()
  const key = `${theme}:${fontFamily}`
  if (appliedTheme === key) return mermaid
  const dark = theme === 'dark'
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    suppressErrorRendering: true,
    theme: 'base',
    fontFamily,
    look: 'classic',
    flowchart: {
      htmlLabels: false,
      useMaxWidth: false,
      curve: 'linear',
      nodeSpacing: 24,
      rankSpacing: 32,
      padding: 12,
      wrappingWidth: 200
    },
    sequence: { useMaxWidth: false, actorMargin: 32, messageMargin: 24 },
    themeVariables: {
      darkMode: dark,
      background: 'transparent',
      fontSize: '14px',
      primaryColor: dark ? '#203d55' : '#e8f3ff',
      primaryTextColor: dark ? '#e4edf5' : '#1f2937',
      primaryBorderColor: dark ? '#6daee8' : '#397bb7',
      lineColor: dark ? '#a4b6c8' : '#60758a',
      secondaryColor: dark ? '#203f39' : '#e8f5ef',
      secondaryTextColor: dark ? '#e4edf5' : '#1f2937',
      secondaryBorderColor: dark ? '#6cc7ae' : '#24866d',
      tertiaryColor: dark ? '#443a27' : '#fff4dc',
      tertiaryTextColor: dark ? '#e4edf5' : '#1f2937',
      tertiaryBorderColor: dark ? '#e4bc72' : '#a77923',
      nodeTextColor: dark ? '#e4edf5' : '#1f2937',
      edgeLabelBackground: dark ? '#191f26' : '#f5f7fa',
      clusterBkg: dark ? '#1c2834' : '#f1f6fc',
      clusterBorder: dark ? '#51677d' : '#b7c9dc',
      actorBkg: dark ? '#203d55' : '#e8f3ff',
      actorBorder: dark ? '#6daee8' : '#397bb7',
      actorTextColor: dark ? '#e4edf5' : '#1f2937',
      signalColor: dark ? '#a4b6c8' : '#60758a',
      signalTextColor: dark ? '#e4edf5' : '#1f2937'
    }
  })
  appliedTheme = key
  return mermaid
}

export function pageMermaidTheme(): MermaidTheme {
  return document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark'
}

export function disposeMermaidDiagrams(root: HTMLElement) {
  cleanups.get(root)?.()
  cleanups.delete(root)
}

export async function paintMermaidDiagrams(root: HTMLElement, theme: MermaidTheme) {
  disposeMermaidDiagrams(root)
  const nodes = [...root.querySelectorAll<HTMLElement>('.ad-mermaid[data-source]')]
  if (!nodes.length) return
  let disposed = false
  const disposers: (() => void)[] = []
  cleanups.set(root, () => {
    disposed = true
    disposers.forEach(dispose => dispose())
  })
  const fontFamily = getComputedStyle(root).fontFamily || 'sans-serif'
  const mermaid = await ensureTheme(theme, fontFamily)
  const batch = ++paintSeq
  for (const [index, node] of nodes.entries()) {
    if (disposed) return
    const source = decodeURIComponent(node.dataset.source || '')
    if (!source.trim()) continue
    const toolbar = document.createElement('div')
    toolbar.className = 'ad-diagram-toolbar'
    const label = document.createElement('span')
    label.className = 'ad-diagram-label'
    label.textContent = /^(?:flowchart|graph)\b/m.test(source) ? '流程图' : '图表'
    toolbar.append(label)
    const controls = document.createElement('div')
    controls.className = 'ad-diagram-controls'
    controls.setAttribute('role', 'group')
    controls.setAttribute('aria-label', '图表操作')
    toolbar.append(controls)
    const viewport = document.createElement('div')
    viewport.className = 'ad-diagram-viewport'
    viewport.tabIndex = 0
    viewport.setAttribute('role', 'region')
    viewport.setAttribute('aria-label', '图表，可滚动查看')
    const canvas = document.createElement('div')
    canvas.className = 'ad-diagram-canvas'
    viewport.append(canvas)
    const code = document.createElement('pre')
    code.className = 'ad-diagram-source'
    code.textContent = source
    code.hidden = true
    node.replaceChildren(toolbar, viewport, code)

    let zoom = 1
    let fitMode: 'width' | 'all' | null = 'width'
    let horizontal = false
    let renderSeq = 0
    let diagram: SVGSVGElement | null = null
    let width = 0
    let height = 0
    const button = (text: string, title: string, action: () => void) => {
      const el = document.createElement('button')
      el.type = 'button'
      el.textContent = text
      el.title = title
      el.setAttribute('aria-label', title)
      el.addEventListener('click', action)
      controls.append(el)
      return el
    }
    const updateZoom = () => {
      if (!diagram) return
      if (fitMode) {
        zoom = Math.min(1, Math.max(1, viewport.clientWidth - 24) / width)
        if (fitMode === 'all') zoom = Math.min(zoom, 360 / height)
        // Keep default labels readable; large graphs scroll inside their own viewport.
        else zoom = Math.max(0.75, zoom)
      }
      diagram.style.width = `${width * zoom}px`
      diagram.style.height = `${height * zoom}px`
      scale.textContent = `${Math.round(zoom * 100)}%`
      fit.setAttribute('aria-pressed', String(fitMode === 'all'))
      minus.disabled = zoom <= 0.2
      plus.disabled = zoom >= 3
    }
    const changeZoom = (delta: number) => {
      fitMode = null
      zoom = Math.max(0.2, Math.min(3, Math.round((zoom + delta) * 100) / 100))
      updateZoom()
    }
    const minus = button('−', '缩小图表', () => changeZoom(-0.15))
    const scale = button('100%', '按原始大小显示', () => { fitMode = null; zoom = 1; updateZoom() })
    const plus = button('+', '放大图表', () => changeZoom(0.15))
    const fit = button('适应', '完整显示图表', () => { fitMode = 'all'; updateZoom() })
    const direction = source.match(/^\s*(?:flowchart|graph)\s+(TD|TB|BT|LR|RL)\b/m)
    if (direction) {
      const layout = button('横向', '切换横向布局', () => {
        horizontal = !horizontal
        layout.textContent = horizontal ? '原方向' : '横向'
        layout.setAttribute('aria-pressed', String(horizontal))
        void render()
      })
      layout.setAttribute('aria-pressed', 'false')
    }
    const expand = button('展开', '展开图表', () => {
      const expanded = node.classList.toggle('is-expanded')
      expand.textContent = expanded ? '收起' : '展开'
      expand.setAttribute('aria-expanded', String(expanded))
    })
    expand.setAttribute('aria-expanded', 'false')
    const sourceButton = button('源码', '查看图表源码', () => {
      code.hidden = !code.hidden
      sourceButton.setAttribute('aria-expanded', String(!code.hidden))
    })
    sourceButton.setAttribute('aria-expanded', 'false')

    async function render() {
      const seq = ++renderSeq
      const id = `admd${batch}-${index}-${seq}`
      node.setAttribute('aria-busy', 'true')
      try {
        const input = horizontal
          ? source.replace(/^(\s*(?:flowchart|graph)\s+)(TD|TB|BT|LR|RL)\b/m, '$1LR')
          : source
        const { svg, bindFunctions } = await mermaid.render(id, input)
        if (disposed || seq !== renderSeq || !root.contains(node)) return
        if (!canvas.isConnected) viewport.replaceChildren(canvas)
        canvas.innerHTML = svg
        diagram = canvas.querySelector('svg')
        if (!diagram) throw new Error('图表没有生成可显示的内容')
        const box = diagram.viewBox.baseVal
        width = box.width || Number(diagram.getAttribute('width')) || 640
        height = box.height || Number(diagram.getAttribute('height')) || 360
        diagram.style.maxWidth = 'none'
        diagram.setAttribute('role', 'img')
        if (!diagram.hasAttribute('aria-labelledby')) diagram.setAttribute('aria-label', label.textContent || '图表')
        node.dataset.painted = theme
        node.classList.remove('is-error')
        updateZoom()
        bindFunctions?.(canvas)
      } catch (err) {
        dropOrphans(id)
        if (disposed || seq !== renderSeq) return
        node.classList.add('is-error')
        const message = document.createElement('p')
        message.className = 'ad-diagram-error'
        message.textContent = `图表未能渲染，可查看下方源码。${err instanceof Error ? err.message : ''}`
        viewport.replaceChildren(message)
        code.hidden = false
        sourceButton.setAttribute('aria-expanded', 'true')
      } finally {
        if (!disposed && seq === renderSeq) node.removeAttribute('aria-busy')
      }
    }
    const observer = new ResizeObserver(updateZoom)
    observer.observe(viewport)
    disposers.push(() => observer.disconnect())
    await render()
  }
}
