import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'

import { ProjectLayerPanel } from '../src/components/ProjectLayerPanel'
import type { LocaleStore } from '../src/lib/i18n'
import {
  DEFAULT_PROJECT_LAYER_ID,
  type ProjectLayerDocumentV1,
} from '../src/lib/projectLayers'
import { localeFixture } from './localeTestFixture'

const CREASE_LAYER_ID = '10000000-0000-4000-8000-000000000001'
const ANNOTATION_LAYER_ID = '20000000-0000-4000-8000-000000000001'
const UNDERLAY_LAYER_ID = '30000000-0000-4000-8000-000000000001'
const EDGE_ID = '40000000-0000-4000-8000-000000000001'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('ProjectLayerPanel', () => {
  it('creates, renames, reorders, deletes, and assigns through bounded callbacks', async () => {
    const onCreate = vi.fn(async () => true)
    const onRename = vi.fn(async () => true)
    const onMove = vi.fn(async () => true)
    const onDelete = vi.fn(async () => true)
    const onAssignSelectedEdge = vi.fn(async () => true)
    vi.spyOn(window, 'confirm').mockReturnValue(true)
    renderPanel({
      selectedEdgeId: EDGE_ID,
      onCreate,
      onRename,
      onMove,
      onDelete,
      onAssignSelectedEdge,
    })

    fireEvent.change(screen.getByRole('textbox', { name: '名前' }), {
      target: { value: 'Reference notes' },
    })
    fireEvent.change(screen.getByRole('combobox', { name: '内容の種類' }), {
      target: { value: 'annotation' },
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '追加' }))
    })
    expect(onCreate).toHaveBeenCalledWith('Reference notes', 'annotation')

    const rename = screen.getByRole('textbox', {
      name: 'Detailsの新しいレイヤー名',
    })
    fireEvent.change(rename, { target: { value: 'Main folds' } })
    await act(async () => {
      fireEvent.click(screen.getAllByRole('button', {
        name: '名前を保存',
      })[1]!)
    })
    expect(onRename).toHaveBeenCalledWith(CREASE_LAYER_ID, 'Main folds')

    await act(async () => {
      fireEvent.click(screen.getByRole('button', {
        name: 'Detailsを描画順で1つ上へ移動',
      }))
    })
    expect(onMove).toHaveBeenCalledWith(CREASE_LAYER_ID, 0)

    const assigned = screen.getByRole('button', {
      name: '選択中の線はDetailsに割り当て済み',
    })
    expect(assigned.getAttribute('aria-pressed')).toBe('true')
    expect((assigned as HTMLButtonElement).disabled).toBe(true)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', {
        name: '選択中の線を折り線パターンへ割り当て',
      }))
    })
    expect(onAssignSelectedEdge).toHaveBeenCalledWith(
      DEFAULT_PROJECT_LAYER_ID,
    )

    await act(async () => {
      fireEvent.click(screen.getByRole('button', {
        name: 'Detailsを削除',
      }))
    })
    expect(window.confirm).toHaveBeenCalledWith(
      expect.stringContaining('明示割当された折り線1本'),
    )
    expect(onDelete).toHaveBeenCalledWith(CREASE_LAYER_ID)
  })

  it('keeps the default layer undeletable and limits line assignment to crease layers', () => {
    renderPanel({ selectedEdgeId: EDGE_ID })

    const defaultDelete = screen.getByRole('button', {
      name: '既定レイヤー折り線パターンは削除できません',
    })
    expect((defaultDelete as HTMLButtonElement).disabled).toBe(true)
    expect(defaultDelete.getAttribute('title')).toBe(
      '既定レイヤーは削除できません',
    )
    expect(screen.getAllByText('折り線は割当不可')).toHaveLength(2)
    expect(screen.getByText(
      /注釈・下絵レイヤーは空のレイヤーとして作成/u,
    )).toBeTruthy()
    expect(screen.getByText(/上下ボタンで描画順を変更/u)).toBeTruthy()
    expect(screen.queryByText(/重なり順/u)).toBeNull()
    expect(screen.getByRole('list', {
      name: 'プロジェクトのレイヤー一覧',
    }).children).toHaveLength(4)
  })

  it('single-flights operations and discards a result after the binding changes', async () => {
    let settle: ((value: boolean) => void) | undefined
    const onCreate = vi.fn(() => new Promise<boolean>((resolve) => {
      settle = resolve
    }))
    const view = renderPanel({ onCreate })
    fireEvent.change(screen.getByRole('textbox', { name: '名前' }), {
      target: { value: 'Pending' },
    })
    fireEvent.click(screen.getByRole('button', { name: '追加' }))
    await act(async () => {
      await Promise.resolve()
    })

    expect(screen.getByRole('status').textContent).toContain(
      'レイヤー操作を適用しています',
    )
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe('polite')
    expect(
      (screen.getByRole('button', { name: '追加' }) as HTMLButtonElement).disabled,
    ).toBe(true)
    fireEvent.click(screen.getByRole('button', { name: '追加' }))
    expect(onCreate).toHaveBeenCalledTimes(1)

    view.rerender(panel({ bindingKey: 'instance:project:2', onCreate }))
    expect(screen.queryByRole('status')).toBeNull()
    await act(async () => {
      settle?.(false)
    })
    expect(screen.queryByRole('alert')).toBeNull()
    expect(
      (screen.getByRole('button', { name: '追加' }) as HTMLButtonElement).disabled,
    ).toBe(false)
  })

  it('fails closed for rejected operations and invalid layer documents', async () => {
    const view = renderPanel({
      onCreate: async () => false,
    })
    fireEvent.change(screen.getByRole('textbox', { name: '名前' }), {
      target: { value: 'Rejected' },
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '追加' }))
    })

    let alert = screen.getByRole('alert')
    expect(alert.getAttribute('aria-live')).toBe('assertive')
    expect(alert.getAttribute('aria-atomic')).toBe('true')
    expect(alert.textContent).toContain('プロジェクトが更新された可能性')

    view.rerender(panel({ documentInvalid: true }))
    alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('安全に確認できない')
    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true)
    }
    expect(
      (screen.getByRole('textbox', { name: '名前' }) as HTMLInputElement).disabled,
    ).toBe(true)
  })

  it('switches its complete visible and accessible vocabulary to English', () => {
    const localeStore = localeFixture('ja')
    renderPanel({ localeStore })

    act(() => localeStore.setLocale('en'))

    expect(screen.getByRole('heading', { name: 'Layers' })).toBeTruthy()
    expect(screen.getByText('4 layers')).toBeTruthy()
    expect(screen.getByRole('group', { name: 'Add a layer' })).toBeTruthy()
    expect(screen.getByRole('textbox', { name: 'Name' })).toBeTruthy()
    expect(screen.getByRole('combobox', { name: 'Content type' })).toBeTruthy()
    expect(screen.getByRole('list', { name: 'Project layer list' })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Default layer Crease Pattern cannot be deleted',
    })).toBeTruthy()
    expect(screen.getByText(
      /Editing annotation and underlay objects is not yet supported/u,
    )).toBeTruthy()
    expect(screen.getByText(
      /Select a line in the 2D crease pattern/u,
    )).toBeTruthy()
    expect(screen.getByText(/change drawing order/u)).toBeTruthy()
    expect(screen.queryByText(/stacking order/u)).toBeNull()
  })

  it('translates only the untouched persisted default name without renaming it implicitly', async () => {
    const localeStore = localeFixture('ja')
    const onRename = vi.fn(async () => true)
    const view = renderPanel({ localeStore, onRename })

    expect(screen.getByText('折り線パターン')).toBeTruthy()
    expect(
      (screen.getByRole('textbox', {
        name: '折り線パターンの新しいレイヤー名',
      }) as HTMLInputElement).value,
    ).toBe('折り線パターン')
    await act(async () => {
      fireEvent.click(screen.getAllByRole('button', {
        name: '名前を保存',
      })[0]!)
    })
    expect(onRename).not.toHaveBeenCalled()

    act(() => localeStore.setLocale('en'))
    expect(screen.getByText('Crease Pattern')).toBeTruthy()
    expect(
      (screen.getByRole('textbox', {
        name: 'New layer name for Crease Pattern',
      }) as HTMLInputElement).value,
    ).toBe('Crease Pattern')
    await act(async () => {
      fireEvent.click(screen.getAllByRole('button', {
        name: 'Save name',
      })[0]!)
    })
    expect(onRename).not.toHaveBeenCalled()

    const customDefault: ProjectLayerDocumentV1 = {
      ...layerDocument(),
      layers: [
        {
          ...layerDocument().layers[0]!,
          name: 'My base layer',
        },
        ...layerDocument().layers.slice(1),
      ],
    }
    view.rerender(panel({ localeStore, document: customDefault }))
    expect(screen.getByText('My base layer')).toBeTruthy()
    act(() => localeStore.setLocale('ja'))
    expect(screen.getByText('My base layer')).toBeTruthy()
    expect(screen.queryByText('折り線パターン')).toBeNull()
  })
})

function renderPanel(
  overrides: Partial<Parameters<typeof panel>[0]> = {},
) {
  return render(panel(overrides))
}

function panel(overrides: Partial<{
  document: ProjectLayerDocumentV1
  bindingKey: string
  selectedEdgeId: string | null
  disabled: boolean
  documentInvalid: boolean
  onCreate: Parameters<typeof ProjectLayerPanel>[0]['onCreate']
  onRename: Parameters<typeof ProjectLayerPanel>[0]['onRename']
  onMove: Parameters<typeof ProjectLayerPanel>[0]['onMove']
  onDelete: Parameters<typeof ProjectLayerPanel>[0]['onDelete']
  onAssignSelectedEdge:
    Parameters<typeof ProjectLayerPanel>[0]['onAssignSelectedEdge']
  localeStore: LocaleStore
}> = {}) {
  return (
    <ProjectLayerPanel
      document={overrides.document ?? layerDocument()}
      bindingKey={overrides.bindingKey ?? 'instance:project:1'}
      selectedEdgeId={overrides.selectedEdgeId ?? null}
      disabled={overrides.disabled ?? false}
      documentInvalid={overrides.documentInvalid ?? false}
      onCreate={overrides.onCreate ?? (async () => true)}
      onRename={overrides.onRename ?? (async () => true)}
      onMove={overrides.onMove ?? (async () => true)}
      onDelete={overrides.onDelete ?? (async () => true)}
      onAssignSelectedEdge={
        overrides.onAssignSelectedEdge ?? (async () => true)
      }
      localeStore={overrides.localeStore}
    />
  )
}

function layerDocument(): ProjectLayerDocumentV1 {
  return {
    schema_version: 1,
    layers: [
      {
        id: DEFAULT_PROJECT_LAYER_ID,
        name: 'Crease Pattern',
        content_kind: 'crease_pattern',
      },
      {
        id: CREASE_LAYER_ID,
        name: 'Details',
        content_kind: 'crease_pattern',
      },
      {
        id: ANNOTATION_LAYER_ID,
        name: 'Notes',
        content_kind: 'annotation',
      },
      {
        id: UNDERLAY_LAYER_ID,
        name: 'Reference',
        content_kind: 'underlay',
      },
    ],
    edge_assignments: [{
      edge: EDGE_ID,
      layer: CREASE_LAYER_ID,
    }],
  }
}
