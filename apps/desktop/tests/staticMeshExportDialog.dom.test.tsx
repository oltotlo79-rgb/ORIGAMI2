import { StrictMode } from 'react'
import {
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StaticMeshExportDialog } from '../src/components/StaticMeshExportDialog'
import { localeStore } from '../src/lib/i18n.ts'
import type {
  StaticMeshExportFormat,
  StaticMeshExportPreview,
} from '../src/lib/staticMeshExport.ts'

const PREVIEW: StaticMeshExportPreview = Object.freeze({
  exportId: '11111111-1111-4111-8111-111111111111',
  projectInstanceId: '22222222-2222-4222-8222-222222222222',
  projectId: '33333333-3333-4333-8333-333333333333',
  revision: 12,
  sourceFingerprint: 'a'.repeat(64),
  poseGeneration: '9',
  format: 'stl',
  formatSummary: 'Binary STL・mm・右手系Z-up・静的三角形',
  suggestedFileName: '鶴-pose.stl',
  byteCount: 2_345,
  paperThicknessMm: 0.1,
  faceCount: 3,
  vertexCount: 12,
  triangleCount: 6,
  geometryProfile: 'authenticated_exact_coplanar_face_union_solids_v1',
  sourceUnit: 'millimeter',
  encodedUnit: 'millimeter',
  sourceAxis: 'right-handed X-right Y-forward Z-up',
  encodedAxis: 'right-handed X-right Y-forward Z-up',
  warnings: Object.freeze([
    'independent_face_solids',
    'no_textures_animation',
    'no_project_semantics',
    'stl_triangle_soup_facet_normals',
    'stl_printability_not_guaranteed',
  ]),
  printability: Object.freeze({
    status: 'manifold_verified',
    watertight: true,
    consistentlyOriented: true,
    nonzeroVolume: true,
    noDuplicateTriangles: true,
    noDegenerateTriangles: true,
    conservativeSelfIntersectionClear: true,
    connectedComponentCount: 3,
    checkedEdgeCount: 18,
    checkedTrianglePairCount: 12,
    limitations: Object.freeze(['manifold_only_not_printability']),
  }),
})

afterEach(() => {
  cleanup()
  localeStore.setLocale('ja')
  localeStore.dispose()
  document.body.replaceChildren()
})

describe('StaticMeshExportDialog DOM interactions', () => {
  it('shows counts, units, axes, and requires explicit loss confirmation', () => {
    const onSave = vi.fn()
    renderDialog({ onSave })

    expect(screen.getByText('鶴-pose.stl')).toBeTruthy()
    expect(screen.getByText('2.3 KB')).toBeTruthy()
    expect(screen.getByText(/3 面 · 12 頂点 · 6 三角形/u)).toBeTruthy()
    expect(screen.getByText(/紙厚は面ごとの閉じた立体/u)).toBeTruthy()
    expect(screen.getByText(/頂点index.*triangle soup.*facet normal/u)).toBeTruthy()
    expect(screen.getByText(/3Dプリント可能性を保証しません/u)).toBeTruthy()
    expect(screen.getByText(/right-handed X-right Y-forward Z-up/u)).toBeTruthy()
    expect(screen.getByText('プリント適性・マニフォールド検査')).toBeTruthy()
    expect(screen.getByText('限定条件内でマニフォールドを確認')).toBeTruthy()

    const save = screen.getByRole(
      'button',
      { name: '保存先を選んで書き出す…' },
    ) as HTMLButtonElement
    expect(save.disabled).toBe(true)
    fireEvent.click(screen.getByLabelText('上記の情報損失と制約を確認しました'))
    expect(save.disabled).toBe(false)
    fireEvent.click(save)
    expect(onSave).toHaveBeenCalledExactlyOnceWith(true)
  })

  it('changes only to the closed OBJ/STL/GLB format set', () => {
    const onFormatChange = vi.fn()
    renderDialog({ onFormatChange })
    const select = screen.getByRole('combobox', { name: '出力形式' })
    fireEvent.change(select, { target: { value: 'obj' } })
    fireEvent.change(select, { target: { value: 'glb' } })
    fireEvent.change(select, { target: { value: 'fbx' } })
    expect(onFormatChange.mock.calls).toEqual([['obj'], ['glb']])
  })

  it('retains retry and notice UI without exposing native bytes or paths', () => {
    const onRetry = vi.fn()
    renderDialog({
      error: '現在姿勢から作り直してください',
      notice: '同じ不変データで再試行できます',
      onRetry,
    })
    fireEvent.click(screen.getByRole('button', { name: '現在姿勢から作り直す' }))
    expect(onRetry).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('status').textContent).toContain('同じ不変データ')
    expect(document.body.textContent).not.toMatch(
      /positionsMm|triangles\s*:\s*\[|C:\\Users|file:\/\//u,
    )
  })

  it('is single-shot and cleans up Escape listeners under StrictMode', () => {
    const onCancel = vi.fn()
    const onSave = vi.fn()
    render(
      <StrictMode>
        {dialogElement({ onCancel, onSave })}
      </StrictMode>,
    )
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)

    fireEvent.click(screen.getByLabelText('上記の情報損失と制約を確認しました'))
    fireEvent.click(screen.getByRole('button', { name: '保存先を選んで書き出す…' }))
    expect(onSave).toHaveBeenCalledExactlyOnceWith(true)
  })
})

function renderDialog(overrides: Partial<Props> = {}) {
  return render(dialogElement(overrides))
}

type Props = {
  format: StaticMeshExportFormat
  preview: StaticMeshExportPreview | null
  busy: boolean
  error: string | null
  notice: string | null
  onFormatChange: (format: StaticMeshExportFormat) => void
  onRetry: () => void
  onSave: (warningsAcknowledged: boolean) => void
  onCancel: () => void
}

function dialogElement(overrides: Partial<Props> = {}) {
  return (
    <StaticMeshExportDialog
      format={overrides.format ?? 'stl'}
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
