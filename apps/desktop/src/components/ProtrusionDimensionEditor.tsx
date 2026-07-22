import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'
import { ProtrusionLocalOutlineEditor } from './ProtrusionLocalOutlineEditor'

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
  const updateOptionalWidth = (
    field: 'root_width_tenths_mm' | 'tip_width_tenths_mm', value: string,
  ) => {
    if (value.trim() === '') {
      const next = { ...target }
      delete next[field]
      onChange(next)
      return
    }
    const millimetres = Number(value)
    if (!Number.isFinite(millimetres) || millimetres <= 0) return
    const tenths = Math.round(millimetres * 10)
    if (tenths <= 10_000) onChange({ ...target, [field]: tenths })
  }
  const updatePosition = (index: 1 | 2, value: number) => {
    if (!Number.isFinite(value) || Math.abs(value) > 10_000) return
    const position = [...target.position_tenths_mm] as [number, number, number]
    position[index] = Math.round(value * 10)
    onChange({ ...target, position_tenths_mm: position })
  }
  const updateDirection = (index: 0 | 1, value: number) => {
    if (!Number.isFinite(value) || Math.abs(value) > 1) return
    const direction = [...target.direction_milli] as [number, number, number]
    direction[index] = Math.round(value * 1_000)
    if (direction.every((component) => component === 0)) return
    onChange({ ...target, direction_milli: direction })
  }
  const updateMotion = (index: 0 | 1, value: number) => {
    if (!Number.isInteger(value) || value < -360 || value > 360) return
    const motion = [...target.motion_degrees] as [number, number]
    motion[index] = value
    if (motion[0] <= motion[1]) onChange({ ...target, motion_degrees: motion })
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
    <label>{locale === 'ja' ? '根元幅 (mm、任意)' : 'Root width (mm, optional)'}
      <input type="number" min="0.1" max="1000" step="0.1"
        aria-label={`${locale === 'ja' ? '根元幅' : 'Root width'} binding ${target.id} (mm)`}
        value={target.root_width_tenths_mm === undefined ? '' : target.root_width_tenths_mm / 10}
        onChange={(event) => updateOptionalWidth('root_width_tenths_mm', event.currentTarget.value)} />
    </label>
    <label>{locale === 'ja' ? '先端幅 (mm、任意)' : 'Tip width (mm, optional)'}
      <input type="number" min="0.1" max="1000" step="0.1"
        aria-label={`${locale === 'ja' ? '先端幅' : 'Tip width'} binding ${target.id} (mm)`}
        value={target.tip_width_tenths_mm === undefined ? '' : target.tip_width_tenths_mm / 10}
        onChange={(event) => updateOptionalWidth('tip_width_tenths_mm', event.currentTarget.value)} />
    </label>
    <label>{locale === 'ja' ? '長さ (mm)' : 'Length (mm)'}
      <input type="number" min="0.1" max="100000" step="0.1"
        aria-label={`${locale === 'ja' ? '長さ' : 'Length'} binding ${target.id} (mm)`}
        value={target.length_tenths_mm / 10}
        onChange={(event) => update('length_tenths_mm', Number(event.currentTarget.value))} />
    </label>
    <label>{target.symmetry === 'bilateral'
      ? locale === 'ja' ? '左右間隔 (mm)' : 'Bilateral spacing (mm)'
      : locale === 'ja' ? '厚さ (mm)' : 'Thickness (mm)'}
      <input type="number" min="0.1" max="1000" step="0.1"
        aria-label={`${target.symmetry === 'bilateral'
          ? locale === 'ja' ? '左右間隔' : 'Bilateral spacing'
          : locale === 'ja' ? '厚さ' : 'Thickness'} binding ${target.id} (mm)`}
        value={target.thickness_tenths_mm / 10}
        onChange={(event) => update('thickness_tenths_mm', Number(event.currentTarget.value))} />
    </label>
    <label>{locale === 'ja' ? '取付位置 上下 (mm)' : 'Mount vertical (mm)'}
      <input type="number" min="-10000" max="10000" step="0.1"
        aria-label={`${locale === 'ja' ? '取付位置 上下' : 'Mount vertical'} binding ${target.id} (mm)`}
        value={target.position_tenths_mm[1] / 10}
        onChange={(event) => updatePosition(1, Number(event.currentTarget.value))} />
    </label>
    <label>{locale === 'ja' ? '取付位置 前後 (mm)' : 'Mount fore-aft (mm)'}
      <input type="number" min="-10000" max="10000" step="0.1"
        aria-label={`${locale === 'ja' ? '取付位置 前後' : 'Mount fore-aft'} binding ${target.id} (mm)`}
        value={target.position_tenths_mm[2] / 10}
        onChange={(event) => updatePosition(2, Number(event.currentTarget.value))} />
    </label>
    <label>{locale === 'ja' ? '向き 左右' : 'Direction horizontal'}
      <input type="number" min="-1" max="1" step="0.001"
        aria-label={`${locale === 'ja' ? '向き 左右' : 'Direction horizontal'} binding ${target.id}`}
        value={target.direction_milli[0] / 1_000}
        onChange={(event) => updateDirection(0, Number(event.currentTarget.value))} />
    </label>
    <label>{locale === 'ja' ? '向き 上下' : 'Direction vertical'}
      <input type="number" min="-1" max="1" step="0.001"
        aria-label={`${locale === 'ja' ? '向き 上下' : 'Direction vertical'} binding ${target.id}`}
        value={target.direction_milli[1] / 1_000}
        onChange={(event) => updateDirection(1, Number(event.currentTarget.value))} />
    </label>
    <label>{locale === 'ja' ? '曲率 (度)' : 'Curvature (degrees)'}<input type="number" min="-360" max="360" step="1"
      aria-label={`${locale === 'ja' ? '曲率' : 'Curvature'} binding ${target.id} (${locale === 'ja' ? '度' : 'degrees'})`} value={target.curvature_degrees}
      onChange={(event) => { const value = Number(event.currentTarget.value)
        if (Number.isInteger(value) && value >= -360 && value <= 360) onChange({ ...target, curvature_degrees: value }) }} /></label>
    <label>{locale === 'ja' ? '可動範囲の最小 (度)' : 'Motion minimum (degrees)'}<input type="number" min="-360" max="360" step="1"
      aria-label={`${locale === 'ja' ? '可動範囲の最小' : 'Motion minimum'} binding ${target.id} (${locale === 'ja' ? '度' : 'degrees'})`} value={target.motion_degrees[0]}
      onChange={(event) => updateMotion(0, Number(event.currentTarget.value))} /></label>
    <label>{locale === 'ja' ? '可動範囲の最大 (度)' : 'Motion maximum (degrees)'}<input type="number" min="-360" max="360" step="1"
      aria-label={`${locale === 'ja' ? '可動範囲の最大' : 'Motion maximum'} binding ${target.id} (${locale === 'ja' ? '度' : 'degrees'})`} value={target.motion_degrees[1]}
      onChange={(event) => updateMotion(1, Number(event.currentTarget.value))} /></label>
    <label>{locale === 'ja' ? '関節' : 'Joint'}<select aria-label={`${locale === 'ja' ? '関節' : 'Joint'} binding ${target.id}`} value={target.joint}
      onChange={(event) => onChange({ ...target, joint: event.currentTarget.value as Protrusion['joint'] })}>
      <option value="fixed">{locale === 'ja' ? '固定' : 'Fixed'}</option><option value="hinge">{locale === 'ja' ? 'ヒンジ' : 'Hinge'}</option><option value="ball">{locale === 'ja' ? '球状' : 'Ball'}</option>
    </select></label>
    <label>{locale === 'ja' ? '面' : 'Side'}<select aria-label={`${locale === 'ja' ? '面' : 'Side'} binding ${target.id}`} value={target.side}
      onChange={(event) => onChange({ ...target, side: event.currentTarget.value as Protrusion['side'] })}>
      <option value="front">{locale === 'ja' ? '表' : 'Front'}</option><option value="back">{locale === 'ja' ? '裏' : 'Back'}</option><option value="either">{locale === 'ja' ? 'どちらでも可' : 'Either'}</option>
    </select></label>
    <label>{locale === 'ja' ? '優先度' : 'Priority'}<input type="number" min="1" max="100" step="1" aria-label={`${locale === 'ja' ? '優先度' : 'Priority'} binding ${target.id}`}
      value={target.priority} onChange={(event) => { const priority = Number(event.currentTarget.value)
        if (Number.isInteger(priority) && priority >= 1 && priority <= 100) onChange({ ...target, priority }) }} /></label>
    <ProtrusionLocalOutlineEditor locale={locale} bindingId={target.id}
      symmetry={target.symmetry} points={target.local_outline_tenths_mm ?? []}
      onChange={(points) => {
        const next = { ...target }
        if (points === undefined) delete next.local_outline_tenths_mm
        else next.local_outline_tenths_mm = points
        onChange(next)
      }} />
    <button type="button" disabled={!canRemove} onClick={onRemove}>{locale === 'ja' ? '削除' : 'Remove'}</button>
    <button type="button" disabled={!canMoveUp} onClick={onMoveUp}>
      {locale === 'ja' ? '上へ' : 'Move up'}
    </button>
    <button type="button" disabled={!canMoveDown} onClick={onMoveDown}>
      {locale === 'ja' ? '下へ' : 'Move down'}
    </button>
  </li>
}
