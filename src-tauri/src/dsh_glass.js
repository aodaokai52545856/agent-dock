(function () {
if (location.hostname !== '127.0.0.1') {
  return
}

var STORE = 'ad-dsh-glass'
var STYLE_ID = 'ad-dsh-glass'
var applying = false
var VAR_NAMES = [
  '--dsw-alias-bg-base',
  '--dsw-alias-bg-layer-1',
  '--dsw-alias-bg-layer-2',
  '--dsw-alias-bg-layer-3',
  '--dsw-alias-bg-module-platform',
  '--dsw-alias-bg-overlay',
  '--dsw-specific-sidebar-fill',
  '--dsw-specific-input-major',
  '--dsw-specific-bubble',
  '--dsw-specific-bubble-highlight',
  '--dsw-alias-label-primary',
  '--dsh-boot-bg'
]

function parseHash() {
  var raw = String(location.hash || '').replace(/^#/, '')
  if (!raw) return null
  var q = new URLSearchParams(raw)
  if (q.get('ad') !== '1') return null
  var bg = String(q.get('bg') || '').replace('#', '')
  var fg = String(q.get('fg') || '').replace('#', '')
  if (bg.length !== 6) return null
  var veil = Number(q.get('v'))
  if (!Number.isFinite(veil)) veil = 0.9
  var frost = Number(q.get('f'))
  if (!Number.isFinite(frost)) frost = 65
  return {
    bg: '#' + bg.toUpperCase(),
    fg: fg.length === 6 ? '#' + fg.toUpperCase() : '',
    veil: Math.max(0.12, Math.min(0.9, veil)),
    frost: Math.max(0, Math.min(100, Math.round(frost))),
    scheme: q.get('s') === 'l' ? 'light' : 'dark'
  }
}

function readStored() {
  try {
    var raw = sessionStorage.getItem(STORE)
    if (!raw) return null
    var value = JSON.parse(raw)
    if (!value || typeof value.bg !== 'string') return null
    return value
  } catch (err) {
    return null
  }
}

function fallbackTheme() {
  var light = false
  try {
    light = window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches
  } catch (err) {
    light = false
  }
  return light
    ? { bg: '#F3F3F3', fg: '#171717', veil: 0.9, frost: 100, scheme: 'light' }
    : { bg: '#0B0F13', fg: '#ECECEC', veil: 0.9, frost: 100, scheme: 'dark' }
}

function save(theme) {
  try {
    sessionStorage.setItem(STORE, JSON.stringify({
      bg: theme.bg,
      fg: theme.fg,
      veil: theme.veil,
      frost: theme.frost,
      scheme: theme.scheme
    }))
  } catch (err) {
    /* ignore quota / private mode */
  }
}

function hexRgb(hex) {
  var raw = String(hex || '').replace('#', '')
  if (raw.length !== 6) return '11, 15, 19'
  var r = parseInt(raw.slice(0, 2), 16)
  var g = parseInt(raw.slice(2, 4), 16)
  var b = parseInt(raw.slice(4, 6), 16)
  if ([r, g, b].some(function (n) { return !Number.isFinite(n) })) return '11, 15, 19'
  return r + ', ' + g + ', ' + b
}

function rgba(rgb, alpha) {
  return 'rgba(' + rgb + ', ' + alpha + ')'
}

function tokenVars(theme) {
  if (theme && theme.vars) return theme.vars
  var rgb = hexRgb(theme.bg)
  var veil = theme.veil
  var lift = Math.min(0.96, veil + 0.08)
  var panel = Math.min(0.97, veil + 0.14)
  var ink = theme.fg || (theme.scheme === 'light' ? '#171717' : '#ECECEC')
  return {
    '--dsw-alias-bg-base': 'transparent',
    '--dsw-alias-bg-layer-1': 'transparent',
    '--dsw-alias-bg-layer-2': rgba(rgb, lift),
    '--dsw-alias-bg-layer-3': rgba(rgb, panel),
    '--dsw-alias-bg-module-platform': 'transparent',
    '--dsw-alias-bg-overlay': rgba(rgb, panel),
    '--dsw-specific-sidebar-fill': 'transparent',
    '--dsw-specific-input-major': rgba(rgb, panel),
    '--dsw-specific-bubble': rgba(rgb, panel),
    '--dsw-specific-bubble-highlight': rgba(rgb, Math.min(0.98, panel + 0.04)),
    '--dsw-alias-label-primary': ink,
    '--dsh-boot-bg': 'transparent'
  }
}

function cssText(theme) {
  if (theme && theme.css) return theme.css
  var vars = tokenVars(theme)
  var frost = Number(((theme.frost || 65) * 0.42).toFixed(1))
  var saturate = (1 + (theme.frost || 65) * 0.003).toFixed(3)
  var tokens = VAR_NAMES.map(function (name) {
    return '  ' + name + ': ' + vars[name] + ' !important;'
  }).join('\n')
  var canvas = rgba(hexRgb(theme.bg), theme.veil)
  return [
    'html, body, #app, #__next, [data-ds-root] {',
    '  background: transparent !important;',
    '  background-color: transparent !important;',
    '}',
    '#root, [data-ad-dsh-glass] {',
    '  background: ' + canvas + ' !important;',
    '  background-color: ' + canvas + ' !important;',
    '  backdrop-filter: blur(' + frost + 'px) saturate(' + saturate + ');',
    '  -webkit-backdrop-filter: blur(' + frost + 'px) saturate(' + saturate + ');',
    '}',
    ':root, html, body, body[data-ds-dark-theme] {',
    '  color-scheme: ' + theme.scheme + ' !important;',
    tokens,
    '}'
  ].join('\n')
}

function stamp(el, vars) {
  if (!el || !el.style) return
  for (var i = 0; i < VAR_NAMES.length; i++) {
    var name = VAR_NAMES[i]
    if (vars[name]) el.style.setProperty(name, vars[name], 'important')
  }
  el.style.setProperty('background', 'transparent', 'important')
  el.style.setProperty('background-color', 'transparent', 'important')
}

function markBody(theme) {
  if (!document.body) return
  if (theme.scheme === 'dark') document.body.setAttribute('data-ds-dark-theme', '')
  else document.body.removeAttribute('data-ds-dark-theme')
}

function apply(theme) {
  if (!theme || !theme.bg) return
  applying = true
  save(theme)
  var vars = tokenVars(theme)
  var style = document.getElementById(STYLE_ID)
  if (!style) {
    style = document.createElement('style')
    style.id = STYLE_ID
    ;(document.head || document.documentElement).appendChild(style)
  }
  style.textContent = cssText(theme)
  document.documentElement.style.colorScheme = theme.scheme
  document.documentElement.setAttribute('data-ad-dsh-glass', theme.scheme)
  stamp(document.documentElement, vars)
  markBody(theme)
  if (document.body) stamp(document.body, vars)
  else {
    document.addEventListener('DOMContentLoaded', function () {
      markBody(theme)
      if (document.body) stamp(document.body, vars)
    })
  }
  function release() {
    applying = false
  }
  if (typeof queueMicrotask === 'function') queueMicrotask(release)
  else setTimeout(release, 0)
}

function currentTheme() {
  return parseHash() || readStored() || fallbackTheme()
}

window.__adDshGlass = apply
apply(currentTheme())
window.addEventListener('hashchange', function () {
  apply(parseHash() || readStored() || fallbackTheme())
})
window.addEventListener('message', function (ev) {
  if (!ev || !ev.data || ev.data.type !== 'ad-dsh-glass') return
  apply(ev.data.theme)
})

var observer = new MutationObserver(function () {
  if (applying) return
  apply(currentTheme())
})
observer.observe(document.documentElement, {
  attributes: true,
  attributeFilter: ['style', 'class', 'data-ds-dark-theme', 'data-ad-dsh-glass']
})
function watchBody() {
  if (!document.body) return
  observer.observe(document.body, {
    attributes: true,
    attributeFilter: ['style', 'class', 'data-ds-dark-theme']
  })
}
if (document.body) watchBody()
else document.addEventListener('DOMContentLoaded', watchBody)

var ticks = 0
var timer = setInterval(function () {
  ticks += 1
  apply(currentTheme())
  if (ticks >= 20) clearInterval(timer)
}, 250)
})();

