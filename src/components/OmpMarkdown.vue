<script setup lang="ts">
import { computed } from 'vue'
import { renderOmpMarkdown } from '../lib/ompMarkdown'
import { showToast } from '../lib/store'
import * as api from '../lib/api'

const props = defineProps<{source:string}>()
const html = computed(() => renderOmpMarkdown(props.source))
async function onClick(event: MouseEvent) {
  const target = event.target instanceof Element ? event.target : null
  const button = target?.closest<HTMLButtonElement>('.omp-code-copy')
  if (button) {
    const text = button.closest('.omp-code-block')?.querySelector('code')?.textContent || ''
    try { await navigator.clipboard.writeText(text); showToast('代码已复制') }
    catch { showToast('复制失败，请选中代码后复制') }
  }
  const link = target?.closest<HTMLAnchorElement>('a[href]')
  if (link && /^https?:|^mailto:/i.test(link.getAttribute('href') || '')) {
    event.preventDefault()
    try { await api.openExternal(link.href) } catch { showToast('无法打开链接') }
  }
}
</script>

<template><div class="omp-markdown ad-markdown" @click="onClick" v-html="html" /></template>
