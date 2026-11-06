import assert from 'node:assert/strict'
import { createTermStream } from './termStream.ts'

const events: string[] = []
const parsed: Array<() => void> = []
const stream = createTermStream((data, done) => { events.push(`write:${data}`); parsed.push(done) })
stream.write('old-grid')
let resized: (() => void) | undefined
const resizing = stream.resize(() => new Promise<void>(resolve => { events.push('resize'); resized = resolve }))
stream.write('new-grid')
assert.deepEqual(events, ['write:old-grid'])
parsed.shift()!()
await Promise.resolve()
assert.deepEqual(events, ['write:old-grid', 'resize'])
resized!()
await resizing
assert.deepEqual(events, ['write:old-grid', 'resize', 'write:new-grid'])
parsed.shift()!()
await Promise.resolve()
const failing = stream.resize(() => { throw new Error('closed') })
stream.write('after-failure')
await assert.rejects(failing, /closed/)
assert.equal(events.at(-1), 'write:after-failure')
parsed.shift()!()
await Promise.resolve()
stream.dispose()
stream.write('closed')
await stream.resize(() => { events.push('closed-resize') })
assert.equal(events.at(-1), 'write:after-failure')
console.log('ok terminal output drains before resize, waits for PTY resize, and survives a failed resize')

const callbacks: Array<() => void> = []
const credits: number[] = []
const bounded = createTermStream((_data, done) => callbacks.push(done))
bounded.write('first', () => credits.push(1))
bounded.write('second', () => credits.push(2))
bounded.write('third', () => credits.push(3))
assert.deepEqual(credits, [])
callbacks.shift()!()
await Promise.resolve()
assert.deepEqual(credits, [1])
bounded.dispose()
assert.deepEqual(credits, [1, 2, 3])
callbacks.shift()!() // xterm can finish an already-disposed write later.
assert.deepEqual(credits, [1, 2, 3])
bounded.write('disposed', () => credits.push(4))
assert.deepEqual(credits, [1, 2, 3, 4])
console.log('ok terminal flow credits return only after parsing, and disposal releases each credit once')
