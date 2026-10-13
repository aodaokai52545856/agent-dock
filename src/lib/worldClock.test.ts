import assert from 'node:assert/strict'
import {
  CLOCK_CITIES,
  clockFace,
  effectiveOffsetMinutes,
  featuredCities,
  filterCities,
  formatDelta,
  formatOffset,
  groupCities,
  parseFeaturedIds,
  readFeaturedIds,
  readSelectedCity,
  suggestCity,
  toggleFeatured,
  writeFeaturedIds,
  writeSelectedCityId,
  type ClockCity,
  type ClockStorage
} from './worldClock.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function city(id: string): ClockCity {
  const found = CLOCK_CITIES.find((item) => item.id === id)
  assert.ok(found, id)
  return found
}

function memory(): ClockStorage & { dump: Record<string, string> } {
  const dump: Record<string, string> = {}
  return {
    dump,
    getItem(key) {
      return Object.prototype.hasOwnProperty.call(dump, key) ? dump[key] : null
    },
    setItem(key, value) {
      dump[key] = value
    },
    removeItem(key) {
      delete dump[key]
    }
  }
}

const MAJOR_OFFSETS = [
  -660, -600, -570, -540, -480, -420, -360, -300, -240, -210, -180, -120, -60, 0, 60, 120, 180, 210, 240, 270, 300,
  330, 345, 360, 390, 420, 480, 525, 540, 570, 600, 630, 660, 720, 765, 780, 840
]

test('catalog ids are unique and cover the major offsets', () => {
  const ids = CLOCK_CITIES.map((item) => item.id)
  assert.equal(new Set(ids).size, ids.length)
  const offsets = new Set(CLOCK_CITIES.map((item) => item.offsetMinutes))
  for (const minutes of MAJOR_OFFSETS) assert.equal(offsets.has(minutes), true, String(minutes))
  for (const kind of ['none', 'us', 'eu', 'au', 'nz', 'chatham', 'cl', 'eg', 'il', 'lh', 'cu'] as const) {
    assert.equal(
      CLOCK_CITIES.some((item) => item.dst === kind),
      true,
      kind
    )
  }
})

test('fixed offsets ignore the calendar', () => {
  const beijing = city('beijing')
  const utc = Date.UTC(2026, 9, 13, 4, 0, 7)
  assert.equal(effectiveOffsetMinutes(beijing, utc), 480)
  const face = clockFace(beijing, utc)
  assert.equal(face.hms, '12:00:07')
  assert.equal(face.dateLine, '2026年10月13日 周二')
  assert.equal(face.offsetLabel, 'UTC+8')
  assert.equal(face.seasonLabel, '')
  assert.equal(formatOffset(330), 'UTC+5:30')
  assert.equal(formatOffset(-210), 'UTC-3:30')
  assert.equal(clockFace(city('kathmandu'), Date.UTC(2026, 9, 13, 0, 0)).hm, '05:45')
})

test('daylight rules switch the offset', () => {
  const ny = city('new-york')
  assert.equal(effectiveOffsetMinutes(ny, Date.UTC(2026, 0, 15, 17, 0)), -300)
  assert.equal(clockFace(ny, Date.UTC(2026, 0, 15, 17, 0)).hm, '12:00')
  assert.equal(effectiveOffsetMinutes(ny, Date.UTC(2026, 6, 15, 16, 0)), -240)
  assert.equal(clockFace(ny, Date.UTC(2026, 6, 15, 16, 0)).hm, '12:00')
  assert.equal(effectiveOffsetMinutes(ny, Date.UTC(2026, 2, 8, 6, 59)), -300)
  assert.equal(effectiveOffsetMinutes(ny, Date.UTC(2026, 2, 8, 7, 0)), -240)

  const london = city('london')
  assert.equal(effectiveOffsetMinutes(london, Date.UTC(2026, 0, 15, 12, 0)), 0)
  assert.equal(effectiveOffsetMinutes(london, Date.UTC(2026, 9, 13, 12, 0)), 60)
  assert.equal(clockFace(london, Date.UTC(2026, 9, 13, 12, 0)).hm, '13:00')
  assert.equal(effectiveOffsetMinutes(london, Date.UTC(2026, 9, 25, 0, 30)), 60)
  assert.equal(effectiveOffsetMinutes(london, Date.UTC(2026, 9, 25, 1, 0)), 0)

  const sydney = city('sydney')
  assert.equal(effectiveOffsetMinutes(sydney, Date.UTC(2026, 9, 13, 0, 0)), 660)
  assert.equal(clockFace(sydney, Date.UTC(2026, 9, 13, 0, 0)).hms, '11:00:00')
  assert.equal(effectiveOffsetMinutes(sydney, Date.UTC(2026, 6, 15, 2, 0)), 600)

  const howe = city('lord-howe')
  assert.equal(effectiveOffsetMinutes(howe, Date.UTC(2026, 0, 15, 0, 0)), 660)
  assert.equal(clockFace(howe, Date.UTC(2026, 0, 15, 0, 0)).offsetLabel, 'UTC+11')
  assert.equal(effectiveOffsetMinutes(howe, Date.UTC(2026, 6, 15, 0, 0)), 630)
  assert.equal(clockFace(howe, Date.UTC(2026, 6, 15, 0, 0)).offsetLabel, 'UTC+10:30')

  assert.equal(effectiveOffsetMinutes(city('santiago'), Date.UTC(2026, 9, 13, 15, 0)), -180)
  assert.equal(effectiveOffsetMinutes(city('santiago'), Date.UTC(2026, 5, 15, 16, 0)), -240)
  assert.equal(effectiveOffsetMinutes(city('cairo'), Date.UTC(2026, 9, 13, 9, 0)), 180)
  assert.equal(effectiveOffsetMinutes(city('cairo'), Date.UTC(2026, 0, 15, 10, 0)), 120)
  assert.equal(effectiveOffsetMinutes(city('jerusalem'), Date.UTC(2026, 9, 13, 9, 0)), 180)
  assert.equal(effectiveOffsetMinutes(city('jerusalem'), Date.UTC(2026, 9, 24, 23, 0)), 120)
  assert.equal(effectiveOffsetMinutes(city('chatham'), Date.UTC(2026, 0, 15, 0, 0)), 825)
  assert.equal(clockFace(city('chatham'), Date.UTC(2026, 0, 15, 0, 0)).hm, '13:45')
})

test('search matches city, country, english and offset words', () => {
  const ids = (query: string) => filterCities(CLOCK_CITIES, query).map((item) => item.id)
  assert.deepEqual(ids('东京'), ['tokyo'])
  assert.deepEqual(ids('Tokyo'), ['tokyo'])
  assert.ok(ids('日本').includes('tokyo'))
  assert.ok(ids('东八区').includes('beijing'))
  assert.equal(ids('东八区').includes('tokyo'), false)
  assert.deepEqual(ids('美国 纽约'), ['new-york'])
  assert.ok(ids('UTC+5:30').includes('delhi'))
  assert.ok(ids('utc＋5：45').includes('kathmandu'))
  assert.equal(ids('没有这座城').length, 0)
  assert.equal(filterCities(CLOCK_CITIES, '  ').length, CLOCK_CITIES.length)
})

test('continent and country groups keep order', () => {
  const continents = groupCities(CLOCK_CITIES, 'continent').map((group) => group.label)
  assert.deepEqual(continents, ['亚洲', '欧洲', '非洲', '北美洲', '南美洲', '大洋洲', '南极洲'])
  const china = groupCities(CLOCK_CITIES, 'country').find((group) => group.id === '中国')
  assert.ok(china)
  assert.deepEqual(
    china.cities.map((item) => item.id).sort(),
    ['beijing', 'chengdu', 'guangzhou', 'hong-kong', 'shanghai', 'shenzhen', 'taipei', 'urumqi']
  )
  const ordered = china.cities.map((item) => item.city)
  assert.deepEqual(ordered, [...ordered].sort((a, b) => new Intl.Collator('zh-CN').compare(a, b)))
  const asia = groupCities(filterCities(CLOCK_CITIES, '东京'), 'continent')
  assert.equal(asia.length, 1)
  assert.equal(asia[0].id, 'asia')
})

test('featured cities can be starred and restored', () => {
  assert.equal(featuredCities(null)[0].id, 'beijing')
  assert.equal(featuredCities(null).some((item) => item.id === 'sydney'), true)
  const added = toggleFeatured(null, 'chengdu')
  assert.ok(added)
  assert.equal(added.at(-1), 'chengdu')
  assert.equal(featuredCities(added).at(-1)?.id, 'chengdu')
  const removed = toggleFeatured(null, 'beijing')
  assert.ok(removed)
  assert.equal(removed.includes('beijing'), false)
  assert.equal(toggleFeatured(added, 'chengdu'), null)
  assert.equal(toggleFeatured(null, 'missing-city'), null)
})

test('selection and featured lists persist', () => {
  const store = memory()
  assert.equal(readSelectedCity(store, 480, Date.UTC(2026, 9, 13)).id, 'beijing')
  assert.equal(readSelectedCity(store, 345, Date.UTC(2026, 0, 1)).id, 'kathmandu')
  assert.equal(suggestCity(123, Date.UTC(2026, 0, 1)).id, 'beijing')
  writeSelectedCityId(store, 'tokyo')
  assert.equal(readSelectedCity(store, 480, Date.UTC(2026, 9, 13)).id, 'tokyo')
  writeSelectedCityId(store, 'nope')
  assert.equal(readSelectedCity(store, 480, Date.UTC(2026, 9, 13)).id, 'tokyo')

  assert.equal(readFeaturedIds(store), null)
  writeFeaturedIds(store, ['chengdu', 'chengdu', 'nope'])
  assert.deepEqual(readFeaturedIds(store), ['chengdu'])
  writeFeaturedIds(store, [])
  assert.deepEqual(readFeaturedIds(store), [])
  assert.equal(featuredCities([]).length, 0)
  writeFeaturedIds(store, null)
  assert.equal(readFeaturedIds(store), null)
  assert.equal(parseFeaturedIds('not-json'), null)
  assert.equal(parseFeaturedIds('["nope"]'), null)
})

test('delta copy counts hours and minutes', () => {
  assert.equal(formatDelta(0), '同一时刻')
  assert.equal(formatDelta(60), '快 1 小时')
  assert.equal(formatDelta(-150), '慢 2 小时 30 分')
})
