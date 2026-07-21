import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]
type PartKind = BeginnerGenerationConstraintsV1['target_parts'][number]['kind']
const partKinds: PartKind[] = ['leg', 'horn', 'ear', 'wing', 'fin', 'antenna', 'tail']

export function ProtrusionDimensionEditor({ locale, target, onChange, onRemove,
  kind, onKindChange, onMoveUp, onMoveDown, canRemove = true, canMoveUp = false, canMoveDown = false }: {
  locale: 'ja' | 'en'
  target: Protrusion
  onChange: (target: Protrusion) => void
  onRemove: () => void
  kind?: PartKind
  onKindChange?: (kind: PartKind) => void
  onMoveUp?: () => void
  onMoveDown?: () => void
  canRemove?: boolean
  canMoveUp?: boolean
  canMoveDown?: boolean
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
    {kind && onKindChange && <label>{locale === 'ja' ? '種類' : 'Part kind'}
      <select aria-label={`${locale === 'ja' ? '種類' : 'Part kind'} binding ${target.id}`}
        value={kind} onChange={(event) => onKindChange(event.currentTarget.value as PartKind)}>
        {partKinds.map((partKind) => <option key={partKind} value={partKind}>{partKind}</option>)}
      </select>
    </label>}
    <label>{locale === 'ja' ? '対称性' : 'Symmetry'}
      <select aria-label={`${locale === 'ja' ? '対称性' : 'Symmetry'} binding ${target.id}`}
        value={target.symmetry}
        onChange={(event) => {
          const next = event.currentTarget.value as 'none' | 'bilateral'
          onChange({ ...target, symmetry: next, count: next === 'none' ? 1 : 2 })
        }}>
        <option value="none">{locale === 'ja' ? '非対称単独' : 'Asymmetric single'}</option>
        <option value="bilateral">{locale === 'ja' ? '左右対称' : 'Bilateral'}</option>
      </select>
    </label>
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
    <button type="button" disabled={!canRemove} onClick={onRemove}>{locale === 'ja' ? '削除' : 'Remove'}</button>
    <button type="button" disabled={!canMoveUp} onClick={onMoveUp}>
      {locale === 'ja' ? '上へ' : 'Move up'}
    </button>
    <button type="button" disabled={!canMoveDown} onClick={onMoveDown}>
      {locale === 'ja' ? '下へ' : 'Move down'}
    </button>
  </li>
}
