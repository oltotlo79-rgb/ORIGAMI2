import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { DiagnosticsDialog } from '../src/components/DiagnosticsDialog.tsx'
import {
  prepareDiagnosticsSharePreview,
  saveDiagnosticsSharePreview,
} from '../src/lib/diagnosticsShare.ts'
import { localeStore } from '../src/lib/i18n.ts'

vi.mock('../src/lib/diagnosticsShare.ts', () => ({
  prepareDiagnosticsSharePreview: vi.fn(),
  saveDiagnosticsSharePreview: vi.fn(),
}))

const PREVIEW = Object.freeze({
  preview_generation: 7,
  json: '{"schema":"origami2.redacted-diagnostics.v1","unexpected":[]}',
  byte_length: 61,
})

const originalLocalStorageDescriptor = Object.getOwnPropertyDescriptor(
  window,
  'localStorage',
)

function installMemoryStorage() {
  const values = new Map<string, string>()
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

beforeEach(() => {
  installMemoryStorage()
  localeStore.dispose()
  vi.mocked(prepareDiagnosticsSharePreview).mockResolvedValue(PREVIEW)
  vi.mocked(saveDiagnosticsSharePreview).mockResolvedValue({
    preview_generation: 7,
    byte_length: 61,
    canceled: false,
  })
})

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

describe('DiagnosticsDialog localization', () => {
  it('keeps Japanese as the default and translates ready-state controls live', async () => {
    render(<DiagnosticsDialog open onClose={vi.fn()} />)

    expect(screen.getByRole('dialog', { name: '診断情報を確認' })).toBeTruthy()
    expect(screen.getByText('診断情報を準備しています…')).toBeTruthy()
    await screen.findByRole('button', { name: 'JSONファイルとして保存…' })

    fireEvent.click(screen.getByRole('button', { name: '内容をすべて選択' }))
    expect(screen.getByRole('status').textContent).toContain(
      '内容をすべて選択しました。',
    )

    act(() => {
      localeStore.setLocale('en')
    })

    expect(screen.getByRole('dialog', { name: 'Review diagnostics' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Save as JSON file…' })).toBeTruthy()
    expect(screen.getByRole('status').textContent).toContain(
      'All contents are selected.',
    )
  })

  it('translates failures and retry controls without exposing native details', async () => {
    vi.mocked(prepareDiagnosticsSharePreview).mockRejectedValueOnce(
      new Error('C:\\secret\\project.ori2'),
    )
    localeStore.initialize()
    act(() => {
      localeStore.setLocale('en')
    })

    render(<DiagnosticsDialog open onClose={vi.fn()} />)

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('Diagnostics could not be prepared.')
    expect(document.body.textContent).not.toContain('C:\\secret')
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy()
  })

  it('keeps save failure assertive and retranslates it after a locale change', async () => {
    vi.mocked(saveDiagnosticsSharePreview).mockRejectedValueOnce(
      new Error('private native error'),
    )
    render(<DiagnosticsDialog open onClose={vi.fn()} />)
    await screen.findByRole('button', { name: 'JSONファイルとして保存…' })

    fireEvent.click(screen.getByRole('button', {
      name: 'JSONファイルとして保存…',
    }))

    await waitFor(() => {
      expect(screen.getByRole('alert').textContent).toContain(
        '診断JSONを保存できませんでした。',
      )
    })
    expect(document.body.textContent).not.toContain('private native error')

    act(() => {
      localeStore.setLocale('en')
    })
    expect(screen.getByRole('alert').textContent).toContain(
      'Diagnostics JSON could not be saved.',
    )
  })
})
