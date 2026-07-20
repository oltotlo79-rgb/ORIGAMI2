import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { FoldTechniqueTimelinePreviewDialog } from '../src/components/FoldTechniqueTimelinePreviewDialog.tsx'
import {
  createInitialFoldTechniqueDocumentV1,
} from '../src/lib/foldTechniqueEditor.ts'
import {
  createFoldTechniqueTimelineProposalV1,
} from '../src/lib/foldTechniqueTimelineProposal.ts'
import { localeStore } from '../src/lib/i18n.ts'

const result = createFoldTechniqueTimelineProposalV1(
  createInitialFoldTechniqueDocumentV1(),
  0,
  'ja',
  0,
)
if (!result.ok) throw new Error('built-in proposal fixture must be valid')
const PREVIEW = result

afterEach(() => {
  cleanup()
  localeStore.setLocale('ja')
  localeStore.dispose()
  document.body.replaceChildren()
  vi.restoreAllMocks()
})

describe('FoldTechniqueTimelinePreviewDialog', () => {
  it('focuses the safe cancel action and requires an explicit confirmation', () => {
    const onConfirm = vi.fn()
    const onCancel = vi.fn()
    render(
      <FoldTechniqueTimelinePreviewDialog
        preview={PREVIEW}
        busy={false}
        stale={false}
        error={null}
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    )

    expect(screen.getByRole('dialog', {
      name: '折り手順タイムライン案',
    })).toBeTruthy()
    expect(document.activeElement).toBe(
      screen.getByText('キャンセル'),
    )
    expect(screen.getByText(/1回のUndoで戻せる/u)).toBeTruthy()
    expect(screen.getAllByText(/説明専用/u).length).toBeGreaterThan(0)
    expect(onConfirm).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', {
      name: '説明専用手順を追加',
    }))
    expect(onConfirm).toHaveBeenCalledTimes(1)
    expect(onCancel).not.toHaveBeenCalled()
  })

  it('blocks stale or busy confirmation and reports a fixed failure', () => {
    const onConfirm = vi.fn()
    const onCancel = vi.fn()
    const { rerender } = render(
      <FoldTechniqueTimelinePreviewDialog
        preview={PREVIEW}
        busy={false}
        stale
        error="追加できませんでした"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    )
    expect((screen.getByRole('button', {
      name: '説明専用手順を追加',
    }) as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getAllByRole('alert')).toHaveLength(2)

    rerender(
      <FoldTechniqueTimelinePreviewDialog
        preview={PREVIEW}
        busy
        stale={false}
        error={null}
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    )
    expect((screen.getByText('キャンセル') as HTMLButtonElement).disabled)
      .toBe(true)
    expect(screen.getByRole('status').textContent).toMatch(/原子的に追加/u)
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(onCancel).not.toHaveBeenCalled()
  })

  it('updates all confirmation text when the locale changes', () => {
    render(
      <FoldTechniqueTimelinePreviewDialog
        preview={PREVIEW}
        busy={false}
        stale={false}
        error={null}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    act(() => localeStore.setLocale('en'))
    expect(screen.getByRole('dialog', {
      name: 'Instruction timeline proposal',
    })).toBeTruthy()
    expect(screen.getByText(/one undoable edit/iu)).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Add description-only steps',
    })).toBeTruthy()
  })
})
