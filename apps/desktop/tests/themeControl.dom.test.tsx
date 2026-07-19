import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { ThemeControl } from '../src/components/ThemeControl.tsx'
import {
  createThemeStore,
  type EffectiveTheme,
  type ThemeMediaChangeListener,
  type ThemePreference,
} from '../src/lib/theme.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

function createFixture(systemDark = false) {
  const applied: EffectiveTheme[] = []
  const written: ThemePreference[] = []
  const listeners = new Set<ThemeMediaChangeListener>()
  const media = {
    matches: systemDark,
    addEventListener(_type: 'change', listener: ThemeMediaChangeListener) {
      listeners.add(listener)
    },
    removeEventListener(_type: 'change', listener: ThemeMediaChangeListener) {
      listeners.delete(listener)
    },
    emit(matches: boolean) {
      media.matches = matches
      for (const listener of listeners) listener({ matches })
    },
  }
  const store = createThemeStore({
    readStoredPreference: () => null,
    writeStoredPreference: (preference) => written.push(preference),
    getSystemTheme: () => media,
    applyEffectiveTheme: (theme) => applied.push(theme),
  })
  store.initialize()
  return { applied, media, store, written }
}

describe('ThemeControl', () => {
  it('exposes an accessible native select and the current effective theme', () => {
    const target = createFixture(false)
    render(<ThemeControl store={target.store} />)

    const select = screen.getByRole('combobox', {
      name: '表示テーマ',
    }) as HTMLSelectElement
    expect([...select.options].map((option) => ({
      value: option.value,
      text: option.textContent,
    }))).toEqual([
      { value: 'system', text: 'OS設定に合わせる' },
      { value: 'light', text: 'ライト' },
      { value: 'dark', text: 'ダーク' },
    ])
    expect(select.value).toBe('system')
    expect(screen.getByRole('status', {
      name: '現在の実効テーマ',
    }).textContent).toBe('現在: ライト')
  })

  it('applies and persists a manual selection immediately', () => {
    const target = createFixture(false)
    render(<ThemeControl store={target.store} />)

    fireEvent.change(screen.getByRole('combobox', {
      name: '表示テーマ',
    }), { target: { value: 'dark' } })

    expect(target.written).toEqual(['dark'])
    expect(target.applied.at(-1)).toBe('dark')
    expect(screen.getByRole('status', {
      name: '現在の実効テーマ',
    }).textContent).toBe('現在: ダーク')
  })

  it('announces OS changes only while system mode is selected', () => {
    const target = createFixture(false)
    render(<ThemeControl store={target.store} />)

    act(() => target.media.emit(true))
    expect(screen.getByRole('status', {
      name: '現在の実効テーマ',
    }).textContent).toBe('現在: ダーク')

    fireEvent.change(screen.getByRole('combobox', {
      name: '表示テーマ',
    }), { target: { value: 'light' } })
    act(() => target.media.emit(true))
    expect(screen.getByRole('status', {
      name: '現在の実効テーマ',
    }).textContent).toBe('現在: ライト')
  })
})
