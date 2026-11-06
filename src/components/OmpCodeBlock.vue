<script setup lang="ts">
import { computed, ref } from 'vue'
import { highlightOmpCode, renderOmpTerminal } from '../lib/ompMarkdown'
import { showToast } from '../lib/store'
const props = withDefaults(defineProps<{source:string;language?:string;label?:string;terminal?:boolean;limit?:boolean}>(), {language:'plaintext'})
const expanded = ref(false), wrap = ref(false)
const lines = computed(() => props.source.split('\n').length)
const html = computed(() => props.terminal && (props.language==='plaintext'||props.source.includes('\x1b')) ? renderOmpTerminal(props.source) : highlightOmpCode(props.source, props.language))
async function copy() {
  try { await navigator.clipboard.writeText(props.source); showToast('内容已复制') }
  catch { showToast('复制失败，请选中内容后复制') }
}
</script>

<template>
  <section class="omp-code-block" :class="{'is-limited':limit&&!expanded,'is-wrapped':wrap}">
    <div class="omp-code-toolbar"><span>{{ label || language }}<small v-if="lines>1">{{ lines }} 行</small></span><div><button type="button" :aria-pressed="wrap" @click="wrap=!wrap">换行</button><button v-if="limit&&lines>12" type="button" :aria-expanded="expanded" @click="expanded=!expanded">{{ expanded?'收起':'展开' }}</button><button type="button" :aria-label="'复制'+(label||'代码')" @click="copy">复制</button></div></div>
    <pre tabindex="0"><code class="hljs" v-html="html" /></pre>
  </section>
</template>
