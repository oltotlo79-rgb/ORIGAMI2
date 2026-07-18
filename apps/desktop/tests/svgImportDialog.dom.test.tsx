import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { SvgImportDialog } from '../src/components/SvgImportDialog'
import type {
  SvgImportPreview,
  SvgImportSettingsValidation,
} from '../src/lib/svgImport'

const PREVIEW: SvgImportPreview = {
  import_id: '018f47d1-5c9f-7d42-8e91-2b90da8f61a4',
  file_name: '選択したSVGファイル',
  suggested_name: 'テスト作品',
  default_mm_per_unit: 1,
  root_view_box: {
    x: 0,
    y: 0,
    width: 100,
    height: 80,
  },
  root_physical_size: {
    width_millimetres: 100,
    height_millimetres: 80,
    width_unit: 'mm',
    height_unit: 'mm',
  },
  source_segment_count: 4,
  style_groups: [{
    group_id: 0,
    element_count: 1,
    segment_count: 4,
    stroke: '#000000',
    stroke_color: '#000000',
    dash_array: null,
    classes: ['paper'],
    layer: '外周',
    representative_id: 'paper-boundary',
    semantic_hint: 'boundary',
  }],
  boundary_candidates: [],
  preview_vertices: [
    { x: 0, y: 0 },
    { x: 100, y: 0 },
    { x: 100, y: 80 },
    { x: 0, y: 80 },
  ],
  preview_edges: [
    { start: 0, end: 1, group_id: 0 },
    { start: 1, end: 2, group_id: 0 },
    { start: 2, end: 3, group_id: 0 },
    { start: 3, end: 0, group_id: 0 },
  ],
  preview_truncated: false,
  warnings: [],
}

const VALIDATION: SvgImportSettingsValidation = {
  validation_id: '018f47d1-5ca0-75b1-a53a-c579f39f9659',
  preview_id: PREVIEW.import_id,
  expected_project_id: '018f47d1-5ca0-75b1-a53a-c579f39f9660',
  expected_revision: 0,
  millimeters_per_unit: 1,
  boundary_candidate_id: null,
  width_mm: 100,
  height_mm: 80,
  has_cuts: false,
}

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('SvgImportDialog DOM interactions', () => {
  it('traps forward, reverse, and escaped focus inside the rendered dialog', () => {
    const onCancel = vi.fn()
    renderDialog({ onCancel })

    const dialog = screen.getByRole('dialog')
    const close = screen.getByRole('button', { name: '閉じる' })
    const cancel = screen.getByRole('button', { name: 'キャンセル' })

    close.focus()
    fireEvent.keyDown(window, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(cancel)

    fireEvent.keyDown(window, { key: 'Tab' })
    expect(document.activeElement).toBe(close)

    const outside = document.createElement('button')
    document.body.append(outside)
    outside.focus()
    expect(document.activeElement).toBe(dialog)
    expect(onCancel).not.toHaveBeenCalled()
  })

  it('ignores composing Escape and handles an ordinary Escape once', () => {
    const onCancel = vi.fn()
    renderDialog({ onCancel })

    fireEvent.keyDown(window, { key: 'Escape', isComposing: true })
    expect(onCancel).not.toHaveBeenCalled()

    fireEvent.keyDown(window, { key: 'Escape', isComposing: false })
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it('invalidates boundary confirmation after scale and mapping changes', () => {
    const onValidate = vi.fn()
    const onInvalidateValidation = vi.fn()
    const { rerender } = renderDialog({ onValidate, onInvalidateValidation })

    fireEvent.change(screen.getByLabelText('外周の指定方法'), {
      target: { value: 'groups' },
    })
    fireEvent.click(screen.getByRole('button', { name: '外周と寸法を検証' }))
    expect(onValidate).toHaveBeenCalledTimes(1)
    rerender(dialogElement({ validation: VALIDATION, onValidate, onInvalidateValidation }))

    let confirmation = boundaryConfirmation()
    fireEvent.click(confirmation)
    expect(confirmation.checked).toBe(true)

    fireEvent.change(screen.getByLabelText(/1 SVG単位の長さ/), {
      target: { value: '2' },
    })
    expect(screen.queryByLabelText(/検証済みの境界/)).toBeNull()
    expect(onInvalidateValidation).toHaveBeenCalled()

    rerender(dialogElement({
      validation: {
        ...VALIDATION,
        millimeters_per_unit: 2,
        width_mm: 200,
        height_mm: 160,
      },
      onValidate,
      onInvalidateValidation,
    }))
    confirmation = boundaryConfirmation()

    fireEvent.click(confirmation)
    fireEvent.change(screen.getByLabelText('線種候補 1 の割当'), {
      target: { value: 'mountain' },
    })
    rerender(dialogElement({ validation: null, onValidate, onInvalidateValidation }))
    expect(screen.queryByLabelText(/検証済みの境界/)).toBeNull()

    fireEvent.change(screen.getByLabelText('線種候補 1 の割当'), {
      target: { value: 'boundary' },
    })
    expect(screen.queryByLabelText(/検証済みの境界/)).toBeNull()
  })

  it('keeps an errored busy dialog mounted and permits a later retry or cancel', () => {
    const onCancel = vi.fn()
    const onImport = vi.fn()
    const { rerender } = renderDialog({ onCancel, onImport })

    rerender(
      <SvgImportDialog
        preview={PREVIEW}
        validation={null}
        busy
        error="外周の検証に失敗しました"
        onInvalidateValidation={vi.fn()}
        onValidate={vi.fn()}
        onCancel={onCancel}
        onImport={onImport}
      />,
    )
    expect(screen.getByRole('dialog')).toBeTruthy()
    expect(screen.getByRole('alert').textContent).toContain('外周の検証に失敗しました')
    expect((screen.getByRole('button', { name: '閉じる' }) as HTMLButtonElement).disabled).toBe(true)
    expect((screen.getByRole('button', { name: 'キャンセル' }) as HTMLButtonElement).disabled)
      .toBe(true)

    rerender(
      <SvgImportDialog
        preview={PREVIEW}
        validation={null}
        busy={false}
        error="外周の検証に失敗しました"
        onInvalidateValidation={vi.fn()}
        onValidate={vi.fn()}
        onCancel={onCancel}
        onImport={onImport}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    expect(onCancel).toHaveBeenCalledTimes(1)
    expect(onImport).not.toHaveBeenCalled()
  })

  it('uses the native converted result instead of a raw Cut mapping for confirmation', () => {
    const candidatePreview: SvgImportPreview = {
      ...PREVIEW,
      style_groups: [{
        ...PREVIEW.style_groups[0],
        semantic_hint: 'cut',
      }],
      boundary_candidates: [{
        candidate_id: 0,
        kind: 'polygon',
        segment_count: 4,
        width: 100,
        height: 80,
        vertices: PREVIEW.preview_vertices,
      }],
    }
    const validation = {
      ...VALIDATION,
      preview_id: candidatePreview.import_id,
      boundary_candidate_id: 0,
      has_cuts: false,
    }
    renderDialog({ preview: candidatePreview, validation })

    fireEvent.change(screen.getByLabelText('外周の指定方法'), {
      target: { value: '0' },
    })
    expect(screen.queryByRole('heading', { name: '切断を許可' })).toBeNull()
  })
})

function renderDialog(overrides: Partial<{
  preview: SvgImportPreview
  busy: boolean
  error: string | null
  validation: SvgImportSettingsValidation | null
  onInvalidateValidation: () => void
  onValidate: () => void
  onCancel: () => void
  onImport: () => void
}> = {}) {
  return render(dialogElement(overrides))
}

function boundaryConfirmation() {
  return screen.getByLabelText(/検証済みの境界/) as HTMLInputElement
}

function dialogElement(overrides: Partial<{
  preview: SvgImportPreview
  busy: boolean
  error: string | null
  validation: SvgImportSettingsValidation | null
  onInvalidateValidation: () => void
  onValidate: () => void
  onCancel: () => void
  onImport: () => void
}> = {}) {
  return (
    <SvgImportDialog
      preview={overrides.preview ?? PREVIEW}
      validation={overrides.validation ?? null}
      busy={overrides.busy ?? false}
      error={overrides.error ?? null}
      onInvalidateValidation={overrides.onInvalidateValidation ?? vi.fn()}
      onValidate={overrides.onValidate ?? vi.fn()}
      onCancel={overrides.onCancel ?? vi.fn()}
      onImport={overrides.onImport ?? vi.fn()}
    />
  )
}
