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
import { LENGTH_UNIT_CONTROL_TEXT as LENGTH_UNIT_TEXT } from '../lib/lengthUnitControlText.ts'

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
                    locale,
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
