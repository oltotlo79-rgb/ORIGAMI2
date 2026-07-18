import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  createDiagnostics,
  DIAGNOSTIC_SCOPES,
  MAX_SERIALIZED_DIAGNOSTICS_BYTES,
} from '../src/lib/diagnostics.ts'
import {
  createDiagnosticsShareClient,
  DiagnosticsShareUnavailableError,
  type DiagnosticsSharePreview,
} from '../src/lib/diagnosticsShare.ts'

const UTF8_ENCODER = new TextEncoder()

test('production commands have no preview arguments and save only one generation', () => {
  const source = readFileSync(
    new URL('../src/lib/diagnosticsShare.ts', import.meta.url),
    'utf8',
  )
  assert.match(
    source,
    /transport\.invoke\(\s*'prepare_diagnostics_share_preview'\s*,?\s*\)/u,
  )
  const saveCall = source.match(
    /transport\.invoke\(\s*'save_diagnostics_share_preview'\s*,\s*(\{[^}]+\})/u,
  )
  assert.ok(saveCall)
  assert.equal(
    saveCall[1]?.replace(/\s/gu, ''),
    '{previewGeneration:preview.preview_generation}',
  )
  assert.doesNotMatch(source, /navigator\.clipboard|fetch\(|open\(/u)
})

test('prepare accepts and freezes one exact canonical native preview', async () => {
  const expected = preview(7, diagnosticsJson())
  const client = createDiagnosticsShareClient({
    invoke: (command, arguments_) => {
      assert.equal(command, 'prepare_diagnostics_share_preview')
      assert.equal(arguments_, undefined)
      return expected
    },
  })

  const actual = await client.preparePreview()

  assert.deepEqual(actual, expected)
  assert.notEqual(actual, expected)
  assert.equal(Object.isFrozen(actual), true)
})

test('prepare rejects malformed, noncanonical, mismatched, and oversized JSON', async () => {
  const validJson = diagnosticsJson()
  const parsed = JSON.parse(validJson) as {
    schema: string
    unexpected: Array<{ scope: string; count: string }>
  }
  const cases: unknown[] = [
    null,
    {},
    { ...preview(1, validJson), extra: true },
    { ...preview(1, validJson), preview_generation: -0 },
    { ...preview(1, validJson), preview_generation: 0x1_0000_0000 },
    { ...preview(1, validJson), byte_length: 1 },
    preview(1, ` ${validJson}`),
    preview(1, '{bad json'),
    preview(1, JSON.stringify({ ...parsed, extra: true })),
    preview(1, JSON.stringify({
      ...parsed,
      unexpected: parsed.unexpected.slice(0, -1),
    })),
    preview(1, JSON.stringify({
      ...parsed,
      unexpected: parsed.unexpected.map((entry, index) => (
        index === 0 ? { ...entry, scope: 'app.secret_path' } : entry
      )),
    })),
    preview(1, JSON.stringify({
      ...parsed,
      unexpected: parsed.unexpected.map((entry, index) => (
        index === 1 ? { ...entry, count: '2' } : entry
      )),
    })),
    {
      preview_generation: 1,
      json: 'x'.repeat(MAX_SERIALIZED_DIAGNOSTICS_BYTES + 1),
      byte_length: MAX_SERIALIZED_DIAGNOSTICS_BYTES + 1,
    },
  ]

  for (const value of cases) {
    const client = createDiagnosticsShareClient({ invoke: () => value })
    await assert.rejects(
      client.preparePreview(),
      fixedUnavailable,
    )
  }
})

test('prepare contains hostile records and getters without reading them', async () => {
  const accesses: string[] = []
  const hostile = new Proxy(Object.create(null) as object, {
    ownKeys: () => {
      accesses.push('ownKeys')
      throw new Error('private path')
    },
  })
  const getter = Object.defineProperty({}, 'preview_generation', {
    enumerable: true,
    get: () => {
      accesses.push('getter')
      throw new Error('private path')
    },
  })

  for (const value of [hostile, getter]) {
    const client = createDiagnosticsShareClient({ invoke: () => value })
    await assert.rejects(client.preparePreview(), fixedUnavailable)
  }
  assert.deepEqual(accesses, ['ownKeys'])
})

test('save sends only the validated generation and accepts a matching result', async () => {
  const prepared = preview(29, diagnosticsJson())
  const calls: Array<{
    command: string
    arguments_: Readonly<Record<string, unknown>> | undefined
  }> = []
  const client = createDiagnosticsShareClient({
    invoke: (command, arguments_) => {
      calls.push({ command, arguments_ })
      return {
        preview_generation: prepared.preview_generation,
        byte_length: prepared.byte_length,
        canceled: false,
      }
    },
  })

  const result = await client.savePreview(prepared)

  assert.deepEqual(calls, [{
    command: 'save_diagnostics_share_preview',
    arguments_: { previewGeneration: 29 },
  }])
  assert.deepEqual(result, {
    preview_generation: 29,
    byte_length: prepared.byte_length,
    canceled: false,
  })
  assert.equal(Object.isFrozen(result), true)
})

test('save accepts cancel but rejects stale generation, byte length, and extra fields', async () => {
  const prepared = preview(4, diagnosticsJson())
  const canceledClient = createDiagnosticsShareClient({
    invoke: () => ({
      preview_generation: 4,
      byte_length: prepared.byte_length,
      canceled: true,
    }),
  })
  assert.equal((await canceledClient.savePreview(prepared)).canceled, true)

  for (const response of [
    {
      preview_generation: 5,
      byte_length: prepared.byte_length,
      canceled: false,
    },
    {
      preview_generation: 4,
      byte_length: prepared.byte_length + 1,
      canceled: false,
    },
    {
      preview_generation: 4,
      byte_length: prepared.byte_length,
      canceled: 0,
    },
    {
      preview_generation: 4,
      byte_length: prepared.byte_length,
      canceled: false,
      path: 'C:\\private\\diagnostics.json',
    },
  ]) {
    const client = createDiagnosticsShareClient({ invoke: () => response })
    await assert.rejects(client.savePreview(prepared), fixedUnavailable)
  }
})

test('save revalidates the supplied preview before invoking native code', async () => {
  let calls = 0
  const client = createDiagnosticsShareClient({
    invoke: () => {
      calls += 1
      return null
    },
  })
  const invalid = {
    ...preview(1, diagnosticsJson()),
    json: '{"schema":"forged"}',
  } as DiagnosticsSharePreview

  await assert.rejects(client.savePreview(invalid), fixedUnavailable)
  assert.equal(calls, 0)
})

test('transport failures are replaced with one fixed path-free error', async () => {
  const raw = new Error('C:\\Users\\alice\\private-project.ori2')
  const client = createDiagnosticsShareClient({
    invoke: () => Promise.reject(raw),
  })

  await assert.rejects(client.preparePreview(), (error: unknown) => {
    assert.equal(fixedUnavailable(error), true)
    assert.notEqual(error, raw)
    assert.doesNotMatch(String(error), /alice|private-project/iu)
    return true
  })
})

function diagnosticsJson() {
  const diagnostics = createDiagnostics()
  for (const scope of DIAGNOSTIC_SCOPES) {
    diagnostics.reportUnexpected(scope)
  }
  return diagnostics.serialize()
}

function preview(
  previewGeneration: number,
  json: string,
): DiagnosticsSharePreview {
  return {
    preview_generation: previewGeneration,
    json,
    byte_length: UTF8_ENCODER.encode(json).byteLength,
  }
}

function fixedUnavailable(error: unknown) {
  return error instanceof DiagnosticsShareUnavailableError
    && error.message === 'diagnostics share unavailable'
    && error.name === 'DiagnosticsShareUnavailableError'
}
