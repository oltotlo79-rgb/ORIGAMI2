import type { FormEventHandler } from 'react'

import { APP_TEXT } from '../lib/appText.ts'
import type {
  ProjectSnapshot,
  VertexCoordinateExpressionBinding,
} from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  isUnverifiedLegacyV1EdgeGeometryBinding,
} from '../lib/edgeGeometryReferences.ts'
import {
  formatLengthInput,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'
import { MAX_NUMERIC_EXPRESSION_SOURCE_BYTES } from '../lib/numericExpressionNative.ts'
import {
  isUnverifiedLegacyV1PolarConstructionBinding,
} from '../lib/polarConstructionExpressions.ts'
import type { CompassConstructionCircle } from './CreaseCanvas.tsx'

type SelectedVertex = ProjectSnapshot['crease_pattern']['vertices'][number]

export type SelectedVertexInspectorProps = Readonly<{
  locale: Locale
  vertex: SelectedVertex
  expression: VertexCoordinateExpressionBinding | undefined
  displayUnit: ResolvedLengthDisplayUnit
  displayUnitLabel: string
  coreBusy: boolean
  locked: boolean
  boundary: boolean
  boundaryVertexCount: number
  cuttingAllowed: boolean
  compassCircleCount: number
  onSubmit: FormEventHandler<HTMLFormElement>
  onDeleteSelection: () => void | Promise<void>
  onAddCompassCircle: (circle: CompassConstructionCircle) => void
  onClearCompassCircles: () => void
}>

export function SelectedVertexInspector({
  locale,
  vertex,
  expression,
  displayUnit,
  displayUnitLabel,
  coreBusy,
  locked,
  boundary,
  boundaryVertexCount,
  cuttingAllowed,
  compassCircleCount,
  onSubmit,
  onDeleteSelection,
  onAddCompassCircle,
  onClearCompassCircles,
}: SelectedVertexInspectorProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)

  return (
    <>
      <dl>
        <div><dt>ID</dt><dd>{vertex.id}</dd></div>
        <div>
          <dt>{text(APP_TEXT.type)}</dt>
          <dd>{text(APP_TEXT.vertex)}</dd>
        </div>
      </dl>
      <form
        key={`${vertex.id}:${vertex.position.x}:${vertex.position.y}:${
          displayUnit.key
        }:${expression?.x_source ?? ''}:${expression?.y_source ?? ''}`}
        className="coordinate-form"
        onSubmit={onSubmit}
      >
        <label className="field">
          {`X (${displayUnitLabel})`}
          <input
            name="x_display"
            type="text"
            inputMode="text"
            maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
            defaultValue={displayUnit.millimetresPerUnit === 1 && expression
              ? expression.x_source
              : formatLengthInput(vertex.position.x, displayUnit)}
            disabled={coreBusy || locked}
            aria-label={formattedText(APP_TEXT.vertexXCoordinateUnit, {
              unit: displayUnitLabel,
            })}
          />
        </label>
        <label className="field">
          {`Y (${displayUnitLabel})`}
          <input
            name="y_display"
            type="text"
            inputMode="text"
            maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
            defaultValue={displayUnit.millimetresPerUnit === 1 && expression
              ? expression.y_source
              : formatLengthInput(vertex.position.y, displayUnit)}
            disabled={coreBusy || locked}
            aria-label={formattedText(APP_TEXT.vertexYCoordinateUnit, {
              unit: displayUnitLabel,
            })}
          />
        </label>
        <div className="property-actions">
          <button
            type="submit"
            name="vertex_action"
            value="update_coordinates"
            disabled={coreBusy || locked}
          >
            {text(APP_TEXT.updateCoordinates)}
          </button>
          <button
            type="button"
            className="danger"
            disabled={
              coreBusy || locked || (boundary && boundaryVertexCount <= 3)
            }
            onClick={() => void onDeleteSelection()}
          >
            {boundary
              ? text(APP_TEXT.deleteBoundaryVertexAndMergeEdges)
              : text(APP_TEXT.deleteVertex)}
          </button>
        </div>
        {isUnverifiedLegacyV1EdgeGeometryBinding(expression) ? (
          <p
            className="muted"
            role="note"
            data-unverified-legacy-edge-geometry-binding
          >
            {text(APP_TEXT.legacyV1EdgeGeometryReferenceIsUnverified)}
          </p>
        ) : null}
        {isUnverifiedLegacyV1PolarConstructionBinding(expression) ? (
          <p
            className="muted"
            role="note"
            data-unverified-legacy-polar-construction-binding
          >
            {text(APP_TEXT.legacyV1PolarConstructionIsUnverified)}
          </p>
        ) : null}
        {expression?.polar_construction ? (
          <p className="muted" data-vertex-polar-expression>
            {formattedText(
              APP_TEXT.constructionExpressionLengthLengthMmAngleAngleEvaluatedLengthValueMm,
              {
                length: expression.polar_construction.length_source,
                angle: expression.polar_construction.angle_degrees_source,
                lengthValue:
                  expression.polar_construction.adopted_length_mm,
                angleValue:
                  expression.polar_construction.adopted_angle_degrees,
              },
            )}
          </p>
        ) : null}
        <fieldset>
          <legend>{text(APP_TEXT.endpointByLengthAndAngle)}</legend>
          <label className="field">
            {`${text(APP_TEXT.length)} (${displayUnitLabel})`}
            <input
              name="polar_length_display"
              type="text"
              inputMode="text"
              maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
              defaultValue={formatLengthInput(10, displayUnit)}
              disabled={coreBusy || locked}
              aria-label={formattedText(
                APP_TEXT.lengthFromTheStartVertexUnit,
                { unit: displayUnitLabel },
              )}
            />
          </label>
          <label className="field">
            {text(APP_TEXT.angleDegrees)}
            <input
              name="polar_angle_degrees"
              type="text"
              inputMode="text"
              maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
              defaultValue="0"
              disabled={coreBusy || locked}
              aria-label={text(APP_TEXT.angleFromTheStartVertexDegrees)}
            />
          </label>
          <label className="field">
            {text(APP_TEXT.lineType)}
            <select
              name="polar_edge_kind"
              defaultValue="mountain"
              disabled={coreBusy || locked}
              aria-label={text(APP_TEXT.lineTypeForLengthAndAngleDrawing)}
            >
              <option value="mountain">{text(APP_TEXT.mountainFold)}</option>
              <option value="valley">{text(APP_TEXT.valleyFold)}</option>
              <option value="auxiliary">
                {text(APP_TEXT.auxiliaryLine)}
              </option>
              {cuttingAllowed && (
                <option value="cut">{text(APP_TEXT.cut2)}</option>
              )}
            </select>
          </label>
          <div className="property-actions">
            <button
              type="submit"
              name="vertex_action"
              value="polar_endpoint"
              disabled={coreBusy || locked}
            >
              {text(APP_TEXT.drawLineByLengthAndAngle)}
            </button>
            <button
              type="submit"
              name="vertex_action"
              value="ray_to_target"
              data-testid="draw-ray-to-first-target"
              disabled={coreBusy || locked}
            >
              {text(APP_TEXT.drawToFirstTargetByAngle)}
            </button>
          </div>
        </fieldset>
        <fieldset>
          <legend>{text(APP_TEXT.compassCircle)}</legend>
          <label className="field">
            {`${text(APP_TEXT.radius)} (${displayUnitLabel})`}
            <input
              name="compass_radius_display"
              type="number"
              inputMode="decimal"
              min="0.000001"
              step="any"
              defaultValue="10"
              disabled={coreBusy}
            />
          </label>
          <div className="property-actions">
            <button
              type="button"
              disabled={coreBusy}
              onClick={(event) => {
                const form = event.currentTarget.form
                const input = form?.elements.namedItem(
                  'compass_radius_display',
                )
                if (!(input instanceof HTMLInputElement)) return
                const displayRadius = Number(input.value)
                const radius = displayRadius * displayUnit.millimetresPerUnit
                if (!Number.isFinite(radius) || radius <= 0) return
                onAddCompassCircle({
                  centerVertexId: vertex.id,
                  centerX: vertex.position.x,
                  centerY: vertex.position.y,
                  radius,
                })
              }}
            >
              {text(APP_TEXT.addCircleAtSelectedVertex)}
            </button>
            <button
              type="button"
              disabled={coreBusy || compassCircleCount === 0}
              onClick={onClearCompassCircles}
            >
              {text(APP_TEXT.clearCompassCircles)}
            </button>
          </div>
          <p className="muted">
            {formattedText(
              APP_TEXT.countConstructionCirclesTheVertexToolSnapsToCircleLine,
              { count: compassCircleCount },
            )}
          </p>
        </fieldset>
        {locked && (
          <p className="muted">
            {text(APP_TEXT.thisVertexIsConnectedToALineOnALocked)}
          </p>
        )}
        <p className="muted">
          {boundary
            ? formattedText(
                APP_TEXT.aBoundaryNeedsAtLeastThreePointsCountCurrentlyThis,
                { count: boundaryVertexCount },
              )
            : text(APP_TEXT.deleteConnectedLinesBeforeDeletingTheirVertex)}
        </p>
      </form>
    </>
  )
}

export function BenchmarkVertexInspector({
  locale,
  vertex,
}: Readonly<{
  locale: Locale
  vertex: Readonly<{ id: string; x: number; y: number }>
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  return (
    <>
      <dl>
        <div><dt>ID</dt><dd>{vertex.id}</dd></div>
        <div>
          <dt>{text(APP_TEXT.type)}</dt>
          <dd>{text(APP_TEXT.benchmarkVertex)}</dd>
        </div>
        <div><dt>X</dt><dd>{vertex.x}</dd></div>
        <div><dt>Y</dt><dd>{vertex.y}</dd></div>
      </dl>
      <p className="muted">
        {text(APP_TEXT.dragTheBenchmarkVertexIn2DToMoveItAnd)}
      </p>
    </>
  )
}

export type DirectVertexInspectorProps = Readonly<{
  locale: Locale
  projectInstanceId: string
  displayUnit: ResolvedLengthDisplayUnit
  displayUnitLabel: string
  coreBusy: boolean
  defaultLayerLocked: boolean
  onSubmit: FormEventHandler<HTMLFormElement>
}>

export function DirectVertexInspector({
  locale,
  projectInstanceId,
  displayUnit,
  displayUnitLabel,
  coreBusy,
  defaultLayerLocked,
  onSubmit,
}: DirectVertexInspectorProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  return (
    <>
      <p className="muted">
        {text(APP_TEXT.selectALineOrVertexOrAddAVertexBy)}
      </p>
      <form
        key={`${projectInstanceId}:${displayUnit.key}`}
        className="coordinate-form"
        onSubmit={onSubmit}
      >
        <label className="field">
          {`X (${displayUnitLabel})`}
          <input
            name="direct_x_display"
            type="text"
            inputMode="text"
            maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
            defaultValue="0"
            disabled={coreBusy || defaultLayerLocked}
            aria-label={formattedText(APP_TEXT.newVertexXCoordinateUnit, {
              unit: displayUnitLabel,
            })}
          />
        </label>
        <label className="field">
          {`Y (${displayUnitLabel})`}
          <input
            name="direct_y_display"
            type="text"
            inputMode="text"
            maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
            defaultValue="0"
            disabled={coreBusy || defaultLayerLocked}
            aria-label={formattedText(APP_TEXT.newVertexYCoordinateUnit, {
              unit: displayUnitLabel,
            })}
          />
        </label>
        <div className="property-actions">
          <button
            type="submit"
            disabled={coreBusy || defaultLayerLocked}
          >
            {text(APP_TEXT.addVertexByCoordinates)}
          </button>
        </div>
        {defaultLayerLocked && (
          <p className="muted">
            {text(APP_TEXT.unlockTheDefaultLayerBeforeAddingAVertex)}
          </p>
        )}
      </form>
    </>
  )
}
