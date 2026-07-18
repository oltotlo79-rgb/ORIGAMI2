import { useEffect, useMemo, useRef, useState } from 'react'
import {
  foldAssignmentLabel,
  foldImportTargetOptions,
  foldPreviewBounds,
  initialFoldImportMapping,
  isValidFoldImportName,
  parseFoldImportScale,
  unresolvedFoldAssignments,
  type FoldImportMapping,
  type FoldImportPreview,
  type FoldImportSettings,
  type FoldImportTarget,
} from '../lib/foldImport'

type FoldImportDialogProps = Readonly<{
  preview: FoldImportPreview
  busy: boolean
  error: string | null
  onCancel: () => void
  onImport: (settings: FoldImportSettings) => void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function FoldImportDialog({
  preview,
  busy,
  error,
  onCancel,
  onImport,
}: FoldImportDialogProps) {
  const [name, setName] = useState(preview.suggested_name)
  const [scaleInput, setScaleInput] = useState(
    preview.default_mm_per_unit === null ? '' : String(preview.default_mm_per_unit),
  )
  const [mapping, setMapping] = useState<FoldImportMapping>(
    () => initialFoldImportMapping(preview.assignments),
  )
  const [warningsAcknowledged, setWarningsAcknowledged] = useState(
    preview.warnings.length === 0,
  )
  const dialogRef = useRef<HTMLElement>(null)
  const nameInputRef = useRef<HTMLInputElement>(null)
  const scale = parseFoldImportScale(scaleInput)
  const unresolved = unresolvedFoldAssignments(preview.assignments, mapping)
  const nameIsValid = isValidFoldImportName(name)
  const canImport = !busy
    && nameIsValid
    && scale !== null
    && unresolved.length === 0
    && warningsAcknowledged
  const bounds = useMemo(
    () => foldPreviewBounds(preview.preview_vertices),
    [preview.preview_vertices],
  )

  useEffect(() => {
    nameInputRef.current?.focus()
  }, [])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) {
        event.preventDefault()
        onCancel()
        return
      }
      if (event.key !== 'Tab') return
      const dialog = dialogRef.current
      if (!dialog) return
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
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
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [busy, onCancel])

  const submit = () => {
    if (!canImport || scale === null) return
    onImport({
      importId: preview.import_id,
      name: name.trim(),
      mmPerUnit: scale,
      mappings: mapping,
    })
  }

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="fold-import-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="fold-import-title"
        aria-describedby="fold-import-description"
        tabIndex={-1}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">FOLD 1.0–1.2 取込</span>
            <h2 id="fold-import-title">線種と縮尺を確認</h2>
          </div>
          <button
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label="閉じる"
          >
            ×
          </button>
        </header>

        <div className="fold-import-dialog-body">
          <p id="fold-import-description" className="dialog-note">
            元のFOLDファイルは変更しません。確認後、編集可能な未保存プロジェクトとして取り込みます。
          </p>

          <div className="fold-import-overview">
            <div className="fold-import-preview">
              {bounds ? (
                <svg
                  viewBox={`${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`}
                  role="img"
                  aria-label="取り込む展開図のプレビュー"
                  preserveAspectRatio="xMidYMid meet"
                >
                  {preview.preview_edges.map((edge, index) => {
                    const start = preview.preview_vertices[edge.start]
                    const end = preview.preview_vertices[edge.end]
                    if (!start || !end) return null
                    return (
                      <line
                        key={`${edge.start}:${edge.end}:${index}`}
                        className={`fold-preview-edge assignment-${edge.assignment.toLowerCase()}`}
                        x1={start.x}
                        y1={start.y}
                        x2={end.x}
                        y2={end.y}
                        vectorEffect="non-scaling-stroke"
                      />
                    )
                  })}
                </svg>
              ) : (
                <p>プレビューを表示できません。</p>
              )}
              {preview.preview_truncated && (
                <span>表示用に一部の線だけを描画しています。</span>
              )}
            </div>
            <dl className="fold-import-metadata">
              <div><dt>ファイル</dt><dd>{preview.file_name}</dd></div>
              <div><dt>仕様</dt><dd>{preview.file_spec ?? '記載なし'}</dd></div>
              <div><dt>単位</dt><dd>{preview.frame_unit ?? '記載なし'}</dd></div>
              <div>
                <dt>形状</dt>
                <dd>{preview.vertex_count.toLocaleString()}頂点・{preview.edge_count.toLocaleString()}辺</dd>
              </div>
              <div><dt>境界</dt><dd>{preview.boundary_edge_count.toLocaleString()}辺</dd></div>
            </dl>
          </div>

          <div className="fold-import-fields">
            <label className="dialog-field">
              <span>作品名</span>
              <input
                ref={nameInputRef}
                value={name}
                maxLength={120}
                disabled={busy}
                aria-invalid={!nameIsValid}
                aria-describedby={!nameIsValid ? 'fold-import-name-help' : undefined}
                onChange={(event) => setName(event.target.value)}
              />
              {!nameIsValid && (
                <small id="fold-import-name-help">
                  制御文字を含まない120文字以内の名前が必要です。
                </small>
              )}
            </label>
            <label className="dialog-field">
              <span>1 FOLD単位の長さ</span>
              <span className="number-with-unit">
                <input
                  value={scaleInput}
                  type="number"
                  min="0"
                  max="1000000000"
                  step="any"
                  disabled={busy}
                  aria-invalid={scale === null}
                  aria-describedby="fold-import-scale-help"
                  onChange={(event) => setScaleInput(event.target.value)}
                />
                mm
              </span>
              <small id="fold-import-scale-help">
                {preview.default_mm_per_unit === null
                  ? '単位情報がないため、実寸への換算値を指定してください。'
                  : `${preview.frame_unit ?? '元の単位'}から換算した値です。必要なら変更できます。`}
              </small>
            </label>
          </div>

          <section className="fold-import-mapping" aria-labelledby="fold-import-mapping-title">
            <h3 id="fold-import-mapping-title">線種の割当</h3>
            <p>
              F・U・JはORIGAMI2に同じ意味の線種がないため、用途を明示的に選んでください。
            </p>
            <div className="fold-import-mapping-list">
              {preview.assignments.map(({ assignment, count }) => (
                <label key={assignment}>
                  <span>{foldAssignmentLabel(assignment)} <b>{count.toLocaleString()}本</b></span>
                  {assignment === 'B' ? (
                    <span className="fold-import-fixed-mapping">用紙境界（固定）</span>
                  ) : (
                    <select
                      value={mapping[assignment] ?? ''}
                      disabled={busy}
                      aria-label={`${foldAssignmentLabel(assignment)}の割当`}
                      onChange={(event) => {
                        const value = event.target.value as FoldImportTarget | ''
                        setMapping((current) => ({
                          ...current,
                          [assignment]: value || undefined,
                        }))
                      }}
                    >
                      <option value="">選択してください</option>
                      {foldImportTargetOptions(assignment).map((option) => (
                        <option key={option.value} value={option.value}>{option.label}</option>
                      ))}
                    </select>
                  )}
                </label>
              ))}
            </div>
            {unresolved.length > 0 && (
              <p className="fold-import-attention" role="status">
                未選択: {unresolved.map(foldAssignmentLabel).join('、')}
              </p>
            )}
          </section>

          {preview.warnings.length > 0 && (
            <section className="fold-import-warnings" aria-labelledby="fold-import-warnings-title">
              <h3 id="fold-import-warnings-title">取り込まれない情報</h3>
              <ul>
                {preview.warnings.map((warning, index) => <li key={index}>{warning}</li>)}
              </ul>
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={warningsAcknowledged}
                  disabled={busy}
                  onChange={(event) => setWarningsAcknowledged(event.target.checked)}
                />
                上記を確認し、展開図として取り込む
              </label>
            </section>
          )}

          {error && <p className="dialog-error" role="alert">{error}</p>}
        </div>

        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>キャンセル</button>
          <button type="button" className="primary" disabled={!canImport} onClick={submit}>
            {busy ? '取込中…' : '取り込む'}
          </button>
        </footer>
      </section>
    </div>
  )
}
