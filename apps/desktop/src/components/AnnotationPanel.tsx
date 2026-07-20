import { useEffect, useState, type FormEvent } from 'react'
import type { AnnotationRecordV1 } from '../lib/coreClient'
import type { LayerRecordV1 } from '../lib/projectLayers'
import type { Locale } from '../lib/i18n'

type Props = {
  locale: Locale
  annotations: readonly AnnotationRecordV1[]
  layers: readonly LayerRecordV1[]
  vertices: readonly { id: string; x: number; y: number }[]
  disabled?: boolean
  onAdd: (record: AnnotationRecordV1) => void
  onUpdate: (record: AnnotationRecordV1) => void
  onRemove: (id: string) => void
}

const DEFAULT_COLOR = { red: 17, green: 24, blue: 39, alpha: 255 }

export function AnnotationPanel({
  locale, annotations, layers, vertices, disabled, onAdd, onUpdate, onRemove,
}: Props) {
  const text = (ja: string, en: string) => locale === 'ja' ? ja : en
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<AnnotationRecordV1 | null>(null)
  const selected = annotations.find(({ id }) => id === selectedId) ?? null
  const layer = layers.find(({ id }) => id === draft?.layer)
  const locked = layer?.locked ?? false

  useEffect(() => {
    if (selected) setDraft(structuredClone(selected))
    else if (selectedId) {
      setSelectedId(null)
      setDraft(null)
    }
  }, [selected, selectedId])

  function createDraft() {
    const firstLayer = layers.find(({ content_kind }) => content_kind === 'annotation')
    if (!firstLayer) return
    setSelectedId(null)
    setDraft({
      id: crypto.randomUUID(),
      text: '',
      anchor: { kind: 'absolute', position: { x: 0, y: 0 } },
      style: { color: DEFAULT_COLOR, font_size_mm: 4, bold: false, italic: false },
      layer: firstLayer.id,
    })
  }

  function submit(event: FormEvent) {
    event.preventDefault()
    if (!draft || !draft.text.trim() || locked) return
    const record = { ...draft, text: draft.text.trim() }
    if (selected) onUpdate(record)
    else onAdd(record)
  }

  const annotationLayers = layers.filter(({ content_kind }) => content_kind === 'annotation')
  return <section className="panel" aria-labelledby="annotation-panel-title">
    <div className="panel-heading">
      <span id="annotation-panel-title">{text('注釈', 'Annotations')}</span>
      <button type="button" onClick={createDraft} disabled={disabled || annotationLayers.length === 0}>
        {text('新規', 'New')}
      </button>
    </div>
    {annotationLayers.length === 0 && <p role="status">
      {text('注釈レイヤーを先に作成してください。', 'Create an annotation layer first.')}
    </p>}
    <ul aria-label={text('注釈一覧', 'Annotation list')}>
      {annotations.map((annotation) => <li key={annotation.id}>
        <button type="button" aria-pressed={annotation.id === selectedId}
          onClick={() => setSelectedId(annotation.id)}>
          {annotation.text}
        </button>
      </li>)}
    </ul>
    {draft && <form onSubmit={submit} aria-label={text('注釈編集', 'Edit annotation')}>
      <label>{text('本文', 'Text')}
        <textarea value={draft.text} maxLength={4000} disabled={disabled || locked}
          onChange={(event) => setDraft({ ...draft, text: event.target.value })} required />
      </label>
      <label>{text('レイヤー', 'Layer')}
        <select value={draft.layer} disabled={disabled || locked}
          onChange={(event) => setDraft({ ...draft, layer: event.target.value })}>
          {annotationLayers.map((item) => <option key={item.id} value={item.id}>
            {item.name}{item.locked ? ` (${text('ロック', 'locked')})` : ''}
          </option>)}
        </select>
      </label>
      <label>{text('基準', 'Anchor')}
        <select value={draft.anchor.kind} disabled={disabled || locked}
          onChange={(event) => setDraft({
            ...draft,
            anchor: event.target.value === 'vertex' && vertices[0]
              ? { kind: 'vertex' as const, vertex: vertices[0].id, offset: { x: 0, y: 0 } }
              : { kind: 'absolute', position: { x: 0, y: 0 } },
          })}>
          <option value="absolute">{text('座標', 'Position')}</option>
          <option value="vertex" disabled={vertices.length === 0}>{text('頂点', 'Vertex')}</option>
        </select>
      </label>
      {draft.anchor.kind === 'vertex' && <label>{text('頂点', 'Vertex')}
        <select value={draft.anchor.vertex} disabled={disabled || locked}
          onChange={(event) => {
            if (draft.anchor.kind !== 'vertex') return
            setDraft({ ...draft, anchor: { kind: 'vertex', offset: draft.anchor.offset, vertex: event.target.value } })
          }}>
          {vertices.map((vertex) => <option key={vertex.id} value={vertex.id}>{vertex.id}</option>)}
        </select>
      </label>}
      {(['x', 'y'] as const).map((axis) => <label key={axis}>{axis.toUpperCase()} (mm)
        <input type="number" step="any" required disabled={disabled || locked}
          value={draft.anchor.kind === 'absolute' ? draft.anchor.position[axis] : draft.anchor.offset[axis]}
          onChange={(event) => {
            const value = Number(event.target.value)
            if (!Number.isFinite(value)) return
            setDraft(draft.anchor.kind === 'absolute'
              ? { ...draft, anchor: { ...draft.anchor, position: { ...draft.anchor.position, [axis]: value } } }
              : { ...draft, anchor: { ...draft.anchor, offset: { ...draft.anchor.offset, [axis]: value } } })
          }} />
      </label>)}
      <label>{text('文字サイズ', 'Font size')} (mm)
        <input type="number" min="0.1" max="1000" step="0.1" required disabled={disabled || locked}
          value={draft.style.font_size_mm}
          onChange={(event) => setDraft({ ...draft, style: { ...draft.style, font_size_mm: Number(event.target.value) } })} />
      </label>
      <label><input type="checkbox" checked={draft.style.bold} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, style: { ...draft.style, bold: event.target.checked } })} />
        {text('太字', 'Bold')}</label>
      <label><input type="checkbox" checked={draft.style.italic} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, style: { ...draft.style, italic: event.target.checked } })} />
        {text('斜体', 'Italic')}</label>
      {locked && <p role="status">{text('このレイヤーはロックされています。', 'This layer is locked.')}</p>}
      <button type="submit" disabled={disabled || locked || !draft.text.trim()}>{text('保存', 'Save')}</button>
      {selected && <button type="button" disabled={disabled || locked}
        onClick={() => onRemove(selected.id)}>{text('削除', 'Delete')}</button>}
    </form>}
  </section>
}
