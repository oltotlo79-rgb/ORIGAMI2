import { useEffect, useRef, useState } from 'react'

import { useLocale } from '../lib/i18n.ts'
import type { MeshAnimationPreviewResponse } from '../lib/meshAnimationExport.ts'

type Props = Readonly<{
  preview: MeshAnimationPreviewResponse | null
  busy: boolean
  error: string | null
  notice: string | null
  onRetry(): void
  onSave(): void
  onCancel(): void
}>

const COPY = {
  ja: {
    title: '手順アニメーションを書き出す',
    description: '認証済みの手順タイムラインを glTF 2.0 の GLB アニメーションとして保存します。',
    warning: '重要: キーフレーム間は線形補間です。紙厚、テクスチャ、衝突保証、編集可能な手順情報は含まれません。',
    acknowledge: '制限と情報損失を確認しました',
    frames: 'フレーム',
    duration: '再生時間',
    geometry: '形状',
    size: 'サイズ',
    name: '保存名',
    retry: '現在の手順から再作成',
    cancel: 'キャンセル',
    save: '保存先を選ぶ',
    processing: '処理中…',
  },
  en: {
    title: 'Export instruction animation',
    description: 'Save the authenticated instruction timeline as a glTF 2.0 GLB animation.',
    warning: 'Important: keyframes use linear interpolation. Paper thickness, textures, collision guarantees, and editable instruction semantics are not included.',
    acknowledge: 'I understand the limitations and information loss',
    frames: 'Frames',
    duration: 'Duration',
    geometry: 'Geometry',
    size: 'Size',
    name: 'Suggested name',
    retry: 'Rebuild from current instructions',
    cancel: 'Cancel',
    save: 'Choose destination',
    processing: 'Processing…',
  },
} as const

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
