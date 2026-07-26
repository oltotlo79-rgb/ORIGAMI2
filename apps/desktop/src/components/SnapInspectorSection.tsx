import type { RefObject } from 'react'

import { APP_TEXT } from '../lib/appText.ts'
import {
  formatAngleDegrees,
} from '../lib/appGeometry.ts'
import { lineKindLabel } from '../lib/appPresentation.ts'
import { updateGridPreferenceInput } from '../lib/gridPreference.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  ANGLE_SNAP_PRESETS,
  toggleSnapSetting,
  type AngleSnapReferenceKind,
  type SnapSettings,
} from '../lib/snap.ts'
import { SNAP_INSPECTOR_OPTIONS } from '../lib/snapInspectorOptions.ts'
import type { CreaseLine } from './CreaseCanvas.tsx'

export type SnapInspectorSectionProps = Readonly<{
  locale: Locale
  coreBusy: boolean
  snapSettings: SnapSettings
  gridDivisionsInput: string
  gridDivisionsValid: boolean
  gridDivisions: number | null
  gridDiagonals: boolean
  selectedAnglePreset: string
  angleDegrees: number
  angleDegreesInput: string
  angleInputIsValid: boolean
  angleInputRef: RefObject<HTMLInputElement | null>
  angleReferenceKind: AngleSnapReferenceKind
  parallelReferenceLine: CreaseLine | null | undefined
  onSnapSettingsChange: (settings: SnapSettings) => void
  onGridPreferenceChange: (input: string, diagonals: boolean) => void
  onGridDiagonalsChange: (enabled: boolean) => void
  onAngleDegreesChange: (degrees: number) => void
  onAngleDegreesInputChange: (input: string) => void
  onAngleReferenceKindChange: (kind: AngleSnapReferenceKind) => void
  onClearParallelReference: () => void
}>

export function SnapInspectorSection({
  locale,
  coreBusy,
  snapSettings,
  gridDivisionsInput,
  gridDivisionsValid,
  gridDivisions,
  gridDiagonals,
  selectedAnglePreset,
  angleDegrees,
  angleDegreesInput,
  angleInputIsValid,
  angleInputRef,
  angleReferenceKind,
  parallelReferenceLine,
  onSnapSettingsChange,
  onGridPreferenceChange,
  onGridDiagonalsChange,
  onAngleDegreesChange,
  onAngleDegreesInputChange,
  onAngleReferenceKindChange,
  onClearParallelReference,
}: SnapInspectorSectionProps) {
  const text = (localized: LocalizedText) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: LocalizedText,
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)

  return (
    <section>
      <h2>{text(APP_TEXT.snap)}</h2>
      <div
        className="chip-row"
        aria-label={text(APP_TEXT.snapSettings)}
      >
        {SNAP_INSPECTOR_OPTIONS.map(({ kind, label }) => (
          <button
            key={kind}
            type="button"
            className={`chip${snapSettings[kind] ? ' active' : ''}`}
            aria-pressed={snapSettings[kind]}
            disabled={coreBusy}
            onClick={() => onSnapSettingsChange(
              toggleSnapSetting(snapSettings, kind),
            )}
          >
            {text(label)}
          </button>
        ))}
      </div>
      <label className="angle-snap-field">
        <span>{text(APP_TEXT.dividePaperIntoN)}</span>
        <input
          type="number"
          min="2"
          max="63"
          step="1"
          value={gridDivisionsInput}
          placeholder={text(APP_TEXT.auto)}
          aria-invalid={!gridDivisionsValid}
          disabled={coreBusy}
          onChange={(event) => {
            const next = updateGridPreferenceInput(
              event.target.value,
              gridDiagonals,
            )
            if (next) onGridPreferenceChange(next.input, next.diagonals)
          }}
        />
        <small>
          {text(APP_TEXT.leaveBlankForAutomaticSpacingUse3ForThirdsOr)}
        </small>
      </label>
      <button
        type="button"
        className={`chip${gridDiagonals ? ' active' : ''}`}
        aria-pressed={gridDiagonals}
        disabled={
          coreBusy || !gridDivisionsValid || gridDivisions === null
        }
        onClick={() => onGridDiagonalsChange(!gridDiagonals)}
      >
        {text(APP_TEXT.paperDiagonals)}
      </button>
      <div className="angle-snap-settings">
        <h3>{text(APP_TEXT.angleSnap)}</h3>
        <label className="angle-snap-field">
          <span>{text(APP_TEXT.preset)}</span>
          <select
            value={selectedAnglePreset}
            disabled={coreBusy}
            onChange={(event) => {
              if (event.target.value === 'custom') {
                angleInputRef.current?.focus()
                angleInputRef.current?.select()
                return
              }
              const nextDegrees = Number(event.target.value)
              onAngleDegreesChange(nextDegrees)
              onAngleDegreesInputChange(String(nextDegrees))
            }}
          >
            {ANGLE_SNAP_PRESETS.map((preset) => (
              <option key={preset} value={preset}>{preset}°</option>
            ))}
            <option value="custom">
              {text(APP_TEXT.customAngle)}
            </option>
          </select>
        </label>
        <label className="angle-snap-field">
          <span>{text(APP_TEXT.angle)}</span>
          <span className="angle-input-with-unit">
            <input
              ref={angleInputRef}
              type="number"
              min="0"
              max="90"
              step="any"
              value={angleDegreesInput}
              disabled={coreBusy}
              aria-invalid={!angleInputIsValid}
              aria-describedby={
                !angleInputIsValid ? 'angle-snap-error' : undefined
              }
              onChange={(event) => {
                const nextInput = event.target.value
                const nextDegrees = Number(nextInput)
                onAngleDegreesInputChange(nextInput)
                if (
                  nextInput.trim().length > 0
                  && Number.isFinite(nextDegrees)
                  && nextDegrees > 0
                  && nextDegrees <= 90
                ) onAngleDegreesChange(nextDegrees)
              }}
            />
            <span>°</span>
          </span>
        </label>
        {!angleInputIsValid && (
          <p id="angle-snap-error" className="field-error" role="alert">
            {text(APP_TEXT.enterAnAngleGreaterThan0AndNoMoreThan)}
          </p>
        )}
        <div className="angle-reference-setting">
          <span>{text(APP_TEXT.reference)}</span>
          <div
            className="chip-row"
            role="group"
            aria-label={text(APP_TEXT.angleSnapReference)}
          >
            <button
              type="button"
              className={`chip${
                angleReferenceKind === 'global-horizontal' ? ' active' : ''
              }`}
              aria-pressed={angleReferenceKind === 'global-horizontal'}
              disabled={coreBusy}
              onClick={() => onAngleReferenceKindChange('global-horizontal')}
            >
              {text(APP_TEXT.horizontal)}
            </button>
            <button
              type="button"
              className={`chip${
                angleReferenceKind === 'edge' ? ' active' : ''
              }`}
              aria-pressed={angleReferenceKind === 'edge'}
              disabled={coreBusy}
              onClick={() => onAngleReferenceKindChange('edge')}
            >
              {text(APP_TEXT.directionReferenceEdge)}
            </button>
          </div>
        </div>
        <p className="muted">
          {formattedText(APP_TEXT.currentAngleReference, {
            angle: formatAngleDegrees(angleDegrees),
            reference: angleReferenceKind === 'global-horizontal'
              ? text(APP_TEXT.horizontalReference)
              : text(APP_TEXT.directionEdgeReference),
          })}
        </p>
        {snapSettings.angle
          && angleReferenceKind === 'edge'
          && !parallelReferenceLine && (
          <p className="field-error" role="status">
            {text(APP_TEXT.selectALineAndSetItAsTheDirectionReference)}
          </p>
        )}
      </div>
      {parallelReferenceLine ? (
        <div className="property-actions">
          <span className="muted" title={parallelReferenceLine.id}>
            {formattedText(
              APP_TEXT.directionReferenceParallelAndAngleKind,
              {
                kind: lineKindLabel(
                  parallelReferenceLine.kind,
                  locale,
                ),
              },
            )}
          </span>
          <button
            type="button"
            disabled={coreBusy}
            onClick={onClearParallelReference}
          >
            {text(APP_TEXT.clearReference)}
          </button>
        </div>
      ) : (
        <p className="muted">
          {text(APP_TEXT.selectALineAndChooseSetAsDirectionReferenceTo)}
        </p>
      )}
    </section>
  )
}
