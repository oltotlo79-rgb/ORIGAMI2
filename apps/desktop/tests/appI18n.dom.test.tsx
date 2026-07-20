import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'

const coreMocks = vi.hoisted(() => ({
  generateBenchmarkPattern: vi.fn(),
}))

vi.mock('../src/lib/coreClient', async (importOriginal) => {
  const actual = await importOriginal<
    typeof import('../src/lib/coreClient.ts')
  >()
  return {
    ...actual,
    generateBenchmarkPattern: coreMocks.generateBenchmarkPattern,
  }
})

import App from '../src/App.tsx'
import { appConfirmationText } from '../src/lib/appMessages.ts'
import {
  LOCALE_STORAGE_KEY,
  localeStore,
} from '../src/lib/i18n.ts'
import { themeStore } from '../src/lib/theme.ts'

vi.mock('../src/components/CreaseCanvas', () => ({
  CreaseCanvas: () => <div data-testid="crease-canvas" />,
}))

vi.mock('../src/components/FoldPreview', () => ({
  FoldPreview: () => <div data-testid="fold-preview" />,
}))

vi.mock('../src/components/InstructionTimelinePanel', () => ({
  InstructionTimelinePanel: () => <div data-testid="instruction-timeline" />,
}))

vi.mock('../src/components/GlobalFlatFoldabilityPanel', () => ({
  GlobalFlatFoldabilityPanel: () => <div data-testid="global-flat-foldability" />,
}))

vi.mock('../src/components/WorkspaceLayoutSeparator', () => ({
  WorkspaceLayoutSeparator: () => <div data-testid="workspace-separator" />,
}))

const originalLocalStorageDescriptor = Object.getOwnPropertyDescriptor(
  window,
  'localStorage',
)

beforeEach(() => {
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
})

afterEach(() => {
  cleanup()
  coreMocks.generateBenchmarkPattern.mockReset()
  localeStore.dispose()
  themeStore.dispose()
  window.localStorage.removeItem(LOCALE_STORAGE_KEY)
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

describe('App locale integration', () => {
  it('switches major file, edit, status, and ARIA text from Japanese to English', () => {
    localeStore.dispose()
    localeStore.initialize()
    render(<App />)

    expect(screen.getByRole('navigation', {
      name: 'プロジェクト操作',
    })).toBeTruthy()
    expect(
      screen.getByRole('button', { name: '新規' }).getAttribute('title'),
    ).toMatch(
      /^新規 \(/u,
    )
    expect(screen.getByRole('button', { name: '元に戻す' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'やり直す' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '開く' })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: '展開フォルダーを開く',
    })).toBeTruthy()
    expect(screen.getByRole('button', { name: '保存' })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: '展開フォルダー保存',
    }).getAttribute('title')).toContain('それ以外の保存先では新規保存だけを行います')
    expect(screen.getByText('ブラウザ試作モード')).toBeTruthy()
    expect(screen.getByText(/^ツール: 選択$/u)).toBeTruthy()
    expect(screen.getByRole('complementary', {
      name: '作図ツール',
    })).toBeTruthy()

    fireEvent.change(screen.getByRole('combobox', {
      name: '表示言語',
    }), { target: { value: 'en' } })

    expect(document.documentElement.lang).toBe('en')
    expect(screen.getByRole('navigation', {
      name: 'Project actions',
    })).toBeTruthy()
    expect(
      screen.getByRole('button', { name: 'New' }).getAttribute('title'),
    ).toMatch(
      /^New \(/u,
    )
    expect(screen.getByRole('button', { name: 'Undo' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Redo' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Open' })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Open expanded folder',
    })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Save' })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Save expanded folder',
    }).getAttribute('title')).toContain('never overwritten')
    expect(screen.getByText('Browser prototype mode')).toBeTruthy()
    expect(screen.getByText(/^Tool: Select$/u)).toBeTruthy()
    expect(screen.getByRole('complementary', {
      name: 'Drawing tools',
    })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Mountain fold',
    })).toBeTruthy()
  })

  it('retranslates an asynchronous error without exposing its payload', async () => {
    const privateError = String.raw`C:\Users\alice\作品\private-project.ori`
    coreMocks.generateBenchmarkPattern.mockRejectedValueOnce(
      new Error(privateError),
    )
    localeStore.dispose()
    localeStore.initialize()
    render(<App />)

    fireEvent.click(screen.getByRole('button', {
      name: '10,000本テスト',
    }))
    expect(await screen.findByText(
      'ベンチマーク失敗: 性能テストを完了できませんでした。',
    )).toBeTruthy()
    expect(document.body.textContent).not.toContain(privateError)

    fireEvent.change(screen.getByRole('combobox', {
      name: '表示言語',
    }), { target: { value: 'en' } })

    expect(await screen.findByText(
      'Benchmark failed: The performance test could not be completed.',
    )).toBeTruthy()
    expect(document.body.textContent).not.toContain(privateError)
  })

  it('provides complete Japanese and English confirmation text', () => {
    expect(appConfirmationText('ja', 'quitDiscard')).toContain(
      '変更を破棄して終了しますか',
    )
    expect(appConfirmationText('en', 'quitDiscard')).toContain(
      'Discard them and quit?',
    )
    expect(appConfirmationText('ja', 'newProject')).toContain(
      '新しいプロジェクト',
    )
    expect(appConfirmationText('en', 'newProject')).toContain(
      'Create a new project',
    )
    expect(appConfirmationText('ja', 'openProject')).toContain(
      '別のプロジェクト',
    )
    expect(appConfirmationText('en', 'openProject')).toContain(
      'Open another project',
    )
    expect(appConfirmationText('ja', 'replaceWithFold')).toContain(
      'FOLD展開図',
    )
    expect(appConfirmationText('en', 'replaceWithFold')).toContain(
      'FOLD crease pattern',
    )
    expect(appConfirmationText('ja', 'replaceWithSvg')).toContain(
      'SVG展開図',
    )
    expect(appConfirmationText('en', 'replaceWithSvg')).toContain(
      'SVG crease pattern',
    )
  })
})
