import { useEffect, useState, type FormEvent } from 'react'
import type { AnnotationRecordV1 } from '../lib/coreClient'
import type { LayerRecordV1 } from '../lib/projectLayers'
import { selectLocalizedText, type Locale } from '../lib/i18n'
import { ANNOTATION_PANEL_TEXT } from '../lib/annotationPanelText.ts'

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
const toHex = (value: number) => Math.max(0, Math.min(255, value)).toString(16).padStart(2, '0')
const colorHex = (color: AnnotationRecordV1['style']['color']) =>
  `#${toHex(color.red)}${toHex(color.green)}${toHex(color.blue)}`
function parseColor(value: string, alpha: number) {
  const match = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/iu.exec(value)
  return match ? {
    red: Number.parseInt(match[1], 16),
    green: Number.parseInt(match[2], 16),
    blue: Number.parseInt(match[3], 16),
    alpha,
  } : null
}

export function AnnotationPanel({
  locale, annotations, layers, vertices, disabled, onAdd, onUpdate, onRemove,
}: Props) {
  const text = (key: keyof typeof ANNOTATION_PANEL_TEXT) =>
    selectLocalizedText(locale, ANNOTATION_PANEL_TEXT[key])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<AnnotationRecordV1 | null>(null)
  const [pendingCreatedId, setPendingCreatedId] = useState<string | null>(null)
  const selected = annotations.find(({ id }) => id === selectedId) ?? null
  const layer = layers.find(({ id }) => id === draft?.layer)
  const locked = layer?.locked ?? false
  const annotationLayers = layers.filter(({ content_kind }) => content_kind === 'annotation')
  const firstUnlockedAnnotationLayer = annotationLayers.find(({ locked: layerLocked }) => !layerLocked)

  useEffect(() => {
    if (selected) setDraft(structuredClone(selected))
    else if (selectedId) {
      setSelectedId(null)
      setDraft(null)
    }
  }, [selected, selectedId])

  useEffect(() => {
    if (!pendingCreatedId) return
    const created = annotations.find(({ id }) => id === pendingCreatedId)
    if (!created) return
    setPendingCreatedId(null)
    setSelectedId(created.id)
    setDraft(structuredClone(created))
  }, [annotations, pendingCreatedId])

  function createDraft() {
    if (!firstUnlockedAnnotationLayer) return
    setPendingCreatedId(null)
    setSelectedId(null)
    setDraft({
      id: crypto.randomUUID(),
      text: '',
      anchor: { kind: 'absolute', position: { x: 0, y: 0 } },
      style: { color: DEFAULT_COLOR, font_size_mm: 4, bold: false, italic: false },
      layer: firstUnlockedAnnotationLayer.id,
    })
  }

  function submit(event: FormEvent) {
    event.preventDefault()
    if (!draft || !draft.text.trim() || locked) return
    const record = { ...draft, text: draft.text.trim() }
    if (selected) onUpdate(record)
    else {
      setPendingCreatedId(record.id)
      onAdd(record)
    }
  }

  return <section className="panel" aria-labelledby="annotation-panel-title">
    <div className="panel-heading">
      <span id="annotation-panel-title">{text('title')}</span>
      <button type="button" onClick={createDraft} disabled={disabled || !firstUnlockedAnnotationLayer}>
        {text('new')}
      </button>
    </div>
    {annotationLayers.length === 0 && <p role="status">
      {text('createLayer')}
    </p>}
    <ul aria-label={text('list')}>
      {annotations.map((annotation) => <li key={annotation.id}>
        <button type="button" aria-pressed={annotation.id === selectedId}
          onClick={() => {
            setPendingCreatedId(null)
            setSelectedId(annotation.id)
          }}>
          {annotation.text}
        </button>
      </li>)}
    </ul>
    {draft && <form onSubmit={submit} aria-label={text('edit')}>
      <label>{text('text')}
        <textarea value={draft.text} maxLength={4000} disabled={disabled || locked}
          onChange={(event) => setDraft({ ...draft, text: event.target.value })} required />
      </label>
      <label>{text('layer')}
        <select value={draft.layer} disabled={disabled || locked}
          onChange={(event) => setDraft({ ...draft, layer: event.target.value })}>
          {annotationLayers.map((item) => <option key={item.id} value={item.id} disabled={item.locked}>
            {item.name}{item.locked ? ` (${text('lockedOption')})` : ''}
          </option>)}
        </select>
      </label>
      <label>{text('anchor')}
        <select value={draft.anchor.kind} disabled={disabled || locked}
          onChange={(event) => setDraft({
            ...draft,
            anchor: event.target.value === 'vertex' && vertices[0]
              ? { kind: 'vertex' as const, vertex: vertices[0].id, offset: { x: 0, y: 0 } }
              : { kind: 'absolute', position: { x: 0, y: 0 } },
          })}>
          <option value="absolute">{text('position')}</option>
          <option value="vertex" disabled={vertices.length === 0}>{text('vertex')}</option>
        </select>
      </label>
      {draft.anchor.kind === 'vertex' && <label>{text('vertex')}
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
      <label>{text('fontSize')} (mm)
        <input type="number" min="0.1" max="1000" step="0.1" required disabled={disabled || locked}
          value={draft.style.font_size_mm}
          onChange={(event) => setDraft({ ...draft, style: { ...draft.style, font_size_mm: Number(event.target.value) } })} />
      </label>
      <label>{text('textColor')}
        <input type="color" value={colorHex(draft.style.color)} disabled={disabled || locked}
          onChange={(event) => {
            const color = parseColor(event.target.value, draft.style.color.alpha)
            if (color) setDraft({ ...draft, style: { ...draft.style, color } })
          }} />
      </label>
      <label>{text('textOpacity')} (%)
        <input type="number" min="0" max="100" step="1" disabled={disabled || locked}
          value={Math.round(draft.style.color.alpha / 255 * 100)}
          onChange={(event) => {
            const percent = Number(event.target.value)
            if (!Number.isFinite(percent)) return
            setDraft({ ...draft, style: {
              ...draft.style,
              color: { ...draft.style.color, alpha: Math.round(Math.max(0, Math.min(100, percent)) * 2.55) },
            } })
          }} />
      </label>
      <label><input type="checkbox" checked={draft.style.bold} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, style: { ...draft.style, bold: event.target.checked } })} />
        {text('bold')}</label>
      <label><input type="checkbox" checked={draft.style.italic} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, style: { ...draft.style, italic: event.target.checked } })} />
        {text('italic')}</label>
      {locked && <p role="status">{text('locked')}</p>}
      <button type="submit" disabled={disabled || locked || !draft.text.trim()}>{text('save')}</button>
      {selected && <button type="button" disabled={disabled || locked}
        onClick={() => onRemove(selected.id)}>{text('delete')}</button>}
    </form>}
  </section>
}
