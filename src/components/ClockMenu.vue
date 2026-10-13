<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  CLOCK_CITIES,
  clockFace,
  continentName,
  featuredCities,
  filterCities,
  formatDelta,
  groupCities,
  machineOffsetMinutes,
  readFeaturedIds,
  readSelectedCity,
  toggleFeatured,
  writeFeaturedIds,
  writeSelectedCityId,
  type ClockBrowse,
  type ClockCity,
  type ClockStorage
} from '../lib/worldClock'

const props = defineProps<{
  burst: number
}>()

const emit = defineEmits<{
  open: []
}>()

const open = ref(false)
const mode = ref<ClockBrowse>('featured')
const query = ref('')
const now = ref(Date.now())
const searchEl = ref<HTMLInputElement | null>(null)
const featuredIds = ref<string[] | null>(null)
const active = ref<ClockCity>(CLOCK_CITIES[0])

let timer = 0

function browserStorage(): ClockStorage | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

function loadPrefs() {
  const utc = Date.now()
  featuredIds.value = readFeaturedIds(browserStorage())
  active.value = readSelectedCity(browserStorage(), machineOffsetMinutes(new Date(utc)), utc)
}

const face = computed(() => clockFace(active.value, now.value))
const featuredList = computed(() => featuredCities(featuredIds.value))
const featuredSet = computed(() => new Set(featuredList.value.map((city) => city.id)))
const customized = computed(() => featuredIds.value !== null)

const browse = computed<'featured' | 'continent' | 'country'>(() => {
  if (mode.value === 'featured' && query.value.trim()) return 'country'
  return mode.value
})

const groups = computed(() => {
  if (browse.value === 'featured') {
    return featuredList.value.length ? [{ id: 'featured', label: '主流城市', cities: featuredList.value }] : []
  }
  return groupCities(filterCities(CLOCK_CITIES, query.value), browse.value)
})

const shownCount = computed(() => groups.value.reduce((sum, group) => sum + group.cities.length, 0))

const vsMachine = computed(() => {
  const delta = face.value.offsetMinutes - machineOffsetMinutes(new Date(now.value))
  return delta === 0 ? '与本机同一时刻' : `比本机${formatDelta(delta)}`
})

const buttonLabel = computed(() => `时钟，${active.value.city} ${face.value.hm}`)

function rowFace(city: ClockCity) {
  return clockFace(city, now.value)
}

function selectCity(city: ClockCity) {
  active.value = city
  writeSelectedCityId(browserStorage(), city.id)
}

function onToggleFeatured(id: string) {
  const next = toggleFeatured(featuredIds.value, id)
  featuredIds.value = next
  writeFeaturedIds(browserStorage(), next)
}

function restoreFeatured() {
  featuredIds.value = null
  writeFeaturedIds(browserStorage(), null)
}

function toggleOpen() {
  open.value = !open.value
  if (open.value) emit('open')
}

watch(
  () => props.burst,
  () => {
    open.value = false
  }
)

watch(open, async (isOpen) => {
  if (!isOpen) return
  await nextTick()
  searchEl.value?.focus()
})

onMounted(() => {
  loadPrefs()
  timer = window.setInterval(() => {
    now.value = Date.now()
  }, 250)
})

onUnmounted(() => {
  window.clearInterval(timer)
})
</script>

<template>
  <div class="clock" @click.stop @mousedown.stop>
    <button
      type="button"
      class="file-btn"
      :class="{ 'is-open': open }"
      aria-haspopup="dialog"
      :aria-expanded="open"
      :aria-label="buttonLabel"
      :title="`${active.city} ${face.dateLine} ${face.hms} ${face.offsetLabel}`"
      @click="toggleOpen"
    >
      <span class="kicker">时钟</span>
      <span class="clock-name">{{ active.city }}</span>
      <span class="clock-hm">{{ face.hm }}</span>
    </button>
    <div v-if="open" class="ad-menu clock-panel" role="dialog" aria-label="世界时钟">
      <div class="readout">
        <div class="readout-top">
          <div class="readout-place">
            <div class="readout-city">{{ active.city }}</div>
            <div class="readout-meta">{{ active.country }} · {{ continentName(active.continent) }}</div>
          </div>
          <button
            type="button"
            class="text-btn"
            :aria-pressed="featuredSet.has(active.id)"
            @click="onToggleFeatured(active.id)"
          >
            {{ featuredSet.has(active.id) ? '移出主流' : '设为主流' }}
          </button>
        </div>
        <div class="readout-time">{{ face.hms }}</div>
        <div class="readout-sub">
          <span>{{ face.dateLine }}</span>
          <span>{{ face.offsetLabel }}</span>
          <span v-if="face.seasonLabel">{{ face.seasonLabel }}</span>
          <span>{{ vsMachine }}</span>
        </div>
      </div>

      <input
        ref="searchEl"
        v-model="query"
        class="search"
        type="search"
        placeholder="搜索城市、国家或时区"
        aria-label="搜索城市、国家或时区"
      />

      <div class="tabs" role="tablist" aria-label="城市分类">
        <button type="button" role="tab" class="tab" :class="{ 'is-on': mode === 'featured' }" :aria-selected="mode === 'featured'" @click="mode = 'featured'">
          主流
        </button>
        <button type="button" role="tab" class="tab" :class="{ 'is-on': mode === 'continent' }" :aria-selected="mode === 'continent'" @click="mode = 'continent'">
          大洲
        </button>
        <button type="button" role="tab" class="tab" :class="{ 'is-on': mode === 'country' }" :aria-selected="mode === 'country'" @click="mode = 'country'">
          国家
        </button>
      </div>

      <div class="meta-row">
        <span>{{ shownCount }} 个城市</span>
        <button v-if="customized" type="button" class="text-btn" @click="restoreFeatured">恢复默认</button>
      </div>

      <div class="groups" :class="{ 'is-empty': shownCount === 0 }">
        <p v-if="shownCount === 0" class="empty">
          {{ mode === 'featured' && !query.trim() ? '还没有主流城市。到大洲或国家里点星标添加。' : '没有匹配的城市' }}
        </p>
        <section v-for="group in groups" :key="group.id">
          <div class="glabel">{{ group.label }}</div>
          <div v-for="city in group.cities" :key="city.id" class="row">
            <button
              type="button"
              class="pick"
              :class="{ 'is-active': city.id === active.id }"
              @click="selectCity(city)"
            >
              <span class="name">{{ city.city }}</span>
              <span v-if="browse !== 'country'" class="country">{{ city.country }}</span>
              <span class="hm">{{ rowFace(city).hm }}</span>
              <span class="off">{{ rowFace(city).offsetLabel }}</span>
            </button>
            <button
              type="button"
              class="star"
              :class="{ 'is-on': featuredSet.has(city.id) }"
              :aria-pressed="featuredSet.has(city.id)"
              :aria-label="featuredSet.has(city.id) ? `移出主流：${city.city}` : `设为主流：${city.city}`"
              @click="onToggleFeatured(city.id)"
            >
              {{ featuredSet.has(city.id) ? '★' : '☆' }}
            </button>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.clock {
  position: relative;
  height: 100%;
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.file-btn {
  height: 100%;
  padding: 0 12px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ad-font);
  line-height: 1.5;
  color: var(--ad-muted);
}

.file-btn:hover,
.file-btn.is-open {
  background: var(--ad-hover);
  color: var(--ad-text);
}

.kicker {
  color: var(--ad-faint);
}

.clock-name {
  max-width: 6em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.clock-hm {
  font-variant-numeric: tabular-nums;
  color: var(--ad-text);
}

.clock-panel {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 50;
  width: min(372px, calc(100vw - 24px));
  padding: 10px;
  background: var(--ad-float-solid);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.readout-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.readout-city {
  font-weight: 600;
  color: var(--ad-text);
}

.readout-meta,
.readout-sub,
.meta-row,
.glabel,
.country,
.off,
.empty {
  color: var(--ad-muted);
  font-size: var(--ad-font-sm);
}

.readout-time {
  margin-top: 2px;
  font-size: 22px;
  font-weight: 600;
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
  color: var(--ad-text);
}

.readout-sub {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}

.search {
  width: 100%;
  height: 28px;
  margin-top: 10px;
  padding: 0 8px;
  font-size: var(--ad-font-sm);
}

.tabs {
  display: flex;
  height: 28px;
  margin-top: 8px;
  padding: 2px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
  background: var(--ad-harbor);
}

.tab {
  flex: 1;
  height: 100%;
  border-radius: 6px;
  font-size: var(--ad-font-sm);
  color: var(--ad-muted);
}

.tab:hover {
  color: var(--ad-text);
}

.tab.is-on {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.meta-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 24px;
  margin-top: 6px;
  padding: 0 2px;
}

.text-btn {
  height: 24px;
  padding: 0 8px;
  border-radius: 6px;
  color: var(--ad-muted);
  font-size: var(--ad-font-sm);
  white-space: nowrap;
}

.text-btn:hover {
  background: var(--ad-hover);
  color: var(--ad-text);
}

.groups {
  max-height: 280px;
  margin-top: 4px;
  overflow: auto;
  overscroll-behavior: contain;
}

.groups.is-empty {
  min-height: 72px;
}

.empty {
  margin: 16px 8px;
  text-align: center;
}

.glabel {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 6px 8px 2px;
  background: var(--ad-float-solid);
  color: var(--ad-faint);
  font-size: var(--ad-font-xs);
}

.row {
  display: flex;
  align-items: center;
  gap: 2px;
}

.pick {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  min-height: calc(var(--ad-font) + 16px);
  padding: 0 8px;
  border-radius: 6px;
  text-align: left;
  color: var(--ad-text);
  font-size: var(--ad-font);
}

.pick:hover {
  background: var(--ad-hover);
}

.pick.is-active,
.pick.is-active:hover {
  background: var(--ad-selected);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.country {
  max-width: 7em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hm,
.off {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.hm {
  color: var(--ad-text);
}

.star {
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  border-radius: 6px;
  color: var(--ad-faint);
  font-size: 14px;
}

.star:hover {
  background: var(--ad-hover);
  color: var(--ad-text);
}

.star.is-on {
  color: var(--ad-warning);
}
</style>
