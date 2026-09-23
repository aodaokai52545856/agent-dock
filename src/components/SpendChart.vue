<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import {
  SPEND_CHART_HEIGHT,
  SPEND_CHART_WIDTH,
  nearestHitIndex,
  type SpendChartModel
} from '../lib/grokSpend'

const props = defineProps<{
  chart: SpendChartModel
  hasData: boolean
  loading?: boolean
  rangeLabel: string
  emptyText: string
}>()

const hoverIndex = ref(-1)
const box = ref<HTMLElement | null>(null)
let frame = 0
let pendingX = 0

const hover = computed(() => (hoverIndex.value >= 0 ? props.chart.hits[hoverIndex.value] ?? null : null))
const tipStyle = computed(() => {
  const hit = hover.value
  const el = box.value
  if (!hit || !el) return { display: 'none' }
  const width = el.clientWidth
  const x = (hit.x / SPEND_CHART_WIDTH) * width
  return {
    left: `${Math.min(Math.max(8, width - 188), Math.max(8, x + 14))}px`,
    top: '42px'
  }
})

function onChartMove(event: PointerEvent) {
  const svg = event.currentTarget as SVGSVGElement
  const rect = svg.getBoundingClientRect()
  if (rect.width <= 0) return
  pendingX = ((event.clientX - rect.left) / rect.width) * SPEND_CHART_WIDTH
  if (frame) return
  frame = requestAnimationFrame(() => {
    frame = 0
    const next = nearestHitIndex(props.chart.hits, pendingX)
    if (next !== hoverIndex.value) hoverIndex.value = next
  })
}

function onChartLeave() {
  if (frame) cancelAnimationFrame(frame)
  frame = 0
  hoverIndex.value = -1
}

onUnmounted(() => {
  if (frame) cancelAnimationFrame(frame)
})
</script>

<template>
  <div ref="box" class="chart-block">
    <div class="chart-head">
      <p>使用趋势</p>
      <span>{{ rangeLabel }}</span>
    </div>
    <svg
      v-if="hasData"
      class="chart"
      :class="{ 'is-loading': loading }"
      :viewBox="`0 0 ${SPEND_CHART_WIDTH} ${SPEND_CHART_HEIGHT}`"
      role="img"
      aria-label="token 趋势"
      @pointermove="onChartMove"
      @pointerleave="onChartLeave"
    >
      <g v-memo="[chart]">
        <defs>
          <linearGradient
            v-for="item in chart.series"
            :id="`spend-fill-${item.key}`"
            :key="`def-${item.key}`"
            x1="0"
            y1="0"
            x2="0"
            y2="1"
          >
            <stop offset="0%" :stop-color="item.color" stop-opacity="0.22" />
            <stop offset="100%" :stop-color="item.color" stop-opacity="0" />
          </linearGradient>
        </defs>
        <line
          v-for="(line, index) in chart.grid"
          :key="`g-${index}`"
          :x1="chart.padLeft"
          :x2="SPEND_CHART_WIDTH - chart.padRight"
          :y1="line.y"
          :y2="line.y"
          class="grid"
        />
        <text v-for="(line, index) in chart.grid" :key="`gl-${index}`" class="axis" x="8" :y="line.y + 3">
          {{ line.label }}
        </text>
        <text
          v-for="(line, index) in chart.costTicks"
          :key="`cl-${index}`"
          class="axis"
          :x="SPEND_CHART_WIDTH - 8"
          :y="line.y + 3"
          text-anchor="end"
        >
          {{ line.label }}
        </text>
        <path
          v-for="item in chart.series.filter((row) => row.area)"
          :key="`${item.key}-area`"
          :d="item.area"
          :fill="`url(#spend-fill-${item.key})`"
        />
        <path
          v-for="item in chart.series"
          :key="item.key"
          :d="item.path"
          fill="none"
          :stroke="item.color"
          :stroke-width="item.dashed ? 1.5 : 2"
          stroke-linejoin="round"
          stroke-linecap="round"
          :stroke-dasharray="item.dashed ? '4 4' : undefined"
        />
        <text
          v-for="(tick, index) in chart.ticks"
          :key="`t-${index}`"
          class="axis"
          :x="tick.x"
          :y="SPEND_CHART_HEIGHT - 8"
          text-anchor="middle"
        >
          {{ tick.label }}
        </text>
      </g>
      <g v-if="hover" class="hover">
        <line :x1="hover.x" :x2="hover.x" :y1="chart.padTop" :y2="chart.bottom" class="cross" />
        <circle
          v-for="dot in hover.dots"
          :key="dot.key"
          :cx="dot.x"
          :cy="dot.y"
          r="3.2"
          :fill="dot.color"
          class="dot"
        />
      </g>
    </svg>
    <div v-if="hover" class="tip" :style="tipStyle">
      <p class="tip-time">{{ hover.label }}</p>
      <p v-for="item in hover.values" :key="item.key">
        <i :style="{ background: item.color }" />
        {{ item.label }} {{ item.text }}
      </p>
    </div>
    <p v-else-if="!hasData" class="empty">{{ emptyText }}</p>
    <div class="legend">
      <span v-for="item in chart.series" :key="item.key">
        <i :class="{ dashed: item.dashed }" :style="{ background: item.dashed ? 'transparent' : item.color, borderColor: item.color }" />
        {{ item.label }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.chart-block {
  position: relative;
  margin-top: 14px;
  padding: 12px 8px 8px;
  border-radius: var(--ad-radius-card);
  border: 1px solid var(--ad-border);
  background: color-mix(in srgb, var(--ad-hover) 55%, transparent);
}

.chart-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: 0 8px 4px;
  color: var(--ad-muted);
  font-size: 12px;
}

.chart-head p {
  margin: 0;
  color: var(--ad-text);
  font-size: 13px;
  font-weight: 600;
}

.chart {
  display: block;
  width: 100%;
  height: auto;
}

.chart.is-loading {
  opacity: 0.55;
}

.grid {
  stroke: var(--ad-border);
  stroke-width: 1;
}

.axis {
  fill: var(--ad-faint);
  font-size: 10px;
}

.cross {
  stroke: var(--ad-border-strong);
  stroke-width: 1;
}

.dot {
  stroke: var(--ad-float-solid);
  stroke-width: 1.5;
}

.tip {
  position: absolute;
  z-index: 2;
  min-width: 168px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--ad-float-solid);
  border: 1px solid var(--ad-border-strong);
  box-shadow: var(--ad-shadow);
  pointer-events: none;
  color: var(--ad-text);
  font-size: 12px;
  line-height: 18px;
}

.tip-time {
  margin: 0 0 6px;
  color: var(--ad-muted);
}

.tip p {
  margin: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  font-variant-numeric: tabular-nums;
}

.tip i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 14px;
  padding: 4px 8px 2px;
  color: var(--ad-muted);
  font-size: 12px;
}

.legend i {
  display: inline-block;
  width: 10px;
  height: 3px;
  margin-right: 6px;
  border-radius: 99px;
  vertical-align: middle;
  border: 1.5px solid transparent;
}

.legend i.dashed {
  border-style: dashed;
  height: 0;
}

.empty {
  margin: 0;
  padding: 36px 8px;
  text-align: center;
  color: var(--ad-faint);
}
</style>
