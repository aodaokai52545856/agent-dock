/**
 * World clock driven by explicit UTC offsets.
 * Local civil time is UTC plus an offset in minutes. Where a place observes
 * daylight saving, the rule below switches between two fixed offsets.
 * This does not consult the IANA time zone database.
 *
 * Morocco is stored as UTC+1 year round; Ramadan suspensions are not modeled.
 * Urumqi uses China civil time (UTC+8), the same as Beijing.
 */

export type ContinentId =
  | 'asia'
  | 'europe'
  | 'africa'
  | 'north-america'
  | 'south-america'
  | 'oceania'
  | 'antarctica'

export type DstKind = 'none' | 'us' | 'eu' | 'au' | 'nz' | 'chatham' | 'cl' | 'eg' | 'il' | 'lh' | 'cu'

export type ClockBrowse = 'featured' | 'continent' | 'country'

export type ClockCity = {
  id: string
  city: string
  cityEn: string
  country: string
  countryEn: string
  continent: ContinentId
  /** Standard-time offset east of UTC, in minutes. West is negative. */
  offsetMinutes: number
  dst: DstKind
  tags?: string[]
}

export type ClockGroup = {
  id: string
  label: string
  cities: ClockCity[]
}

export type ClockStorage = {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export type ZonedClock = {
  year: number
  month: number
  day: number
  hour: number
  minute: number
  second: number
  weekday: number
}

export type ClockFace = {
  offsetMinutes: number
  daylight: boolean
  seasonLabel: '' | '夏令时' | '标准时'
  hm: string
  hms: string
  dateLine: string
  offsetLabel: string
}

export const CLOCK_CITY_KEY = 'ad-clock-city'
export const CLOCK_FEATURED_KEY = 'ad-clock-featured'

export const CONTINENTS: { id: ContinentId; label: string }[] = [
  { id: 'asia', label: '亚洲' },
  { id: 'europe', label: '欧洲' },
  { id: 'africa', label: '非洲' },
  { id: 'north-america', label: '北美洲' },
  { id: 'south-america', label: '南美洲' },
  { id: 'oceania', label: '大洋洲' },
  { id: 'antarctica', label: '南极洲' }
]

/** Curated default. A saved list replaces this until the user restores it. */
export const DEFAULT_FEATURED_IDS = [
  'beijing',
  'shanghai',
  'hong-kong',
  'taipei',
  'tokyo',
  'seoul',
  'singapore',
  'delhi',
  'dubai',
  'moscow',
  'paris',
  'london',
  'new-york',
  'los-angeles',
  'sydney',
  'auckland'
] as const

const DST_DELTA: Record<DstKind, number> = {
  none: 0,
  us: 60,
  eu: 60,
  au: 60,
  nz: 60,
  chatham: 60,
  cl: 60,
  eg: 60,
  il: 60,
  lh: 30,
  cu: 60
}

const WEEKDAYS = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']

const CN_HOUR: Record<number, string> = {
  1: '一',
  2: '二',
  3: '三',
  4: '四',
  5: '五',
  6: '六',
  7: '七',
  8: '八',
  9: '九',
  10: '十',
  11: '十一',
  12: '十二',
  13: '十三',
  14: '十四'
}

const collator = new Intl.Collator('zh-CN')

function c(
  id: string,
  city: string,
  cityEn: string,
  country: string,
  countryEn: string,
  continent: ContinentId,
  offsetMinutes: number,
  dst: DstKind,
  tags?: string
): ClockCity {
  return {
    id,
    city,
    cityEn,
    country,
    countryEn,
    continent,
    offsetMinutes,
    dst,
    tags: tags ? tags.split(/\s+/) : undefined
  }
}

export const CLOCK_CITIES: readonly ClockCity[] = [
  c('pago-pago', '帕果帕果', 'Pago Pago', '美属萨摩亚', 'American Samoa', 'oceania', -660, 'none'),
  c('honolulu', '火奴鲁鲁', 'Honolulu', '美国', 'United States', 'north-america', -600, 'none', '夏威夷 Hawaii'),
  c('adak', '埃达克', 'Adak', '美国', 'United States', 'north-america', -600, 'us', '阿留申 Aleutian'),
  c('nuku-hiva', '努库希瓦', 'Nuku Hiva', '法属波利尼西亚', 'French Polynesia', 'oceania', -570, 'none', '马克萨斯 Marquesas'),
  c('anchorage', '安克雷奇', 'Anchorage', '美国', 'United States', 'north-america', -540, 'us', '阿拉斯加 Alaska'),
  c('los-angeles', '洛杉矶', 'Los Angeles', '美国', 'United States', 'north-america', -480, 'us', '加州 LA'),
  c('san-francisco', '旧金山', 'San Francisco', '美国', 'United States', 'north-america', -480, 'us', '三藩'),
  c('seattle', '西雅图', 'Seattle', '美国', 'United States', 'north-america', -480, 'us'),
  c('vancouver', '温哥华', 'Vancouver', '加拿大', 'Canada', 'north-america', -480, 'us'),
  c('tijuana', '蒂华纳', 'Tijuana', '墨西哥', 'Mexico', 'north-america', -480, 'us'),
  c('denver', '丹佛', 'Denver', '美国', 'United States', 'north-america', -420, 'us', '山地'),
  c('phoenix', '凤凰城', 'Phoenix', '美国', 'United States', 'north-america', -420, 'none', '亚利桑那 Arizona'),
  c('edmonton', '埃德蒙顿', 'Edmonton', '加拿大', 'Canada', 'north-america', -420, 'us'),
  c('whitehorse', '怀特霍斯', 'Whitehorse', '加拿大', 'Canada', 'north-america', -420, 'none', '育空 Yukon'),
  c('hermosillo', '埃莫西约', 'Hermosillo', '墨西哥', 'Mexico', 'north-america', -420, 'none', '索诺拉'),
  c('chicago', '芝加哥', 'Chicago', '美国', 'United States', 'north-america', -360, 'us'),
  c('mexico-city', '墨西哥城', 'Mexico City', '墨西哥', 'Mexico', 'north-america', -360, 'none'),
  c('winnipeg', '温尼伯', 'Winnipeg', '加拿大', 'Canada', 'north-america', -360, 'us'),
  c('regina', '里贾纳', 'Regina', '加拿大', 'Canada', 'north-america', -360, 'none', '萨斯喀彻温 Saskatchewan'),
  c('galapagos', '加拉帕戈斯', 'Galapagos', '厄瓜多尔', 'Ecuador', 'south-america', -360, 'none'),
  c('hanga-roa', '汉加罗阿', 'Hanga Roa', '智利', 'Chile', 'oceania', -360, 'cl', '复活节岛 Easter'),
  c('new-york', '纽约', 'New York', '美国', 'United States', 'north-america', -300, 'us', 'NYC'),
  c('washington', '华盛顿', 'Washington', '美国', 'United States', 'north-america', -300, 'us', 'DC'),
  c('toronto', '多伦多', 'Toronto', '加拿大', 'Canada', 'north-america', -300, 'us'),
  c('bogota', '波哥大', 'Bogota', '哥伦比亚', 'Colombia', 'south-america', -300, 'none'),
  c('lima', '利马', 'Lima', '秘鲁', 'Peru', 'south-america', -300, 'none'),
  c('havana', '哈瓦那', 'Havana', '古巴', 'Cuba', 'north-america', -300, 'cu'),
  c('rio-branco', '里约布兰科', 'Rio Branco', '巴西', 'Brazil', 'south-america', -300, 'none', '阿克里'),
  c('kingston', '金斯敦', 'Kingston', '牙买加', 'Jamaica', 'north-america', -300, 'none'),
  c('st-johns', '圣约翰斯', 'St. Johns', '加拿大', 'Canada', 'north-america', -210, 'us', '纽芬兰 Newfoundland'),
  c('halifax', '哈利法克斯', 'Halifax', '加拿大', 'Canada', 'north-america', -240, 'us', '大西洋'),
  c('caracas', '加拉加斯', 'Caracas', '委内瑞拉', 'Venezuela', 'south-america', -240, 'none'),
  c('la-paz', '拉巴斯', 'La Paz', '玻利维亚', 'Bolivia', 'south-america', -240, 'none'),
  c('santiago', '圣地亚哥', 'Santiago', '智利', 'Chile', 'south-america', -240, 'cl'),
  c('san-juan', '圣胡安', 'San Juan', '波多黎各', 'Puerto Rico', 'north-america', -240, 'none'),
  c('manaus', '马瑙斯', 'Manaus', '巴西', 'Brazil', 'south-america', -240, 'none', '亚马孙'),
  c('punta-arenas', '蓬塔阿雷纳斯', 'Punta Arenas', '智利', 'Chile', 'south-america', -180, 'none', '麦哲伦'),
  c('buenos-aires', '布宜诺斯艾利斯', 'Buenos Aires', '阿根廷', 'Argentina', 'south-america', -180, 'none'),
  c('sao-paulo', '圣保罗', 'Sao Paulo', '巴西', 'Brazil', 'south-america', -180, 'none'),
  c('montevideo', '蒙得维的亚', 'Montevideo', '乌拉圭', 'Uruguay', 'south-america', -180, 'none'),
  c('cayenne', '卡宴', 'Cayenne', '法属圭亚那', 'French Guiana', 'south-america', -180, 'none'),
  c('noronha', '诺罗尼亚', 'Fernando de Noronha', '巴西', 'Brazil', 'south-america', -120, 'none', '费尔南多'),
  c('azores', '蓬塔德尔加达', 'Ponta Delgada', '葡萄牙', 'Portugal', 'europe', -60, 'eu', '亚速尔 Azores'),
  c('praia', '普拉亚', 'Praia', '佛得角', 'Cabo Verde', 'africa', -60, 'none'),
  c('reykjavik', '雷克雅未克', 'Reykjavik', '冰岛', 'Iceland', 'europe', 0, 'none'),
  c('london', '伦敦', 'London', '英国', 'United Kingdom', 'europe', 0, 'eu'),
  c('lisbon', '里斯本', 'Lisbon', '葡萄牙', 'Portugal', 'europe', 0, 'eu'),
  c('accra', '阿克拉', 'Accra', '加纳', 'Ghana', 'africa', 0, 'none'),
  c('dakar', '达喀尔', 'Dakar', '塞内加尔', 'Senegal', 'africa', 0, 'none'),
  c('lagos', '拉各斯', 'Lagos', '尼日利亚', 'Nigeria', 'africa', 60, 'none'),
  c('paris', '巴黎', 'Paris', '法国', 'France', 'europe', 60, 'eu'),
  c('berlin', '柏林', 'Berlin', '德国', 'Germany', 'europe', 60, 'eu'),
  c('rome', '罗马', 'Rome', '意大利', 'Italy', 'europe', 60, 'eu'),
  c('madrid', '马德里', 'Madrid', '西班牙', 'Spain', 'europe', 60, 'eu'),
  c('amsterdam', '阿姆斯特丹', 'Amsterdam', '荷兰', 'Netherlands', 'europe', 60, 'eu'),
  c('zurich', '苏黎世', 'Zurich', '瑞士', 'Switzerland', 'europe', 60, 'eu'),
  c('stockholm', '斯德哥尔摩', 'Stockholm', '瑞典', 'Sweden', 'europe', 60, 'eu'),
  c('warsaw', '华沙', 'Warsaw', '波兰', 'Poland', 'europe', 60, 'eu'),
  c('algiers', '阿尔及尔', 'Algiers', '阿尔及利亚', 'Algeria', 'africa', 60, 'none'),
  c('casablanca', '卡萨布兰卡', 'Casablanca', '摩洛哥', 'Morocco', 'africa', 60, 'none'),
  c('tunis', '突尼斯', 'Tunis', '突尼斯', 'Tunisia', 'africa', 60, 'none'),
  c('kinshasa', '金沙萨', 'Kinshasa', '刚果（金）', 'DR Congo', 'africa', 60, 'none', '民主刚果'),
  c('cairo', '开罗', 'Cairo', '埃及', 'Egypt', 'africa', 120, 'eg'),
  c('johannesburg', '约翰内斯堡', 'Johannesburg', '南非', 'South Africa', 'africa', 120, 'none'),
  c('athens', '雅典', 'Athens', '希腊', 'Greece', 'europe', 120, 'eu'),
  c('helsinki', '赫尔辛基', 'Helsinki', '芬兰', 'Finland', 'europe', 120, 'eu'),
  c('kyiv', '基辅', 'Kyiv', '乌克兰', 'Ukraine', 'europe', 120, 'eu'),
  c('bucharest', '布加勒斯特', 'Bucharest', '罗马尼亚', 'Romania', 'europe', 120, 'eu'),
  c('jerusalem', '耶路撒冷', 'Jerusalem', '以色列', 'Israel', 'asia', 120, 'il'),
  c('kaliningrad', '加里宁格勒', 'Kaliningrad', '俄罗斯', 'Russia', 'europe', 120, 'none'),
  c('moscow', '莫斯科', 'Moscow', '俄罗斯', 'Russia', 'europe', 180, 'none'),
  c('istanbul', '伊斯坦布尔', 'Istanbul', '土耳其', 'Turkey', 'asia', 180, 'none'),
  c('riyadh', '利雅得', 'Riyadh', '沙特阿拉伯', 'Saudi Arabia', 'asia', 180, 'none'),
  c('nairobi', '内罗毕', 'Nairobi', '肯尼亚', 'Kenya', 'africa', 180, 'none'),
  c('doha', '多哈', 'Doha', '卡塔尔', 'Qatar', 'asia', 180, 'none'),
  c('baghdad', '巴格达', 'Baghdad', '伊拉克', 'Iraq', 'asia', 180, 'none'),
  c('addis-ababa', '亚的斯亚贝巴', 'Addis Ababa', '埃塞俄比亚', 'Ethiopia', 'africa', 180, 'none'),
  c('tehran', '德黑兰', 'Tehran', '伊朗', 'Iran', 'asia', 210, 'none'),
  c('dubai', '迪拜', 'Dubai', '阿联酋', 'United Arab Emirates', 'asia', 240, 'none'),
  c('abu-dhabi', '阿布扎比', 'Abu Dhabi', '阿联酋', 'United Arab Emirates', 'asia', 240, 'none'),
  c('baku', '巴库', 'Baku', '阿塞拜疆', 'Azerbaijan', 'asia', 240, 'none'),
  c('samara', '萨马拉', 'Samara', '俄罗斯', 'Russia', 'europe', 240, 'none'),
  c('tbilisi', '第比利斯', 'Tbilisi', '格鲁吉亚', 'Georgia', 'asia', 240, 'none'),
  c('port-louis', '路易港', 'Port Louis', '毛里求斯', 'Mauritius', 'africa', 240, 'none'),
  c('kabul', '喀布尔', 'Kabul', '阿富汗', 'Afghanistan', 'asia', 270, 'none'),
  c('karachi', '卡拉奇', 'Karachi', '巴基斯坦', 'Pakistan', 'asia', 300, 'none'),
  c('tashkent', '塔什干', 'Tashkent', '乌兹别克斯坦', 'Uzbekistan', 'asia', 300, 'none'),
  c('yekaterinburg', '叶卡捷琳堡', 'Yekaterinburg', '俄罗斯', 'Russia', 'asia', 300, 'none'),
  c('astana', '阿斯塔纳', 'Astana', '哈萨克斯坦', 'Kazakhstan', 'asia', 300, 'none', '努尔苏丹'),
  c('almaty', '阿拉木图', 'Almaty', '哈萨克斯坦', 'Kazakhstan', 'asia', 300, 'none'),
  c('male', '马累', 'Male', '马尔代夫', 'Maldives', 'asia', 300, 'none'),
  c('delhi', '新德里', 'New Delhi', '印度', 'India', 'asia', 330, 'none', '德里'),
  c('mumbai', '孟买', 'Mumbai', '印度', 'India', 'asia', 330, 'none'),
  c('colombo', '科伦坡', 'Colombo', '斯里兰卡', 'Sri Lanka', 'asia', 330, 'none'),
  c('kathmandu', '加德满都', 'Kathmandu', '尼泊尔', 'Nepal', 'asia', 345, 'none'),
  c('dhaka', '达卡', 'Dhaka', '孟加拉国', 'Bangladesh', 'asia', 360, 'none'),
  c('omsk', '鄂木斯克', 'Omsk', '俄罗斯', 'Russia', 'asia', 360, 'none'),
  c('bishkek', '比什凯克', 'Bishkek', '吉尔吉斯斯坦', 'Kyrgyzstan', 'asia', 360, 'none'),
  c('thimphu', '廷布', 'Thimphu', '不丹', 'Bhutan', 'asia', 360, 'none'),
  c('yangon', '仰光', 'Yangon', '缅甸', 'Myanmar', 'asia', 390, 'none'),
  c('bangkok', '曼谷', 'Bangkok', '泰国', 'Thailand', 'asia', 420, 'none'),
  c('jakarta', '雅加达', 'Jakarta', '印度尼西亚', 'Indonesia', 'asia', 420, 'none'),
  c('ho-chi-minh', '胡志明市', 'Ho Chi Minh City', '越南', 'Vietnam', 'asia', 420, 'none', '西贡'),
  c('hanoi', '河内', 'Hanoi', '越南', 'Vietnam', 'asia', 420, 'none'),
  c('krasnoyarsk', '克拉斯诺亚尔斯克', 'Krasnoyarsk', '俄罗斯', 'Russia', 'asia', 420, 'none'),
  c('khovd', '科布多', 'Khovd', '蒙古', 'Mongolia', 'asia', 420, 'none'),
  c('beijing', '北京', 'Beijing', '中国', 'China', 'asia', 480, 'none', '北平'),
  c('shanghai', '上海', 'Shanghai', '中国', 'China', 'asia', 480, 'none'),
  c('guangzhou', '广州', 'Guangzhou', '中国', 'China', 'asia', 480, 'none'),
  c('shenzhen', '深圳', 'Shenzhen', '中国', 'China', 'asia', 480, 'none'),
  c('chengdu', '成都', 'Chengdu', '中国', 'China', 'asia', 480, 'none'),
  c('urumqi', '乌鲁木齐', 'Urumqi', '中国', 'China', 'asia', 480, 'none', '新疆 北京时间'),
  c('hong-kong', '香港', 'Hong Kong', '中国', 'China', 'asia', 480, 'none', '香港特别行政区'),
  c('taipei', '台北', 'Taipei', '中国', 'China', 'asia', 480, 'none', '台湾 臺灣'),
  c('singapore', '新加坡', 'Singapore', '新加坡', 'Singapore', 'asia', 480, 'none'),
  c('kuala-lumpur', '吉隆坡', 'Kuala Lumpur', '马来西亚', 'Malaysia', 'asia', 480, 'none'),
  c('manila', '马尼拉', 'Manila', '菲律宾', 'Philippines', 'asia', 480, 'none'),
  c('perth', '珀斯', 'Perth', '澳大利亚', 'Australia', 'oceania', 480, 'none'),
  c('denpasar', '登巴萨', 'Denpasar', '印度尼西亚', 'Indonesia', 'asia', 480, 'none', '巴厘岛 Bali'),
  c('ulaanbaatar', '乌兰巴托', 'Ulaanbaatar', '蒙古', 'Mongolia', 'asia', 480, 'none'),
  c('irkutsk', '伊尔库茨克', 'Irkutsk', '俄罗斯', 'Russia', 'asia', 480, 'none'),
  c('eucla', '尤克拉', 'Eucla', '澳大利亚', 'Australia', 'oceania', 525, 'none'),
  c('tokyo', '东京', 'Tokyo', '日本', 'Japan', 'asia', 540, 'none'),
  c('seoul', '首尔', 'Seoul', '韩国', 'South Korea', 'asia', 540, 'none'),
  c('pyongyang', '平壤', 'Pyongyang', '朝鲜', 'North Korea', 'asia', 540, 'none'),
  c('yakutsk', '雅库茨克', 'Yakutsk', '俄罗斯', 'Russia', 'asia', 540, 'none'),
  c('adelaide', '阿德莱德', 'Adelaide', '澳大利亚', 'Australia', 'oceania', 570, 'au'),
  c('darwin', '达尔文', 'Darwin', '澳大利亚', 'Australia', 'oceania', 570, 'none'),
  c('brisbane', '布里斯班', 'Brisbane', '澳大利亚', 'Australia', 'oceania', 600, 'none'),
  c('sydney', '悉尼', 'Sydney', '澳大利亚', 'Australia', 'oceania', 600, 'au'),
  c('melbourne', '墨尔本', 'Melbourne', '澳大利亚', 'Australia', 'oceania', 600, 'au'),
  c('vladivostok', '符拉迪沃斯托克', 'Vladivostok', '俄罗斯', 'Russia', 'asia', 600, 'none', '海参崴'),
  c('hagatna', '阿加尼亚', 'Hagatna', '关岛', 'Guam', 'oceania', 600, 'none'),
  c('lord-howe', '豪勋爵岛', 'Lord Howe', '澳大利亚', 'Australia', 'oceania', 630, 'lh'),
  c('noumea', '努美阿', 'Noumea', '新喀里多尼亚', 'New Caledonia', 'oceania', 660, 'none'),
  c('honiara', '霍尼亚拉', 'Honiara', '所罗门群岛', 'Solomon Islands', 'oceania', 660, 'none'),
  c('magadan', '马加丹', 'Magadan', '俄罗斯', 'Russia', 'asia', 660, 'none'),
  c('norfolk', '诺福克岛', 'Norfolk Island', '澳大利亚', 'Australia', 'oceania', 660, 'au'),
  c('auckland', '奥克兰', 'Auckland', '新西兰', 'New Zealand', 'oceania', 720, 'nz'),
  c('suva', '苏瓦', 'Suva', '斐济', 'Fiji', 'oceania', 720, 'none'),
  c('petropavlovsk', '彼得罗巴甫洛夫斯克', 'Petropavlovsk-Kamchatsky', '俄罗斯', 'Russia', 'asia', 720, 'none', '堪察加'),
  c('chatham', '查塔姆', 'Chatham', '新西兰', 'New Zealand', 'oceania', 765, 'chatham', '查塔姆群岛'),
  c('apia', '阿皮亚', 'Apia', '萨摩亚', 'Samoa', 'oceania', 780, 'none'),
  c('nukualofa', '努库阿洛法', 'Nukualofa', '汤加', 'Tonga', 'oceania', 780, 'none'),
  c('kiritimati', '基里蒂马蒂', 'Kiritimati', '基里巴斯', 'Kiribati', 'oceania', 840, 'none', '莱恩群岛'),
  c('mcmurdo', '麦克默多', 'McMurdo', '南极洲', 'Antarctica', 'antarctica', 720, 'nz', '南极')
]

const CITY_BY_ID = new Map<string, ClockCity>(CLOCK_CITIES.map((city) => [city.id, city]))

const CONTINENT_LABEL = new Map(CONTINENTS.map((item) => [item.id, item.label]))

function continentLabel(id: ContinentId) {
  return CONTINENT_LABEL.get(id) ?? id
}

function fold(text: string) {
  return text
    .trim()
    .toLowerCase()
    .replace(/＋/g, '+')
    .replace(/－/g, '-')
    .replace(/：/g, ':')
    .replace(/\s+/g, ' ')
}

function zoneWords(minutes: number) {
  if (minutes === 0) return ['零时区', '中时区']
  if (minutes % 60 !== 0) return []
  const label = CN_HOUR[Math.abs(minutes) / 60]
  if (!label) return []
  return [`${minutes > 0 ? '东' : '西'}${label}区`]
}

function offsetWords(minutes: number) {
  const sign = minutes < 0 ? '-' : '+'
  const abs = Math.abs(minutes)
  const hour = Math.floor(abs / 60)
  const minute = abs % 60
  const hh = String(hour).padStart(2, '0')
  const mm = String(minute).padStart(2, '0')
  const words = [`utc${sign}${hour}`, `utc${sign}${hh}`, `gmt${sign}${hour}`, `gmt${sign}${hh}`, `${sign}${hour}`, `${sign}${hh}`]
  if (minute) {
    words.push(
      `utc${sign}${hour}:${mm}`,
      `utc${sign}${hh}:${mm}`,
      `gmt${sign}${hour}:${mm}`,
      `${sign}${hour}:${mm}`,
      `${sign}${hh}:${mm}`
    )
  } else {
    words.push(`utc${sign}${hour}:00`, `utc${sign}${hh}:00`, `${sign}${hh}:00`)
  }
  words.push(...zoneWords(minutes))
  return words
}

function buildHaystack(city: ClockCity) {
  const parts = [
    city.city,
    city.cityEn,
    city.country,
    city.countryEn,
    continentLabel(city.continent),
    ...(city.tags ?? []),
    ...offsetWords(city.offsetMinutes),
    ...offsetWords(city.offsetMinutes + DST_DELTA[city.dst])
  ]
  return fold(parts.join(' '))
}

const SEARCH = new Map(CLOCK_CITIES.map((city) => [city.id, buildHaystack(city)]))

const seenIds = new Set<string>()
for (const city of CLOCK_CITIES) {
  if (seenIds.has(city.id)) throw new Error(`duplicate clock city ${city.id}`)
  seenIds.add(city.id)
}
for (const id of DEFAULT_FEATURED_IDS) {
  if (!CITY_BY_ID.has(id)) throw new Error(`missing featured city ${id}`)
}

export function cityById(id: string) {
  return CITY_BY_ID.get(id)
}

export function continentName(id: ContinentId) {
  return continentLabel(id)
}

/** weekday: 0 Sunday … 6 Saturday. n is 1-based. month is 0-based. */
export function nthWeekday(year: number, month: number, weekday: number, n: number) {
  const first = new Date(Date.UTC(year, month, 1)).getUTCDay()
  return 1 + ((weekday - first + 7) % 7) + (n - 1) * 7
}

export function lastWeekday(year: number, month: number, weekday: number) {
  const lastDate = new Date(Date.UTC(year, month + 1, 0)).getUTCDate()
  const last = new Date(Date.UTC(year, month, lastDate)).getUTCDay()
  return lastDate - ((last - weekday + 7) % 7)
}

function utcFromLocal(year: number, month: number, day: number, hour: number, minute: number, offsetMinutes: number) {
  return Date.UTC(year, month, day, hour, minute) - offsetMinutes * 60_000
}

function usDst(utcMs: number, standardOffset: number) {
  const year = new Date(utcMs).getUTCFullYear()
  const start = utcFromLocal(year, 2, nthWeekday(year, 2, 0, 2), 2, 0, standardOffset)
  const end = utcFromLocal(year, 10, nthWeekday(year, 10, 0, 1), 2, 0, standardOffset + 60)
  return utcMs >= start && utcMs < end
}

function euDst(utcMs: number) {
  const year = new Date(utcMs).getUTCFullYear()
  const start = Date.UTC(year, 2, lastWeekday(year, 2, 0), 1, 0)
  const end = Date.UTC(year, 9, lastWeekday(year, 9, 0), 1, 0)
  return utcMs >= start && utcMs < end
}

function southernDst(
  utcMs: number,
  standardOffset: number,
  delta: number,
  startMonth: number,
  startDay: number,
  startHour: number,
  startMinute: number,
  endMonth: number,
  endDay: number,
  endHour: number,
  endMinute: number
) {
  const year = new Date(utcMs).getUTCFullYear()
  const start = utcFromLocal(year, startMonth, startDay, startHour, startMinute, standardOffset)
  const end = utcFromLocal(year, endMonth, endDay, endHour, endMinute, standardOffset + delta)
  return utcMs < end || utcMs >= start
}

function isDst(kind: DstKind, utcMs: number, standardOffset: number) {
  const year = new Date(utcMs).getUTCFullYear()
  switch (kind) {
    case 'none':
      return false
    case 'us':
      return usDst(utcMs, standardOffset)
    case 'eu':
      return euDst(utcMs)
    case 'au':
      return southernDst(
        utcMs,
        standardOffset,
        60,
        9,
        nthWeekday(year, 9, 0, 1),
        2,
        0,
        3,
        nthWeekday(year, 3, 0, 1),
        3,
        0
      )
    case 'nz':
      return southernDst(
        utcMs,
        standardOffset,
        60,
        8,
        lastWeekday(year, 8, 0),
        2,
        0,
        3,
        nthWeekday(year, 3, 0, 1),
        3,
        0
      )
    case 'chatham':
      return southernDst(
        utcMs,
        standardOffset,
        60,
        8,
        lastWeekday(year, 8, 0),
        2,
        45,
        3,
        nthWeekday(year, 3, 0, 1),
        3,
        45
      )
    case 'cl': {
      // First Saturday 24:00 local, standard in September and daylight in April.
      const sep = nthWeekday(year, 8, 6, 1) + 1
      const apr = nthWeekday(year, 3, 6, 1) + 1
      return southernDst(utcMs, standardOffset, 60, 8, sep, 0, 0, 3, apr, 0, 0)
    }
    case 'eg': {
      const start = utcFromLocal(year, 3, lastWeekday(year, 3, 5), 0, 0, standardOffset)
      const end = utcFromLocal(year, 9, lastWeekday(year, 9, 4), 24, 0, standardOffset + 60)
      return utcMs >= start && utcMs < end
    }
    case 'il': {
      const start = utcFromLocal(year, 2, lastWeekday(year, 2, 0) - 2, 2, 0, standardOffset)
      const end = utcFromLocal(year, 9, lastWeekday(year, 9, 0), 2, 0, standardOffset + 60)
      return utcMs >= start && utcMs < end
    }
    case 'lh':
      return southernDst(
        utcMs,
        standardOffset,
        30,
        9,
        nthWeekday(year, 9, 0, 1),
        2,
        0,
        3,
        nthWeekday(year, 3, 0, 1),
        2,
        0
      )
    case 'cu': {
      const start = utcFromLocal(year, 2, nthWeekday(year, 2, 0, 2), 0, 0, standardOffset)
      const end = utcFromLocal(year, 10, nthWeekday(year, 10, 0, 1), 1, 0, standardOffset + 60)
      return utcMs >= start && utcMs < end
    }
    default:
      return false
  }
}

export function effectiveOffsetMinutes(city: ClockCity, utcMs: number) {
  if (city.dst === 'none') return city.offsetMinutes
  return city.offsetMinutes + (isDst(city.dst, utcMs, city.offsetMinutes) ? DST_DELTA[city.dst] : 0)
}

export function machineOffsetMinutes(date: Date) {
  return -date.getTimezoneOffset()
}

export function formatOffset(minutes: number) {
  const sign = minutes < 0 ? '-' : '+'
  const abs = Math.abs(minutes)
  const hour = Math.floor(abs / 60)
  const minute = abs % 60
  if (minute === 0) return `UTC${sign}${hour}`
  return `UTC${sign}${hour}:${String(minute).padStart(2, '0')}`
}

export function formatDelta(minutes: number) {
  if (minutes === 0) return '同一时刻'
  const abs = Math.abs(minutes)
  const hour = Math.floor(abs / 60)
  const minute = abs % 60
  const parts: string[] = []
  if (hour) parts.push(`${hour} 小时`)
  if (minute) parts.push(`${minute} 分`)
  return `${minutes > 0 ? '快' : '慢'} ${parts.join(' ')}`
}

export function zonedClock(utcMs: number, offsetMinutes: number): ZonedClock {
  const shifted = new Date(utcMs + offsetMinutes * 60_000)
  return {
    year: shifted.getUTCFullYear(),
    month: shifted.getUTCMonth() + 1,
    day: shifted.getUTCDate(),
    hour: shifted.getUTCHours(),
    minute: shifted.getUTCMinutes(),
    second: shifted.getUTCSeconds(),
    weekday: shifted.getUTCDay()
  }
}

function pad(value: number) {
  return String(value).padStart(2, '0')
}

export function formatHm(clock: ZonedClock, withSeconds = false) {
  const hm = `${pad(clock.hour)}:${pad(clock.minute)}`
  return withSeconds ? `${hm}:${pad(clock.second)}` : hm
}

export function formatDateLine(clock: ZonedClock) {
  return `${clock.year}年${clock.month}月${clock.day}日 ${WEEKDAYS[clock.weekday]}`
}

export function clockFace(city: ClockCity, utcMs: number): ClockFace {
  const offsetMinutes = effectiveOffsetMinutes(city, utcMs)
  const clock = zonedClock(utcMs, offsetMinutes)
  const daylight = city.dst !== 'none' && offsetMinutes !== city.offsetMinutes
  return {
    offsetMinutes,
    daylight,
    seasonLabel: city.dst === 'none' ? '' : daylight ? '夏令时' : '标准时',
    hm: formatHm(clock),
    hms: formatHm(clock, true),
    dateLine: formatDateLine(clock),
    offsetLabel: formatOffset(offsetMinutes)
  }
}

export function defaultFeaturedIds() {
  return [...DEFAULT_FEATURED_IDS]
}

export function featuredCities(customIds: readonly string[] | null) {
  const ids = customIds ?? DEFAULT_FEATURED_IDS
  const cities: ClockCity[] = []
  const seen = new Set<string>()
  for (const id of ids) {
    const city = CITY_BY_ID.get(id)
    if (!city || seen.has(city.id)) continue
    seen.add(city.id)
    cities.push(city)
  }
  return cities
}

function sameIds(left: readonly string[], right: readonly string[]) {
  return left.length === right.length && left.every((id, index) => id === right[index])
}

export function toggleFeatured(current: readonly string[] | null, id: string) {
  if (!CITY_BY_ID.has(id)) return current ? [...current] : null
  const next = current ? [...current] : defaultFeaturedIds()
  const index = next.indexOf(id)
  if (index >= 0) next.splice(index, 1)
  else next.push(id)
  return sameIds(next, DEFAULT_FEATURED_IDS) ? null : next
}

export function filterCities(cities: readonly ClockCity[], query: string) {
  const tokens = fold(query).split(' ').filter(Boolean)
  if (!tokens.length) return [...cities]
  return cities.filter((city) => {
    const hay = SEARCH.get(city.id) ?? ''
    return tokens.every((token) => hay.includes(token))
  })
}

function sortCities(cities: readonly ClockCity[]) {
  return [...cities].sort((left, right) => {
    const byCountry = collator.compare(left.country, right.country)
    if (byCountry) return byCountry
    return collator.compare(left.city, right.city)
  })
}

export function groupCities(cities: readonly ClockCity[], mode: 'continent' | 'country'): ClockGroup[] {
  if (mode === 'continent') {
    const buckets = new Map<ContinentId, ClockCity[]>()
    for (const city of cities) {
      const list = buckets.get(city.continent)
      if (list) list.push(city)
      else buckets.set(city.continent, [city])
    }
    const groups: ClockGroup[] = []
    for (const continent of CONTINENTS) {
      const list = buckets.get(continent.id)
      if (!list?.length) continue
      groups.push({ id: continent.id, label: continent.label, cities: sortCities(list) })
    }
    return groups
  }
  const buckets = new Map<string, ClockCity[]>()
  for (const city of cities) {
    const list = buckets.get(city.country)
    if (list) list.push(city)
    else buckets.set(city.country, [city])
  }
  return [...buckets.entries()]
    .sort((left, right) => collator.compare(left[0], right[0]))
    .map(([country, list]) => ({ id: country, label: country, cities: sortCities(list) }))
}

export function suggestCity(localOffsetMinutes: number, utcMs: number) {
  const featured = featuredCities(null).find((city) => effectiveOffsetMinutes(city, utcMs) === localOffsetMinutes)
  if (featured) return featured
  return (
    CLOCK_CITIES.find((city) => effectiveOffsetMinutes(city, utcMs) === localOffsetMinutes) ??
    CITY_BY_ID.get('beijing')!
  )
}

function readItem(storage: ClockStorage | null | undefined, key: string) {
  if (!storage) return null
  try {
    return storage.getItem(key)
  } catch {
    return null
  }
}

function writeItem(storage: ClockStorage | null | undefined, key: string, value: string) {
  if (!storage) return
  try {
    storage.setItem(key, value)
  } catch {
    /* ignore quota */
  }
}

function removeItem(storage: ClockStorage | null | undefined, key: string) {
  if (!storage) return
  try {
    storage.removeItem(key)
  } catch {
    /* ignore */
  }
}

export function parseSelectedCityId(raw: string | null) {
  if (!raw || !CITY_BY_ID.has(raw)) return null
  return raw
}

export function parseFeaturedIds(raw: string | null) {
  if (raw == null || raw === '') return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (!Array.isArray(parsed)) return null
  const ids: string[] = []
  for (const item of parsed) {
    if (typeof item !== 'string' || !CITY_BY_ID.has(item) || ids.includes(item)) continue
    ids.push(item)
  }
  if (!ids.length && parsed.length) return null
  return ids
}

export function readSelectedCity(storage: ClockStorage | null | undefined, localOffsetMinutes: number, utcMs: number) {
  const id = parseSelectedCityId(readItem(storage, CLOCK_CITY_KEY))
  if (id) return CITY_BY_ID.get(id)!
  return suggestCity(localOffsetMinutes, utcMs)
}

export function writeSelectedCityId(storage: ClockStorage | null | undefined, id: string) {
  if (!CITY_BY_ID.has(id)) return
  writeItem(storage, CLOCK_CITY_KEY, id)
}

export function readFeaturedIds(storage: ClockStorage | null | undefined) {
  return parseFeaturedIds(readItem(storage, CLOCK_FEATURED_KEY))
}

export function writeFeaturedIds(storage: ClockStorage | null | undefined, ids: readonly string[] | null) {
  if (!ids) {
    removeItem(storage, CLOCK_FEATURED_KEY)
    return
  }
  writeItem(storage, CLOCK_FEATURED_KEY, JSON.stringify(ids))
}
