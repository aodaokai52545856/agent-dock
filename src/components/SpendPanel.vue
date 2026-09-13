<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  SPEND_CHART_HEIGHT,
  SPEND_CHART_WIDTH,
  SPEND_PRESETS,
  buildSpendChart,
  dateInRange,
  formatCacheHit,
  formatExactTokens,
  formatFieldDate,
  formatSpendCost,
  formatTokenCount,
  fromLocalDateTime,
  monthCells,
  nearestHitIndex,
  resolveSpendRange,
  toLocalDateTime,
  type SpendPreset
} from '../lib/grokSpend'
import type { GrokSpend } from '../lib/types'

const props = defineProps<{
  spend: GrokSpend | null
  loading?: boolean
  preset: SpendPreset
  start: number
  end: number
  followEnd: boolean
}>()

const emit = defineEmits<{
  applyRange: [{ preset: SpendPreset; start: number; end: number; followEnd: boolean }]
  refresh: []
}>()

const rangeOpen = ref(false)
const draftPreset = ref<SpendPreset>(props.preset)
const draftFollow = ref(props.followEnd)
const focusing = ref<'start' | 'end'>('start')
const startParts = ref(toLocalDateTime(props.start))
const endParts = ref(toLocalDateTime(props.end))
const calCursor = ref(new Date(props.start * 1000))

const chart = computed(() => buildSpendChart(props.spend?.points ?? []))
const hasData = computed(() => (props.spend?.totalTokens ?? 0) > 0)
const rangeLabel = computed(() => SPEND_PRESETS.find((item) => item.id === props.preset)?.label ?? '自定义')
const hitWidth = computed(() => Math.max(0, Math.min(100, props.spend?.cacheHitPercent ?? 0)))
const createdLabel = computed(() =>
  (props.spend?.cacheCreationTokens ?? 0) > 0 ? formatTokenCount(props.spend?.cacheCreationTokens) : 'N/A'
)
const weekdays = ['日', '一', '二', '三', '四', '五', '六']
const cells = computed(() => monthCells(calCursor.value.getFullYear(), calCursor.value.getMonth()))
const calTitle = computed(() => `${calCursor.value.getFullYear()}年${calCursor.value.getMonth() + 1}月`)
const hoverIndex = ref(-1)
const chartBox = ref<HTMLElement | null>(null)
const shownTotal = ref(0)
const shownCost = ref(0)
const hover = computed(() => (hoverIndex.value >= 0 ? chart.value.hits[hoverIndex.value] ?? null : null))
const tipStyle = computed(() => {
  const hit = hover.value
  const box = chartBox.value
  if (!hit || !box) return { display: 'none' }
  const width = box.clientWidth
  const x = (hit.x / SPEND_CHART_WIDTH) * width
  return {
    left: `${Math.min(width - 188, Math.max(8, x + 14))}px`,
    top: '42px'
  }
})

function prefersReduce() {
  return Boolean(window.matchMedia?.('(prefers-reduced-motion: reduce)')?.matches)
}

function tweenTo(target: { value: number }, next: number) {
  const from = target.value
  if (from === next || prefersReduce()) {
    target.value = next
    return
  }
  const started = performance.now()
  const dur = 280
  const step = (now: number) => {
    const t = Math.min(1, (now - started) / dur)
    const ease = 1 - (1 - t) * (1 - t)
    target.value = from + (next - from) * ease
    if (t < 1) requestAnimationFrame(step)
  }
  requestAnimationFrame(step)
}

watch(
  () => props.spend?.totalTokens ?? 0,
  (next) => tweenTo(shownTotal, next),
  { immediate: true }
)
watch(
  () => props.spend?.costUsd ?? 0,
  (next) => tweenTo(shownCost, next),
  { immediate: true }
)

function onChartMove(event: MouseEvent) {
  const svg = event.currentTarget as SVGSVGElement
  const rect = svg.getBoundingClientRect()
  if (rect.width <= 0) return
  const x = ((event.clientX - rect.left) / rect.width) * SPEND_CHART_WIDTH
  hoverIndex.value = nearestHitIndex(chart.value.hits, x)
}

function onChartLeave() {
  hoverIndex.value = -1
}

function dayValue(day: number | null) {
  if (!day) return ''
  const year = calCursor.value.getFullYear()
  const month = String(calCursor.value.getMonth() + 1).padStart(2, '0')
  return `${year}-${month}-${String(day).padStart(2, '0')}`
}

function isInSpan(day: number | null) {
  const value = dayValue(day)
  return Boolean(value && dateInRange(value, startParts.value.date, endParts.value.date))
}

function isRangeEdge(day: number | null) {
  const value = dayValue(day)
  return value === startParts.value.date || value === endParts.value.date
}

watch(
  () => [props.start, props.end, props.preset, props.followEnd] as const,
  ([start, end, preset, follow]) => {
    draftPreset.value = preset
    draftFollow.value = follow
    startParts.value = toLocalDateTime(start)
    endParts.value = toLocalDateTime(end)
    calCursor.value = new Date(start * 1000)
  }
)

function openRange() {
  rangeOpen.value = !rangeOpen.value
  if (rangeOpen.value) {
    draftPreset.value = props.preset
    draftFollow.value = props.followEnd
    startParts.value = toLocalDateTime(props.start)
    endParts.value = toLocalDateTime(props.end)
  }
}

function pickPreset(id: SpendPreset) {
  draftPreset.value = id
  const range = resolveSpendRange(id)
  startParts.value = toLocalDateTime(range.start)
  endParts.value = toLocalDateTime(range.end)
  draftFollow.value = range.followEnd
  calCursor.value = new Date(range.start * 1000)
}

function shiftMonth(delta: number) {
  const next = new Date(calCursor.value)
  next.setMonth(next.getMonth() + delta)
  calCursor.value = next
}

function selectDay(day: number | null) {
  if (!day) return
  const year = calCursor.value.getFullYear()
  const month = String(calCursor.value.getMonth() + 1).padStart(2, '0')
  const value = `${year}-${month}-${String(day).padStart(2, '0')}`
  if (focusing.value === 'start') {
    startParts.value = { ...startParts.value, date: value }
    focusing.value = 'end'
  } else {
    endParts.value = { ...endParts.value, date: value }
  }
  draftPreset.value = 'custom'
  draftFollow.value = false
}

function isToday(day: number | null) {
  if (!day) return false
  const now = new Date()
  return (
    day === now.getDate() &&
    calCursor.value.getMonth() === now.getMonth() &&
    calCursor.value.getFullYear() === now.getFullYear()
  )
}

function applyRange() {
  const start = fromLocalDateTime(startParts.value.date, startParts.value.time)
  let end = fromLocalDateTime(endParts.value.date, endParts.value.time)
  if (draftFollow.value) end = Math.floor(Date.now() / 1000)
  const preset = draftPreset.value
  emit('applyRange', {
    preset,
    start: Math.min(start, end),
    end: Math.max(start + 1, end),
    followEnd: draftFollow.value
  })
  rangeOpen.value = false
}

function cancelRange() {
  rangeOpen.value = false
}

function onPanelKey(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  if (rangeOpen.value) {
    event.stopPropagation()
    rangeOpen.value = false
  }
}
</script>

<template>
  <div class="panel" role="dialog" aria-label="token消耗" @keydown="onPanelKey">
    <div class="toolbar">
      <p class="kicker">Grok Build · 真实消耗 Tokens · 本机全局</p>
      <div class="range-wrap">
        <button
          type="button"
          class="range-btn refresh-btn"
          :disabled="loading"
          title="立即刷新，默认每 60 秒自动更新"
          @click="emit('refresh')"
        >
          {{ loading ? '刷新中' : '刷新' }}
        </button>
        <button type="button" class="range-btn" :class="{ on: rangeOpen }" @click="openRange">
          {{ rangeLabel }}
          <span aria-hidden="true">▾</span>
        </button>
        <Transition name="pop">
          <div v-if="rangeOpen" class="range-pop" @click.stop>
            <div class="presets">
              <button
                v-for="item in SPEND_PRESETS"
                :key="item.id"
                type="button"
                class="preset"
                :class="{ on: draftPreset === item.id }"
                @click="pickPreset(item.id)"
              >
                {{ item.label }}
              </button>
            </div>
            <p class="range-hint">支持日期与时间</p>
            <div class="range-body">
              <div class="fields">
                <div class="field" :class="{ on: focusing === 'start' }" @click="focusing = 'start'">
                  <span class="field-kicker">开始时间</span>
                  <span class="field-row">
                    <b>{{ formatFieldDate(startParts.date) }}</b>
                    <input v-model="startParts.time" type="time" @click.stop @change="draftPreset = 'custom'" />
                  </span>
                </div>
                <div class="field" :class="{ on: focusing === 'end' }" @click="focusing = 'end'">
                  <span class="field-kicker">结束时间</span>
                  <span class="field-row">
                    <b>{{ formatFieldDate(endParts.date) }}</b>
                    <input
                      v-model="endParts.time"
                      type="time"
                      :disabled="draftFollow"
                      @click.stop
                      @change="draftPreset = 'custom'"
                    />
                  </span>
                </div>
                <label class="follow">
                  <input v-model="draftFollow" type="checkbox" />
                  结束时间跟随当前时刻
                </label>
              </div>
              <div class="cal">
                <div class="cal-head">
                  <button type="button" class="cal-nav" @click="shiftMonth(-1)">‹</button>
                  <span>{{ calTitle }}</span>
                  <button type="button" class="cal-nav" @click="shiftMonth(1)">›</button>
                </div>
                <div class="cal-week">
                  <span v-for="day in weekdays" :key="day">{{ day }}</span>
                </div>
                <div class="cal-grid">
                  <button
                    v-for="(day, index) in cells"
                    :key="index"
                    type="button"
                    class="cal-day"
                    :class="{ empty: !day, in: isInSpan(day), on: isRangeEdge(day), today: isToday(day) }"
                    :disabled="!day"
                    @click="selectDay(day)"
                  >
                    {{ day || '' }}
                  </button>
                </div>
              </div>
            </div>
            <div class="range-actions">
              <button type="button" class="text-btn" @click="cancelRange">取消</button>
              <button type="button" class="ok-btn" @click="applyRange">确定</button>
            </div>
          </div>
        </Transition>
      </div>
    </div>

    <p v-if="spend?.ok === false" class="error">{{ spend.message || '读取 token 消耗失败' }}</p>
    <template v-else>
      <div class="hero">
        <div>
          <p class="total">{{ formatExactTokens(shownTotal) }}</p>
          <p class="approx">≈ {{ formatTokenCount(shownTotal) }}</p>
        </div>
        <div class="side">
          <div class="side-card">
            <span>总请求数</span>
            <strong>{{ spend?.turnCount ?? 0 }}</strong>
          </div>
          <div class="side-card">
            <span>总成本</span>
            <strong class="cost">{{ formatSpendCost(shownCost) }}</strong>
          </div>
        </div>
      </div>

      <div class="cards">
        <div class="card">
          <span>↓ 新增输入</span>
          <strong>{{ formatTokenCount(spend?.inputTokens ?? 0) }}</strong>
        </div>
        <div class="card">
          <span>↑ Output</span>
          <strong>{{ formatTokenCount(spend?.outputTokens ?? 0) }}</strong>
        </div>
        <div class="card">
          <span>创建</span>
          <strong :class="{ faint: createdLabel === 'N/A' }">{{ createdLabel }}</strong>
        </div>
        <div class="card">
          <span>命中</span>
          <strong>{{ formatTokenCount(spend?.cacheReadTokens ?? 0) }}</strong>
        </div>
        <div class="card card-rate">
          <span>缓存命中率</span>
          <strong>{{ formatCacheHit(spend?.cacheHitPercent) }}</strong>
          <i class="bar"><b :style="{ width: `${hitWidth}%` }" /></i>
        </div>
      </div>

      <div ref="chartBox" class="chart-block">
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
          @mousemove="onChartMove"
          @mouseleave="onChartLeave"
        >
          <line
            v-for="(line, index) in chart.grid"
            :key="`g-${index}`"
            :x1="48"
            :x2="SPEND_CHART_WIDTH - 48"
            :y1="line.y"
            :y2="line.y"
            class="grid"
          />
          <text
            v-for="(line, index) in chart.grid"
            :key="`gl-${index}`"
            class="axis"
            x="8"
            :y="line.y + 3"
          >
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
            :fill="item.color"
            opacity="0.18"
          />
          <path
            v-for="item in chart.series"
            :key="item.key"
            :d="item.path"
            fill="none"
            :stroke="item.color"
            :stroke-width="item.dashed ? 1.6 : 2"
            stroke-linejoin="round"
            stroke-linecap="round"
            :stroke-dasharray="item.dashed ? '5 4' : undefined"
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
          <g v-if="hover" class="hover">
            <line
              :x1="hover.x"
              :x2="hover.x"
              :y1="chart.padTop"
              :y2="chart.bottom"
              class="cross"
            />
            <circle
              v-for="dot in hover.dots"
              :key="dot.key"
              :cx="dot.x"
              :cy="dot.y"
              r="3.4"
              :fill="dot.color"
              stroke="#111"
              stroke-width="1.4"
            />
          </g>
        </svg>
        <div v-if="hover" class="tip" :style="tipStyle">
          <p class="tip-time">{{ hover.label }}</p>
          <p v-for="item in hover.values" :key="item.key">
            <i :style="{ background: item.color }" />
            {{ item.label }}：{{ item.text }}
          </p>
        </div>
        <p v-else-if="!hasData" class="empty">{{ loading ? '正在读取本机会话用量…' : '这个时间段还没有完成的会话轮次' }}</p>
        <div class="legend">
          <span v-for="item in chart.series" :key="item.key">
            <i :style="{ background: item.color, outline: item.dashed ? `1px dashed ${item.color}` : undefined }" />
            {{ item.label }}
          </span>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.panel {
  padding: 12px 14px 14px;
  animation: in 160ms ease-out;
}

@keyframes in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.kicker {
  margin: 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-faint);
}

.range-wrap {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
}

.range-btn,
.preset,
.text-btn,
.ok-btn,
.cal-nav,
.cal-day,
.field {
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
}

.range-btn {
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
  color: var(--ad-text);
  background: var(--ad-hover);
}

.range-btn.on {
  border-color: #3b82f6;
}

.refresh-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.range-pop {
  position: absolute;
  right: 0;
  top: 34px;
  width: 520px;
  padding: 12px;
  border-radius: 12px;
  background: var(--ad-float-inset-solid);
  border: 1px solid var(--ad-border);
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
  z-index: 2;
}

.pop-enter-active,
.pop-leave-active {
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.pop-enter-from,
.pop-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

.presets {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.preset {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  background: var(--ad-hover);
  color: var(--ad-muted);
}

.preset.on {
  background: #2563eb;
  color: #fff;
}

.range-hint {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--ad-faint);
}

.range-body {
  display: grid;
  grid-template-columns: 1fr 1.2fr;
  gap: 14px;
}

.field {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 4px;
  min-height: 56px;
  margin-bottom: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  color: var(--ad-text);
  text-align: left;
  cursor: pointer;
}

.field.on {
  border-color: #3b82f6;
  background: rgba(37, 99, 235, 0.08);
}

.field-kicker {
  font-size: 11px;
  line-height: 16px;
  color: var(--ad-faint);
}

.field-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.field-row b {
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}

.field input {
  width: 92px;
  border: 0;
  background: transparent;
  color: var(--ad-text);
}

.follow {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  font-size: 12px;
  color: var(--ad-muted);
}

.cal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  color: var(--ad-text);
}

.cal-nav {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  color: var(--ad-muted);
}

.cal-week,
.cal-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  text-align: center;
}

.cal-week {
  color: var(--ad-faint);
  font-size: 11px;
  margin-bottom: 4px;
}

.cal-day {
  height: 30px;
  border-radius: 8px;
  color: var(--ad-text);
}

.cal-day.empty {
  visibility: hidden;
}

.cal-day.today {
  box-shadow: inset 0 0 0 1px #3b82f6;
}

.cal-day.in {
  background: rgba(37, 99, 235, 0.22);
}

.cal-day.on {
  background: #2563eb;
  color: #fff;
}

.range-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}

.text-btn,
.ok-btn {
  height: 30px;
  padding: 0 14px;
  border-radius: 8px;
}

.ok-btn {
  background: #2563eb;
  color: #fff;
}

.hero {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-top: 10px;
}

.total {
  margin: 0;
  font-size: 34px;
  line-height: 40px;
  font-variant-numeric: tabular-nums;
  color: var(--ad-text);
}

.approx {
  margin: 2px 0 0;
  color: var(--ad-faint);
  font-size: 13px;
}

.side {
  display: flex;
  gap: 8px;
}

.side-card,
.card {
  min-width: 92px;
  padding: 8px 10px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--ad-border);
}

.side-card span,
.card span {
  display: block;
  color: var(--ad-faint);
  font-size: 11px;
}

.side-card strong,
.card strong {
  display: block;
  margin-top: 4px;
  color: var(--ad-text);
  font-size: 16px;
  font-variant-numeric: tabular-nums;
}

.cost {
  color: #4ade80;
}

.cards {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
  margin-top: 12px;
}

.faint {
  color: var(--ad-faint);
}

.card-rate .bar {
  display: block;
  height: 6px;
  margin-top: 8px;
  border-radius: 99px;
  background: rgba(255, 255, 255, 0.06);
  overflow: hidden;
}

.card-rate b {
  display: block;
  height: 100%;
  background: #4ade80;
  transition: width 280ms ease;
}

.chart-block {
  position: relative;
  margin-top: 14px;
  padding: 10px 8px 6px;
  border-radius: 12px;
  border: 1px solid var(--ad-border);
  background: rgba(255, 255, 255, 0.02);
}

.chart-head {
  display: flex;
  justify-content: space-between;
  padding: 0 8px 6px;
  color: var(--ad-muted);
}

.chart-head p {
  margin: 0;
  color: var(--ad-text);
}

.chart {
  display: block;
  width: 100%;
  height: auto;
  transition: opacity 180ms ease;
}

.chart.is-loading {
  opacity: 0.42;
}

.cross {
  stroke: rgba(255, 255, 255, 0.45);
  stroke-width: 1;
}

.tip {
  position: absolute;
  z-index: 2;
  min-width: 176px;
  padding: 8px 10px;
  border-radius: 8px;
  background: #1a1a1a;
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
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

.grid {
  stroke: rgba(255, 255, 255, 0.06);
  stroke-width: 1;
}

.axis {
  fill: var(--ad-faint);
  font-size: 10px;
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 16px;
  padding: 6px 8px 2px;
  color: var(--ad-muted);
  font-size: 12px;
}

.legend i {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 6px;
  border-radius: 50%;
}

.empty,
.error {
  margin: 0;
  padding: 28px 8px;
  text-align: center;
  color: var(--ad-faint);
}

@media (prefers-reduced-motion: reduce) {
  .pop-enter-active,
  .pop-leave-active,
  .chart,
  .card-rate b {
    transition: none;
  }
}
</style>
