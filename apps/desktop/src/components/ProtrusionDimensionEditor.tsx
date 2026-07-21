import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function ProtrusionDimensionEditor({ locale, target, onChange, onRemove }: {
  locale: 'ja' | 'en'
  target: Protrusion
  onChange: (target: Protrusion) => void
  onRemove: () => void
}) {
  const update = (field: 'length_tenths_mm' | 'thickness_tenths_mm', value: number) => {
    if (!Number.isFinite(value) || value <= 0) return
    const tenths = Math.round(value * 10)
    if (field === 'length_tenths_mm' && tenths <= 1_000_000) onChange({ ...target, [field]: tenths })
    if (field === 'thickness_tenths_mm' && tenths <= 10_000) onChange({ ...target, [field]: tenths })
  }
  const symmetry = locale === 'ja'
    ? target.symmetry === 'none' ? '非対称単独' : '左右対称'
    : target.symmetry === 'none' ? 'Asymmetric single' : 'Bilateral'
  return <li>
    <span>{locale === 'ja'
      ? `binding ${target.id}・${symmetry}・数 ${target.count}`
      : `Binding ${target.id} · ${symmetry} · count ${target.count}`}</span>
    <label>{locale === 'ja' ? '長さ (mm)' : 'Length (mm)'}
      <input type="number" min="0.1" max="100000" step="0.1"
        aria-label={`${locale === 'ja' ? '長さ' : 'Length'} binding ${target.id} (mm)`}
        value={target.length_tenths_mm / 10}
        onChange={(event) => update('length_tenths_mm', Number(event.currentTarget.value))} />
    </label>
    <label>{locale === 'ja' ? '厚さ (mm)' : 'Thickness (mm)'}
      <input type="number" min="0.1" max="1000" step="0.1"
        aria-label={`${locale === 'ja' ? '厚さ' : 'Thickness'} binding ${target.id} (mm)`}
        value={target.thickness_tenths_mm / 10}
        onChange={(event) => update('thickness_tenths_mm', Number(event.currentTarget.value))} />
    </label>
    <button type="button" onClick={onRemove}>{locale === 'ja' ? '削除' : 'Remove'}</button>
  </li>
}
