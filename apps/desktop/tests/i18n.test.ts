import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createLocaleStore,
  formatLocalizedText,
  formatMessage,
  isLocale,
  selectLocalizedText,
  type Locale,
  type LocaleEnvironment,
} from '../src/lib/i18n.ts'

function fixture(options: Readonly<{
  stored?: unknown
  readThrows?: boolean
  writeThrows?: boolean
  applyThrows?: boolean
}> = {}) {
  const applied: Locale[] = []
  const written: Locale[] = []
  const environment: LocaleEnvironment = {
    readStoredLocale() {
      if (options.readThrows) throw new Error('storage is unavailable')
      return options.stored ?? null
    },
    writeStoredLocale(locale) {
      if (options.writeThrows) throw new Error('storage quota exceeded')
      written.push(locale)
    },
    applyDocumentLanguage(locale) {
      if (options.applyThrows) throw new Error('document is unavailable')
      applied.push(locale)
    },
  }
  return { applied, environment, written }
}

test('locale uses the exact ja en allowlist', () => {
  for (const value of ['ja', 'en']) {
    assert.equal(isLocale(value), true)
  }
  for (const value of [
    null,
    undefined,
    '',
    'JA',
    ' ja ',
    'fr',
    1,
    {},
  ]) {
    assert.equal(isLocale(value), false)
  }
})

test('valid storage is initialized once and invalid storage defaults to Japanese', () => {
  const english = fixture({ stored: 'en' })
  const englishStore = createLocaleStore(english.environment)

  assert.deepEqual(englishStore.initialize(), { locale: 'en' })
  assert.equal(englishStore.getSnapshot(), englishStore.initialize())
  assert.deepEqual(english.applied, ['en'])

  for (const stored of [null, '', 'JA', 'fr', 1, {}]) {
    const target = fixture({ stored })
    const store = createLocaleStore(target.environment)
    assert.deepEqual(store.initialize(), { locale: 'ja' })
    assert.deepEqual(target.applied, ['ja'])
  }
})

test('storage and document failures never block the active session locale', () => {
  const target = fixture({
    readThrows: true,
    writeThrows: true,
    applyThrows: true,
  })
  const store = createLocaleStore(target.environment)

  assert.doesNotThrow(() => store.initialize())
  assert.deepEqual(store.getSnapshot(), { locale: 'ja' })
  assert.equal(store.setLocale('en'), true)
  assert.deepEqual(store.getSnapshot(), { locale: 'en' })
  assert.deepEqual(target.written, [])
  assert.deepEqual(target.applied, [])
})

test('only actual locale changes notify active subscribers', () => {
  const target = fixture({ stored: 'ja' })
  const store = createLocaleStore(target.environment)
  let firstNotifications = 0
  let removedNotifications = 0
  const unsubscribeFirst = store.subscribe(() => {
    firstNotifications += 1
  })
  const unsubscribeRemoved = store.subscribe(() => {
    removedNotifications += 1
  })
  unsubscribeRemoved()

  assert.equal(store.setLocale('ja'), true)
  assert.equal(firstNotifications, 0)
  assert.equal(removedNotifications, 0)
  assert.equal(store.setLocale('en'), true)
  assert.equal(firstNotifications, 1)
  assert.equal(removedNotifications, 0)
  assert.equal(store.setLocale('en'), true)
  assert.equal(firstNotifications, 1)
  assert.equal(store.setLocale('fr'), false)
  assert.equal(firstNotifications, 1)
  assert.deepEqual(target.written, ['ja', 'en', 'en'])

  unsubscribeFirst()
  assert.equal(store.setLocale('ja'), true)
  assert.equal(firstNotifications, 1)
})

test('dispose clears subscribers and permits a clean reinitialization', () => {
  const target = fixture({ stored: 'en' })
  const store = createLocaleStore(target.environment)
  let notifications = 0
  store.subscribe(() => {
    notifications += 1
  })

  store.dispose()
  assert.deepEqual(store.initialize(), { locale: 'en' })
  assert.deepEqual(target.applied, ['en', 'en'])
  store.setLocale('ja')
  assert.equal(notifications, 0)
})

test('localized fixed text falls back safely to Japanese', () => {
  const text = Object.freeze({
    ja: '角度',
    en: 'Angle',
  })

  assert.equal(selectLocalizedText('ja', text), '角度')
  assert.equal(selectLocalizedText('en', text), 'Angle')
  assert.equal(selectLocalizedText('fr', text), '角度')
})

test('message formatting reads only safe own primitive values', () => {
  const inherited = Object.create({ inherited: 'leak' }) as Record<
    string,
    string | number
  >
  inherited.name = '鶴'
  inherited.count = 12
  Object.defineProperty(inherited, 'accessor', {
    enumerable: true,
    get() {
      throw new Error('an accessor must not be invoked')
    },
  })
  Object.defineProperty(inherited, 'infinite', {
    enumerable: true,
    value: Number.POSITIVE_INFINITY,
  })

  assert.equal(
    formatMessage(
      '{name}: {count}, {inherited}, {accessor}, {infinite}, {missing}',
      inherited,
    ),
    '鶴: 12, {inherited}, {accessor}, {infinite}, {missing}',
  )
  assert.equal(
    formatMessage('{constructor}', inherited),
    '{constructor}',
  )
  assert.equal(
    formatMessage('{name}', null as unknown as Record<string, string>),
    '{name}',
  )
})

test('localized formatting selects before applying explicit variables', () => {
  const text = Object.freeze({
    ja: '{shape}の角は{count}個です',
    en: '{shape} has {count} corners',
  })

  assert.equal(
    formatLocalizedText('ja', text, { shape: '正方形', count: 4 }),
    '正方形の角は4個です',
  )
  assert.equal(
    formatLocalizedText('en', text, { shape: 'Square', count: 4 }),
    'Square has 4 corners',
  )
})
