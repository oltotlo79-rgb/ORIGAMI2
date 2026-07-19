import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'

import { InstructionExportDialog } from '../src/components/InstructionExportDialog'
import {
  INSTRUCTION_EXPORT_PROFILE,
  INSTRUCTION_EXPORT_PROJECTION_PROFILE,
  type InstructionExportFormat,
  type InstructionExportPreview,
} from '../src/lib/instructionExport'
import { localeStore } from '../src/lib/i18n.ts'

const PREVIEW: InstructionExportPreview = {
  export_id: '018f47d1-5ca0-75b1-a53a-c579f39f9661',
  expected_project_id: '018f47d1-5ca0-75b1-a53a-c579f39f9662',
  expected_revision: 12,
  format: 'pdf',
  profile: INSTRUCTION_EXPORT_PROFILE,
  projection_profile: INSTRUCTION_EXPORT_PROJECTION_PROFILE,
  format_summary: 'PDF 1.7・固定アイソメトリック投影・A4縦',
  suggested_file_name: '鶴-折り図.pdf',
  byte_count: 2_345,
  step_count: 18,
  page_count: 6,
  caution_count: 2,
  warnings: [
    {
      category: 'fixed_automatic_camera',
      message_ja: '手順8は紙が重なるため、ゆっくり折ってください。',
    },
    {
      category: 'discrete_step_endpoints_only',
      message_ja: '手順15では内側の折り線が隠れます。',
    },
  ],
}

afterEach(() => {
  cleanup()
  localeStore.dispose()
  document.body.replaceChildren()
})

describe('InstructionExportDialog DOM interactions', () => {
  it('shows export metadata and requires warning acknowledgement before save', () => {
    const onSave = vi.fn()
    renderDialog({ onSave })

    const save = screen.getByRole(
      'button',
      { name: '保存先を選んで書き出す…' },
    ) as HTMLButtonElement
    expect(save.disabled).toBe(true)
    expect(screen.getByText('鶴-折り図.pdf')).toBeTruthy()
    expect(screen.getByText('2.3 KB')).toBeTruthy()
    expect(screen.getByText('PDF 1.7・固定アイソメトリック投影・A4縦')).toBeTruthy()
    expect(screen.getByText('instruction_export_v1')).toBeTruthy()
    expect(screen.getByText('orthographic_isometric_v1')).toBeTruthy()
    expect(screen.getByText('18手順')).toBeTruthy()
    expect(screen.getByText('6ページ')).toBeTruthy()
    expect(screen.getByText('2件')).toBeTruthy()

    fireEvent.click(screen.getByLabelText('上記の注意事項を確認しました'))
    expect(save.disabled).toBe(false)
    fireEvent.click(save)
    expect(onSave).toHaveBeenCalledWith(true)
  })

  it('permits a warning-free preview without a confirmation checkbox', () => {
    const onSave = vi.fn()
    renderDialog({
      preview: { ...PREVIEW, caution_count: 0, warnings: [] },
      onSave,
    })

    expect(screen.queryByRole('checkbox')).toBeNull()
    expect(screen.getByText('この折り図について追加確認が必要な注意事項はありません。'))
      .toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '保存先を選んで書き出す…' }))
    expect(onSave).toHaveBeenCalledWith(true)
  })

  it('translates metadata, trusted warnings, progress, and controls live', () => {
    localeStore.initialize()
    localeStore.setLocale('en')
    const { rerender } = renderDialog()

    expect(screen.getByRole('dialog', {
      name: 'Review format and output',
    })).toBeTruthy()
    expect(screen.getByRole('combobox', { name: 'Export format' })).toBeTruthy()
    expect(screen.getByText('18 steps')).toBeTruthy()
    expect(screen.getByText('6 pages')).toBeTruthy()
    expect(screen.getByText('2 notices')).toBeTruthy()
    expect(screen.getByText(
      'PDF 1.7 · A4 portrait · fixed isometric projection · multiple pages',
    )).toBeTruthy()
    expect(screen.getByText(/A fixed automatic camera is used/u)).toBeTruthy()
    expect(document.body.textContent).not.toContain(
      '手順8は紙が重なるため',
    )
    expect(screen.getByLabelText(
      'I have reviewed the notices above',
    )).toBeTruthy()

    rerender(dialogElement({
      preview: null,
      busy: true,
      generationActive: true,
      phase: 'analyzing_topology',
    }))
    expect(screen.getByText(/Analyzing face topology/u)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Stop generation' })).toBeTruthy()

    act(() => {
      localeStore.setLocale('ja')
    })
    expect(screen.getByRole('dialog', {
      name: '形式と出力内容を確認',
    })).toBeTruthy()
    expect(screen.getByText(/面構造を解析しています/u)).toBeTruthy()
    expect(screen.getByRole('button', { name: '生成を中止' })).toBeTruthy()
  })

  it('changes to every supported format and rejects unknown values', () => {
    const onFormatChange = vi.fn()
    renderDialog({ onFormatChange })

    const format = screen.getByLabelText('出力形式')
    fireEvent.change(format, { target: { value: 'svg_zip' } })
    fireEvent.change(format, { target: { value: 'pdf' } })
    fireEvent.change(format, { target: { value: 'svg' } })
    expect(onFormatChange.mock.calls).toEqual([['svg_zip'], ['pdf']])
    expect(document.body.textContent).not.toContain('<svg')
    expect(document.body.textContent).not.toContain('image_bytes')
  })

  it('keeps failures visible and exposes retry only after native work finishes', () => {
    const onRetry = vi.fn()
    const { rerender } = renderDialog({
      preview: null,
      busy: true,
      error: '生成できませんでした',
      onRetry,
    })
    expect(screen.getByRole('dialog').getAttribute('aria-busy')).toBe('true')
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

  it('retains the prepared preview as the primary retry after a save failure', () => {
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
    fireEvent.click(screen.getByLabelText('上記の注意事項を確認しました'))
    expect(save.disabled).toBe(false)
    fireEvent.click(save)
    expect(onSave).toHaveBeenCalledWith(true)
    expect(onRetry).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '現在の編集内容から作り直す' }))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  it('traps focus, protects save work, and permits generation cancellation', () => {
    const onCancel = vi.fn()
    const warningFreePreview = { ...PREVIEW, caution_count: 0, warnings: [] }
    const { rerender } = renderDialog({ preview: warningFreePreview, onCancel })
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

    rerender(dialogElement({
      preview: warningFreePreview,
      busy: true,
      generationActive: false,
      onCancel,
    }))
    outside.focus()
    expect(document.activeElement).toBe(dialog)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)

    rerender(dialogElement({
      preview: null,
      busy: true,
      generationActive: true,
      phase: 'analyzing_topology',
      onCancel,
    }))
    expect(screen.getByText(/面構造を解析しています/u)).toBeTruthy()
    expect(screen.getByRole('button', { name: '生成を中止' })).toBeTruthy()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(2)
  })
})

function renderDialog(overrides: Partial<Props> = {}) {
  return render(dialogElement(overrides))
}

type Props = {
  format: InstructionExportFormat
  preview: InstructionExportPreview | null
  busy: boolean
  generationActive: boolean
  phase: 'validating' | 'analyzing_topology' | 'building_document' | 'ready'
  error: string | null
  notice: string | null
  onFormatChange: (format: InstructionExportFormat) => void
  onRetry: () => void
  onSave: (warningsAcknowledged: boolean) => void
  onCancel: () => void
}

function dialogElement(overrides: Partial<Props> = {}) {
  return (
    <InstructionExportDialog
      format={overrides.format ?? 'pdf'}
      preview={overrides.preview === undefined ? PREVIEW : overrides.preview}
      busy={overrides.busy ?? false}
      generationActive={overrides.generationActive ?? false}
      phase={overrides.phase ?? 'validating'}
      error={overrides.error ?? null}
      notice={overrides.notice ?? null}
      onFormatChange={overrides.onFormatChange ?? vi.fn()}
      onRetry={overrides.onRetry ?? vi.fn()}
      onSave={overrides.onSave ?? vi.fn()}
      onCancel={overrides.onCancel ?? vi.fn()}
    />
  )
}
