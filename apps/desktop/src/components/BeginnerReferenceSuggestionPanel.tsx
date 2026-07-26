import { APP_TEXT } from '../lib/appText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerEditorState } from '../lib/useBeginnerEditorState.ts'
import type { useBeginnerReferenceWorkflow } from '../lib/useBeginnerReferenceWorkflow.ts'
import { RecognitionContourCopyAction } from './RecognitionContourCopyAction.tsx'

type EditorState = ReturnType<typeof useBeginnerEditorState>
type ReferenceWorkflow = ReturnType<typeof useBeginnerReferenceWorkflow>

export function BeginnerReferenceSuggestionPanel({
  locale,
  editor,
  workflow,
}: Readonly<{
  locale: Locale
  editor: EditorState
  workflow: ReferenceWorkflow
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const {
    beginnerComponentBridgeOverride,
    setBeginnerComponentBridgeOverride,
  } = editor
  const {
    beginnerReferenceGeometry,
    beginnerReferenceSuggestion,
    beginnerSurfaceAssignments,
    setBeginnerSurfaceAssignments,
    beginnerSurfaceEdits,
    setBeginnerSurfaceEdits,
    confirmBeginnerReferenceSuggestion,
    copyBeginnerReferenceContours,
    copyBeginnerGeneralReferenceTarget,
  } = workflow

  return (
    <>
      {beginnerReferenceSuggestion && (
        <div role="status">
          <p>{text(APP_TEXT.thisIsNot3DRecognitionItIsAReadOnly)}</p>
          <p>{formattedText(
            APP_TEXT.countProtrusionsLengthLengthMmThicknessThicknessMm,
            {
              count: beginnerReferenceSuggestion.protrusions.reduce(
                (sum, target) => sum + target.count,
                0,
              ),
              length: (beginnerReferenceSuggestion.protrusions[0]
                ?.length_tenths_mm ?? 0) / 10,
              thickness: (beginnerReferenceSuggestion.protrusions[0]
                ?.thickness_tenths_mm ?? 0) / 10,
            },
          )}</p>
          <p>{formattedText(
            APP_TEXT.general3DProposalQualityScore100PrincipalExtentsXY,
            {
              score: beginnerReferenceSuggestion.quality_score,
              x: beginnerReferenceSuggestion
                .principal_axis_extents_tenths_mm[0],
              y: beginnerReferenceSuggestion
                .principal_axis_extents_tenths_mm[1],
              z: beginnerReferenceSuggestion
                .principal_axis_extents_tenths_mm[2],
              protrusions: beginnerReferenceSuggestion
                .general_protrusion_candidates.length,
              bars: beginnerReferenceSuggestion.stick_bars.length,
            },
          )}</p>
          {beginnerReferenceSuggestion.insufficiency_reasons.length > 0 && (
            <p>{formattedText(
              APP_TEXT.general3DProposalInsufficiencyReasons,
              {
                reasons: beginnerReferenceSuggestion
                  .insufficiency_reasons.join(', '),
              },
            )}</p>
          )}
          <fieldset>
            <legend>
              {text(APP_TEXT.explicitlyAssignMeasuredSurfaceRangesTo28Parts)}
            </legend>
            {beginnerReferenceSuggestion.surface_ranges.map(
              (range, index) => {
                const target = beginnerReferenceSuggestion.protrusions[index]
                if (!target) return null
                const edit = beginnerSurfaceEdits.find(
                  (item) => item.range_id === range.id,
                )
                return (
                  <div key={range.id}>
                    <input
                      type="checkbox"
                      aria-label={formattedText(
                        APP_TEXT.assignSurfaceRangeRangeIdToPartPartId,
                        { rangeId: range.id, partId: target.id },
                      )}
                      checked={beginnerSurfaceAssignments.some(
                        (item) => item.range_id === range.id,
                      )}
                      onChange={(event) =>
                        setBeginnerSurfaceAssignments((current) => (
                          event.currentTarget.checked
                            ? [...current, {
                                range_id: range.id,
                                protrusion_id: target.id,
                              }]
                            : current.filter(
                                (item) => item.range_id !== range.id,
                              )
                        ))}
                    />
                    {formattedText(
                      APP_TEXT.surfaceRangeIdCenterXYZLengthLengthMm,
                      {
                        id: range.id,
                        x: target.position_tenths_mm[0] / 10,
                        y: target.position_tenths_mm[1] / 10,
                        z: target.position_tenths_mm[2] / 10,
                        length: target.length_tenths_mm / 10,
                      },
                    )}
                    <span>{formattedText(APP_TEXT.partId, {
                      id: target.id,
                    })}</span>
                    <span>
                      {text(APP_TEXT.triangleIndicesAddRemoveAdjacentFacesOnly)}
                    </span>
                    <input
                      type="text"
                      aria-label={formattedText(
                        APP_TEXT.surfaceRangeRangeIdTriangleIndices,
                        { rangeId: range.id },
                      )}
                      value={edit?.triangle_indices.join(',') ?? ''}
                      onChange={(event) => {
                        const indices = event.currentTarget.value
                          .split(',')
                          .map((value) => Number(value.trim()))
                          .filter((value) => (
                            Number.isInteger(value) && value >= 0
                          ))
                        setBeginnerSurfaceEdits((current) =>
                          current.map((item) => item.range_id === range.id
                            ? {
                                ...item,
                                triangle_indices: [...new Set(indices)],
                              }
                            : item))
                      }}
                    />
                    {(['X', 'Y', 'Z'] as const).map((axis, axisIndex) => (
                      <label key={axis}>
                        <span>{`Bulge direction ${axis}`}</span>
                        <input
                          type="number"
                          min="-1"
                          max="1"
                          step="0.001"
                          value={
                            (edit?.bulge_direction_milli[axisIndex] ?? 0)
                            / 1_000
                          }
                          onChange={(event) =>
                            setBeginnerSurfaceEdits((current) =>
                              current.map((item) => {
                                if (item.range_id !== range.id) return item
                                const direction = [
                                  ...item.bulge_direction_milli,
                                ] as [number, number, number]
                                direction[axisIndex] = Math.round(
                                  Number(event.currentTarget.value) * 1_000,
                                )
                                return {
                                  ...item,
                                  bulge_direction_milli: direction,
                                }
                              }))}
                        />
                      </label>
                    ))}
                    <label>
                      <span>{text(APP_TEXT.bulgeAmountMm)}</span>
                      <input
                        type="number"
                        min="0.1"
                        max="100000"
                        step="0.1"
                        value={(edit?.bulge_amount_tenths_mm ?? 1) / 10}
                        onChange={(event) =>
                          setBeginnerSurfaceEdits((current) =>
                            current.map((item) => item.range_id === range.id
                              ? {
                                  ...item,
                                  bulge_amount_tenths_mm: Math.round(
                                    Number(event.currentTarget.value) * 10,
                                  ),
                                }
                              : item))}
                      />
                    </label>
                  </div>
                )
              },
            )}
            <p>
              {text(APP_TEXT.onlyGLBMeasuredRangesAreShownDuplicateUnconfirmedOrTampered)}
            </p>
          </fieldset>
          <button
            type="button"
            onClick={confirmBeginnerReferenceSuggestion}
            disabled={beginnerSurfaceAssignments.length < 2}
          >
            {text(APP_TEXT.confirmAndApplySuggestedRanges)}
          </button>
          {(beginnerReferenceSuggestion.generic_body_outline_tenths_mm
            || beginnerReferenceSuggestion.protrusions.some(
              (target) => target.local_outline_tenths_mm,
            )) && (
            <>
              <p>{formattedText(
                APP_TEXT.editableBodyContourBodyPointsLocalContoursLocal,
                {
                  body: beginnerReferenceSuggestion
                    .generic_body_outline_tenths_mm?.length ?? 0,
                  local: beginnerReferenceSuggestion.protrusions.filter(
                    (target) => target.local_outline_tenths_mm,
                  ).length,
                },
              )}</p>
              <button
                type="button"
                hidden
                onClick={copyBeginnerReferenceContours}
              >
                {text(APP_TEXT.reviewAndCopyContoursToEditor)}
              </button>
            </>
          )}
          <RecognitionContourCopyAction
            locale={locale}
            bodyPointCount={beginnerReferenceSuggestion
              .generic_body_outline_tenths_mm?.length ?? 0}
            localContourCount={beginnerReferenceSuggestion.protrusions.filter(
              (target) => target.local_outline_tenths_mm,
            ).length}
            onCopy={copyBeginnerReferenceContours}
          />
          <button
            type="button"
            onClick={copyBeginnerGeneralReferenceTarget}
          >
            {text(APP_TEXT.reviewAndCopyGeneral3DProposalToEditor)}
          </button>
          {beginnerComponentBridgeOverride && (
            <fieldset
              aria-label={text(APP_TEXT.reviewedComponentBridgeOverrides)}
            >
              <legend>Component bridges (reviewed, maximum 7)</legend>
              {beginnerComponentBridgeOverride.bridges.map(
                (bridge, index) => (
                  <label key={bridge.id}>
                    <input
                      type="checkbox"
                      checked={bridge.accepted}
                      onChange={(event) =>
                        setBeginnerComponentBridgeOverride({
                          ...beginnerComponentBridgeOverride,
                          bridges: beginnerComponentBridgeOverride.bridges.map(
                            (item, itemIndex) => itemIndex === index
                              ? { ...item, accepted: event.target.checked }
                              : item,
                          ),
                        })}
                    />
                    {`Bridge ${bridge.id}: component `}
                    <select
                      value={bridge.start_component_id}
                      onChange={(event) =>
                        setBeginnerComponentBridgeOverride({
                          ...beginnerComponentBridgeOverride,
                          bridges: beginnerComponentBridgeOverride.bridges.map(
                            (item, itemIndex) => itemIndex === index
                              ? {
                                  ...item,
                                  start_component_id: Number(
                                    event.target.value,
                                  ),
                                }
                              : item,
                          ),
                        })}
                    >
                      {Array.from(
                        {
                          length:
                            beginnerComponentBridgeOverride.component_count,
                        },
                        (_, id) => (
                          <option key={id} value={id}>{id}</option>
                        ),
                      )}
                    </select>
                    {' to '}
                    <select
                      value={bridge.end_component_id}
                      onChange={(event) =>
                        setBeginnerComponentBridgeOverride({
                          ...beginnerComponentBridgeOverride,
                          bridges: beginnerComponentBridgeOverride.bridges.map(
                            (item, itemIndex) => itemIndex === index
                              ? {
                                  ...item,
                                  end_component_id: Number(
                                    event.target.value,
                                  ),
                                }
                              : item,
                          ),
                        })}
                    >
                      {Array.from(
                        {
                          length:
                            beginnerComponentBridgeOverride.component_count,
                        },
                        (_, id) => (
                          <option key={id} value={id}>{id}</option>
                        ),
                      )}
                    </select>
                  </label>
                ),
              )}
            </fieldset>
          )}
        </div>
      )}
      {beginnerReferenceGeometry && (
        <svg
          viewBox="-100 -100 200 200"
          role="img"
          aria-label={text(APP_TEXT.readOnly3DReferenceModel)}
        >
          {beginnerReferenceGeometry.triangle_indices.map(
            (triangle, index) => {
              const points = triangle.map((vertex) => {
                const position = beginnerReferenceGeometry.positions[vertex]
                return `${position[0]},${-position[1]}`
              }).join(' ')
              return (
                <polygon
                  key={index}
                  points={points}
                  fill="none"
                  stroke="currentColor"
                />
              )
            },
          )}
        </svg>
      )}
    </>
  )
}
