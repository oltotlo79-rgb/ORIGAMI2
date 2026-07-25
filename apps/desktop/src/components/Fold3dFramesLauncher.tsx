import { useEffect, useRef, useState } from 'react'
import {
  cancelFold3dFrames,
  applyFold3dAppliedPose,
  pickFold3dFrames,
  prepareFold3dAppliedPose,
  prepareFold3dInstructionTimeline,
  applyFold3dInstructionTimeline,
  selectFold3dFrame,
  type Fold3dFrameSelection,
  type Fold3dFramesMetadata,
  type Fold3dPoseCompatibility,
  type Fold3dTimelineCompatibility,
} from '../lib/fold3dFrames.ts'
import {
  FOLD3D_FRAMES_LAUNCHER_TEXT as TEXT,
} from '../lib/fold3dFramesLauncherText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type LocaleStore,
  useLocale,
} from '../lib/i18n.ts'

type ErrorTextKey =
  | 'openError'
  | 'timelineError'
  | 'selectionError'
  | 'poseError'

export function Fold3dFramesLauncher({
  disabled,
  onApplied,
  localeStore,
}: Readonly<{
  disabled: boolean
  onApplied?(): void | Promise<void>
  localeStore?: LocaleStore
}>) {
  const locale = useLocale(localeStore)
  const [preview, setPreview] = useState<Fold3dFramesMetadata | null>(null)
  const [selection, setSelection] = useState<Fold3dFrameSelection | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorTextKey | null>(null)
  const [compatibility, setCompatibility] = useState<Fold3dPoseCompatibility | null>(null)
  const [confirmed, setConfirmed] = useState(false)
  const [applied, setApplied] = useState(false)
  const [timeline, setTimeline] = useState<Fold3dTimelineCompatibility | null>(null)
  const [timelineConfirmed, setTimelineConfirmed] = useState(false)
  const launcher = useRef<HTMLButtonElement>(null)
  const dialog = useRef<HTMLElement>(null)

  useEffect(() => {
    if (!preview) return
    dialog.current?.focus()
    const background = Array.from(document.querySelectorAll<HTMLElement>('header, main, footer'))
    background.forEach((element) => { element.inert = true })
    return () => background.forEach((element) => { element.inert = false })
  }, [preview])

  async function open() {
    if (busy) return
    setBusy(true); setError(null)
    try {
      const result = await pickFold3dFrames()
      if (!result.canceled && result.preview) {
        setPreview(result.preview)
        setSelection(await selectFold3dFrame(result.preview, 0))
        setCompatibility(await prepareFold3dAppliedPose(result.preview, 0))
        setTimeline(await prepareFold3dInstructionTimeline(result.preview).catch(() => null))
      }
    } catch {
      setError('openError')
    } finally { setBusy(false) }
  }

  async function close() {
    const token = preview?.token
    setPreview(null); setSelection(null); setCompatibility(null); setTimeline(null); setError(null)
    if (token) await cancelFold3dFrames(token).catch(() => undefined)
    requestAnimationFrame(() => launcher.current?.focus())
  }

  async function applyTimeline() {
    if (!preview || !timeline || !timelineConfirmed || busy) return
    setBusy(true); setError(null)
    try {
      await applyFold3dInstructionTimeline(preview, timeline.durationMs)
      await onApplied?.()
      setPreview(null)
      requestAnimationFrame(() => launcher.current?.focus())
    } catch {
      setTimeline(null)
      setError('timelineError')
    } finally { setBusy(false) }
  }

  async function choose(index: number) {
    if (!preview || busy) return
    setBusy(true); setError(null)
    try {
      setSelection(await selectFold3dFrame(preview, index))
      setCompatibility(await prepareFold3dAppliedPose(preview, index))
      setConfirmed(false); setApplied(false)
    }
    catch { setError('selectionError') }
    finally { setBusy(false) }
  }

  async function applyPose() {
    if (!preview || !selection || !compatibility || !confirmed || busy) return
    setBusy(true); setError(null)
    try {
      await applyFold3dAppliedPose(preview, selection.frameIndex)
      await onApplied?.()
      setApplied(true)
    } catch {
      setCompatibility(null)
      setError('poseError')
    } finally { setBusy(false) }
  }

  function trapFocus(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape' && !busy) { void close(); return }
    if (event.key !== 'Tab') return
    const items = Array.from(dialog.current?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])',
    ) ?? [])
    const first = items[0]
    const last = items.at(-1)
    if (!first || !last) return
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault(); last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus()
    }
  }

  return <>
    <button ref={launcher} type="button" disabled={disabled || busy}
      aria-haspopup="dialog" onClick={() => void open()}>
      {selectLocalizedText(locale, TEXT.launcher)}
    </button>
    {preview && <div className="dialog-backdrop">
      <section ref={dialog} className="new-project-dialog" role="dialog" aria-modal="true"
        tabIndex={-1} aria-labelledby="fold-3d-title" aria-busy={busy}
        onKeyDown={trapFocus}>
        <header><h2 id="fold-3d-title">{selectLocalizedText(locale, TEXT.title)}</h2>
          <button type="button" disabled={busy}
            aria-label={selectLocalizedText(locale, TEXT.close)}
            onClick={() => void close()}>×</button></header>
        <p>{selectLocalizedText(locale, TEXT.readOnlyExplanation)}</p>
        <label>{selectLocalizedText(locale, TEXT.frame)}
          <select value={selection?.frameIndex ?? 0} disabled={busy}
            onChange={(event) => void choose(Number(event.target.value))}>
            {preview.frames.map((frame) => <option key={frame.index} value={frame.index}>
              {formatLocalizedText(locale, TEXT.frameOption, {
                index: frame.index + 1,
                vertexCount: frame.vertexCount,
              })}
            </option>)}
          </select>
        </label>
        {selection && <img src={selection.previewImageDataUrl}
          width={selection.previewWidth} height={selection.previewHeight}
          alt={formatLocalizedText(locale, TEXT.framePreviewAlt, {
            index: selection.frameIndex + 1,
          })} />}
        <p role="status">
          {compatibility
            ? formatLocalizedText(locale, TEXT.compatiblePose, {
                hingeCount: compatibility.hingeCount,
              })
            : selectLocalizedText(locale, TEXT.incompatiblePose)}
        </p>
        {compatibility && <>
          <label><input type="checkbox" checked={confirmed} disabled={busy || applied}
            onChange={(event) => setConfirmed(event.target.checked)} />
            {selectLocalizedText(locale, TEXT.confirmPoseReplacement)}
          </label>
          <p>{selectLocalizedText(locale, TEXT.poseHistoryExplanation)}</p>
          <button type="button" disabled={busy || !confirmed || applied}
            onClick={() => void applyPose()}>
            {selectLocalizedText(
              locale,
              applied ? TEXT.poseApplied : TEXT.applyPose,
            )}
          </button>
        </>}
        {timeline && <section>
          <h3>{selectLocalizedText(locale, TEXT.timelineTitle)}</h3>
          <p>{formatLocalizedText(locale, TEXT.timelineSummary, {
            frameCount: timeline.frameCount,
          })}</p>
          <label><input type="checkbox" checked={timelineConfirmed} disabled={busy}
            onChange={(event) => setTimelineConfirmed(event.target.checked)} />
            {selectLocalizedText(locale, TEXT.confirmTimeline)}
          </label>
          <button type="button" disabled={busy || !timelineConfirmed}
            onClick={() => void applyTimeline()}>
            {selectLocalizedText(locale, TEXT.applyTimeline)}
          </button>
        </section>}
        {error && <p role="alert">{selectLocalizedText(locale, TEXT[error])}</p>}
        <button type="button" disabled={busy} onClick={() => void close()}>
          {selectLocalizedText(locale, TEXT.close)}
        </button>
      </section>
    </div>}
  </>
}
