import type { LengthDisplayUnit } from '../lib/coreClient.ts'
import {
  formatLength,
  lengthDisplaySelectionValue,
  makePaperEdgeRatioUnit,
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
  type BoundaryLengthReference,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'

export type LengthUnitControlProps = Readonly<{
  unit: ResolvedLengthDisplayUnit
  references: readonly BoundaryLengthReference[]
  disabled: boolean
  onChange: (unit: LengthDisplayUnit) => void
}>

export function LengthUnitControl({
  unit,
  references,
  disabled,
  onChange,
}: LengthUnitControlProps) {
  const ratioSelected = unit.mode !== 'absolute'
  const selectedReference = unit.mode === 'paper_edge_ratio'
    ? unit.reference.edgeId
    : ''

  function selectUnit(value: string) {
    if (value === 'mm' || value === 'cm' || value === 'inch') {
      onChange(value)
      return
    }
    if (value !== 'paper_edge_ratio') return
    const reference = unit.mode === 'paper_edge_ratio'
      ? unit.reference
      : references[0]
    if (reference) onChange(makePaperEdgeRatioUnit(reference.edgeId))
  }

  return (
    <fieldset className="length-unit-control">
      <legend>長さの表示単位</legend>
      <label className="field">
        <span>単位</span>
        <select
          aria-label="長さの表示単位"
          value={lengthDisplaySelectionValue(unit)}
          disabled={disabled}
          onChange={(event) => selectUnit(event.currentTarget.value)}
        >
          <option value="mm">ミリメートル (mm)</option>
          <option value="cm">センチメートル (cm)</option>
          <option value="inch">インチ (in)</option>
          <option value="paper_edge_ratio" disabled={references.length === 0}>
            紙辺比
          </option>
        </select>
      </label>
      {ratioSelected && (
        <label className="field">
          <span>基準にする輪郭辺</span>
          <select
            aria-label="紙辺比の基準輪郭辺"
            value={selectedReference}
            disabled={disabled || references.length === 0}
            aria-invalid={unit.mode === 'invalid_paper_edge_ratio'}
            onChange={(event) => {
              if (event.currentTarget.value) {
                onChange(makePaperEdgeRatioUnit(event.currentTarget.value))
              }
            }}
          >
            {unit.mode === 'invalid_paper_edge_ratio' && (
              <option value="">
                保存された基準辺は無効です
              </option>
            )}
            {references.map((reference) => (
              <option value={reference.edgeId} key={reference.edgeId}>
                {`辺 ${reference.boundaryIndex + 1} · ${reference.edgeId} · ${
                  formatLength(reference.lengthMm, MILLIMETRE_LENGTH_DISPLAY_UNIT)
                }`}
              </option>
            ))}
          </select>
        </label>
      )}
      {unit.mode === 'paper_edge_ratio' && (
        <p className="length-unit-note">
          基準辺を 1 として表示します。基準辺の長さ変更には自動追従し、
          別の辺への自動切り替えは行いません。
        </p>
      )}
      {unit.mode === 'invalid_paper_edge_ratio' && (
        <p className="length-unit-error" role="alert">
          {unit.invalidReferenceEdgeId
            ? `保存された基準辺「${unit.invalidReferenceEdgeId}」を現在の輪郭で一意に確認できません。`
            : '保存された紙辺比の基準辺が不正です。'}
          長さは修復用に mm で表示しています。単位または有効な基準辺を選び直してください。
        </p>
      )}
      {ratioSelected && references.length === 0 && (
        <p className="length-unit-error" role="alert">
          紙辺比に使用できる、一意で長さが正の輪郭辺がありません。
          先に輪郭を修復するか、mm・cm・in を選択してください。
        </p>
      )}
    </fieldset>
  )
}
