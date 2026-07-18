import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  clearDiagnostics,
  createDiagnostics,
  DIAGNOSTIC_SCOPES,
  snapshotDiagnostics,
  type DiagnosticScope,
} from '../src/lib/diagnostics.ts'
import {
  createDiagnosticsRuntime,
  reportUnexpected,
} from '../src/lib/diagnosticsRuntime.ts'

test('production uses the exact native command and scope-only payload', () => {
  const source = readFileSync(
    new URL('../src/lib/diagnosticsRuntime.ts', import.meta.url),
    'utf8',
  )
  const nativeCall = source.match(
    /invoke\(\s*'record_unexpected_diagnostic'\s*,\s*(\{[^}]*\})\s*\)/u,
  )

  assert.ok(nativeCall)
  assert.equal(nativeCall[1]?.replace(/\s/gu, ''), '{scope}')
  assert.equal(
    source.match(/record_unexpected_diagnostic/gu)?.length,
    1,
  )
  assert.match(
    source,
    /typeof window !== 'undefined'\s*&&\s*'__TAURI_INTERNALS__' in window/u,
  )
})

test('frontend and native v1 diagnostics contracts stay identical', () => {
  const nativeSource = readFileSync(
    new URL('../src-tauri/src/diagnostics.rs', import.meta.url),
    'utf8',
  )
  const scopeBlock = nativeSource.match(
    /pub\(crate\) enum DiagnosticScope \{(?<body>[\s\S]*?)\n\}/u,
  )?.groups?.body
  assert.ok(scopeBlock)
  assert.deepEqual(
    Array.from(
      scopeBlock.matchAll(/#\[serde\(rename = "([^"]+)"\)\]/gu),
      (match) => match[1],
    ),
    [...DIAGNOSTIC_SCOPES],
  )

  const storedShape = nativeSource.match(
    /struct StoredDiagnostics \{(?<body>[\s\S]*?)\n\}/u,
  )?.groups?.body
  assert.ok(storedShape)
  assert.deepEqual(
    Array.from(
      storedShape.matchAll(/^\s+([a-z_]+):/gmu),
      (match) => match[1],
    ),
    ['schema', 'unexpected'],
  )
  assert.match(
    nativeSource,
    /#\[serde\(rename = "origami2\.redacted-diagnostics\.v1"\)\]/u,
  )
  assert.match(
    nativeSource,
    /pub\(crate\) async fn record_unexpected_diagnostic/u,
  )
  assert.match(
    nativeSource,
    /tauri::async_runtime::spawn_blocking/u,
  )
  assert.match(
    nativeSource,
    /persistence_gate\.lock_owned\(\)\.await/u,
  )
})

test('non-native reports update memory without invoking or consuming the native cap', () => {
  const memory = createDiagnostics()
  let available = false
  let memoryCalls = 0
  const nativeScopes: DiagnosticScope[] = []
  const runtime = createDiagnosticsRuntime({
    reportInMemory: (scope) => {
      memoryCalls += 1
      return memory.reportUnexpected(scope)
    },
    isNativeAvailable: () => available,
    recordNative: (scope) => {
      nativeScopes.push(scope)
    },
  })

  for (let index = 0; index < 100; index += 1) {
    assert.equal(runtime.reportUnexpected('fold_preview.render'), true)
  }
  assert.deepEqual(nativeScopes, [])

  available = true
  for (let index = 0; index < 70; index += 1) {
    assert.equal(runtime.reportUnexpected('fold_preview.render'), true)
  }
  assert.equal(nativeScopes.length, 65)
  assert.equal(memoryCalls, 170)
  assert.equal(
    memory.snapshot().unexpected.find(
      ({ scope }) => scope === 'fold_preview.render',
    )?.count,
    '65_plus',
  )
})

test('valid scopes deliver one primitive scope while invalid scopes never deliver', () => {
  const memory = createDiagnostics()
  const nativeArguments: unknown[][] = []
  let availabilityChecks = 0
  const runtime = createDiagnosticsRuntime({
    reportInMemory: memory.reportUnexpected,
    isNativeAvailable: () => {
      availabilityChecks += 1
      return true
    },
    recordNative: ((...values: unknown[]) => {
      nativeArguments.push(values)
    }) as (scope: DiagnosticScope) => unknown,
  })
  const looseReport = runtime.reportUnexpected as unknown as (
    scope: unknown,
  ) => boolean

  assert.equal(looseReport('fold_preview.camera'), true)
  assert.equal(looseReport('fold_preview.camera '), false)
  assert.equal(looseReport('C:\\private\\model.ori2'), false)
  assert.equal(looseReport(null), false)
  assert.deepEqual(nativeArguments, [['fold_preview.camera']])
  assert.equal(availabilityChecks, 1)
})

test('availability and native recorder failures remain synchronous no-throws', () => {
  const memory = createDiagnostics()
  let availabilityThrows = true
  let nativeCalls = 0
  const runtime = createDiagnosticsRuntime({
    reportInMemory: memory.reportUnexpected,
    isNativeAvailable: () => {
      if (availabilityThrows) throw new Error('availability failed')
      return true
    },
    recordNative: () => {
      nativeCalls += 1
      throw new Error('invoke failed')
    },
  })

  assert.doesNotThrow(() =>
    runtime.reportUnexpected('fold_preview.scene_initialization'))
  assert.equal(nativeCalls, 0)

  availabilityThrows = false
  assert.doesNotThrow(() =>
    runtime.reportUnexpected('fold_preview.scene_initialization'))
  assert.equal(nativeCalls, 1)

  const failingMemory = createDiagnosticsRuntime({
    reportInMemory: () => {
      throw new Error('memory failed')
    },
    isNativeAvailable: () => true,
    recordNative: () => {
      nativeCalls += 1
    },
  })
  assert.equal(failingMemory.reportUnexpected('fold_preview.render'), false)
  assert.equal(nativeCalls, 1)
})

test('async native rejection is handled without an unhandled rejection', async () => {
  const unhandled: unknown[] = []
  const listener = (reason: unknown) => {
    unhandled.push(reason)
  }
  process.on('unhandledRejection', listener)
  try {
    const runtime = createDiagnosticsRuntime({
      reportInMemory: createDiagnostics().reportUnexpected,
      isNativeAvailable: () => true,
      recordNative: () => Promise.reject(new Error('native rejected')),
    })

    assert.equal(runtime.reportUnexpected('app.close_guard'), true)
    await new Promise<void>((resolve) => setImmediate(resolve))
    assert.deepEqual(unhandled, [])
  } finally {
    process.off('unhandledRejection', listener)
  }
})

test('native caps are per scope and memory still receives reports after each cap', () => {
  const memoryCalls = new Map<DiagnosticScope, number>()
  const nativeCalls = new Map<DiagnosticScope, number>()
  const runtime = createDiagnosticsRuntime({
    reportInMemory: (scope) => {
      memoryCalls.set(scope, (memoryCalls.get(scope) ?? 0) + 1)
      return DIAGNOSTIC_SCOPES.includes(scope)
    },
    isNativeAvailable: () => true,
    recordNative: (scope) => {
      nativeCalls.set(scope, (nativeCalls.get(scope) ?? 0) + 1)
    },
  })

  for (let index = 0; index < 80; index += 1) {
    runtime.reportUnexpected('fold_preview.render')
  }
  for (let index = 0; index < 72; index += 1) {
    runtime.reportUnexpected('fold_preview.camera')
  }

  assert.equal(nativeCalls.get('fold_preview.render'), 65)
  assert.equal(nativeCalls.get('fold_preview.camera'), 65)
  assert.equal(memoryCalls.get('fold_preview.render'), 80)
  assert.equal(memoryCalls.get('fold_preview.camera'), 72)
})

test('hostile scope objects are rejected without traps or native checks', () => {
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
  let availabilityChecks = 0
  let nativeCalls = 0
  const runtime = createDiagnosticsRuntime({
    reportInMemory: createDiagnostics().reportUnexpected,
    isNativeAvailable: () => {
      availabilityChecks += 1
      return true
    },
    recordNative: () => {
      nativeCalls += 1
    },
  })
  const looseReport = runtime.reportUnexpected as unknown as (
    scope: unknown,
  ) => boolean

  assert.equal(looseReport(hostile), false)
  assert.equal(looseReport(Symbol('fold_preview.render')), false)
  assert.deepEqual(accesses, [])
  assert.equal(availabilityChecks, 0)
  assert.equal(nativeCalls, 0)
})

test('the production singleton is safe in a non-native Node runtime', () => {
  clearDiagnostics()
  try {
    assert.equal(reportUnexpected('app.validation'), true)
    assert.equal(
      snapshotDiagnostics().unexpected.find(
        ({ scope }) => scope === 'app.validation',
      )?.count,
      '1',
    )
  } finally {
    clearDiagnostics()
  }
})
