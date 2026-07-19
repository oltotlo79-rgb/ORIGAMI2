import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { KeyboardShortcutControl } from '../src/components/KeyboardShortcutControl.tsx'
import {
  DEFAULT_KEYBOARD_SHORTCUTS,
  createKeyboardShortcutStore,
  type KeyboardShortcutStore,
} from '../src/lib/keyboardShortcutSettings.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

function store(): KeyboardShortcutStore {
  return createKeyboardShortcutStore({
    readStoredShortcuts: () => null,
    writeStoredShortcuts: () => undefined,
  })
}

describe('KeyboardShortcutControl', () => {
  it('edits a portable shortcut and exposes the active value', () => {
    const target = store()
    render(<KeyboardShortcutControl store={target} />)

    fireEvent.change(screen.getByRole('combobox', {
      name: '新規のキー',
    }), { target: { value: 'p' } })
    fireEvent.click(screen.getByRole('checkbox', {
      name: '新規でAltを使う',
    }))
    fireEvent.click(screen.getByRole('checkbox', {
      name: '新規でShiftを使う',
    }))

    expect(target.getSnapshot().new).toEqual({
      key: 'p',
      alt: true,
      shift: true,
    })
    expect(screen.getByRole('status', {
      name: '新規の現在のショートカット',
    }).textContent).toBe('Ctrl/Cmd+Alt+Shift+P')
  })

  it('shows a duplicate and leaves the accepted setting unchanged', () => {
    const target = store()
    render(<KeyboardShortcutControl store={target} />)
    const openKey = screen.getByRole('combobox', { name: '開くのキー' })

    fireEvent.change(openKey, { target: { value: 'n' } })

    expect(target.getSnapshot().open).toEqual(
      DEFAULT_KEYBOARD_SHORTCUTS.open,
    )
    expect((openKey as HTMLSelectElement).value).toBe('o')
    expect(screen.getByRole('alert').textContent).toContain(
      '開くは新規と重複します（Windows・macOS）',
    )
  })

  it('detects the fixed Windows redo alias and can restore defaults', () => {
    const target = store()
    target.setShortcut('new', { key: 'p', alt: false, shift: false })
    render(<KeyboardShortcutControl store={target} />)

    fireEvent.change(screen.getByRole('combobox', {
      name: '新規のキー',
    }), { target: { value: 'y' } })
    expect(screen.getByRole('alert').textContent).toContain(
      '新規はやり直すと重複します（Windows）',
    )

    fireEvent.click(screen.getByRole('button', {
      name: '標準設定に戻す',
    }))
    expect(target.getSnapshot()).toBe(DEFAULT_KEYBOARD_SHORTCUTS)
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('renders all six commands with unique accessible controls', () => {
    render(<KeyboardShortcutControl store={store()} />)
    expect(screen.getByRole('group', {
      name: 'ショートカット設定',
    })).toBeTruthy()
    expect(screen.getAllByRole('combobox')).toHaveLength(6)
    expect(screen.getAllByRole('checkbox')).toHaveLength(12)
    expect(screen.getAllByRole('status')).toHaveLength(6)
  })
})
