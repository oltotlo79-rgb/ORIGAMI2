import assert from 'node:assert/strict'
import test from 'node:test'

import {
  DEFAULT_UPDATE_CHECK_SETTINGS,
  DISABLED_UPDATE_CHECK_SETTINGS,
  UPDATE_CHECK_SETTINGS_STORAGE_KEY,
  UPDATE_CHECK_SETTINGS_VERSION,
  createUpdateCheckSettingsStore,
  decodeUpdateCheckSettings,
  encodeUpdateCheckSettings,
  isUpdateCheckSettingsSnapshot,
  type UpdateCheckSettingsEnvironment,
} from '../src/lib/updateCheckSettings.ts'

test('first-run settings use one frozen enabled default outside project data', () => {
  assert.deepEqual(DEFAULT_UPDATE_CHECK_SETTINGS, { enabled: true })
  assert.deepEqual(DISABLED_UPDATE_CHECK_SETTINGS, { enabled: false })
  assert.equal(Object.isFrozen(DEFAULT_UPDATE_CHECK_SETTINGS), true)
  assert.equal(Object.isFrozen(DISABLED_UPDATE_CHECK_SETTINGS), true)
  assert.equal(
    UPDATE_CHECK_SETTINGS_STORAGE_KEY,
    'origami2.update-check-settings',
  )
  assert.equal(UPDATE_CHECK_SETTINGS_VERSION, 1)
})

test('the versioned terminal setting round trips with an exact schema', () => {
  for (const snapshot of [
    DEFAULT_UPDATE_CHECK_SETTINGS,
    DISABLED_UPDATE_CHECK_SETTINGS,
  ]) {
    const serialized = encodeUpdateCheckSettings(snapshot)
    assert.deepEqual(Object.keys(JSON.parse(serialized)), [
      'version',
      'enabled',
    ])
    assert.deepEqual(decodeUpdateCheckSettings(serialized), snapshot)
    assert.equal(isUpdateCheckSettingsSnapshot(snapshot), true)
  }
})

test('old malformed oversized and hostile stored settings fail closed', () => {
  const valid = JSON.parse(
    encodeUpdateCheckSettings(DEFAULT_UPDATE_CHECK_SETTINGS),
  )
  let getterRead = false
  const accessor = Object.defineProperty({}, 'enabled', {
    enumerable: true,
    get() {
      getterRead = true
      return true
    },
  })

  for (const value of [
    '',
    '{',
    '[]',
    '{}',
    JSON.stringify({ ...valid, version: 0 }),
    JSON.stringify({ ...valid, enabled: 'true' }),
    JSON.stringify({ ...valid, surprise: true }),
    'x'.repeat(257),
    accessor,
    new Proxy({}, {
      getPrototypeOf() {
        throw new Error('private-project.ori')
      },
    }),
  ]) {
    assert.equal(decodeUpdateCheckSettings(value), null)
  }
  assert.equal(getterRead, false)

  for (const value of [
    null,
    undefined,
    true,
    { enabled: true, surprise: true },
    accessor,
    new Date(),
  ]) {
    assert.equal(isUpdateCheckSettingsSnapshot(value), false)
  }
  assert.equal(getterRead, false)
})

test('missing storage uses the default while corruption and read failure disable checks', () => {
  const missing = fixture({ stored: null })
  assert.equal(
    createUpdateCheckSettingsStore(missing.environment)
      .initialize(),
    DEFAULT_UPDATE_CHECK_SETTINGS,
  )

  for (const stored of ['', '{', '{"version":0,"enabled":true}']) {
    const corrupt = fixture({ stored })
    assert.equal(
      createUpdateCheckSettingsStore(corrupt.environment)
        .initialize(),
      DISABLED_UPDATE_CHECK_SETTINGS,
    )
  }

  const unreadable = fixture({ readThrows: true })
  assert.equal(
    createUpdateCheckSettingsStore(unreadable.environment)
      .initialize(),
    DISABLED_UPDATE_CHECK_SETTINGS,
  )
})

test('explicit disable persists canonically and notifies only real changes', () => {
  const target = fixture()
  const store = createUpdateCheckSettingsStore(target.environment)
  let notifications = 0
  const unsubscribe = store.subscribe(() => {
    notifications += 1
  })

  const disabled = store.setEnabled(false)
  assert.deepEqual(disabled, {
    ok: true,
    persisted: true,
    snapshot: DISABLED_UPDATE_CHECK_SETTINGS,
  })
  assert.equal(Object.isFrozen(disabled), true)
  assert.equal(notifications, 1)
  assert.deepEqual(
    decodeUpdateCheckSettings(target.writes.at(-1)),
    DISABLED_UPDATE_CHECK_SETTINGS,
  )

  assert.equal(store.setEnabled(false).ok, true)
  assert.equal(notifications, 1)
  assert.equal(target.writes.length, 2)

  const enabled = store.setEnabled(true)
  assert.equal(enabled.ok, true)
  assert.equal(enabled.persisted, true)
  assert.equal(enabled.snapshot, DEFAULT_UPDATE_CHECK_SETTINGS)
  assert.equal(notifications, 2)

  unsubscribe()
  store.setEnabled(false)
  assert.equal(notifications, 2)
})

test('a corrupt disabled fallback can be explicitly persisted without a state change', () => {
  const target = fixture({ stored: '{' })
  const store = createUpdateCheckSettingsStore(target.environment)
  assert.equal(store.initialize(), DISABLED_UPDATE_CHECK_SETTINGS)

  const result = store.setEnabled(false)
  assert.deepEqual(result, {
    ok: true,
    persisted: true,
    snapshot: DISABLED_UPDATE_CHECK_SETTINGS,
  })
  assert.equal(target.writes.length, 1)
  assert.deepEqual(
    decodeUpdateCheckSettings(target.writes[0]),
    DISABLED_UPDATE_CHECK_SETTINGS,
  )
})

test('invalid selections never change or persist the setting', () => {
  const target = fixture()
  const store = createUpdateCheckSettingsStore(target.environment)

  for (const value of [
    null,
    undefined,
    0,
    1,
    'true',
    {},
    new Boolean(false),
  ]) {
    const result = store.setEnabled(value)
    assert.deepEqual(result, {
      ok: false,
      reason: 'invalid',
      snapshot: DEFAULT_UPDATE_CHECK_SETTINGS,
    })
  }
  assert.equal(target.writes.length, 0)
})

test('write failure keeps the session choice and reports that it was not saved', () => {
  const target = fixture({ writeThrows: true })
  const store = createUpdateCheckSettingsStore(target.environment)
  let laterObserverCalls = 0
  store.subscribe(() => {
    throw new Error('observer failure')
  })
  store.subscribe(() => {
    laterObserverCalls += 1
  })

  const result = store.setEnabled(false)
  assert.deepEqual(result, {
    ok: true,
    persisted: false,
    snapshot: DISABLED_UPDATE_CHECK_SETTINGS,
  })
  assert.equal(store.getSnapshot(), DISABLED_UPDATE_CHECK_SETTINGS)
  assert.equal(laterObserverCalls, 1)
})

test('reset saves the default and dispose reloads the latest terminal value', () => {
  const target = fixture()
  const store = createUpdateCheckSettingsStore(target.environment)
  store.setEnabled(false)
  assert.equal(store.reset().snapshot, DEFAULT_UPDATE_CHECK_SETTINGS)
  assert.equal(target.writes.length, 2)

  store.dispose()
  target.stored = encodeUpdateCheckSettings(
    DISABLED_UPDATE_CHECK_SETTINGS,
  )
  assert.equal(store.initialize(), DISABLED_UPDATE_CHECK_SETTINGS)
})

function fixture(options: Readonly<{
  stored?: unknown
  readThrows?: boolean
  writeThrows?: boolean
}> = {}): {
  environment: UpdateCheckSettingsEnvironment
  writes: string[]
  stored: unknown
} {
  const target = {
    writes: [] as string[],
    stored: options.stored ?? null as unknown,
    environment: null as unknown as UpdateCheckSettingsEnvironment,
  }
  target.environment = {
    readStoredSettings() {
      if (options.readThrows) throw new Error('read blocked')
      return target.stored
    },
    writeStoredSettings(serialized) {
      if (options.writeThrows) throw new Error('write blocked')
      target.writes.push(serialized)
      target.stored = serialized
    },
  }
  return target
}
