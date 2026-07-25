import { useState, type FormEvent } from 'react'
import type { UnderlayRecordV1 } from '../lib/coreClient'
import type { LayerRecordV1 } from '../lib/projectLayers'
import { formatLocalizedText, selectLocalizedText, type Locale } from '../lib/i18n'
import { UNDERLAY_PANEL_TEXT } from '../lib/underlayPanelText.ts'

type Props = {
  locale: Locale
  underlays: readonly UnderlayRecordV1[]
  layers: readonly LayerRecordV1[]
  disabled?: boolean
  onImport: (draft: Omit<UnderlayRecordV1, 'asset'>) => void
  onUpdate: (record: UnderlayRecordV1) => void
  onRemove: (id: string) => void
}

export function UnderlayPanel({ locale, underlays, layers, disabled, onImport, onUpdate, onRemove }: Props) {
  const text = (key: keyof typeof UNDERLAY_PANEL_TEXT) =>
    selectLocalizedText(locale, UNDERLAY_PANEL_TEXT[key])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const selected = underlays.find(({ id }) => id === selectedId) ?? null
  const [draft, setDraft] = useState<UnderlayRecordV1 | null>(null)
  const underlayLayers = layers.filter(({ content_kind }) => content_kind === 'underlay')
  const layer = underlayLayers.find(({ id }) => id === draft?.layer)
  const locked = layer?.locked ?? false
  function select(record: UnderlayRecordV1) {
    setSelectedId(record.id)
    setDraft(structuredClone(record))
  }
  function importImage() {
    const target = underlayLayers.find(({ locked }) => !locked)
    if (!target) return
    onImport({
      id: crypto.randomUUID(),
      transform: { position: { x: 0, y: 0 }, scale_x: 0.1, scale_y: 0.1, rotation_degrees: 0 },
      opacity: 1,
      layer: target.id,
    })
  }
  function submit(event: FormEvent) {
    event.preventDefault()
    if (draft && selected && !locked) onUpdate(draft)
  }
  return <section className="panel" aria-labelledby="underlay-title">
    <div className="panel-heading">
      <span id="underlay-title">{text('title')}</span>
      <button type="button" onClick={importImage}
        disabled={disabled || !underlayLayers.some(({ locked }) => !locked)}>
        {text('add')}
      </button>
    </div>
    {underlayLayers.length === 0 && <p role="status">
      {text('createLayer')}
    </p>}
    <ul aria-label={text('list')}>
      {underlays.map((record, index) => <li key={record.id}>
        <button type="button" aria-pressed={record.id === selectedId} onClick={() => select(record)}>
          {formatLocalizedText(locale, UNDERLAY_PANEL_TEXT.item, { index: index + 1 })}
        </button>
      </li>)}
    </ul>
    {draft && <form onSubmit={submit} aria-label={text('form')}>
      <label>{text('layer')}<select value={draft.layer} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, layer: event.target.value })}>
        {underlayLayers.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
      </select></label>
      {(['x', 'y'] as const).map((axis) => <label key={axis}>{axis.toUpperCase()} (mm)
        <input type="number" step="any" value={draft.transform.position[axis]} disabled={disabled || locked}
          onChange={(event) => setDraft({ ...draft, transform: {
            ...draft.transform, position: { ...draft.transform.position, [axis]: Number(event.target.value) },
          } })} />
      </label>)}
      {(['scale_x', 'scale_y'] as const).map((field) => <label key={field}>
        {field === 'scale_x' ? text('scaleX') : text('scaleY')}
        <input type="number" min="0.000001" max="1000000" step="0.01" value={draft.transform[field]}
          disabled={disabled || locked} onChange={(event) => setDraft({ ...draft, transform: {
            ...draft.transform, [field]: Number(event.target.value),
          } })} />
      </label>)}
      <label>{text('rotation')} (°)<input type="number" step="0.1"
        value={draft.transform.rotation_degrees} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, transform: {
          ...draft.transform, rotation_degrees: Number(event.target.value),
        } })} /></label>
      <label>{text('opacity')} (%)
        <input type="number" min="0" max="100" value={Math.round(draft.opacity * 100)}
          disabled={disabled || locked} onChange={(event) => setDraft({
            ...draft, opacity: Math.max(0, Math.min(100, Number(event.target.value))) / 100,
          })} />
      </label>
      {locked && <p role="status">{text('locked')}</p>}
      <button type="submit" disabled={disabled || locked}>{text('save')}</button>
      <button type="button" disabled={disabled || locked}
        onClick={() => onRemove(draft.id)}>{text('delete')}</button>
    </form>}
  </section>
}
