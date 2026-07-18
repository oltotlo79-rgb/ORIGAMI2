import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'

import { CreaseExportDialog } from '../src/components/CreaseExportDialog'
import type {
  CreasePatternExportFormat,
  CreasePatternExportPreview,
} from '../src/lib/creaseExport'

const PREVIEW: CreasePatternExportPreview = {
  export_id: '018f47d1-5ca0-75b1-a53a-c579f39f9661',
  expected_project_id: '018f47d1-5ca0-75b1-a53a-c579f39f9662',
  expected_revision: 12,
  format: 'fold',
  format_summary: 'FOLD 1.2・2D creasePattern・座標単位mm',
  suggested_file_name: '鶴.fold',
  byte_count: 2_345,
  vertex_count: 8,
  edge_count: 12,
  assignment_counts: {
    boundary: 4,
    mountain: 2,
    valley: 3,
    auxiliary: 2,
    cut: 1,
  },
  has_cuts: true,
  warnings: [
    '紙の色・厚み・テクスチャはFOLD 1.2出力に含まれません。',
    '折り手順と現在の3D表示姿勢は出力に含まれません。',
  ],
}

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('CreaseExportDialog DOM interactions', () => {
  it('requires explicit loss acknowledgement before native save', () => {
    const onSave = vi.fn()
    renderDialog({ onSave })

    const save = screen.getByRole(
      'button',
      { name: '保存先を選んで書き出す…' },
    ) as HTMLButtonElement
    expect(save.disabled).toBe(true)
    expect(screen.getByText('鶴.fold')).toBeTruthy()
    expect(screen.getByText('2.3 KB')).toBeTruthy()
    expect(screen.getByText('FOLD 1.2・2D creasePattern・座標単位mm')).toBeTruthy()
    expect(screen.getByText('1本')).toBeTruthy()

    fireEvent.click(screen.getByLabelText('上記の情報が出力に含まれないことを確認しました'))
    expect(save.disabled).toBe(false)
    fireEvent.click(save)
    expect(onSave).toHaveBeenCalledWith(true)
  })

  it('permits a lossless preview without a confirmation checkbox', () => {
    const onSave = vi.fn()
    renderDialog({
      preview: { ...PREVIEW, warnings: [] },
      onSave,
    })

    expect(screen.queryByRole('checkbox')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: '保存先を選んで書き出す…' }))
    expect(onSave).toHaveBeenCalledWith(true)
  })

  it('changes to every supported format, rejects unknown values, and keeps native data out of the UI', () => {
    const onFormatChange = vi.fn()
    renderDialog({ onFormatChange })

    const format = screen.getByLabelText('出力形式')
    fireEvent.change(format, { target: { value: 'svg' } })
    fireEvent.change(format, { target: { value: 'pdf' } })
    fireEvent.change(format, { target: { value: 'dxf' } })
    fireEvent.change(format, { target: { value: 'obj' } })
    expect(onFormatChange.mock.calls).toEqual([['svg'], ['pdf'], ['dxf']])
    expect(document.body.textContent).not.toContain('vertices_coords')
    expect(document.body.textContent).not.toContain('<svg')
  })

  it('keeps failures visible and exposes retry after native work finishes', () => {
    const onRetry = vi.fn()
    const { rerender } = renderDialog({
      preview: null,
      busy: true,
      error: '生成できませんでした',
      onRetry,
    })
    expect(screen.getByRole('dialog')).toBeTruthy()
    expect(screen.getByRole('alert').textContent).toContain('生成できませんでした')
    expect(screen.queryByRole('button', { name: '同じ形式で再試行' })).toBeNull()

    rerender(dialogElement({
      preview: null,
      busy: false,
      error: '生成できませんでした',
      onRetry,
    }))
    fireEvent.click(screen.getByRole('button', { name: '同じ形式で再試行' }))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  it('retains the prepared stage as the primary retry after a save failure', () => {
    const onSave = vi.fn()
    const onRetry = vi.fn()
    renderDialog({
      error: '一時ファイルへ書き込めませんでした。',
      onSave,
      onRetry,
    })

    const save = screen.getByRole(
      'button',
      { name: '保存先を選んで書き出す…' },
    ) as HTMLButtonElement
    fireEvent.click(screen.getByLabelText('上記の情報が出力に含まれないことを確認しました'))
    expect(save.disabled).toBe(false)
    fireEvent.click(save)
    expect(onSave).toHaveBeenCalledWith(true)
    expect(onRetry).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '現在の編集内容から作り直す' }))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  it('traps focus and ignores Escape while composing or busy', () => {
    const onCancel = vi.fn()
    const losslessPreview = { ...PREVIEW, warnings: [] }
    const { rerender } = renderDialog({ preview: losslessPreview, onCancel })
    const dialog = screen.getByRole('dialog')
    const close = screen.getByRole('button', { name: '閉じる' })
    const format = screen.getByLabelText('出力形式')
    const save = screen.getByRole('button', { name: '保存先を選んで書き出す…' })

    close.focus()
    fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(save)

    const outside = document.createElement('button')
    document.body.append(outside)
    outside.focus()
    expect(document.activeElement).toBe(format)

    fireEvent.keyDown(document, { key: 'Escape', isComposing: true })
    expect(onCancel).not.toHaveBeenCalled()
    fireEvent.keyDown(document, { key: 'Escape', isComposing: false })
    expect(onCancel).toHaveBeenCalledTimes(1)

    rerender(dialogElement({ preview: losslessPreview, busy: true, onCancel }))
    outside.focus()
    expect(document.activeElement).toBe(dialog)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
  })
})

function renderDialog(overrides: Partial<Props> = {}) {
  return render(dialogElement(overrides))
}

type Props = {
  format: CreasePatternExportFormat
  preview: CreasePatternExportPreview | null
  busy: boolean
  error: string | null
  notice: string | null
  onFormatChange: (format: CreasePatternExportFormat) => void
  onRetry: () => void
  onSave: (warningsAcknowledged: boolean) => void
  onCancel: () => void
}

function dialogElement(overrides: Partial<Props> = {}) {
  return (
    <CreaseExportDialog
      format={overrides.format ?? 'fold'}
      preview={overrides.preview === undefined ? PREVIEW : overrides.preview}
      busy={overrides.busy ?? false}
      error={overrides.error ?? null}
      notice={overrides.notice ?? null}
      onFormatChange={overrides.onFormatChange ?? vi.fn()}
      onRetry={overrides.onRetry ?? vi.fn()}
      onSave={overrides.onSave ?? vi.fn()}
      onCancel={overrides.onCancel ?? vi.fn()}
    />
  )
}
