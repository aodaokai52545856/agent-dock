export function sectionBody(text: string, heading: string) {
  const lines = text.split(/\r?\n/)
  const level = /^#+/.exec(heading)?.[0].length ?? 0
  const start = lines.findIndex((line) => line.trim() === heading)
  if (start < 0) return null
  const body: string[] = []
  for (const line of lines.slice(start + 1)) {
    const depth = /^(#+)\s/.exec(line)?.[1].length ?? 0
    if (depth && depth <= level) break
    body.push(line)
  }
  return body.join('\n').trim()
}
