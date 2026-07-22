import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createThemeStore,
  decodeThemePreference,
  encodeThemePreference,
  isThemePreference,
  type EffectiveTheme,
  type ThemeEnvironment,
  type ThemeMediaChangeListener,
} from '../src/lib/theme.ts'

class FakeMediaQuery {
  matches: boolean
  readonly listeners = new Set<ThemeMediaChangeListener>()
  addCount = 0
  removeCount = 0

  constructor(matches: boolean) {
    this.matches = matches
  }

  addEventListener(type: 'change', listener: ThemeMediaChangeListener) {
    assert.equal(type, 'change')
    this.addCount += 1
    this.listeners.add(listener)
  }

  removeEventListener(type: 'change', listener: ThemeMediaChangeListener) {
    assert.equal(type, 'change')
    this.removeCount += 1
    this.listeners.delete(listener)
  }

  emit(matches: boolean) {
    this.matches = matches
    for (const listener of this.listeners) listener({ matches })
  }
}

function fixture(options: Readonly<{
  stored?: unknown
  systemDark?: boolean
  readThrows?: boolean
  writeThrows?: boolean
  mediaThrows?: boolean
}> = {}) {
  const applied: EffectiveTheme[] = []
  const written: string[] = []
  const media = new FakeMediaQuery(options.systemDark ?? false)
  const environment: ThemeEnvironment = {
    readStoredPreference() {
      if (options.readThrows) throw new Error('storage is unavailable')
      return options.stored ?? null
    },
    writeStoredPreference(preference) {
      if (options.writeThrows) throw new Error('storage quota exceeded')
      written.push(preference)
    },
    getSystemTheme() {
      if (options.mediaThrows) throw new Error('matchMedia is unavailable')
      return media
    },
    applyEffectiveTheme(theme) {
      applied.push(theme)
    },
  }
  return {
    applied,
    environment,
    media,
    written,
  }
}

test('theme preference uses the exact system light dark allowlist', () => {
  for (const value of ['system', 'light', 'dark']) {
    assert.equal(isThemePreference(value), true)
  }
  for (const value of [
    null,
    undefined,
    '',
    'SYSTEM',
    ' light ',
    'sepia',
    1,
    {},
  ]) {
    assert.equal(isThemePreference(value), false)
  }
})

test('a valid legacy preference is migrated and applied without subscribing to OS changes', () => {
  const target = fixture({ stored: 'dark', systemDark: false })
  const store = createThemeStore(target.environment)

  assert.deepEqual(store.initialize(), {
    preference: 'dark',
    effectiveTheme: 'dark',
  })
  assert.deepEqual(target.applied, ['dark'])
  assert.equal(target.media.listeners.size, 0)
  assert.equal(target.media.addCount, 0)
  assert.deepEqual(target.written, [encodeThemePreference('dark')])
})

test('versioned theme storage is strict and rejects stale corrupt or extra data', () => {
  for (const preference of ['system', 'light', 'dark'] as const) {
    assert.equal(decodeThemePreference(encodeThemePreference(preference)), preference)
  }
  for (const value of [
    '{',
    '[]',
    JSON.stringify({ version: 0, preference: 'dark' }),
    JSON.stringify({ version: 1, preference: 'sepia' }),
    JSON.stringify({ version: 1, preference: 'dark', stale: true }),
    'x'.repeat(129),
  ]) assert.equal(decodeThemePreference(value), null)
})

test('missing or malformed storage defaults to system and follows OS changes', () => {
  for (const stored of [null, '', 'SYSTEM', 'sepia', 1]) {
    const target = fixture({ stored, systemDark: true })
    const store = createThemeStore(target.environment)
    let notifications = 0
    const unsubscribe = store.subscribe(() => {
      notifications += 1
    })

    assert.deepEqual(store.getSnapshot(), {
      preference: 'system',
      effectiveTheme: 'dark',
    })
    assert.equal(target.media.listeners.size, 1)
    target.media.emit(false)
    assert.deepEqual(store.getSnapshot(), {
      preference: 'system',
      effectiveTheme: 'light',
    })
    assert.equal(notifications, 1)
    assert.deepEqual(target.applied, ['dark', 'light'])
    unsubscribe()
    store.dispose()
  }
})

test('manual selection persists, detaches OS tracking, and system restores it', () => {
  const target = fixture({ systemDark: false })
  const store = createThemeStore(target.environment)
  store.initialize()

  assert.equal(store.setPreference('dark'), true)
  assert.deepEqual(store.getSnapshot(), {
    preference: 'dark',
    effectiveTheme: 'dark',
  })
  assert.deepEqual(target.written, [encodeThemePreference('dark')])
  assert.equal(target.media.listeners.size, 0)
  assert.equal(target.media.removeCount, 1)

  target.media.emit(false)
  assert.equal(store.getSnapshot().effectiveTheme, 'dark')

  target.media.matches = true
  assert.equal(store.setPreference('system'), true)
  assert.deepEqual(store.getSnapshot(), {
    preference: 'system',
    effectiveTheme: 'dark',
  })
  assert.deepEqual(target.written, [
    encodeThemePreference('dark'),
    encodeThemePreference('system'),
  ])
  assert.equal(target.media.listeners.size, 1)
  assert.equal(target.media.addCount, 2)
})

test('invalid programmatic selection never changes or persists state', () => {
  const target = fixture({ systemDark: false })
  const store = createThemeStore(target.environment)
  store.initialize()

  assert.equal(store.setPreference('sepia'), false)
  assert.deepEqual(store.getSnapshot(), {
    preference: 'system',
    effectiveTheme: 'light',
  })
  assert.deepEqual(target.written, [])
  assert.deepEqual(target.applied, ['light'])
})

test('storage and matchMedia failures safely fall back without blocking selection', () => {
  const target = fixture({
    readThrows: true,
    writeThrows: true,
    mediaThrows: true,
  })
  const store = createThemeStore(target.environment)

  assert.doesNotThrow(() => store.initialize())
  assert.deepEqual(store.getSnapshot(), {
    preference: 'system',
    effectiveTheme: 'light',
  })
  assert.doesNotThrow(() => store.setPreference('dark'))
  assert.deepEqual(store.getSnapshot(), {
    preference: 'dark',
    effectiveTheme: 'dark',
  })
  assert.deepEqual(target.applied, ['light', 'dark'])
})

test('initialization is idempotent and dispose removes the global OS listener', () => {
  const target = fixture({ systemDark: true })
  const store = createThemeStore(target.environment)

  store.initialize()
  store.initialize()
  store.getSnapshot()
  assert.equal(target.media.addCount, 1)
  assert.equal(target.media.listeners.size, 1)

  store.dispose()
  assert.equal(target.media.removeCount, 1)
  assert.equal(target.media.listeners.size, 0)
})
