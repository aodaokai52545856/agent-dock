<script setup lang="ts">
import { computed, ref } from 'vue'
import { buildUsageCalendar, USAGE_METRIC_LABELS, type UsageMetric } from '../lib/usage'
import { formatCacheHit, formatExactTokens, formatSpendCost } from '../lib/grokSpend'
import type { UsageHeatCell } from '../lib/types'

const props = defineProps<{ rows: UsageHeatCell[]; metric: UsageMetric; end: number; loading: boolean }>()
const page = ref(0)
const calendar = computed(() => buildUsageCalendar(props.rows, props.metric, props.end, page.value))
const active = ref<ReturnType<typeof buildUsageCalendar>['weeks'][number][number] | null>(null)
const tip = ref({ left: '0px', top: '0px' })
const box = ref<HTMLElement>()
function show(day: NonNullable<typeof active.value>, event: MouseEvent | FocusEvent) {
  active.value = day
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
  const parent = box.value!.getBoundingClientRect()
  tip.value = { left: `${Math.max(8, Math.min(parent.width - 232, rect.left - parent.left - 100))}px`, top: `${rect.bottom - parent.top + 8}px` }
}
</script>

<template>
  <section ref="box" class="usage-calendar" aria-label="每日用量热力图" :aria-busy="loading">
    <div class="calendar-heading"><div><h2>每日用量 · {{ USAGE_METRIC_LABELS[metric] }}</h2><p>{{ calendar.start }} — {{ calendar.end }}</p></div><span>{{ calendar.activeDays }} 个活跃日 · {{ formatExactTokens(calendar.requests) }} 次请求</span></div>
    <div class="calendar-scroll">
      <div class="calendar-grid">
        <span /> <span v-for="(label, index) in calendar.months" :key="index" class="month">{{ label }}</span>
        <template v-for="dayIndex in 7" :key="dayIndex">
          <span class="weekday">{{ ['一', '', '三', '', '五', '', ''][dayIndex - 1] }}</span>
          <button v-for="(week, weekIndex) in calendar.weeks" :key="weekIndex" type="button" class="day" :class="[`level-${week[dayIndex - 1].level}`, { future: week[dayIndex - 1].future }]" :disabled="week[dayIndex - 1].future" :aria-label="week[dayIndex - 1].key + '，' + (week[dayIndex - 1].row?.requests ?? 0) + ' 次请求'" @mouseenter="show(week[dayIndex - 1], $event)" @mouseleave="active = null" @focus="show(week[dayIndex - 1], $event)" @blur="active = null" />
        </template>
      </div>
    </div>
    <div class="calendar-footer"><div class="calendar-pages"><button v-if="calendar.hasOlder" type="button" @click="page++; active = null">← 更早记录</button><button v-if="page > 0" type="button" @click="page--; active = null">较新记录 →</button><span v-if="!calendar.hasOlder && page === 0">最近一年</span></div><div class="levels"><span>少</span><i v-for="level in [0, 1, 2, 3, 4]" :key="level" :class="'level-' + level" /><span>多</span></div></div>
    <div v-if="active && !active.future" class="calendar-tip" :style="tip" role="tooltip"><strong>{{ active.key }}</strong><dl><dt>请求</dt><dd>{{ formatExactTokens(active.row?.requests) }}</dd><dt>Tokens</dt><dd>{{ formatExactTokens(active.row?.tokens) }}</dd><dt>成本</dt><dd>{{ formatSpendCost(active.row?.costUsd) }}</dd><template v-if="metric === 'cache'"><dt>缓存命中率</dt><dd>{{ active.value == null ? '—' : formatCacheHit(active.value) }}</dd></template></dl></div>
  </section>
</template>

<style scoped>
.usage-calendar { position: relative; padding: 24px 20px 16px; }
.calendar-heading { display: flex; align-items: baseline; justify-content: space-between; flex-wrap: wrap; gap: 8px; margin-bottom: 16px; }
h2 { margin: 0; font-size: 13px; font-weight: 600; }
.calendar-heading p { margin: 4px 0 0; }
.calendar-heading p, .calendar-heading > span, .calendar-footer { font-size: 12px; color: var(--ad-muted); font-variant-numeric: tabular-nums; }
.calendar-scroll { overflow-x: auto; padding-bottom: 4px; }
.calendar-grid { display: grid; grid-template-columns: 20px repeat(53, minmax(0, 1fr)); gap: 4px; align-items: center; min-width: 680px; }
.month { overflow: visible; white-space: nowrap; height: 20px; font-size: 11px; color: var(--ad-muted); }
.weekday { font-size: 10px; color: var(--ad-muted); }
.day { padding: 0; width: 100%; aspect-ratio: 1; max-height: 16px; border-radius: 3px; border: 1px solid var(--ad-border); }
.day:hover, .day:focus-visible { outline: 1px solid var(--ad-text); outline-offset: 1px; }
.day.future { opacity: .2; }
.level-0 { background: var(--ad-hover); }
.level-1 { background: color-mix(in srgb, var(--usage-chart) 25%, var(--ad-harbor)); }
.level-2 { background: color-mix(in srgb, var(--usage-chart) 45%, var(--ad-harbor)); }
.level-3 { background: color-mix(in srgb, var(--usage-chart) 70%, var(--ad-harbor)); }
.level-4 { background: var(--usage-chart); }
.calendar-footer { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-top: 12px; }
.calendar-pages { display: flex; gap: 16px; }
.calendar-pages button { color: var(--ad-muted); font-size: 12px; }
.calendar-pages button:hover { color: var(--ad-text); }
.levels { display: flex; align-items: center; gap: 4px; }
.levels i { width: 12px; height: 12px; border-radius: 3px; }
.calendar-tip { position: absolute; z-index: 5; min-width: 216px; padding: 12px; background: var(--ad-float-solid); border: 1px solid var(--ad-border-strong); border-radius: 6px; box-shadow: var(--ad-shadow); pointer-events: none; font-size: 12px; }
.calendar-tip strong { font-weight: 600; }
dl { display: grid; grid-template-columns: 1fr auto; gap: 4px 16px; margin: 8px 0 0; }
dt { color: var(--ad-muted); }
dd { margin: 0; text-align: right; font-variant-numeric: tabular-nums; }
</style>
