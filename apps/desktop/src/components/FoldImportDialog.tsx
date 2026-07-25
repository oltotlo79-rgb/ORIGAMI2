import { useEffect, useMemo, useRef, useState } from 'react'
import {
  foldBoundaryCandidate,
  foldBoundaryCandidateLabel,
  foldBoundaryPreviewEdgeSet,
  foldAssignmentLabel,
  foldImportPreviewFileName,
  foldImportSuggestedName,
  foldImportTargetOptions,
  foldImportTargetLabel,
  foldImportWarningMessage,
  foldPreviewBounds,
  initialFoldBoundaryCandidateId,
  initialFoldImportMapping,
  isFoldImportFallbackName,
  isValidFoldImportName,
  parseFoldImportScale,
  unresolvedFoldAssignments,
  type FoldImportMapping,
  type FoldImportPreview,
  type FoldImportSettings,
  type FoldImportTarget,
} from '../lib/foldImport'
import {
  FOLD_IMPORT_DIALOG_TEXT as FOLD_IMPORT_COPY,
  formatFoldImportAssignmentLabel,
  formatFoldImportBoundaryAssigned,
  formatFoldImportBoundaryEdgeCount,
  formatFoldImportConvertedScale,
  formatFoldImportGeometry,
  formatFoldImportLineCount,
  formatFoldImportUnresolvedAssignments,
} from '../lib/foldImportDialogText.ts'
import { useLocale } from '../lib/i18n.ts'

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
  const locale = useLocale()
  const copy = FOLD_IMPORT_COPY[locale]
  const [name, setName] = useState(preview.suggested_name)
  const [usesFallbackName, setUsesFallbackName] = useState(
    () => isFoldImportFallbackName(preview.suggested_name),
  )
  const displayedName = usesFallbackName
    ? foldImportSuggestedName(preview.suggested_name, locale)
    : name
  const [scaleInput, setScaleInput] = useState(
    preview.default_mm_per_unit === null ? '' : String(preview.default_mm_per_unit),
  )
  const [mapping, setMapping] = useState<FoldImportMapping>(
    () => initialFoldImportMapping(preview.assignments),
  )
  const [boundaryCandidateId, setBoundaryCandidateId] = useState<number | null>(
    () => initialFoldBoundaryCandidateId(preview),
  )
  const [warningsAcknowledged, setWarningsAcknowledged] = useState(
    preview.warnings.length === 0,
  )
  const dialogRef = useRef<HTMLElement>(null)
  const nameInputRef = useRef<HTMLInputElement>(null)
  const scale = parseFoldImportScale(scaleInput)
  const unresolved = unresolvedFoldAssignments(preview.assignments, mapping)
  const selectedBoundary = foldBoundaryCandidate(preview, boundaryCandidateId)
  const selectedBoundaryEdges = useMemo(
    () => foldBoundaryPreviewEdgeSet(preview, boundaryCandidateId),
    [boundaryCandidateId, preview],
  )
  const nameIsValid = isValidFoldImportName(displayedName)
  const canImport = !busy
    && nameIsValid
    && scale !== null
    && selectedBoundary !== null
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
    if (!canImport || scale === null || selectedBoundary === null) return
    onImport({
      importId: preview.import_id,
      name: displayedName.trim(),
      mmPerUnit: scale,
      mappings: mapping,
      boundaryCandidateId: selectedBoundary.id,
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
            <span className="dialog-eyebrow">{copy.eyebrow}</span>
            <h2 id="fold-import-title">{copy.title}</h2>
          </div>
          <button
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label={copy.close}
          >
            {copy.closeGlyph}
          </button>
        </header>

        <div className="fold-import-dialog-body">
          <p id="fold-import-description" className="dialog-note">
            {copy.description}
          </p>

          <div className="fold-import-overview">
            <div className="fold-import-preview">
              {bounds ? (
                <svg
                  viewBox={`${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`}
                  role="img"
                  aria-label={copy.preview}
                  preserveAspectRatio="xMidYMid meet"
                >
                  {preview.preview_edges.map((edge, index) => {
                    const start = preview.preview_vertices[edge.start]
                    const end = preview.preview_vertices[edge.end]
                    if (!start || !end) return null
                    return (
                      <line
                        key={`${edge.source_index}:${edge.start}:${edge.end}:${index}`}
                        className={[
                          'fold-preview-edge',
                          `assignment-${edge.assignment.toLowerCase()}`,
                          selectedBoundaryEdges.has(edge.source_index)
                            ? 'is-selected-boundary'
                            : '',
                        ].filter(Boolean).join(' ')}
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
                <p>{copy.previewUnavailable}</p>
              )}
              {preview.preview_truncated && (
                <span>{copy.previewTruncated}</span>
              )}
            </div>
            <dl className="fold-import-metadata">
              <div>
                <dt>{copy.metadata.file}</dt>
                <dd>{foldImportPreviewFileName(preview.file_name, locale)}</dd>
              </div>
              <div>
                <dt>{copy.metadata.specification}</dt>
                <dd>{preview.file_spec ?? copy.unspecified}</dd>
              </div>
              <div>
                <dt>{copy.metadata.unit}</dt>
                <dd>{preview.frame_unit ?? copy.unspecified}</dd>
              </div>
              <div>
                <dt>{copy.metadata.geometry}</dt>
                <dd>{formatFoldImportGeometry(
                  preview.vertex_count,
                  preview.edge_count,
                  locale,
                )}</dd>
              </div>
              <div>
                <dt>{copy.metadata.boundary}</dt>
                <dd>{formatFoldImportBoundaryEdgeCount(
                  selectedBoundary?.edge_indices.length
                    ?? preview.boundary_edge_count,
                  locale,
                )}</dd>
              </div>
            </dl>
          </div>

          <div className="fold-import-fields">
            <label className="dialog-field">
              <span>{copy.name}</span>
              <input
                ref={nameInputRef}
                value={displayedName}
                maxLength={120}
                disabled={busy}
                aria-invalid={!nameIsValid}
                aria-describedby={!nameIsValid ? 'fold-import-name-help' : undefined}
                onChange={(event) => {
                  setUsesFallbackName(false)
                  setName(event.target.value)
                }}
              />
              {!nameIsValid && (
                <small id="fold-import-name-help">
                  {copy.invalidName}
                </small>
              )}
            </label>
            <label className="dialog-field">
              <span>{copy.scale}</span>
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
                {copy.millimetresUnit}
              </span>
              <small id="fold-import-scale-help">
                {preview.default_mm_per_unit === null
                  ? copy.missingScale
                  : formatFoldImportConvertedScale(preview.frame_unit, locale)}
              </small>
            </label>
          </div>

          <section
            className="fold-import-boundary"
            aria-labelledby="fold-import-boundary-title"
          >
            <h3 id="fold-import-boundary-title">{copy.boundaryTitle}</h3>
            <p>{copy.boundaryDescription}</p>
            {preview.boundary_candidates.length === 0 ? (
              <p className="fold-import-attention" role="alert">
                {copy.boundaryUnavailable}
              </p>
            ) : preview.fixed_boundary_candidate_id !== null ? (
              <p className="fold-import-fixed-mapping">
                {selectedBoundary
                  ? formatFoldImportBoundaryAssigned(
                      foldBoundaryCandidateLabel(selectedBoundary, locale),
                      locale,
                    )
                  : copy.boundaryUnavailable}
              </p>
            ) : (
              <fieldset disabled={busy}>
                <legend>{copy.boundarySelect}</legend>
                {preview.boundary_candidates.map((candidate) => (
                  <label key={candidate.id}>
                    <input
                      type="radio"
                      name="fold_boundary_candidate"
                      value={candidate.id}
                      checked={boundaryCandidateId === candidate.id}
                      onChange={() => setBoundaryCandidateId(candidate.id)}
                    />
                    {foldBoundaryCandidateLabel(candidate, locale)}
                  </label>
                ))}
              </fieldset>
            )}
          </section>

          <section className="fold-import-mapping" aria-labelledby="fold-import-mapping-title">
            <h3 id="fold-import-mapping-title">{copy.mappingTitle}</h3>
            <p>
              {copy.mappingDescription}
            </p>
            <div className="fold-import-mapping-list">
              {preview.assignments.map(({ assignment, count }) => (
                <label key={assignment}>
                  <span>
                    {foldAssignmentLabel(assignment, locale)}
                    {copy.assignmentCountSeparator}
                    <b>
                      {formatFoldImportLineCount(count, locale)}
                    </b>
                  </span>
                  {assignment === 'B' ? (
                    <span className="fold-import-fixed-mapping">
                      {copy.boundaryFixed}
                    </span>
                  ) : (
                    <select
                      value={mapping[assignment] ?? ''}
                      disabled={busy}
                      aria-label={
                        formatFoldImportAssignmentLabel(
                          foldAssignmentLabel(assignment, locale),
                          locale,
                        )
                      }
                      onChange={(event) => {
                        const value = event.target.value as FoldImportTarget | ''
                        setMapping((current) => ({
                          ...current,
                          [assignment]: value || undefined,
                        }))
                      }}
                    >
                      <option value="">{copy.select}</option>
                      {foldImportTargetOptions(assignment).map((option) => (
                        <option key={option.value} value={option.value}>
                          {foldImportTargetLabel(option.value, locale)}
                        </option>
                      ))}
                    </select>
                  )}
                </label>
              ))}
            </div>
            {unresolved.length > 0 && (
              <p className="fold-import-attention" role="status">
                {formatFoldImportUnresolvedAssignments(
                  unresolved.map(
                    (assignment) => foldAssignmentLabel(assignment, locale),
                  ),
                  locale,
                )}
              </p>
            )}
          </section>

          {preview.warnings.length > 0 && (
            <section className="fold-import-warnings" aria-labelledby="fold-import-warnings-title">
              <h3 id="fold-import-warnings-title">{copy.warningTitle}</h3>
              <ul>
                {preview.warnings.map((warning, index) => (
                  <li key={index}>{foldImportWarningMessage(warning, locale)}</li>
                ))}
              </ul>
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={warningsAcknowledged}
                  disabled={busy}
                  onChange={(event) => setWarningsAcknowledged(event.target.checked)}
                />
                {copy.acknowledge}
              </label>
            </section>
          )}

          {error && <p className="dialog-error" role="alert">{error}</p>}
        </div>

        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>
            {copy.cancel}
          </button>
          <button type="button" className="primary" disabled={!canImport} onClick={submit}>
            {busy ? copy.importing : copy.import}
          </button>
        </footer>
      </section>
    </div>
  )
}
