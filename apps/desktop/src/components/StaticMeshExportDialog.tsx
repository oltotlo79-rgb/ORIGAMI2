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
import { STATIC_MESH_EXPORT_COPY as COPY } from '../lib/staticMeshExportDialogText.ts'

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
            {copy.closeGlyph}
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
                  {option.label}{copy.optionSeparator}{copy.optionDetails[option.value]}
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
                  <dd>{copy.formatSummaries[preview.format]}</dd>
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
                    {preview.faceCount.toLocaleString(copy.numberLocale)} {copy.faces}
                    {copy.metadataSeparator}
                    {preview.vertexCount.toLocaleString(copy.numberLocale)} {copy.vertices}
                    {copy.metadataSeparator}
                    {preview.triangleCount.toLocaleString(copy.numberLocale)} {copy.triangles}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.source}</dt>
                  <dd>
                    {copy.revision} {preview.revision.toLocaleString(copy.numberLocale)}
                    {copy.metadataSeparator}{copy.pose}{' '}
                    {preview.poseGeneration}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.thickness}</dt>
                  <dd>
                    {preview.paperThicknessMm.toLocaleString(copy.numberLocale, {
                      minimumFractionDigits: 2,
                      maximumFractionDigits: 2,
                    })} {copy.millimetres}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.units}</dt>
                  <dd>
                    {copy.sourceUnit}: {copy.unitLabels[preview.sourceUnit]}
                    {copy.metadataSeparator}
                    {copy.encodedUnit}: {copy.unitLabels[preview.encodedUnit]}
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
                  ].every(Boolean) ? copy.pass : copy.failOrUnknown}
                </p>
                <p>
                  {copy.printabilityCounts}:{' '}
                  {preview.printability.connectedComponentCount.toLocaleString(copy.numberLocale)}
                  {' / '}
                  {preview.printability.checkedEdgeCount.toLocaleString(copy.numberLocale)}
                  {' / '}
                  {preview.printability.checkedTrianglePairCount.toLocaleString(copy.numberLocale)}
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
            {notice ?? copy.noticePlaceholder}
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
