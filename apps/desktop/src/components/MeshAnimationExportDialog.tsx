import { useEffect, useRef, useState } from 'react'

import { useLocale } from '../lib/i18n.ts'
import type { MeshAnimationPreviewResponse } from '../lib/meshAnimationExport.ts'
import { MESH_ANIMATION_EXPORT_DIALOG_TEXT as COPY } from '../lib/meshAnimationExportDialogText.ts'

type Props = Readonly<{
  preview: MeshAnimationPreviewResponse | null
  busy: boolean
  error: string | null
  notice: string | null
  onRetry(): void
  onSave(): void
  onCancel(): void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function MeshAnimationExportDialog({
  preview,
  busy,
  error,
  notice,
  onRetry,
  onSave,
  onCancel,
}: Props) {
  const locale = useLocale()
  const copy = COPY[locale]
  const [acknowledged, setAcknowledged] = useState(false)
  const dialogRef = useRef<HTMLElement>(null)
  useEffect(() => setAcknowledged(false), [preview])
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) {
        event.preventDefault()
        onCancel()
        return
      }
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
    document.addEventListener('keydown', onKey, true)
    return () => document.removeEventListener('keydown', onKey, true)
  }, [busy, onCancel])
  useEffect(() => {
    const frame = requestAnimationFrame(() => dialogRef.current?.focus())
    return () => cancelAnimationFrame(frame)
  }, [])
  const numberLocale = locale === 'ja' ? 'ja-JP' : 'en-US'
  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="crease-export-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="mesh-animation-export-title"
        aria-busy={busy}
        tabIndex={-1}
      >
        <header>
          <div><h2 id="mesh-animation-export-title">{copy.title}</h2></div>
          <button type="button" disabled={busy} onClick={onCancel} aria-label={copy.cancel}>×</button>
        </header>
        <div className="crease-export-dialog-body">
          <p className="dialog-note">{copy.description}</p>
          {busy && !preview && <p role="status">{copy.processing}</p>}
          {error && (
            <div className="crease-export-error">
              <p className="dialog-error" role="alert">{error}</p>
              {!busy && <button type="button" onClick={onRetry}>{copy.retry}</button>}
            </div>
          )}
          {preview && (
            <>
              <dl className="crease-export-metadata">
                <div><dt>{copy.name}</dt><dd>{preview.suggestedFileName}</dd></div>
                <div><dt>{copy.frames}</dt><dd>{preview.frameCount.toLocaleString(numberLocale)}</dd></div>
                <div><dt>{copy.duration}</dt><dd>{preview.durationSeconds.toLocaleString(numberLocale)} s</dd></div>
                <div><dt>{copy.geometry}</dt><dd>{preview.vertexCount.toLocaleString(numberLocale)} vertices · {preview.triangleCount.toLocaleString(numberLocale)} triangles</dd></div>
                <div><dt>{copy.size}</dt><dd>{preview.byteCount.toLocaleString(numberLocale)} bytes</dd></div>
              </dl>
              <section className="crease-export-warnings">
                <p>{copy.warning}</p>
                <label>
                  <input
                    type="checkbox"
                    checked={acknowledged}
                    disabled={busy}
                    onChange={(event) => setAcknowledged(event.currentTarget.checked)}
                  />
                  {copy.acknowledge}
                </label>
              </section>
            </>
          )}
          <p role="status" aria-live="polite">{notice ?? '\u00a0'}</p>
        </div>
        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>{copy.cancel}</button>
          <button type="button" className="primary" disabled={busy || !preview || !acknowledged} onClick={onSave}>
            {busy ? copy.processing : copy.save}
          </button>
        </footer>
      </section>
    </div>
  )
}
