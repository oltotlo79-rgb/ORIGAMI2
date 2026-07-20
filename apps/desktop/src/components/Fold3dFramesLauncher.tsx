import { useEffect, useRef, useState } from 'react'
import {
  cancelFold3dFrames,
  applyFold3dAppliedPose,
  pickFold3dFrames,
  prepareFold3dAppliedPose,
  selectFold3dFrame,
  type Fold3dFrameSelection,
  type Fold3dFramesMetadata,
  type Fold3dPoseCompatibility,
} from '../lib/fold3dFrames.ts'
import { useLocale } from '../lib/i18n.ts'

export function Fold3dFramesLauncher({ disabled, onApplied }: Readonly<{
  disabled: boolean
  onApplied?(): void | Promise<void>
}>) {
  const locale = useLocale()
  const en = locale.startsWith('en')
  const [preview, setPreview] = useState<Fold3dFramesMetadata | null>(null)
  const [selection, setSelection] = useState<Fold3dFrameSelection | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [compatibility, setCompatibility] = useState<Fold3dPoseCompatibility | null>(null)
  const [confirmed, setConfirmed] = useState(false)
  const [applied, setApplied] = useState(false)
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
      }
    } catch {
      setError(en ? 'The FOLD 3D preview became stale or invalid.' : 'FOLD 3Dプレビューが古いか無効です。')
    } finally { setBusy(false) }
  }

  async function close() {
    const token = preview?.token
    setPreview(null); setSelection(null); setCompatibility(null); setError(null)
    if (token) await cancelFold3dFrames(token).catch(() => undefined)
    requestAnimationFrame(() => launcher.current?.focus())
  }

  async function choose(index: number) {
    if (!preview || busy) return
    setBusy(true); setError(null)
    try {
      setSelection(await selectFold3dFrame(preview, index))
      setCompatibility(await prepareFold3dAppliedPose(preview, index))
      setConfirmed(false); setApplied(false)
    }
    catch { setError(en ? 'This preview is stale. Close and retry.' : 'プレビューが古くなりました。閉じて再試行してください。') }
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
      setError(en ? 'The project or pose changed. Close and retry.'
        : 'プロジェクトまたは姿勢が変更されました。閉じて再試行してください。')
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
        <p role="status">
          {compatibility
            ? (en ? `Compatible native tree pose · ${compatibility.hingeCount} hinges`
              : `互換性のあるネイティブ木構造姿勢・ヒンジ ${compatibility.hingeCount}`)
            : (en ? 'Not compatible with the current native model.'
              : '現在のネイティブモデルとは互換性がありません。')}
        </p>
        {compatibility && <>
          <label><input type="checkbox" checked={confirmed} disabled={busy || applied}
            onChange={(event) => setConfirmed(event.target.checked)} />
            {en
              ? 'Replace only the current 3D pose. Project geometry and revision stay unchanged.'
              : '現在の3D姿勢だけを置換します。プロジェクト形状とrevisionは変更しません。'}
          </label>
          <p>{en
            ? 'This pose adoption is not an editor geometry command. Editor Undo/Redo does not create a separate geometry-history entry.'
            : '姿勢の適用は形状編集コマンドではありません。エディタの元に戻す／やり直すに形状履歴は追加されません。'}</p>
          <button type="button" disabled={busy || !confirmed || applied}
            onClick={() => void applyPose()}>
            {applied ? (en ? 'Pose applied' : '姿勢を適用しました')
              : (en ? 'Apply current 3D pose' : '現在の3D姿勢へ適用')}
          </button>
        </>}
        {error && <p role="alert">{error}</p>}
        <button type="button" disabled={busy} onClick={() => void close()}>
          {en ? 'Close' : '閉じる'}
        </button>
      </section>
    </div>}
  </>
}
