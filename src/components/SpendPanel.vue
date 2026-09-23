<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import SpendChart from './SpendChart.vue'
import {
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
const emptyText = computed(() =>
  props.loading ? '正在读取本机会话用量…' : '这个时间段还没有完成的会话轮次'
)
const weekdays = ['日', '一', '二', '三', '四', '五', '六']
const cells = computed(() => monthCells(calCursor.value.getFullYear(), calCursor.value.getMonth()))
const calTitle = computed(() => `${calCursor.value.getFullYear()}年${calCursor.value.getMonth() + 1}月`)

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
          <p class="total">{{ formatExactTokens(spend?.totalTokens ?? 0) }}</p>
          <p class="approx">≈ {{ formatTokenCount(spend?.totalTokens ?? 0) }}</p>
        </div>
        <div class="side">
          <div class="side-card">
            <span>总请求数</span>
            <strong>{{ spend?.turnCount ?? 0 }}</strong>
          </div>
          <div class="side-card">
            <span>总成本</span>
            <strong class="cost">{{ formatSpendCost(spend?.costUsd ?? 0) }}</strong>
          </div>
        </div>
      </div>

      <div class="cards">
        <div class="card">
          <span>新增输入</span>
          <strong>{{ formatTokenCount(spend?.inputTokens ?? 0) }}</strong>
        </div>
        <div class="card">
          <span>输出</span>
          <strong>{{ formatTokenCount(spend?.outputTokens ?? 0) }}</strong>
        </div>
        <div class="card">
          <span>缓存写入</span>
          <strong :class="{ faint: createdLabel === 'N/A' }">{{ createdLabel }}</strong>
        </div>
        <div class="card">
          <span>缓存命中</span>
          <strong>{{ formatTokenCount(spend?.cacheReadTokens ?? 0) }}</strong>
        </div>
        <div class="card card-rate">
          <span>缓存命中率</span>
          <strong>{{ formatCacheHit(spend?.cacheHitPercent) }}</strong>
          <i class="bar"><b :style="{ width: `${hitWidth}%` }" /></i>
        </div>
      </div>

      <SpendChart
        :chart="chart"
        :has-data="hasData"
        :loading="loading"
        :range-label="rangeLabel"
        :empty-text="emptyText"
      />
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
  }
  to {
    opacity: 1;
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
  border-color: var(--ad-border-strong);
  color: var(--ad-text);
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
  background: var(--ad-primary);
  color: var(--ad-primary-text);
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
  border-color: var(--ad-border-strong);
  background: var(--ad-selected);
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
  box-shadow: inset 0 0 0 1px var(--ad-border-strong);
}

.cal-day.in {
  background: var(--ad-selected);
}

.cal-day.on {
  background: var(--ad-primary);
  color: var(--ad-primary-text);
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
  background: var(--ad-primary);
  color: var(--ad-primary-text);
}

.hero {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-top: 10px;
}

.total {
  margin: 0;
  font-size: 32px;
  line-height: 38px;
  font-weight: 640;
  letter-spacing: -0.03em;
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
  background: var(--ad-hover);
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
  color: var(--ad-success);
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(108px, 1fr));
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
  background: var(--ad-selected);
  overflow: hidden;
}

.card-rate b {
  display: block;
  height: 100%;
  background: var(--ad-success);
  transition: width 280ms ease;
}

.error {
  margin: 0;
  padding: 28px 8px;
  text-align: center;
  color: var(--ad-faint);
}

@media (prefers-reduced-motion: reduce) {
  .pop-enter-active,
  .pop-leave-active,
  .card-rate b {
    transition: none;
  }
}
</style>
