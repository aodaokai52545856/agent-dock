<script setup lang="ts">
import type { OmpToolView } from '../lib/ompPresentation'
import OmpCodeBlock from './OmpCodeBlock.vue'
defineProps<{tool:OmpToolView}>()
</script>

<template>
  <details class="omp-tool-card" :data-status="tool.status" open>
    <summary>
      <span class="omp-tool-icon" aria-hidden="true">{{ tool.command?'›_':tool.path?'≡':'◇' }}</span>
      <strong>{{ tool.name }}</strong>
      <span class="omp-tool-description" :title="tool.description||tool.path||tool.command">{{ tool.description||tool.path||tool.command||'工具调用' }}</span>
      <span class="omp-tool-status"><i aria-hidden="true" />{{ tool.label }}</span>
      <span class="omp-chevron" aria-hidden="true">›</span>
    </summary>
    <div class="omp-tool-body">
      <OmpCodeBlock v-if="tool.command" :source="tool.command" language="bash" label="命令" />
      <div v-if="tool.path" class="omp-tool-path"><span>文件</span><code>{{ tool.path }}</code></div>
      <details class="omp-tool-arguments"><summary>完整参数</summary><OmpCodeBlock :source="tool.arguments" language="json" label="JSON" limit /></details>
      <OmpCodeBlock v-if="tool.output" :source="tool.output" :language="tool.outputLanguage" label="输出" terminal limit />
      <img v-for="(im,i) in tool.images" :key="i" class="omp-image" :src="`data:${im.mimeType};base64,${im.data}`" alt="工具返回图片" />
      <div v-if="!tool.output&&!tool.images.length" class="omp-tool-placeholder">{{ tool.status==='running'?'正在等待工具输出…':tool.status==='error'?'工具执行失败，未返回输出':tool.status==='ended'?'此历史调用未记录执行结果':'执行完成，无文本输出' }}</div>
    </div>
  </details>
</template>
