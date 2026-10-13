type MermaidTheme = 'light' | 'dark'

let mermaidMod: Promise<typeof import('mermaid')> | null = null
let appliedTheme: MermaidTheme | null = null
let paintSeq = 0

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
  if (appliedTheme === theme) return mermaid
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    suppressErrorRendering: true,
    theme: theme === 'light' ? 'neutral' : 'dark',
    fontFamily,
    flowchart: {
      htmlLabels: false,
      useMaxWidth: true,
      curve: 'basis'
    },
    sequence: { useMaxWidth: true },
    themeVariables:
      theme === 'light'
        ? { background: 'transparent' }
        : {
            darkMode: true,
            background: 'transparent',
            primaryTextColor: '#ececec',
            primaryColor: '#3a4048',
            primaryBorderColor: '#8a8d90',
            lineColor: '#8a8d90',
            secondaryColor: '#2a2e33',
            tertiaryColor: '#1f2327'
          }
  })
  appliedTheme = theme
  return mermaid
}

export function pageMermaidTheme(): MermaidTheme {
  return document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark'
}

export async function paintMermaidDiagrams(root: HTMLElement, theme: MermaidTheme) {
  const nodes = [...root.querySelectorAll<HTMLElement>('.ad-mermaid[data-source]')]
  if (!nodes.length) return
  const fontFamily = getComputedStyle(root).fontFamily || 'sans-serif'
  const mermaid = await ensureTheme(theme, fontFamily)
  const batch = ++paintSeq
  for (const [index, node] of nodes.entries()) {
    if (node.dataset.painted === theme) continue
    const source = decodeURIComponent(node.dataset.source || '')
    if (!source.trim()) continue
    const id = `admd${batch}${index}`
    try {
      const { svg, bindFunctions } = await mermaid.render(id, source)
      if (batch !== paintSeq) return
      node.innerHTML = svg
      node.dataset.painted = theme
      node.classList.remove('is-error')
      bindFunctions?.(node)
    } catch (err) {
      dropOrphans(id)
      if (batch !== paintSeq) return
      node.classList.add('is-error')
      node.dataset.painted = theme
      const message = err instanceof Error ? err.message : '流程图无法渲染'
      node.textContent = `${message}\n\n${source}`
    }
  }
}
