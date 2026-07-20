import { useEffect, useRef, useState } from 'react'
import {
  cancelFold3dFrames,
  pickFold3dFrames,
  selectFold3dFrame,
  type Fold3dFrameSelection,
  type Fold3dFramesMetadata,
} from '../lib/fold3dFrames.ts'
import { useLocale } from '../lib/i18n.ts'

export function Fold3dFramesLauncher({ disabled }: Readonly<{ disabled: boolean }>) {
  const locale = useLocale()
  const en = locale.startsWith('en')
  const [preview, setPreview] = useState<Fold3dFramesMetadata | null>(null)
  const [selection, setSelection] = useState<Fold3dFrameSelection | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
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
      }
    } catch {
      setError(en ? 'The FOLD 3D preview became stale or invalid.' : 'FOLD 3Dプレビューが古いか無効です。')
    } finally { setBusy(false) }
  }

  async function close() {
    const token = preview?.token
    setPreview(null); setSelection(null); setError(null)
    if (token) await cancelFold3dFrames(token).catch(() => undefined)
    requestAnimationFrame(() => launcher.current?.focus())
  }

  async function choose(index: number) {
    if (!preview || busy) return
    setBusy(true); setError(null)
    try { setSelection(await selectFold3dFrame(preview, index)) }
    catch { setError(en ? 'This preview is stale. Close and retry.' : 'プレビューが古くなりました。閉じて再試行してください。') }
    finally { setBusy(false) }
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
      {en ? 'Preview FOLD 3D frames' : 'FOLD 3Dフレームをプレビュー'}
    </button>
    {preview && <div className="dialog-backdrop">
      <section ref={dialog} className="new-project-dialog" role="dialog" aria-modal="true"
        tabIndex={-1} aria-labelledby="fold-3d-title" aria-busy={busy}
        onKeyDown={trapFocus}>
        <header><h2 id="fold-3d-title">{en ? 'FOLD 3D frame preview' : 'FOLD 3Dフレームプレビュー'}</h2>
          <button type="button" disabled={busy} aria-label={en ? 'Close' : '閉じる'}
            onClick={() => void close()}>×</button></header>
        <p>{en ? 'Read-only preview. This never imports or changes the project.'
          : '読み取り専用プレビューです。プロジェクトの取込・変更は行いません。'}</p>
        <label>{en ? 'Frame' : 'フレーム'}
          <select value={selection?.frameIndex ?? 0} disabled={busy}
            onChange={(event) => void choose(Number(event.target.value))}>
            {preview.frames.map((frame) => <option key={frame.index} value={frame.index}>
              {en ? `Frame ${frame.index + 1} · ${frame.vertexCount} vertices`
                : `フレーム ${frame.index + 1}・頂点 ${frame.vertexCount}`}
            </option>)}
          </select>
        </label>
        {selection && <img src={selection.previewImageDataUrl}
          width={selection.previewWidth} height={selection.previewHeight}
          alt={en ? `Native preview of frame ${selection.frameIndex + 1}`
            : `フレーム ${selection.frameIndex + 1} のネイティブプレビュー`} />}
        {error && <p role="alert">{error}</p>}
        <button type="button" disabled={busy} onClick={() => void close()}>
          {en ? 'Close' : '閉じる'}
        </button>
      </section>
    </div>}
  </>
}
