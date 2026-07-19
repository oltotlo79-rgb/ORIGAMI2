import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { FoldImportDialog } from '../src/components/FoldImportDialog.tsx'
import type { FoldImportPreview, FoldImportSettings } from '../src/lib/foldImport.ts'
import { localeStore } from '../src/lib/i18n.ts'

const PREVIEW: FoldImportPreview = {
  import_id: '018f47d1-5ca0-75b1-a53a-c579f39f9661',
  file_name: 'sample.fold',
  suggested_name: 'Sample',
  file_spec: '1.2',
  frame_unit: 'cm',
  default_mm_per_unit: 10,
  vertex_count: 3,
  edge_count: 3,
  boundary_edge_count: 2,
  boundary_candidates: [{
    id: 0,
    source: 'assigned_boundary',
    edge_indices: [0, 1],
  }],
  fixed_boundary_candidate_id: 0,
  assignments: [
    { assignment: 'B', count: 2 },
    { assignment: 'M', count: 1 },
    { assignment: 'F', count: 1 },
  ],
  preview_vertices: [
    { x: 0, y: 0 },
    { x: 10, y: 0 },
    { x: 5, y: 8 },
  ],
  preview_edges: [
    { source_index: 0, start: 0, end: 1, assignment: 'B' },
    { source_index: 1, start: 1, end: 2, assignment: 'B' },
    { source_index: 2, start: 0, end: 2, assignment: 'M' },
  ],
  preview_truncated: false,
  warnings: [
    'F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。',
  ],
}

afterEach(() => {
  cleanup()
  localeStore.setLocale('ja')
  localeStore.dispose()
  document.body.replaceChildren()
})

describe('FoldImportDialog', () => {
  it('requires explicit lossy mapping and acknowledgement before import', () => {
    const onImport = vi.fn<(settings: FoldImportSettings) => void>()
    renderDialog({ onImport })

    const submit = screen.getByRole('button', { name: '取り込む' })
    expect(submit).toHaveProperty('disabled', true)
    expect(screen.getByText('3頂点・3辺')).toBeTruthy()
    expect(screen.getByText('B · 用紙境界')).toBeTruthy()
    expect(screen.getByText('用紙境界（固定）')).toBeTruthy()
    expect(screen.getByText(/未選択: F · 平らな折り筋/u)).toBeTruthy()

    fireEvent.change(screen.getByRole('combobox', {
      name: 'F · 平らな折り筋の割当',
    }), { target: { value: 'auxiliary' } })
    fireEvent.click(screen.getByLabelText(
      '上記を確認し、展開図として取り込む',
    ))

    expect(submit).toHaveProperty('disabled', false)
    fireEvent.click(submit)
    expect(onImport).toHaveBeenCalledWith({
      importId: PREVIEW.import_id,
      name: 'Sample',
      mmPerUnit: 10,
      mappings: {
        M: 'mountain',
        F: 'auxiliary',
      },
      boundaryCandidateId: 0,
    })
  })

  it('translates labels, mapping options, counts, and actions live', () => {
    localeStore.initialize()
    localeStore.setLocale('en')
    renderDialog({
      preview: {
        ...PREVIEW,
        file_name: '選択したFOLDファイル',
        suggested_name: 'FOLDインポート',
        warnings: [
          ...PREVIEW.warnings,
          String.raw`C:\Users\alice\private.fold`,
        ],
      },
    })

    expect(screen.getByRole('dialog', {
      name: 'Review line types and scale',
    })).toBeTruthy()
    expect(screen.getByRole('img', {
      name: 'Preview of the crease pattern to import',
    })).toBeTruthy()
    expect(screen.getByText('3 vertices · 3 edges')).toBeTruthy()
    expect(screen.getByText('B · Paper boundary')).toBeTruthy()
    expect(screen.getByText('Paper boundary (fixed)')).toBeTruthy()
    expect(screen.getByText('Selected FOLD file')).toBeTruthy()
    expect(screen.getByRole('textbox', { name: 'Work name' }))
      .toHaveProperty('value', 'FOLD import')
    expect(screen.getByText(
      'F (flat crease) has no equivalent line type and must be converted to an auxiliary line or excluded.',
    )).toBeTruthy()
    expect(screen.getByText(
      'Some FOLD information will not be imported.',
    )).toBeTruthy()
    expect(document.body.textContent).not.toMatch(
      /(?:[ぁ-んァ-ン一-龯]|alice|private\.fold)/u,
    )
    const flatMapping = screen.getByRole('combobox', {
      name: 'F · Flat crease mapping',
    }) as HTMLSelectElement
    expect([...flatMapping.options].map((option) => option.textContent))
      .toEqual(['Select a mapping', 'Auxiliary line', 'Do not import'])
    expect(screen.getByRole('button', { name: 'Import' })).toBeTruthy()

    act(() => {
      localeStore.setLocale('ja')
    })
    expect(screen.getByRole('dialog', {
      name: '線種と縮尺を確認',
    })).toBeTruthy()
    expect(screen.getByRole('button', { name: '取り込む' })).toBeTruthy()
    expect(screen.getByRole('textbox', { name: '作品名' }))
      .toHaveProperty('value', 'FOLDインポート')
  })

  it('keeps invalid fields blocking and protects a busy import from Escape', () => {
    const onCancel = vi.fn()
    const { rerender } = renderDialog({ onCancel })

    fireEvent.change(screen.getByRole('textbox', { name: '作品名' }), {
      target: { value: '' },
    })
    fireEvent.change(screen.getByRole('spinbutton', {
      name: /^1 FOLD単位の長さ/u,
    }), { target: { value: '0' } })
    expect(screen.getByText(/120文字以内/u)).toBeTruthy()
    expect(screen.getByRole('button', { name: '取り込む' }))
      .toHaveProperty('disabled', true)

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)

    rerender(dialogElement({ busy: true, onCancel }))
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: '取込中…' }))
      .toHaveProperty('disabled', true)
  })

  it('requires an inferred outline choice and highlights only that candidate', () => {
    const onImport = vi.fn<(settings: FoldImportSettings) => void>()
    renderDialog({
      onImport,
      preview: {
        ...PREVIEW,
        boundary_edge_count: 0,
        boundary_candidates: [
          {
            id: 7,
            source: 'inferred_outer_face',
            edge_indices: [0, 1],
          },
          {
            id: 9,
            source: 'inferred_outer_face',
            edge_indices: [1, 2],
          },
        ],
        fixed_boundary_candidate_id: null,
        assignments: [{ assignment: 'M', count: 1 }],
        warnings: [],
      },
    })

    const submit = screen.getByRole('button', { name: '取り込む' })
    expect(submit).toHaveProperty('disabled', true)
    expect(document.querySelectorAll('line.is-selected-boundary')).toHaveLength(0)

    fireEvent.click(screen.getByLabelText('検証済み外周候補 8（2辺）'))
    expect(submit).toHaveProperty('disabled', false)
    expect(document.querySelectorAll('line.is-selected-boundary')).toHaveLength(2)

    fireEvent.click(submit)
    expect(onImport).toHaveBeenCalledWith({
      importId: PREVIEW.import_id,
      name: 'Sample',
      mmPerUnit: 10,
      mappings: { M: 'mountain' },
      boundaryCandidateId: 7,
    })
  })
})

type Props = {
  preview: FoldImportPreview
  busy: boolean
  error: string | null
  onCancel: () => void
  onImport: (settings: FoldImportSettings) => void
}

function renderDialog(overrides: Partial<Props> = {}) {
  return render(dialogElement(overrides))
}

function dialogElement(overrides: Partial<Props> = {}) {
  return (
    <FoldImportDialog
      preview={overrides.preview ?? PREVIEW}
      busy={overrides.busy ?? false}
      error={overrides.error ?? null}
      onCancel={overrides.onCancel ?? vi.fn()}
      onImport={overrides.onImport ?? vi.fn()}
    />
  )
}
