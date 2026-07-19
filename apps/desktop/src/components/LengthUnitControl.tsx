import type { LengthDisplayUnit } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
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
  localeStore?: LocaleStore
}>

export function LengthUnitControl({
  unit,
  references,
  disabled,
  onChange,
  localeStore: localeStore_ = localeStore,
}: LengthUnitControlProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
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
      <legend>{text(LENGTH_UNIT_TEXT.legend)}</legend>
      <label className="field">
        <span>{text(LENGTH_UNIT_TEXT.unit)}</span>
        <select
          aria-label={text(LENGTH_UNIT_TEXT.legend)}
          value={lengthDisplaySelectionValue(unit)}
          disabled={disabled}
          onChange={(event) => selectUnit(event.currentTarget.value)}
        >
          <option value="mm">{text(LENGTH_UNIT_TEXT.millimetres)}</option>
          <option value="cm">{text(LENGTH_UNIT_TEXT.centimetres)}</option>
          <option value="inch">{text(LENGTH_UNIT_TEXT.inches)}</option>
          <option value="paper_edge_ratio" disabled={references.length === 0}>
            {text(LENGTH_UNIT_TEXT.paperEdgeRatio)}
          </option>
        </select>
      </label>
      {ratioSelected && (
        <label className="field">
          <span>{text(LENGTH_UNIT_TEXT.referenceEdge)}</span>
          <select
            aria-label={text(LENGTH_UNIT_TEXT.referenceEdgeAriaLabel)}
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
                {text(LENGTH_UNIT_TEXT.invalidSavedReference)}
              </option>
            )}
            {references.map((reference) => (
              <option value={reference.edgeId} key={reference.edgeId}>
                {formatLocalizedText(locale, LENGTH_UNIT_TEXT.edgeOption, {
                  index: reference.boundaryIndex + 1,
                  edgeId: reference.edgeId,
                  length: formatLength(
                    reference.lengthMm,
                    MILLIMETRE_LENGTH_DISPLAY_UNIT,
                  ),
                })}
              </option>
            ))}
          </select>
        </label>
      )}
      {unit.mode === 'paper_edge_ratio' && (
        <p className="length-unit-note">
          {text(LENGTH_UNIT_TEXT.ratioNote)}
        </p>
      )}
      {unit.mode === 'invalid_paper_edge_ratio' && (
        <p className="length-unit-error" role="alert">
          {unit.invalidReferenceEdgeId
            ? formatLocalizedText(
              locale,
              LENGTH_UNIT_TEXT.invalidReferenceWithId,
              { edgeId: unit.invalidReferenceEdgeId },
            )
            : text(LENGTH_UNIT_TEXT.invalidReference)}
          {text(LENGTH_UNIT_TEXT.repairNote)}
        </p>
      )}
      {ratioSelected && references.length === 0 && (
        <p className="length-unit-error" role="alert">
          {text(LENGTH_UNIT_TEXT.noReference)}
        </p>
      )}
    </fieldset>
  )
}

const LENGTH_UNIT_TEXT = Object.freeze({
  legend: Object.freeze({
    ja: '長さの表示単位',
    en: 'Length display unit',
  }),
  unit: Object.freeze({ ja: '単位', en: 'Unit' }),
  millimetres: Object.freeze({
    ja: 'ミリメートル (mm)',
    en: 'Millimetres (mm)',
  }),
  centimetres: Object.freeze({
    ja: 'センチメートル (cm)',
    en: 'Centimetres (cm)',
  }),
  inches: Object.freeze({ ja: 'インチ (in)', en: 'Inches (in)' }),
  paperEdgeRatio: Object.freeze({ ja: '紙辺比', en: 'Paper-edge ratio' }),
  referenceEdge: Object.freeze({
    ja: '基準にする輪郭辺',
    en: 'Reference boundary edge',
  }),
  referenceEdgeAriaLabel: Object.freeze({
    ja: '紙辺比の基準輪郭辺',
    en: 'Paper-edge ratio reference boundary edge',
  }),
  invalidSavedReference: Object.freeze({
    ja: '保存された基準辺は無効です',
    en: 'The saved reference edge is invalid',
  }),
  edgeOption: Object.freeze({
    ja: '辺 {index} · {edgeId} · {length}',
    en: 'Edge {index} · {edgeId} · {length}',
  }),
  ratioNote: Object.freeze({
    ja: '基準辺を 1 として表示します。基準辺の長さ変更には自動追従し、別の辺への自動切り替えは行いません。',
    en: 'Displays lengths relative to a reference edge of 1. Changes to that edge are followed automatically; another edge is never selected automatically.',
  }),
  invalidReferenceWithId: Object.freeze({
    ja: '保存された基準辺「{edgeId}」を現在の輪郭で一意に確認できません。',
    en: 'The saved reference edge "{edgeId}" cannot be uniquely identified in the current boundary.',
  }),
  invalidReference: Object.freeze({
    ja: '保存された紙辺比の基準辺が不正です。',
    en: 'The saved paper-edge ratio reference is invalid.',
  }),
  repairNote: Object.freeze({
    ja: '長さは修復用に mm で表示しています。単位または有効な基準辺を選び直してください。',
    en: 'Lengths are displayed in mm for repair. Select a unit or a valid reference edge.',
  }),
  noReference: Object.freeze({
    ja: '紙辺比に使用できる、一意で長さが正の輪郭辺がありません。先に輪郭を修復するか、mm・cm・in を選択してください。',
    en: 'No unique, positive-length boundary edge is available for a paper-edge ratio. Repair the boundary first, or select mm, cm, or in.',
  }),
})
