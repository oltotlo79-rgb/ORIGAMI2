import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  clearDiagnostics,
  createDiagnostics,
  DIAGNOSTIC_SCOPES,
  MAX_SERIALIZED_DIAGNOSTICS_BYTES,
  REDACTED_DIAGNOSTICS_SCHEMA,
  reportUnexpected,
  serializeDiagnostics,
  snapshotDiagnostics,
  type DiagnosticCountBucket,
  type DiagnosticScope,
  type DiagnosticsSnapshot,
} from '../src/lib/diagnostics.ts'

test('an empty snapshot contains only the fixed schema and canonical scope order', () => {
  const diagnostics = createDiagnostics()
  const snapshot = diagnostics.snapshot()

  assert.deepEqual(snapshot, {
    schema: REDACTED_DIAGNOSTICS_SCHEMA,
    unexpected: DIAGNOSTIC_SCOPES.map((scope) => ({ scope, count: '0' })),
  })
  assert.equal(Object.isFrozen(snapshot), true)
  assert.equal(Object.isFrozen(snapshot.unexpected), true)
  assert.equal(snapshot.unexpected.every(Object.isFrozen), true)
})

test('count buckets have stable boundaries and counters saturate at 65', () => {
  const boundaries: ReadonlyArray<readonly [number, DiagnosticCountBucket]> = [
    [0, '0'],
    [1, '1'],
    [2, '2_4'],
    [4, '2_4'],
    [5, '5_16'],
    [16, '5_16'],
    [17, '17_64'],
    [64, '17_64'],
    [65, '65_plus'],
    [256, '65_plus'],
  ]

  for (const [count, expected] of boundaries) {
    const diagnostics = createDiagnostics()
    for (let index = 0; index < count; index += 1) {
      assert.equal(diagnostics.reportUnexpected('fold_preview.render'), true)
    }
    assert.equal(
      bucketFor(diagnostics.snapshot(), 'fold_preview.render'),
      expected,
    )
  }
})

test('serialization is independent of report order and stays within its byte cap', () => {
  const forward = createDiagnostics()
  const reverse = createDiagnostics()

  for (const scope of DIAGNOSTIC_SCOPES) {
    forward.reportUnexpected(scope)
  }
  for (const scope of [...DIAGNOSTIC_SCOPES].reverse()) {
    reverse.reportUnexpected(scope)
  }
  assert.equal(forward.reportUnexpected('app.validation'), true)
  assert.equal(reverse.reportUnexpected('app.validation'), true)

  assert.equal(forward.serialize(), reverse.serialize())
  const serialized = forward.serialize()
  assert.ok(Buffer.byteLength(serialized, 'utf8') <= MAX_SERIALIZED_DIAGNOSTICS_BYTES)
  assert.deepEqual(
    (JSON.parse(serialized) as DiagnosticsSnapshot).unexpected.map(
      ({ scope }) => scope,
    ),
    DIAGNOSTIC_SCOPES,
  )
})

test('invalid primitive and hostile object scopes are rejected without coercion', () => {
  const diagnostics = createDiagnostics()
  const accesses: string[] = []
  const trap = (name: string): never => {
    accesses.push(name)
    throw new Error(`unexpected ${name} trap`)
  }
  const hostile = new Proxy(Object.create(null) as object, {
    get: () => trap('get'),
    getOwnPropertyDescriptor: () => trap('getOwnPropertyDescriptor'),
    getPrototypeOf: () => trap('getPrototypeOf'),
    has: () => trap('has'),
    ownKeys: () => trap('ownKeys'),
  })
  const coercive = {
    toString: () => trap('toString'),
    valueOf: () => trap('valueOf'),
    [Symbol.toPrimitive]: () => trap('toPrimitive'),
  }
  const report = diagnostics.reportUnexpected as unknown as (
    scope: unknown,
  ) => boolean

  assert.equal(report(hostile), false)
  assert.equal(report(coercive), false)
  assert.equal(report(Symbol('fold_preview.render')), false)
  assert.equal(report(1), false)
  assert.equal(report(null), false)
  assert.equal(report('fold_preview.render '), false)
  assert.deepEqual(accesses, [])
  assert.equal(
    diagnostics.snapshot().unexpected.every(({ count }) => count === '0'),
    true,
  )
})

test('raw paths, project content, UUIDs, coordinates, and errors cannot enter output', () => {
  const diagnostics = createDiagnostics()
  const secrets = [
    String.raw`C:\Users\alice\作品\dragon.ori2`,
    '/Users/alice/Documents/private-fold.ori2',
    'private-project-name',
    '123e4567-e89b-12d3-a456-426614174000',
    'x=12.345,y=-67.89,z=4.2',
    'vertex-secret-id',
  ] as const
  const looseReport = diagnostics.reportUnexpected as unknown as (
    ...values: unknown[]
  ) => boolean

  for (const secret of secrets) assert.equal(looseReport(secret), false)
  assert.equal(
    looseReport(
      'fold_preview.render',
      new Error(`failed at ${secrets[0]} for ${secrets[2]}`),
      { coordinates: secrets[4], projectId: secrets[3] },
    ),
    true,
  )

  const serialized = diagnostics.serialize()
  for (const secret of secrets) {
    assert.equal(serialized.includes(secret), false)
  }
  assert.equal(serialized.includes('failed at'), false)
  assert.equal(serialized.includes('coordinates'), false)
  assert.equal(serialized.includes('projectId'), false)
})

test('clear creates a new zero snapshot without mutating prior snapshots', () => {
  const diagnostics = createDiagnostics()
  diagnostics.reportUnexpected('app.close_guard')
  const beforeClear = diagnostics.snapshot()

  diagnostics.reportUnexpected('app.close_guard')
  diagnostics.clear()
  const afterClear = diagnostics.snapshot()

  assert.equal(bucketFor(beforeClear, 'app.close_guard'), '1')
  assert.equal(bucketFor(afterClear, 'app.close_guard'), '0')
  assert.notEqual(beforeClear, afterClear)
  assert.notEqual(beforeClear.unexpected, afterClear.unexpected)
  assert.throws(() => {
    ;(beforeClear.unexpected as Array<unknown>).push({ scope: 'forged', count: '65_plus' })
  }, TypeError)
  assert.equal(bucketFor(beforeClear, 'app.close_guard'), '1')
})

test('singleton helpers expose the same bounded behavior and can be reset', () => {
  clearDiagnostics()
  try {
    assert.equal(reportUnexpected('app.unhandled_error'), true)
    assert.equal(bucketFor(snapshotDiagnostics(), 'app.unhandled_error'), '1')
    assert.equal(
      serializeDiagnostics(),
      JSON.stringify(snapshotDiagnostics()),
    )
  } finally {
    clearDiagnostics()
  }
  assert.equal(bucketFor(snapshotDiagnostics(), 'app.unhandled_error'), '0')
})

test('the diagnostics source has no side-effect or entropy-producing dependencies', () => {
  const source = readFileSync(
    new URL('../src/lib/diagnostics.ts', import.meta.url),
    'utf8',
  )
  const forbidden: ReadonlyArray<readonly [string, RegExp]> = [
    ['React', /\breact\b/iu],
    ['Tauri', /\btauri\b/iu],
    ['network API', /\b(?:fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon)\b/u],
    ['persistent API', /\b(?:localStorage|sessionStorage|indexedDB|CacheStorage|caches)\b/u],
    ['console', /\bconsole\b/u],
    ['Date', /\bDate\b/u],
    ['performance', /\bperformance\b/u],
    ['random source', /\b(?:randomUUID|getRandomValues)\b|\bMath\s*\.\s*random\b/u],
  ]

  assert.doesNotMatch(source, /^\s*import\s/mu)
  for (const [label, pattern] of forbidden) {
    assert.doesNotMatch(source, pattern, label)
  }
})

function bucketFor(
  snapshot: DiagnosticsSnapshot,
  scope: DiagnosticScope,
) {
  const entry = snapshot.unexpected.find((candidate) => candidate.scope === scope)
  assert.ok(entry)
  return entry.count
}
