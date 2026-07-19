import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { LanguageControl } from '../src/components/LanguageControl.tsx'
import {
  createLocaleStore,
  localeStore,
  type Locale,
} from '../src/lib/i18n.ts'

const originalLocalStorageDescriptor = Object.getOwnPropertyDescriptor(
  window,
  'localStorage',
)

afterEach(() => {
  cleanup()
  localeStore.dispose()
  if (originalLocalStorageDescriptor) {
    Object.defineProperty(
      window,
      'localStorage',
      originalLocalStorageDescriptor,
    )
  } else {
    Reflect.deleteProperty(window, 'localStorage')
  }
  document.documentElement.lang = 'ja'
  document.body.replaceChildren()
})

function createFixture(options: Readonly<{
  stored?: Locale
  writeThrows?: boolean
}> = {}) {
  const written: Locale[] = []
  const store = createLocaleStore({
    readStoredLocale: () => options.stored ?? null,
    writeStoredLocale(locale) {
      if (options.writeThrows) throw new Error('storage is unavailable')
      written.push(locale)
    },
    applyDocumentLanguage(locale) {
      document.documentElement.lang = locale
    },
  })
  store.initialize()
  return { store, written }
}

function installMemoryStorage(initial: Readonly<Record<string, string>>) {
  const values = new Map(Object.entries(initial))
  const storage: Storage = {
    get length() {
      return values.size
    },
    clear() {
      values.clear()
    },
    getItem(key) {
      return values.get(key) ?? null
    },
    key(index) {
      return [...values.keys()][index] ?? null
    },
    removeItem(key) {
      values.delete(key)
    },
    setItem(key, value) {
      values.set(key, value)
    },
  }
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    value: storage,
  })
}

describe('LanguageControl', () => {
  it('exposes a labeled native select and switches Japanese to English', () => {
    const target = createFixture({ stored: 'ja' })
    render(<LanguageControl store={target.store} />)

    const select = screen.getByRole('combobox', {
      name: '表示言語',
    }) as HTMLSelectElement
    expect([...select.options].map((option) => ({
      lang: option.lang,
      text: option.textContent,
      value: option.value,
    }))).toEqual([
      { lang: 'ja', text: '日本語', value: 'ja' },
      { lang: 'en', text: 'English', value: 'en' },
    ])
    expect(select.value).toBe('ja')
    expect(document.documentElement.lang).toBe('ja')

    fireEvent.change(select, { target: { value: 'en' } })

    expect(screen.getByRole('combobox', {
      name: 'Display language',
    })).toHaveProperty('value', 'en')
    expect(document.documentElement.lang).toBe('en')
    expect(target.written).toEqual(['en'])
  })

  it('keeps switching in memory when persistence throws', () => {
    const target = createFixture({ stored: 'ja', writeThrows: true })
    render(<LanguageControl store={target.store} />)

    expect(() => {
      fireEvent.change(screen.getByRole('combobox', {
        name: '表示言語',
      }), { target: { value: 'en' } })
    }).not.toThrow()

    expect(target.store.getSnapshot()).toEqual({ locale: 'en' })
    expect(document.documentElement.lang).toBe('en')
  })

  it('the browser store restores a valid saved language onto html', () => {
    installMemoryStorage({ 'origami2.locale': 'en' })
    document.documentElement.lang = 'ja'

    localeStore.initialize()

    expect(localeStore.getSnapshot()).toEqual({ locale: 'en' })
    expect(document.documentElement.lang).toBe('en')
  })
})
