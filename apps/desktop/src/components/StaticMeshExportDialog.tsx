import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useRef,
  useState,
} from 'react'

import { useLocale } from '../lib/i18n.ts'
import {
  STATIC_MESH_EXPORT_FORMATS,
  formatStaticMeshExportBytes,
  isStaticMeshExportFormat,
  staticMeshExportFormatLabel,
  staticMeshExportWarningMessage,
  type StaticMeshExportFormat,
  type StaticMeshExportPreview,
} from '../lib/staticMeshExport.ts'

type StaticMeshExportDialogProps = Readonly<{
  format: StaticMeshExportFormat
  preview: StaticMeshExportPreview | null
  busy: boolean
  error: string | null
  notice: string | null
  onFormatChange: (format: StaticMeshExportFormat) => void
  onRetry: () => void
  onSave: (warningsAcknowledged: boolean) => void
  onCancel: () => void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

const COPY = {
  ja: {
    eyebrow: '現在姿勢の3D書き出し',
    title: '形式と中央面メッシュの制約を確認',
    close: '閉じる',
    description:
      '3Dプレビューに現在表示されている認証済みの完成姿勢を、静的な三角形メッシュとして書き出します。編集履歴や保存状態は変わりません。',
    format: '出力形式',
    optionDetails: {
      obj: 'Blenderなどで扱いやすいテキスト形式・mm・Z-up',
      stl: 'スライサーで広く読めるバイナリ形式・mm・Z-up',
      glb: 'glTF 2.0の単一バイナリ・m・Y-up',
    },
    generating: '現在姿勢を検証・生成しています…',
    retry: '同じ形式で再試行',
    rebuild: '現在姿勢から作り直す',
    midSurface:
      '重要: 出力は紙の「中央面」だけです。紙厚を持つソリッド、閉じた多様体、3Dプリント可能な模型ではありません。',
    faceSolids:
      '重要: 紙厚を面ごとの閉じた立体として出力します。折り目での和集合や3Dプリント可能性は保証しません。',
    metadata: {
      format: '形式',
      specification: '出力仕様',
      suggestedName: '保存名候補',
      size: 'サイズ',
      geometry: '形状',
      source: '固定元',
      thickness: '設定紙厚',
      units: '単位',
      axes: '座標軸',
    },
    faces: '面',
    vertices: '頂点',
    triangles: '三角形',
    sourceUnit: '生成元',
    encodedUnit: 'ファイル',
    lossTitle: '出力に含まれない情報・保証されない性質',
    printabilityTitle: 'プリント適性・マニフォールド検査',
    printabilityStatus: {
      manifold_verified: '限定条件内でマニフォールドを確認',
      not_verified: 'マニフォールドを確認できません',
      not_applicable: '対象外（正厚のSTL/GLBのみ）',
    },
    printabilityChecks: '水密・向き・体積・重複・縮退・交差の保守検査',
    printabilityCounts: '連結成分 / 検査辺 / 検査三角形ペア',
    printabilityDisclaimer:
      '限定的な幾何検査です。最小肉厚、支持材、プリンターやスライサーとの互換性は保証しません。',
    acknowledge: '上記の情報損失と制約を確認しました',
    cancel: 'キャンセル',
    processing: '処理中…',
    save: '保存先を選んで書き出す…',
    formatSummaries: {
      obj: 'Wavefront OBJ・mm・右手系Z-up・静的三角形',
      stl: 'Binary STL・mm・右手系Z-up・静的三角形',
      glb: 'glTF 2.0 GLB・m・右手系Y-up・静的三角形',
    },
  },
  en: {
    eyebrow: 'Export current 3D pose',
    title: 'Review format and mid-surface limitations',
    close: 'Close',
    description:
      'Export the authenticated completed pose currently shown in the 3D preview as a static triangle mesh. Project history and save state are unchanged.',
    format: 'Export format',
    optionDetails: {
      obj: 'Text format for Blender and similar tools · mm · Z-up',
      stl: 'Widely supported binary slicer format · mm · Z-up',
      glb: 'Single-file glTF 2.0 binary · m · Y-up',
    },
    generating: ' current pose is being validated and generated…',
    retry: 'Retry the same format',
    rebuild: 'Rebuild from the current pose',
    midSurface:
      'Important: this exports only the paper mid-surface. It is not a paper-thickness solid, a closed manifold, or a guaranteed printable model.',
    faceSolids:
      'Important: exactly coplanar adjacent faces are welded. A strictly two-triangle, one-hinge pose is also joined only when the native exact thickness corridor issues and revalidates a boundary capability. Other hinge solids remain separate; general unions and 3D printability are not guaranteed.',
    metadata: {
      format: 'Format',
      specification: 'Specification',
      suggestedName: 'Suggested file name',
      size: 'Size',
      geometry: 'Geometry',
      source: 'Source',
      thickness: 'Paper setting',
      units: 'Units',
      axes: 'Axes',
    },
    faces: 'faces',
    vertices: 'vertices',
    triangles: 'triangles',
    sourceUnit: 'Source',
    encodedUnit: 'File',
    lossTitle: 'Information omitted and properties not guaranteed',
    printabilityTitle: 'Printability and manifold report',
    printabilityStatus: {
      manifold_verified: 'Manifold verified within the bounded checks',
      not_verified: 'Manifold not verified',
      not_applicable: 'Not applicable (positive-thickness STL/GLB only)',
    },
    printabilityChecks:
      'Watertightness, orientation, volume, duplicates, degeneracy, conservative intersection',
    printabilityCounts: 'components / checked edges / checked triangle pairs',
    printabilityDisclaimer:
      'This limited geometry report does not guarantee wall thickness, supports, or printer/slicer compatibility.',
    acknowledge: 'I understand the information loss and limitations above',
    cancel: 'Cancel',
    processing: 'Processing…',
    save: 'Choose destination and export…',
    formatSummaries: {
      obj: 'Wavefront OBJ · mm · right-handed Z-up · static triangles',
      stl: 'Binary STL · mm · right-handed Z-up · static triangles',
      glb: 'glTF 2.0 GLB · m · right-handed Y-up · static triangles',
    },
  },
} as const

export function StaticMeshExportDialog({
  format,
  preview,
  busy,
  error,
  notice,
  onFormatChange,
  onRetry,
  onSave,
  onCancel,
}: StaticMeshExportDialogProps) {
  const locale = useLocale()
  const copy = COPY[locale]
  const numberLocale = locale === 'ja' ? 'ja-JP' : 'en-US'
  const [warningsAcknowledged, setWarningsAcknowledged] = useState(false)
  const dialogRef = useRef<HTMLElement>(null)
  const formatRef = useRef<HTMLSelectElement>(null)
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    setWarningsAcknowledged(false)
  }, [preview])

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      if (busy) dialogRef.current?.focus()
      else if (preview) formatRef.current?.focus()
      else closeRef.current?.focus()
    })
    return () => cancelAnimationFrame(frame)
  }, [busy, preview])

  useEffect(() => {
    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      const target = event.target
      if (!dialog || !(target instanceof Node) || dialog.contains(target)) return
      if (busy) dialog.focus()
      else if (preview) formatRef.current?.focus()
      else closeRef.current?.focus()
    }
    document.addEventListener('focusin', handleFocusIn, true)
    return () => document.removeEventListener('focusin', handleFocusIn, true)
  }, [busy, preview])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.isComposing || busy) return
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    }
    document.addEventListener('keydown', handleKeyDown, true)
    return () => document.removeEventListener('keydown', handleKeyDown, true)
  }, [busy, onCancel])

  const trapFocus = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key !== 'Tab') return
    const dialog = dialogRef.current
    if (!dialog) return
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
    )
    if (focusable.length === 0) {
      event.preventDefault()
      dialog.focus()
      return
    }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active = document.activeElement
    if (event.shiftKey && (active === first || !dialog.contains(active))) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
      event.preventDefault()
      first.focus()
    }
  }

  const canSave = Boolean(preview) && !busy && warningsAcknowledged
  const unitLabel = (unit: 'millimeter' | 'meter') => unit === 'meter' ? 'm' : 'mm'

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="crease-export-dialog static-mesh-export-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="static-mesh-export-title"
        aria-describedby="static-mesh-export-description"
        aria-busy={busy}
        tabIndex={-1}
        onKeyDown={trapFocus}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">{copy.eyebrow}</span>
            <h2 id="static-mesh-export-title">{copy.title}</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label={copy.close}
          >
            ×
          </button>
        </header>

        <div className="crease-export-dialog-body">
          <p id="static-mesh-export-description" className="dialog-note">
            {copy.description}
          </p>

          <label className="crease-export-format">
            <span>{copy.format}</span>
            <select
              ref={formatRef}
              value={format}
              disabled={busy}
              onChange={(event) => {
                const next = event.currentTarget.value
                if (isStaticMeshExportFormat(next)) onFormatChange(next)
              }}
            >
              {STATIC_MESH_EXPORT_FORMATS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label} — {copy.optionDetails[option.value]}
                </option>
              ))}
            </select>
          </label>

          {busy && !preview && (
            <p className="crease-export-loading" role="status">
              {staticMeshExportFormatLabel(format)}{copy.generating}
            </p>
          )}

          {error && (
            <div className="crease-export-error">
              <p className="dialog-error" role="alert">{error}</p>
              {!busy && (
                <button type="button" onClick={onRetry}>
                  {preview ? copy.rebuild : copy.retry}
                </button>
              )}
            </div>
          )}

          {preview && (
            <>
              <p className="static-mesh-export-mid-surface">
                {preview.geometryProfile === 'authenticated_exact_coplanar_face_union_solids_v1'
                  ? copy.faceSolids
                  : copy.midSurface}
              </p>
              <dl className="crease-export-metadata">
                <div>
                  <dt>{copy.metadata.format}</dt>
                  <dd>{staticMeshExportFormatLabel(preview.format)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.specification}</dt>
                  <dd>{locale === 'ja'
                    ? preview.formatSummary
                    : copy.formatSummaries[preview.format]}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.suggestedName}</dt>
                  <dd>{preview.suggestedFileName}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.size}</dt>
                  <dd>{formatStaticMeshExportBytes(preview.byteCount, locale)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.geometry}</dt>
                  <dd>
                    {preview.faceCount.toLocaleString(numberLocale)} {copy.faces} ·{' '}
                    {preview.vertexCount.toLocaleString(numberLocale)} {copy.vertices} ·{' '}
                    {preview.triangleCount.toLocaleString(numberLocale)} {copy.triangles}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.source}</dt>
                  <dd>
                    revision {preview.revision.toLocaleString(numberLocale)} · pose{' '}
                    {preview.poseGeneration}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.thickness}</dt>
                  <dd>
                    {preview.paperThicknessMm.toLocaleString(numberLocale, {
                      minimumFractionDigits: 2,
                      maximumFractionDigits: 2,
                    })} mm
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.units}</dt>
                  <dd>
                    {copy.sourceUnit}: {unitLabel(preview.sourceUnit)} ·{' '}
                    {copy.encodedUnit}: {unitLabel(preview.encodedUnit)}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.axes}</dt>
                  <dd>
                    {copy.sourceUnit}: {preview.sourceAxis}<br />
                    {copy.encodedUnit}: {preview.encodedAxis}
                  </dd>
                </div>
              </dl>

              <section
                className="crease-export-warnings"
                aria-labelledby="static-mesh-printability-title"
              >
                <h3 id="static-mesh-printability-title">{copy.printabilityTitle}</h3>
                <p><strong>{copy.printabilityStatus[preview.printability.status]}</strong></p>
                <p>
                  {copy.printabilityChecks}:{' '}
                  {[
                    preview.printability.watertight,
                    preview.printability.consistentlyOriented,
                    preview.printability.nonzeroVolume,
                    preview.printability.noDuplicateTriangles,
                    preview.printability.noDegenerateTriangles,
                    preview.printability.conservativeSelfIntersectionClear,
                  ].every(Boolean) ? 'PASS' : 'FAIL / UNKNOWN'}
                </p>
                <p>
                  {copy.printabilityCounts}:{' '}
                  {preview.printability.connectedComponentCount.toLocaleString(numberLocale)}
                  {' / '}
                  {preview.printability.checkedEdgeCount.toLocaleString(numberLocale)}
                  {' / '}
                  {preview.printability.checkedTrianglePairCount.toLocaleString(numberLocale)}
                </p>
                <p>{copy.printabilityDisclaimer}</p>
              </section>

              <section
                className="crease-export-warnings"
                aria-labelledby="static-mesh-export-loss-title"
              >
                <h3 id="static-mesh-export-loss-title">{copy.lossTitle}</h3>
                <ul>
                  {preview.warnings.map((warning) => (
                    <li key={warning}>
                      {staticMeshExportWarningMessage(warning, locale)}
                    </li>
                  ))}
                </ul>
                <label>
                  <input
                    type="checkbox"
                    checked={warningsAcknowledged}
                    disabled={busy}
                    onChange={(event) => {
                      setWarningsAcknowledged(event.currentTarget.checked)
                    }}
                  />
                  {copy.acknowledge}
                </label>
              </section>
            </>
          )}

          <p className="crease-export-notice" role="status" aria-live="polite">
            {notice ?? '\u00a0'}
          </p>
        </div>

        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>
            {copy.cancel}
          </button>
          <button
            type="button"
            className="primary"
            disabled={!canSave}
            onClick={() => onSave(warningsAcknowledged)}
          >
            {busy ? copy.processing : copy.save}
          </button>
        </footer>
      </section>
    </div>
  )
}
