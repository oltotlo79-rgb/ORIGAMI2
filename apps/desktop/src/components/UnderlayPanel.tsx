import { useState, type FormEvent } from 'react'
import type { UnderlayRecordV1 } from '../lib/coreClient'
import type { LayerRecordV1 } from '../lib/projectLayers'
import type { Locale } from '../lib/i18n'

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
  const t = (ja: string, en: string) => locale === 'ja' ? ja : en
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
      <span id="underlay-title">{t('下絵', 'Underlays')}</span>
      <button type="button" onClick={importImage}
        disabled={disabled || !underlayLayers.some(({ locked }) => !locked)}>
        {t('画像を追加', 'Add image')}
      </button>
    </div>
    {underlayLayers.length === 0 && <p role="status">
      {t('下絵レイヤーを先に作成してください。', 'Create an underlay layer first.')}
    </p>}
    <ul aria-label={t('下絵一覧', 'Underlay list')}>
      {underlays.map((record, index) => <li key={record.id}>
        <button type="button" aria-pressed={record.id === selectedId} onClick={() => select(record)}>
          {t(`下絵 ${index + 1}`, `Underlay ${index + 1}`)}
        </button>
      </li>)}
    </ul>
    {draft && <form onSubmit={submit} aria-label={t('下絵の配置と変形', 'Place and transform underlay')}>
      <label>{t('レイヤー', 'Layer')}<select value={draft.layer} disabled={disabled || locked}
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
        {field === 'scale_x' ? t('横倍率', 'Scale X') : t('縦倍率', 'Scale Y')}
        <input type="number" min="0.000001" max="1000000" step="0.01" value={draft.transform[field]}
          disabled={disabled || locked} onChange={(event) => setDraft({ ...draft, transform: {
            ...draft.transform, [field]: Number(event.target.value),
          } })} />
      </label>)}
      <label>{t('回転', 'Rotation')} (°)<input type="number" step="0.1"
        value={draft.transform.rotation_degrees} disabled={disabled || locked}
        onChange={(event) => setDraft({ ...draft, transform: {
          ...draft.transform, rotation_degrees: Number(event.target.value),
        } })} /></label>
      <label>{t('不透明度', 'Opacity')} (%)
        <input type="number" min="0" max="100" value={Math.round(draft.opacity * 100)}
          disabled={disabled || locked} onChange={(event) => setDraft({
            ...draft, opacity: Math.max(0, Math.min(100, Number(event.target.value))) / 100,
          })} />
      </label>
      {locked && <p role="status">{t('このレイヤーはロックされています。', 'This layer is locked.')}</p>}
      <button type="submit" disabled={disabled || locked}>{t('保存', 'Save')}</button>
      <button type="button" disabled={disabled || locked}
        onClick={() => onRemove(draft.id)}>{t('削除', 'Delete')}</button>
    </form>}
  </section>
}
