import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const rust = readFileSync('src-tauri/src/stacked_fold_read.rs', 'utf8')
const typescript = readFileSync('src/lib/coreClient.ts', 'utf8')
const corpusBytes = readFileSync('tests/fixtures/tauri-event-v1-corpus.json')
const corpusText = corpusBytes.toString('utf8')
const corpus = JSON.parse(corpusText) as Record<string, Record<string, unknown>>

const schemas = {
  'current-cycle-pose-progress-v1': {
    rust: 'CurrentCyclePoseProgressDtoV1', ts: 'CurrentCyclePoseProgressV1',
    keys: ['authorizesProjectMutation', 'completedWork', 'requestId', 'status', 'totalWork', 'version'],
  },
  'stacked-fold-read-progress-v1': {
    rust: 'StackedFoldReadProgressDtoV1', ts: 'StackedFoldReadProgressV1',
    keys: ['authorizesProjectMutation', 'evaluatedTransitionCount', 'exploredStateCount', 'requestId', 'stateLimit', 'transitionLimit', 'version'],
  },
} as const

test('canonical corpus has a bounded duplicate-free byte identity', () => {
  assert.ok(corpusBytes.byteLength > 0 && corpusBytes.byteLength <= 2048)
  assert.equal(
    createHash('sha256').update(corpusBytes).digest('hex'),
    '1551d586dca51d18fbfe3c413c568e7a0d3f69838565d78b4f247d70c4ffd7bb',
  )
  const topLevelKeys = [...corpusText.matchAll(/^  "([^"]+)": \{/gmu)].map((match) => match[1])
  assert.equal(new Set(topLevelKeys).size, topLevelKeys.length)
  for (const event of topLevelKeys) {
    const start = corpusText.indexOf(`  "${event}": {`)
    const end = corpusText.indexOf('\n  }', start)
    assert.ok(start >= 0 && end > start)
    const keys = [...corpusText.slice(start, end).matchAll(/^    "([^"]+)":/gmu)].map((match) => match[1])
    assert.equal(new Set(keys).size, keys.length, `${event}: duplicate JSON key`)
  }
})

test('the v1 corpus cannot silently become a migration or mixed-version fixture', () => {
  for (const [event, payload] of Object.entries(corpus)) {
    assert.match(event, /-v1$/u)
    assert.equal(payload.version, 1)
    assert.equal(Object.hasOwn(payload, 'versionV2'), false)
  }
  assert.doesNotMatch(corpusText, /(?:-v2"|"version"\s*:\s*(?:0|[2-9][0-9]*))/u)
})

test('Rust DTOs TypeScript types and canonical corpus have identical camelCase fields', () => {
  assert.deepEqual(Object.keys(corpus).sort(), Object.keys(schemas).sort())
  for (const [event, schema] of Object.entries(schemas)) {
    const rustBody = rustStruct(schema.rust)
    const tsBody = tsType(schema.ts)
    assert.match(rust.slice(Math.max(0, rust.indexOf(`struct ${schema.rust}`) - 100), rust.indexOf(`struct ${schema.rust}`)), /#\[serde\(rename_all = "camelCase"\)\]/u)
    const rustKeys = [...rustBody.matchAll(/^\s+([a-z][a-z0-9_]*):/gmu)].map((match) => camelCase(match[1])).sort()
    const tsKeys = [...tsBody.matchAll(/^\s+([a-z][A-Za-z0-9]*):/gmu)].map((match) => match[1]).sort()
    assert.deepEqual(rustKeys, schema.keys, `${event}: Rust schema`)
    assert.deepEqual(tsKeys, schema.keys, `${event}: TypeScript schema`)
    assert.deepEqual(Object.keys(corpus[event]).sort(), schema.keys, `${event}: corpus schema`)
  }
})

test('canonical corpus pins version constants limits and mutation authority in both languages', () => {
  const cycle = corpus['current-cycle-pose-progress-v1']
  const stacked = corpus['stacked-fold-read-progress-v1']
  assert.equal(cycle.version, 1); assert.equal(cycle.totalWork, 2)
  assert.equal(stacked.version, 1); assert.equal(stacked.stateLimit, 32); assert.equal(stacked.transitionLimit, 64)
  assert.equal(cycle.authorizesProjectMutation, false); assert.equal(stacked.authorizesProjectMutation, false)
  assert.match(typescript, /value\.version !== 1/u)
  assert.match(typescript, /value\.totalWork !== 2/u)
  assert.match(typescript, /value\.stateLimit !== 32/u)
  assert.match(typescript, /value\.transitionLimit !== 64/u)
  assert.match(rust, /total_work: 2/u)
  assert.match(rust, /state_limit: 32/u)
  assert.match(rust, /transition_limit: 64/u)
})

test('frontend admission rejects unknown fields and version drift from the Rust corpus', () => {
  const cycleListener = functionBody('listenCurrentCyclePoseProgressV1')
  const stackedListener = functionBody('listenStackedFoldReadProgressV1')
  assert.match(cycleListener, /Object\.keys\(value\)\.sort\(\)\.join\(','\) !==/u)
  assert.match(cycleListener, /value\.version !== 1/u)
  assert.match(stackedListener, /Object\.keys\(value\)\.length !== 7/u)
  assert.match(stackedListener, /value\.version !== 1/u)
  for (const [event, value] of Object.entries(corpus)) {
    const drifted = { ...value, version: 2, unknownField: true }
    assert.notDeepEqual(Object.keys(drifted).sort(), schemas[event as keyof typeof schemas].keys)
  }
})

function rustStruct(name: string): string {
  const start = rust.indexOf(`struct ${name} {`); assert.notEqual(start, -1)
  return rust.slice(start, rust.indexOf('\n}', start))
}
function tsType(name: string): string {
  const start = typescript.indexOf(`export type ${name} = Readonly<{`); assert.notEqual(start, -1)
  return typescript.slice(start, typescript.indexOf('\n}>', start))
}
function functionBody(name: string): string {
  const start = typescript.indexOf(`export function ${name}(`); assert.notEqual(start, -1)
  return typescript.slice(start, typescript.indexOf('\n}\n', start))
}
function camelCase(value: string): string {
  return value.replace(/_([a-z])/gu, (_match, letter: string) => letter.toUpperCase())
}
