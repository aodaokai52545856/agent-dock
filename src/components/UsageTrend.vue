<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId, watch } from 'vue'
import { formatCacheHit, formatExactTokens, formatSpendCost, formatTokenCount } from '../lib/grokSpend'
import { completeUsagePoints, pointMetric, USAGE_METRIC_LABELS, type UsageMetric } from '../lib/usage'
import type { UsagePoint } from '../lib/types'

const props = defineProps<{ points: UsagePoint[]; metric: UsageMetric; start: number; end: number; loading: boolean }>()
const gradient = useId()
const active = ref(-1)
const box = ref<HTMLElement>()
const width = ref(1000)
const W = computed(() => Math.max(300, width.value))
const H = 184, left = 64, right = 20, top = 12, bottom = 150
let observer: ResizeObserver | undefined
onMounted(() => {
  observer = new ResizeObserver(([entry]) => { width.value = entry.contentRect.width })
  if (box.value) observer.observe(box.value)
})
onUnmounted(() => observer?.disconnect())
watch(() => [props.metric, props.points], () => { active.value = -1 })
const points = computed(() => completeUsagePoints(props.points, props.start, props.end))
const hasData = computed(() => props.points.some(row => row.requests > 0))
const values = computed(() => points.value.map(row => pointMetric(row, props.metric)))
const max = computed(() => props.metric === 'cache' ? 100 : Math.max(1, ...values.value) * 1.08)
const x = (index: number) => left + (W.value - left - right) * (points.value.length === 1 ? 0.5 : index / Math.max(1, points.value.length - 1))
const y = (value: number) => bottom - (bottom - top) * value / max.value
const line = computed(() => values.value.map((value, index) => `${index ? 'L' : 'M'}${x(index)},${y(value)}`).join(' '))
const area = computed(() => line.value ? `${line.value} L${x(values.value.length - 1)},${bottom} L${x(0)},${bottom} Z` : '')
const ticks = computed(() => Array.from({ length: Math.min(7, Math.max(2, Math.floor(W.value / 140)), points.value.length) }, (_, index) => {
  const count = Math.min(7, Math.max(2, Math.floor(W.value / 140)), points.value.length)
  const i = count === 1 ? 0 : Math.round(index * (points.value.length - 1) / (count - 1))
  return { x: x(i), label: points.value[i].label }
}))
const hit = computed(() => points.value[active.value])
const exact = (value: number) => props.metric === 'cost' ? formatSpendCost(value) : props.metric === 'cache' ? formatCacheHit(value) : formatExactTokens(value)
const axis = (value: number) => props.metric === 'cost' ? formatSpendCost(value) : props.metric === 'cache' ? formatCacheHit(value) : formatTokenCount(value)
const tipStyle = computed(() => ({ left: `${Math.max(8, Math.min((box.value?.clientWidth ?? 0) - 216, x(active.value) + 12))}px` }))
function move(event: PointerEvent) {
  const rect = (event.currentTarget as SVGElement).getBoundingClientRect()
  const fraction = ((event.clientX - rect.left) / rect.width * W.value - left) / (W.value - left - right)
  active.value = Math.max(0, Math.min(points.value.length - 1, Math.round(fraction * (points.value.length - 1))))
}
</script>

<template>
  <section ref="box" class="usage-trend" aria-label="使用趋势" :aria-busy="loading">
    <div class="chart-heading"><h2>{{ USAGE_METRIC_LABELS[metric] }}趋势</h2><span>{{ end - start <= 36 * 3600 ? '按小时' : '按日' }}</span></div>
    <svg v-if="hasData" class="trend-svg" :viewBox="`0 0 ${W} ${H}`" role="img" :aria-label="USAGE_METRIC_LABELS[metric] + '趋势图'" @pointermove="move" @pointerleave="active = -1">
      <defs><linearGradient :id="gradient" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="var(--usage-chart)" stop-opacity=".22" /><stop offset="1" stop-color="var(--usage-chart)" stop-opacity=".02" /></linearGradient></defs>
      <g v-for="part in [1, .5, 0]" :key="part"><line :x1="left" :x2="W - right" :y1="y(max * part)" :y2="y(max * part)" class="grid" /><text :x="left - 12" :y="y(max * part) + 4" text-anchor="end" class="axis">{{ axis(max * part) }}</text></g>
      <template v-if="metric === 'requests'"><rect v-for="(value, index) in values" :key="points[index].ts" :x="x(index) - Math.min(18, 340 / values.length)" :y="y(value)" :width="Math.min(36, 680 / values.length)" :height="bottom - y(value)" rx="3" fill="var(--usage-chart)" /></template>
      <template v-else><path :d="area" :fill="`url(#${gradient})`" /><path :d="line" fill="none" stroke="var(--usage-chart)" stroke-width="2" stroke-linejoin="round" /><circle v-if="values.length === 1" :cx="x(0)" :cy="y(values[0])" r="3" fill="var(--usage-chart)" /></template>
      <text v-for="tick in ticks" :key="tick.x" :x="tick.x" :y="H - 8" text-anchor="middle" class="axis">{{ tick.label }}</text>
      <g v-if="hit"><line :x1="x(active)" :x2="x(active)" :y1="top" :y2="bottom" class="cross" /><circle :cx="x(active)" :cy="y(values[active])" r="4" fill="var(--usage-chart)" /></g>
    </svg>
    <div v-else class="chart-empty" role="status">{{ loading ? '正在读取用量…' : '这个时间段暂无用量记录' }}</div>
    <div v-if="hit && hasData" class="trend-tip" :style="tipStyle"><p>{{ new Date(hit.ts * 1000).toLocaleString() }}</p><strong>{{ USAGE_METRIC_LABELS[metric] }} <span>{{ exact(values[active]) }}</span></strong></div>
  </section>
</template>

<style scoped>
.usage-trend { position: relative; padding: 24px 16px 12px; }
.chart-heading { display: flex; justify-content: space-between; align-items: center; padding: 0 4px 8px; font-size: 12px; color: var(--ad-muted); }
h2 { margin: 0; font-size: 12px; font-weight: 600; }
.trend-svg { display: block; width: 100%; height: 184px; }
.grid { stroke: var(--ad-border); stroke-dasharray: 3 4; }
.axis { fill: var(--ad-muted); font-size: 11px; }
.cross { stroke: var(--ad-muted); stroke-dasharray: 3 3; }
.trend-tip { position: absolute; z-index: 3; top: 52px; min-width: 200px; padding: 12px; border: 1px solid var(--ad-border-strong); border-radius: 6px; background: var(--ad-float-solid); box-shadow: var(--ad-shadow); pointer-events: none; font-size: 12px; }
.trend-tip p { margin: 0 0 8px; color: var(--ad-muted); }
.trend-tip strong { display: flex; gap: 16px; font-weight: 500; }
.trend-tip span { margin-left: auto; font-variant-numeric: tabular-nums; }
.chart-empty { min-height: 184px; display: grid; place-items: center; font-size: 13px; color: var(--ad-muted); }
</style>
