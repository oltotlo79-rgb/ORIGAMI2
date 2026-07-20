import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

const fileMocks = vi.hoisted(() => ({
  open: vi.fn(),
  saveAs: vi.fn(),
}))

vi.mock('../src/lib/foldTechniqueFileClient', async (importOriginal) => {
  const actual = await importOriginal<
    typeof import('../src/lib/foldTechniqueFileClient.ts')
  >()
  return {
    ...actual,
    isNativeFoldTechniqueFileAvailable: () => true,
    openFoldTechniqueFileV1: fileMocks.open,
    saveFoldTechniqueFileAsV1: fileMocks.saveAs,
  }
})

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

import App from '../src/App.tsx'
import {
  createInitialFoldTechniqueDocumentV1,
  type FoldTechniqueFileDocumentV1,
} from '../src/lib/foldTechniqueEditor.ts'
import { localeStore } from '../src/lib/i18n.ts'
import { themeStore } from '../src/lib/theme.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  fileMocks.open.mockReset()
  fileMocks.saveAs.mockReset()
  localeStore.setLocale('ja')
  localeStore.dispose()
  themeStore.dispose()
  document.body.replaceChildren()
})

describe('App fold-technique file workflow', () => {
  it('creates with an initial save, edits in memory, and saves another file', async () => {
    fileMocks.saveAs.mockImplementation(async (
      requestId: number,
      _locale: string,
      document: FoldTechniqueFileDocumentV1,
    ) => ({ requestId, canceled: false, document }))
    render(<App />)
    const section = techniqueSection()
    expect(section.textContent).toContain(
      '折り操作、プロジェクト変更、外部取得は自動実行しません',
    )

    fireEvent.click(within(section).getByRole('button', { name: '新規作成' }))
    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '技法を作成' }))

    await waitFor(() => expect(fileMocks.saveAs).toHaveBeenCalledTimes(1))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeNull()
    })
    expect(within(section).getByText('保存済み')).toBeTruthy()

    fireEvent.click(within(section).getByRole('button', { name: '編集' }))
    const apply = screen.getByRole('button', { name: '変更を確定' })
    expect((apply as HTMLButtonElement).disabled).toBe(true)
    fireEvent.change(screen.getByLabelText('技法名（日本語）'), {
      target: { value: '花の中割り折り' },
    })
    fireEvent.click(apply)
    await waitFor(() => {
      expect(within(section).getByText('変更あり・別名保存が必要')).toBeTruthy()
    })

    fireEvent.click(within(section).getByRole('button', { name: '別名保存' }))
    await waitFor(() => expect(fileMocks.saveAs).toHaveBeenCalledTimes(2))
    await waitFor(() => {
      expect(within(section).getByText('保存済み')).toBeTruthy()
    })
  })

  it('keeps create content open on save cancellation and leaves loaded state unchanged', async () => {
    const loaded = createInitialFoldTechniqueDocumentV1()
    fileMocks.open.mockImplementation(async (requestId: number) => ({
      requestId,
      canceled: false,
      document: loaded,
    }))
    fileMocks.saveAs.mockImplementationOnce(async (requestId: number) => ({
      requestId,
      canceled: true,
      document: null,
    }))
    render(<App />)
    const section = techniqueSection()

    fireEvent.click(within(section).getByRole('button', { name: 'ファイル取込' }))
    await waitFor(() => expect(fileMocks.open).toHaveBeenCalledTimes(1))
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    expect(within(section).getByText('user.local.techniques')).toBeTruthy()

    fireEvent.click(within(section).getByRole('button', { name: '新規作成' }))
    fireEvent.change(screen.getByLabelText('技法名（日本語）'), {
      target: { value: '保存前の編集内容' },
    })
    fireEvent.click(screen.getByRole('button', { name: '技法を作成' }))
    await waitFor(() => expect(fileMocks.saveAs).toHaveBeenCalledTimes(1))

    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()
    expect(
      (screen.getByLabelText('技法名（日本語）') as HTMLInputElement).value,
    ).toBe('保存前の編集内容')
    expect(within(section).getByText('user.local.techniques')).toBeTruthy()
  })

  it('requires confirmation before a dirty workspace is replaced', async () => {
    fileMocks.saveAs.mockImplementation(async (
      requestId: number,
      _locale: string,
      document: FoldTechniqueFileDocumentV1,
    ) => ({ requestId, canceled: false, document }))
    const imported = {
      ...createInitialFoldTechniqueDocumentV1(),
      package_id: 'user.imported.techniques',
    }
    fileMocks.open.mockImplementation(async (requestId: number) => ({
      requestId,
      canceled: false,
      document: imported,
    }))
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false)
    render(<App />)
    const section = techniqueSection()

    fireEvent.click(within(section).getByRole('button', { name: '新規作成' }))
    fireEvent.click(screen.getByRole('button', { name: '技法を作成' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeNull()
    })
    fireEvent.click(within(section).getByRole('button', { name: '編集' }))
    fireEvent.change(screen.getByLabelText('技法名（日本語）'), {
      target: { value: '未保存の折り技法' },
    })
    fireEvent.click(screen.getByRole('button', { name: '変更を確定' }))
    await waitFor(() => {
      expect(within(section).getByText('変更あり・別名保存が必要'))
        .toBeTruthy()
    })

    fireEvent.click(within(section).getByRole('button', {
      name: 'ファイル取込',
    }))
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(confirm.mock.calls[0]?.[0]).toContain('未保存の折り技法')
    expect(fileMocks.open).not.toHaveBeenCalled()
    expect(within(section).getByText('変更あり・別名保存が必要')).toBeTruthy()

    fireEvent.click(within(section).getByRole('button', { name: '新規作成' }))
    expect(confirm).toHaveBeenCalledTimes(2)
    expect(screen.queryByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeNull()

    confirm.mockReturnValue(true)
    fireEvent.click(within(section).getByRole('button', {
      name: 'ファイル取込',
    }))
    await waitFor(() => expect(fileMocks.open).toHaveBeenCalledTimes(1))
    expect(within(section).getByText('user.imported.techniques')).toBeTruthy()
    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()
  })

  it('protects create and invalid edit drafts and restores opener focus', async () => {
    fileMocks.saveAs.mockImplementation(async (
      requestId: number,
      _locale: string,
      document: FoldTechniqueFileDocumentV1,
    ) => ({ requestId, canceled: false, document }))
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false)
    render(<App />)
    const section = techniqueSection()
    const create = within(section).getByRole('button', { name: '新規作成' })
    create.focus()
    fireEvent.click(create)
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByLabelText('パッケージID'))
    })

    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()

    confirm.mockReturnValue(true)
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeNull()
      expect(document.activeElement).toBe(create)
    })

    fireEvent.click(create)
    fireEvent.click(screen.getByRole('button', { name: '技法を作成' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeNull()
    })
    const edit = within(section).getByRole('button', { name: '編集' })
    edit.focus()
    fireEvent.click(edit)
    fireEvent.change(screen.getByLabelText('パッケージID'), {
      target: { value: '../invalid' },
    })
    confirm.mockReturnValue(false)
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()

    confirm.mockReturnValue(true)
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeNull()
      expect(document.activeElement).toBe(edit)
    })
  })

  it('keeps the editor locked until an in-flight save settles', async () => {
    let settleSave: (() => void) | undefined
    fileMocks.saveAs.mockImplementation((
      requestId: number,
    ) => new Promise((resolve) => {
      settleSave = () => resolve({
        requestId,
        canceled: true,
        document: null,
      })
    }))
    render(<App />)
    const section = techniqueSection()
    fireEvent.click(within(section).getByRole('button', { name: '新規作成' }))
    fireEvent.click(screen.getByRole('button', { name: '技法を作成' }))

    await waitFor(() => expect(fileMocks.saveAs).toHaveBeenCalledTimes(1))
    const cancel = screen.getByRole('button', { name: 'キャンセル' })
    expect((cancel as HTMLButtonElement).disabled).toBe(true)
    expect((screen.getByRole('button', {
      name: '処理中…',
    }) as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(cancel)
    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()

    settleSave?.()
    await waitFor(() => {
      expect((screen.getByRole('button', {
        name: 'キャンセル',
      }) as HTMLButtonElement).disabled).toBe(false)
    })
    expect(screen.getByRole('dialog', {
      name: '説明テンプレートを編集',
    })).toBeTruthy()
  })

  it('restores focus to the asynchronous import opener', async () => {
    const imported = createInitialFoldTechniqueDocumentV1()
    fileMocks.open.mockImplementation(async (requestId: number) => ({
      requestId,
      canceled: false,
      document: imported,
    }))
    render(<App />)
    const section = techniqueSection()
    const importButton = within(section).getByRole('button', {
      name: 'ファイル取込',
    })
    importButton.focus()
    fireEvent.click(importButton)
    await waitFor(() => {
      expect(screen.getByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeTruthy()
    })

    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', {
        name: '説明テンプレートを編集',
      })).toBeNull()
      expect(document.activeElement).toBe(importButton)
    })
  })
})

function techniqueSection(): HTMLElement {
  const heading = screen.getByRole('heading', { name: '名前付き折り技法' })
  const section = heading.closest('section')
  if (!section) throw new Error('fold-technique section missing')
  return section
}
