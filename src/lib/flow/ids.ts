export function makeId(prefix: string) {
  const rand = Math.random().toString(36).slice(2, 10)
  return `${prefix}-${rand}`
}
