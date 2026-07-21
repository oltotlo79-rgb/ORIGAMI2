import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const panel = readFileSync('src/components/StackedFoldPanel.tsx', 'utf8')
const native = readFileSync('src-tauri/src/stacked_fold_read.rs', 'utf8')
const capability = JSON.parse(readFileSync('src-tauri/capabilities/default.json', 'utf8'))

const allowedEvents = [
  'current-cycle-pose-progress-v1',
  'stacked-fold-read-progress-v1',
]

test('native emitters and frontend listeners share one closed event-name set', () => {
  const listened = [...client.matchAll(/\blisten<unknown>\('([^']+)'/gu)].map((match) => match[1]).sort()
  const emitted = [...native.matchAll(/const [A-Z0-9_]+_EVENT_V1: &str = "([^"]+)";/gu)]
    .map((match) => match[1]).sort()
  assert.deepEqual(listened, allowedEvents)
  assert.deepEqual(emitted, allowedEvents)
  assert.equal(listened.some((name) => name.includes('*')), false)
  assert.equal(emitted.some((name) => name.includes('*')), false)
  assert.deepEqual(capability.permissions.filter((value: string) => value.includes('event:')), [
    'core:event:allow-listen',
    'core:event:allow-unlisten',
  ])
})

test('event payload admission is own-key and size bounded before callbacks', () => {
  const cycle = functionBody(client, 'listenCurrentCyclePoseProgressV1')
  const stacked = functionBody(client, 'listenStackedFoldReadProgressV1')
  for (const body of [cycle, stacked]) {
    assert.match(body, /typeof payload !== 'object'/u)
    assert.match(body, /payload === null/u)
    assert.match(body, /Array\.isArray\(payload\)/u)
    assert.match(body, /Object\.keys\(value\)/u)
    assert.match(body, /value\.requestId\.length > 128/u)
    assert.match(body, /authorizesProjectMutation !== false/u)
    assert.ok(body.indexOf('Object.keys(value)') < body.indexOf('onProgress('))
    assert.doesNotMatch(body, /Object\.assign|Object\.setPrototypeOf|__proto__|\.\.\.payload/u)
  }
  assert.match(cycle, /Object\.keys\(value\)\.sort\(\)\.join\(','\)/u)
  assert.match(stacked, /Object\.keys\(value\)\.length !== 7/u)
  assert.match(stacked, /Number\(value\.exploredStateCount\) > 32/u)
  assert.match(stacked, /Number\(value\.evaluatedTransitionCount\) > 64/u)
})

test('both asynchronous listeners are disposed on late registration and unmount', () => {
  assert.equal((panel.match(/if \(disposed\) value\(\)/gu) ?? []).length, 2)
  assert.equal((panel.match(/unlisten\?\.\(\)/gu) ?? []).length, 2)
  assert.equal((panel.match(/disposed = true/gu) ?? []).length >= 2, true)
})

function functionBody(source: string, name: string): string {
  const start = source.indexOf(`export function ${name}(`)
  assert.notEqual(start, -1)
  const end = source.indexOf('\n}\n', start)
  assert.notEqual(end, -1)
  return source.slice(start, end + 2)
}
