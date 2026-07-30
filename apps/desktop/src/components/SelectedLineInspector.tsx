import type { FormEventHandler } from 'react'

import {
  formatMeasurementValue,
  measureCreaseLine,
} from '../lib/appGeometry.ts'
import { lineKindLabel } from '../lib/appPresentation.ts'
import { APP_TEXT } from '../lib/appText.ts'
import type {
  LinearArrayPreview,
  LinearArrayRequest,
  RadialArrayPreview,
  RadialArrayRequest,
} from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  formatLength,
  formatLengthPoint,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'
import { MAX_NUMERIC_EXPRESSION_SOURCE_BYTES } from '../lib/numericExpressionNative.ts'
import type { CreaseLine } from './CreaseCanvas.tsx'

type LinearPreview = Readonly<{
  request: LinearArrayRequest
  result: LinearArrayPreview
}>

type RadialPreview = Readonly<{
  request: RadialArrayRequest
  result: RadialArrayPreview
}>

export type SelectedLineInspectorProps = Readonly<{
  locale: Locale
  line: CreaseLine
  displayUnit: ResolvedLengthDisplayUnit
  displayUnitLabel: string
  coreBusy: boolean
  benchmarkActive: boolean
  parallelReferenceEdgeId: string | null
  linearArrayPreview: LinearPreview | null
  radialArrayPreview: RadialPreview | null
  radialArrayCenterVertexIds: readonly string[]
  onDeleteBenchmarkLine: (lineId: string) => void
  onSubmitMove: FormEventHandler<HTMLFormElement>
  onSubmitMirror: FormEventHandler<HTMLFormElement>
  onSubmitRotate: FormEventHandler<HTMLFormElement>
  onSubmitLinearArray: FormEventHandler<HTMLFormElement>
  onInvalidateLinearArray: () => void
  onConfirmLinearArray: () => void | Promise<void>
  onSubmitRadialArray: FormEventHandler<HTMLFormElement>
  onInvalidateRadialArray: () => void
  onConfirmRadialArray: () => void | Promise<void>
  onToggleParallelReference: (lineId: string) => void
  onSplitBoundaryEdge: () => void | Promise<void>
  onDeleteSelection: () => void | Promise<void>
}>

export function SelectedLineInspector({
  locale,
  line,
  displayUnit,
  displayUnitLabel,
  coreBusy,
  benchmarkActive,
  parallelReferenceEdgeId,
  linearArrayPreview,
  radialArrayPreview,
  radialArrayCenterVertexIds,
  onDeleteBenchmarkLine,
  onSubmitMove,
  onSubmitMirror,
  onSubmitRotate,
  onSubmitLinearArray,
  onInvalidateLinearArray,
  onConfirmLinearArray,
  onSubmitRadialArray,
  onInvalidateRadialArray,
  onConfirmRadialArray,
  onToggleParallelReference,
  onSplitBoundaryEdge,
  onDeleteSelection,
}: SelectedLineInspectorProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const measurement = measureCreaseLine(line)
  const radialArrayCenterOptions = [
    {
      id: line.startVertexId,
      label: text(APP_TEXT.startVertex),
    },
    {
      id: line.endVertexId,
      label: text(APP_TEXT.endVertex),
    },
  ].filter(({ id }) => radialArrayCenterVertexIds.includes(id))
  const radialArrayCenterAvailable = radialArrayCenterOptions.length > 0

  return (
    <>
      <dl>
        <div><dt>ID</dt><dd>{line.id}</dd></div>
        <div>
          <dt>{text(APP_TEXT.type)}</dt>
          <dd>{lineKindLabel(line.kind, locale)}</dd>
        </div>
        <div>
          <dt>{text(APP_TEXT.start)}</dt>
          <dd>{formatLengthPoint(line.x1, line.y1, displayUnit, locale)}</dd>
        </div>
        <div>
          <dt>{text(APP_TEXT.end)}</dt>
          <dd>{formatLengthPoint(line.x2, line.y2, displayUnit, locale)}</dd>
        </div>
        <div>
          <dt>ΔX</dt>
          <dd>{formatLength(measurement?.deltaX, displayUnit, locale)}</dd>
        </div>
        <div>
          <dt>ΔY</dt>
          <dd>{formatLength(measurement?.deltaY, displayUnit, locale)}</dd>
        </div>
        <div>
          <dt>{text(APP_TEXT.length)}</dt>
          <dd>{formatLength(measurement?.length, displayUnit, locale)}</dd>
        </div>
        <div>
          <dt>{text(APP_TEXT.angle)}</dt>
          <dd>{formatMeasurementValue(
            measurement?.angleDegrees,
            '°',
            2,
            locale,
          )}</dd>
        </div>
      </dl>
      {benchmarkActive ? (
        <>
          <button
            type="button"
            className="danger"
            onClick={() => onDeleteBenchmarkLine(line.id)}
          >
            {text(APP_TEXT.deleteBenchmarkLine)}
          </button>
          <p className="muted">
            {text(APP_TEXT.selectionMeasurementVertexMovementAndLineDeletionAreAvailableOn)}
          </p>
        </>
      ) : (
        <>
          <form onSubmit={onSubmitMove}>
            <fieldset disabled={coreBusy || line.locked}>
              <legend>{text(APP_TEXT.moveEntireLine)}</legend>
              <label className="field">
                {formattedText(APP_TEXT.horizontalOffsetUnit, {
                  unit: displayUnitLabel,
                })}
                <input
                  name="edge_delta_x_display"
                  type="text"
                  inputMode="text"
                  maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                  defaultValue="0"
                />
              </label>
              <label className="field">
                {formattedText(APP_TEXT.verticalOffsetUnit, {
                  unit: displayUnitLabel,
                })}
                <input
                  name="edge_delta_y_display"
                  type="text"
                  inputMode="text"
                  maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                  defaultValue="0"
                />
              </label>
              <div className="property-actions">
                <button type="submit">{text(APP_TEXT.moveEntireLine)}</button>
              </div>
            </fieldset>
          </form>
          <form onSubmit={onSubmitMirror}>
            <fieldset disabled={coreBusy || line.locked}>
              <legend>{text(APP_TEXT.leftRightSymmetry)}</legend>
              <label className="field">
                {formattedText(APP_TEXT.mirrorAxisXUnit, {
                  unit: displayUnitLabel,
                })}
                <input
                  name="symmetry_axis_x_display"
                  type="text"
                  inputMode="text"
                  maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                  defaultValue="0"
                />
              </label>
              <button type="submit">
                {text(APP_TEXT.applyLeftRightReflection)}
              </button>
            </fieldset>
          </form>
          <form onSubmit={onSubmitRotate}>
            <fieldset disabled={coreBusy || line.locked}>
              <legend>{text(APP_TEXT.rotationalSymmetry)}</legend>
              <label className="field">
                {formattedText(APP_TEXT.centerXUnit, {
                  unit: displayUnitLabel,
                })}
                <input
                  name="rotation_center_x_display"
                  type="text"
                  inputMode="text"
                  maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                  defaultValue="0"
                />
              </label>
              <label className="field">
                {formattedText(APP_TEXT.centerYUnit, {
                  unit: displayUnitLabel,
                })}
                <input
                  name="rotation_center_y_display"
                  type="text"
                  inputMode="text"
                  maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                  defaultValue="0"
                />
              </label>
              <label className="field">
                {text(APP_TEXT.rotationAngle)}
                <input
                  name="rotation_angle_degrees"
                  type="text"
                  inputMode="text"
                  maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                  defaultValue="180"
                />
              </label>
              <button type="submit">{text(APP_TEXT.applyRotation)}</button>
            </fieldset>
          </form>
          {line.kind !== 'boundary' && (
            <form
              onSubmit={onSubmitLinearArray}
              onInput={onInvalidateLinearArray}
              data-testid="linear-array-panel"
            >
              <fieldset disabled={coreBusy || line.locked}>
                <legend>{text(APP_TEXT.linearArray)}</legend>
                <label className="field">
                  {text(APP_TEXT.additionalCopies)}
                  <input
                    name="linear_array_copies"
                    type="number"
                    min="1"
                    max="16"
                    step="1"
                    defaultValue="1"
                  />
                </label>
                <label className="field">
                  {text(APP_TEXT.xOffsetMm)}
                  <input
                    name="linear_array_dx"
                    type="number"
                    step="any"
                    defaultValue="10"
                  />
                </label>
                <label className="field">
                  {text(APP_TEXT.yOffsetMm)}
                  <input
                    name="linear_array_dy"
                    type="number"
                    step="any"
                    defaultValue="0"
                  />
                </label>
                <button type="submit" data-testid="preview-linear-array">
                  {text(APP_TEXT.previewArray)}
                </button>
              </fieldset>
              {linearArrayPreview?.request.edges[0] === line.id && (
                <div data-testid="linear-array-preview" aria-live="polite">
                  <p>{formattedText(
                    APP_TEXT.verticesVerticesAndEdgesEdgeSeedsWillBeAddedThe,
                    {
                      vertices:
                        linearArrayPreview.result.generated_vertex_count,
                      edges:
                        linearArrayPreview.result.generated_edge_seed_count,
                    },
                  )}</p>
                  <button
                    type="button"
                    onClick={() => void onConfirmLinearArray()}
                    data-testid="confirm-linear-array"
                  >
                    {text(APP_TEXT.confirmArray)}
                  </button>
                  <button type="button" onClick={onInvalidateLinearArray}>
                    {text(APP_TEXT.cancel2)}
                  </button>
                </div>
              )}
            </form>
          )}
          {line.kind !== 'boundary' && (
            <form
              onSubmit={onSubmitRadialArray}
              onInput={onInvalidateRadialArray}
              data-testid="radial-array-panel"
            >
              <fieldset disabled={coreBusy || line.locked || !radialArrayCenterAvailable}>
                <legend>{text(APP_TEXT.radialArray)}</legend>
                <label className="field">
                  {text(
                    APP_TEXT.onlyNonBoundaryEndpointsOfTheSelectedLineCanBeUsedAsRotationCenter,
                  )}
                  <select
                    key={`${line.id}:${radialArrayCenterOptions.map(({ id }) => id).join(':')}`}
                    name="radial_array_center"
                    defaultValue={radialArrayCenterOptions[0]?.id ?? ''}
                  >
                    {radialArrayCenterOptions.length === 0 ? (
                      <option value="">{text(APP_TEXT.unavailable)}</option>
                    ) : radialArrayCenterOptions.map(({ id, label }) => (
                      <option key={id} value={id}>{label}</option>
                    ))}
                  </select>
                </label>
                <label className="field">
                  {text(APP_TEXT.additionalCopies)}
                  <input
                    name="radial_array_copies"
                    type="number"
                    min="1"
                    max="3"
                    step="1"
                    defaultValue="1"
                  />
                </label>
                <label className="field">
                  {text(APP_TEXT.rotationAngle2)}
                  <select name="radial_array_angle" defaultValue="90">
                    <option value="90">90°</option>
                    <option value="180">180°</option>
                    <option value="270">270°</option>
                  </select>
                </label>
                <button type="submit" data-testid="preview-radial-array">
                  {text(APP_TEXT.previewRadialArray)}
                </button>
              </fieldset>
              {radialArrayPreview?.request.edges[0] === line.id && (
                <div data-testid="radial-array-preview" aria-live="polite">
                  <p>{formattedText(
                    APP_TEXT.copiesCopiesWillBeAddedAfterConfirmation,
                    { copies: radialArrayPreview.result.additional_copies },
                  )}</p>
                  <button
                    type="button"
                    data-testid="confirm-radial-array"
                    onClick={() => void onConfirmRadialArray()}
                  >
                    {text(APP_TEXT.confirmRadialArray)}
                  </button>
                  <button type="button" onClick={onInvalidateRadialArray}>
                    {text(APP_TEXT.cancel2)}
                  </button>
                </div>
              )}
            </form>
          )}
          <div className="property-actions">
            <button
              type="button"
              aria-pressed={parallelReferenceEdgeId === line.id}
              disabled={coreBusy}
              onClick={() => onToggleParallelReference(line.id)}
            >
              {parallelReferenceEdgeId === line.id
                ? text(APP_TEXT.clearDirectionReference)
                : text(APP_TEXT.setAsDirectionReference)}
            </button>
            {line.kind === 'boundary' ? (
              <button
                type="button"
                disabled={coreBusy || line.locked}
                onClick={() => void onSplitBoundaryEdge()}
              >
                {text(APP_TEXT.splitBoundaryEdgeAtMidpoint)}
              </button>
            ) : (
              <button
                type="button"
                className="danger"
                disabled={coreBusy || line.locked}
                onClick={() => void onDeleteSelection()}
              >
                {text(APP_TEXT.deleteLine)}
              </button>
            )}
          </div>
        </>
      )}
      {line.locked && (
        <p className="muted">
          {text(APP_TEXT.thisLineLayerIsLockedSelectionMeasurementAndReferencesRemain)}
        </p>
      )}
      {line.kind === 'boundary' && (
        <p className="muted">
          {text(APP_TEXT.moveTheNewlySelectedVertexAfterSplittingToEditThe)}
        </p>
      )}
    </>
  )
}
