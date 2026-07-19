import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createHistoryLimitClient,
  HistoryLimitClientError,
  HISTORY_LIMIT_SCHEMA_VERSION,
  MAX_HISTORY_ENTRY_LIMIT,
  MIN_HISTORY_ENTRY_LIMIT,
  parseHistoryLimitSettings,
  type HistoryLimitExpectedProjectBinding,
  type HistoryLimitSettings,
  type SetHistoryEntryLimitRequest,
} from '../src/lib/historyLimitClient.ts'

const INSTANCE_ID = '1aaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'
const PROJECT_ID = '2bbbbbbb-bbbb-4bbb-9bbb-bbbbbbbbbbb2'
const OTHER_INSTANCE_ID = '3ccccccc-cccc-4ccc-accc-ccccccccccc3'
const OTHER_PROJECT_ID = '4ddddddd-dddd-4ddd-bddd-ddddddddddd4'

const EXPECTED: HistoryLimitExpectedProjectBinding = Object.freeze({
  expectedProjectInstanceId: INSTANCE_ID,
  expectedProjectId: PROJECT_ID,
  expectedRevision: 12,
})

const SETTINGS: HistoryLimitSettings = Object.freeze({
  schemaVersion: HISTORY_LIMIT_SCHEMA_VERSION,
  projectInstanceId: INSTANCE_ID,
  projectId: PROJECT_ID,
  revision: 12,
  historyEntryLimit: 128,
})

function request(
  overrides: Readonly<Record<string, unknown>> = {},
): SetHistoryEntryLimitRequest {
  return {
    schemaVersion: HISTORY_LIMIT_SCHEMA_VERSION,
    ...EXPECTED,
    historyEntryLimit: 64,
    ...overrides,
  } as SetHistoryEntryLimitRequest
}

test('get sends no ambient arguments and admits only the caller-bound settings DTO', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const nativeSettings = { ...SETTINGS }
  const client = createHistoryLimitClient((command, arguments_) => {
    calls.push([command, arguments_])
    return nativeSettings
  })

  const result = await client.get(EXPECTED)

  assert.deepEqual(calls, [['get_history_entry_limit', undefined]])
  assert.deepEqual(result, SETTINGS)
  assert.notEqual(result, nativeSettings)
  assert.equal(Object.isFrozen(result), true)
})

test('set sends the exact closed request and requires the same setting in response', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createHistoryLimitClient((command, arguments_) => {
    calls.push([command, arguments_])
    return { ...SETTINGS, historyEntryLimit: 64 }
  })
  const candidate = request()

  const result = await client.set(candidate)

  assert.deepEqual(calls, [[
    'set_history_entry_limit',
    {
      request: {
        schemaVersion: 1,
        expectedProjectInstanceId: INSTANCE_ID,
        expectedProjectId: PROJECT_ID,
        expectedRevision: 12,
        historyEntryLimit: 64,
      },
    },
  ]])
  assert.deepEqual(result, { ...SETTINGS, historyEntryLimit: 64 })
  const firstCall = calls[0]
  assert.ok(firstCall)
  assert.notEqual(
    (firstCall[1] as { request: unknown }).request,
    candidate,
    'the caller-owned request must be detached before IPC',
  )
})

test('settings parser accepts exact inclusive limits and detaches null-prototype data', () => {
  for (const historyEntryLimit of [
    MIN_HISTORY_ENTRY_LIMIT,
    MAX_HISTORY_ENTRY_LIMIT,
  ]) {
    const source = Object.assign(Object.create(null), {
      ...SETTINGS,
      historyEntryLimit,
    })
    const parsed = parseHistoryLimitSettings(source)
    assert.deepEqual(parsed, { ...SETTINGS, historyEntryLimit })
    assert.equal(Object.getPrototypeOf(parsed), Object.prototype)
    assert.equal(Object.isFrozen(parsed), true)
  }
})

test('settings parser rejects every scalar and envelope drift fail-closed', () => {
  class SettingsClass {
    schemaVersion = 1
    projectInstanceId = INSTANCE_ID
    projectId = PROJECT_ID
    revision = 12
    historyEntryLimit = 128
  }

  const symbol = { ...SETTINGS }
  Object.defineProperty(symbol, Symbol('private'), {
    enumerable: true,
    value: 'secret',
  })

  for (const value of [
    { ...SETTINGS, unknown: true },
    { ...SETTINGS, schemaVersion: 2 },
    { ...SETTINGS, projectInstanceId: INSTANCE_ID.toUpperCase() },
    { ...SETTINGS, projectInstanceId: '00000000-0000-0000-0000-000000000000' },
    { ...SETTINGS, projectId: 'not-a-uuid' },
    { ...SETTINGS, revision: -1 },
    { ...SETTINGS, revision: -0 },
    { ...SETTINGS, revision: 1.5 },
    { ...SETTINGS, revision: Number.MAX_SAFE_INTEGER + 1 },
    { ...SETTINGS, historyEntryLimit: 0 },
    { ...SETTINGS, historyEntryLimit: 129 },
    { ...SETTINGS, historyEntryLimit: 1.5 },
    { ...SETTINGS, historyEntryLimit: Number.NaN },
    { ...SETTINGS, historyEntryLimit: 1n },
    symbol,
    new SettingsClass(),
    null,
    [],
  ]) {
    assert.equal(parseHistoryLimitSettings(value), null)
  }
})

test('accessors and hostile response proxies are rejected without exposing or invoking values', async () => {
  let getterCalls = 0
  const accessor = { ...SETTINGS } as Record<string, unknown>
  Object.defineProperty(accessor, 'historyEntryLimit', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('C:\\private\\history-limit.txt')
    },
  })
  assert.equal(parseHistoryLimitSettings(accessor), null)
  assert.equal(getterCalls, 0)

  const proxies = [
    new Proxy({}, {
      getPrototypeOf() {
        throw new Error('C:\\private\\prototype.txt')
      },
    }),
    new Proxy({}, {
      ownKeys() {
        throw new Error('C:\\private\\keys.txt')
      },
    }),
  ]
  for (const response of proxies) {
    const client = createHistoryLimitClient(() => response)
    await assert.rejects(client.get(EXPECTED), hasCode('invalid_response'))
  }
})

test('invalid expected bindings and set requests are rejected before native IPC', async () => {
  let calls = 0
  const client = createHistoryLimitClient(() => {
    calls += 1
    return SETTINGS
  })

  const invalidExpected = [
    { ...EXPECTED, expectedProjectInstanceId: OTHER_INSTANCE_ID.toUpperCase() },
    { ...EXPECTED, expectedProjectId: 'not-a-uuid' },
    { ...EXPECTED, expectedRevision: -1 },
    { ...EXPECTED, expectedRevision: -0 },
    { ...EXPECTED, expectedRevision: Number.MAX_SAFE_INTEGER + 1 },
    { ...EXPECTED, unknown: true },
  ]
  for (const expected of invalidExpected) {
    await assert.rejects(
      client.get(expected as HistoryLimitExpectedProjectBinding),
      hasCode('invalid_request'),
    )
  }

  const invalidRequests = [
    request({ schemaVersion: 2 }),
    request({ expectedProjectInstanceId: 'not-a-uuid' }),
    request({ expectedProjectId: OTHER_PROJECT_ID.toUpperCase() }),
    request({ expectedRevision: -1 }),
    request({ expectedRevision: -0 }),
    request({ expectedRevision: 1.5 }),
    request({ historyEntryLimit: 0 }),
    request({ historyEntryLimit: 129 }),
    request({ historyEntryLimit: 1.5 }),
    { ...request(), unknown: true },
  ]
  for (const candidate of invalidRequests) {
    await assert.rejects(
      client.set(candidate as SetHistoryEntryLimitRequest),
      hasCode('invalid_request'),
    )
  }

  let getterCalls = 0
  const accessor = { ...request() } as Record<string, unknown>
  Object.defineProperty(accessor, 'historyEntryLimit', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 64
    },
  })
  await assert.rejects(
    client.set(accessor as SetHistoryEntryLimitRequest),
    hasCode('invalid_request'),
  )
  assert.equal(getterCalls, 0)

  const hostile = new Proxy({}, {
    ownKeys() {
      throw new Error('C:\\private\\request.txt')
    },
  })
  await assert.rejects(
    client.set(hostile as SetHistoryEntryLimitRequest),
    hasCode('invalid_request'),
  )
  assert.equal(calls, 0)
})

test('get and set reject stale project bindings and stale set values', async () => {
  for (const response of [
    { ...SETTINGS, projectInstanceId: OTHER_INSTANCE_ID },
    { ...SETTINGS, projectId: OTHER_PROJECT_ID },
    { ...SETTINGS, revision: 13 },
  ]) {
    const client = createHistoryLimitClient(() => response)
    await assert.rejects(client.get(EXPECTED), hasCode('stale_response'))
  }

  for (const response of [
    { ...SETTINGS, projectInstanceId: OTHER_INSTANCE_ID, historyEntryLimit: 64 },
    { ...SETTINGS, projectId: OTHER_PROJECT_ID, historyEntryLimit: 64 },
    { ...SETTINGS, revision: 13, historyEntryLimit: 64 },
    { ...SETTINGS, historyEntryLimit: 63 },
  ]) {
    const client = createHistoryLimitClient(() => response)
    await assert.rejects(client.set(request()), hasCode('stale_response'))
  }
})

test('malformed responses and raw synchronous or asynchronous errors become fresh fixed errors', async () => {
  for (const response of [
    { ...SETTINGS, unknown: 'C:\\private\\response.txt' },
    { ...SETTINGS, historyEntryLimit: 0 },
    null,
  ]) {
    const client = createHistoryLimitClient(() => response)
    await assert.rejects(client.get(EXPECTED), hasCode('invalid_response'))
  }

  const rawErrors: readonly (() => unknown)[] = [
    () => {
      throw new Error('C:\\private\\history.ori2')
    },
    () => Promise.reject(new Error('C:\\private\\history.ori2')),
    () => Promise.reject(new Proxy({}, {
      getPrototypeOf() {
        throw new Error('C:\\private\\rejection.txt')
      },
    })),
  ]
  for (const nativeInvoke of rawErrors) {
    const client = createHistoryLimitClient(nativeInvoke)
    await assert.rejects(client.get(EXPECTED), (error: unknown) => {
      assert.ok(error instanceof HistoryLimitClientError)
      assert.equal(error.code, 'native_unavailable')
      assert.equal('cause' in error, false)
      assert.doesNotMatch(error.message, /private|history\.ori2|rejection/u)
      return true
    })
  }
})

function hasCode(code: HistoryLimitClientError['code']) {
  return (error: unknown) => (
    error instanceof HistoryLimitClientError
    && error.code === code
  )
}
