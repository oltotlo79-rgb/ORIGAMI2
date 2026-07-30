import { APP_TEXT } from '../lib/appText.ts'
import type { BeginnerDesignProfileV1 } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
  type MessageVariables,
} from '../lib/i18n.ts'
import {
  MAX_BEGINNER_PART_ASSIGNMENTS_V1,
  type useBeginnerRecognitionWorkflow,
} from '../lib/useBeginnerRecognitionWorkflow.ts'

type RecognitionWorkflow = ReturnType<typeof useBeginnerRecognitionWorkflow>

type BeginnerRecognitionPanelProps = Readonly<{
  locale: Locale
  coreBusy: boolean
  recoveryBlocking: boolean
  workflow: RecognitionWorkflow
}>

export function BeginnerRecognitionPanel({
  locale,
  coreBusy,
  recoveryBlocking,
  workflow,
}: BeginnerRecognitionPanelProps) {
  const text = (localized: LocalizedText) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: LocalizedText,
    variables?: MessageVariables,
  ) => formatLocalizedText(locale, localized, variables)
  const {
    beginnerRecognitionProposal,
    acceptedRecognitionProtrusionIds,
    setAcceptedRecognitionProtrusionIds,
    beginnerRecognitionBusy,
    beginnerSilhouetteThresholds,
    setBeginnerSilhouetteThresholds,
    beginnerSilhouetteCropRoi,
    setBeginnerSilhouetteCropRoi,
    beginnerSilhouetteOrientation,
    setBeginnerSilhouetteOrientation,
    beginnerSilhouetteMirror,
    setBeginnerSilhouetteMirror,
    beginnerOutlineCandidates,
    beginnerPartSuggestions,
    beginnerPartAssignments,
    setBeginnerPartAssignments,
    excludedBeginnerPartAssignments,
    setExcludedBeginnerPartAssignments,
    invalidateBeginnerRecognition,
    requestBeginnerRecognition,
    requestBeginnerOutlineCandidates,
    copyBeginnerOutlineCandidate,
    requestBeginnerPartSuggestions,
    confirmBeginnerPartAssignments,
    copyBeginnerRecognitionProposal,
  } = workflow

  return (
                <div aria-live="polite">
                  <button
                    type="button"
                    onClick={() => requestBeginnerRecognition('marker')}
                    disabled={beginnerRecognitionBusy || coreBusy || recoveryBlocking}
                    aria-describedby="beginner-recognition-help"
                  >
                    {beginnerRecognitionBusy
                      ? text(APP_TEXT.recognizing)
                      : text(APP_TEXT.recognizeMarkerPNG)}
                  </button>
                  <button
                    type="button"
                    onClick={() => requestBeginnerRecognition('silhouette')}
                    disabled={beginnerRecognitionBusy || coreBusy || recoveryBlocking}
                    aria-describedby="beginner-recognition-help"
                  >
                    {beginnerRecognitionBusy
                      ? text(APP_TEXT.recognizing)
                      : text(APP_TEXT.recognizeOutlineFromImage)}
                  </button>
                  <label>
                    {text(APP_TEXT.silhouetteAlphaThreshold)}
                    <input type="range" min="0" max="255" value={beginnerSilhouetteThresholds.alpha}
                      onChange={(event) => { invalidateBeginnerRecognition(); setBeginnerSilhouetteThresholds((value) => ({ ...value, alpha: Number(event.target.value) })) }} />
                    <output>{beginnerSilhouetteThresholds.alpha}</output>
                  </label>
                  <label>
                    {text(APP_TEXT.silhouetteLumaThreshold)}
                    <input type="range" min="0" max="255" value={beginnerSilhouetteThresholds.luma}
                      onChange={(event) => { invalidateBeginnerRecognition(); setBeginnerSilhouetteThresholds((value) => ({ ...value, luma: Number(event.target.value) })) }} />
                    <output>{beginnerSilhouetteThresholds.luma}</output>
                  </label>
                  <label>
                    {text(APP_TEXT.silhouetteForegroundPolarity)}
                    <select value={beginnerSilhouetteThresholds.polarity} onChange={(event) => {
                      invalidateBeginnerRecognition()
                      setBeginnerSilhouetteThresholds((value) => ({ ...value,
                        polarity: event.target.value as 'dark_on_light' | 'light_on_dark' | 'alpha_only' }))
                    }}>
                      <option value="dark_on_light">{text(APP_TEXT.darkOnLight)}</option>
                      <option value="light_on_dark">{text(APP_TEXT.lightOnDark)}</option>
                      <option value="alpha_only">{text(APP_TEXT.alphaOnly)}</option>
                    </select>
                  </label>
                  <fieldset aria-label={text(APP_TEXT.silhouetteCropROI)}>
                    <legend>{text(APP_TEXT.silhouetteCropROI2)}</legend>
                    <label><input type="checkbox" checked={Boolean(beginnerSilhouetteCropRoi)} onChange={(event) => {
                      invalidateBeginnerRecognition()
                      setBeginnerSilhouetteCropRoi(event.target.checked ? { schema_version: 1, x_millionths: 0, y_millionths: 0, width_millionths: 1_000_000, height_millionths: 1_000_000 } : undefined)
                    }} />{text(APP_TEXT.useCrop)}</label>
                    {beginnerSilhouetteCropRoi && (['x_millionths', 'y_millionths', 'width_millionths', 'height_millionths'] as const).map((key) => (
                      <label key={key}>{key}<input type="number" min="0" max="1000000" step="1000" value={beginnerSilhouetteCropRoi[key]}
                        onChange={(event) => { invalidateBeginnerRecognition(); setBeginnerSilhouetteCropRoi({ ...beginnerSilhouetteCropRoi, [key]: Math.max(0, Math.min(1_000_000, Number(event.target.value))) }) }} /></label>
                    ))}
                    <button type="button" onClick={() => setBeginnerSilhouetteCropRoi(undefined)}>{text(APP_TEXT.resetToFullImage)}</button>
                  </fieldset>
                  <label>
                    {text(APP_TEXT.silhouetteOrientation)}
                    <select value={beginnerSilhouetteOrientation} onChange={(event) => {
                      invalidateBeginnerRecognition()
                      setBeginnerSilhouetteOrientation(Number(event.target.value) as 0 | 90 | 180 | 270)
                    }}>
                      {[0, 90, 180, 270].map((angle) => <option key={angle} value={angle}>{angle}°</option>)}
                    </select>
                    <button type="button" onClick={() => setBeginnerSilhouetteOrientation(0)}>{text(APP_TEXT.resetOrientation)}</button>
                  </label>
                  <fieldset aria-label={text(APP_TEXT.silhouetteMirror)}>
                    <legend>{text(APP_TEXT.silhouetteMirror2)}</legend>
                    <label><input type="checkbox" checked={beginnerSilhouetteMirror.mirror_x}
                      onChange={(event) => { invalidateBeginnerRecognition(); setBeginnerSilhouetteMirror((value) => ({ ...value, mirror_x: event.target.checked })) }} />
                      {text(APP_TEXT.mirrorHorizontally)}</label>
                    <label><input type="checkbox" checked={beginnerSilhouetteMirror.mirror_y}
                      onChange={(event) => { invalidateBeginnerRecognition(); setBeginnerSilhouetteMirror((value) => ({ ...value, mirror_y: event.target.checked })) }} />
                      {text(APP_TEXT.mirrorVertically)}</label>
                    <button type="button" onClick={() => setBeginnerSilhouetteMirror({ schema_version: 1, mirror_x: false, mirror_y: false })}>
                      {text(APP_TEXT.resetMirror)}</button>
                  </fieldset>
                  <p id="beginner-recognition-help" className="muted">
                    {text(APP_TEXT.boundedPNGOrJPEGInputProducesAReadOnlyOutline)}
                  </p>
                  <button
                    type="button"
                    onClick={requestBeginnerOutlineCandidates}
                    disabled={beginnerRecognitionBusy || coreBusy || recoveryBlocking}
                  >
                    {text(APP_TEXT.showOutlineCandidates)}
                  </button>
                  {beginnerOutlineCandidates && (
                    <section aria-labelledby="beginner-outline-candidates-heading">
                      <h3 id="beginner-outline-candidates-heading">
                        {text(APP_TEXT.readOnlyOutlineCandidates)}
                      </h3>
                      <p>{text(APP_TEXT.candidatesExposeOnlyBoundsAreaAndReasonTheyGrantNo)}</p>
                      <ol>
                        {beginnerOutlineCandidates.candidates.map((candidate) => (
                          <li key={candidate.id}>
                            {formattedText(APP_TEXT.areaAreaPxBoundsMinXMinYMaxXMaxYReasonReason, {
                              area: candidate.area_pixels,
                              minX: candidate.bounds.min_x, minY: candidate.bounds.min_y,
                              maxX: candidate.bounds.max_x, maxY: candidate.bounds.max_y,
                              reason: candidate.confidence_reason === 'solid_component'
                                ? text(APP_TEXT.solidComponent)
                                : text(APP_TEXT.smallComponent),
                            })}
                            <button
                              type="button"
                              onClick={() => copyBeginnerOutlineCandidate(candidate)}
                              disabled={coreBusy || recoveryBlocking}
                            >
                              {text(APP_TEXT.confirmAndCopyToTarget)}
                            </button>
                            <button type="button" onClick={() => requestBeginnerPartSuggestions(candidate)}>
                              {text(APP_TEXT.suggestParts)}
                            </button>
                          </li>
                        ))}
                      </ol>
                      {beginnerPartSuggestions && (
                        <fieldset>
                          <legend>{text(APP_TEXT.explicitPartAssignments)}</legend>
                          {beginnerPartAssignments.map((assignment, index) => (
                            <label key={`${assignment.candidate_id}:${assignment.split_fragment ?? 'original'}:${index}`}>
                              {formattedText(APP_TEXT.candidateId, { id: assignment.candidate_id + 1 })}
                              <select value={assignment.kind} onChange={(event) => {
                                const kind = event.currentTarget.value as
                                  BeginnerDesignProfileV1['generation_constraints']['target_parts'][number]['kind']
                                setBeginnerPartAssignments((items) => items.map((item, itemIndex) =>
                                  itemIndex === index ? { ...item, kind } : item))
                              }}>
                                <option value="torso">{text(APP_TEXT.torso2)}</option>
                                <option value="head">{text(APP_TEXT.head2)}</option>
                                <option value="leg">{text(APP_TEXT.leg2)}</option>
                                <option value="wing">{text(APP_TEXT.wing2)}</option>
                                <option value="fin">{text(APP_TEXT.fin2)}</option>
                                <option value="ear">{text(APP_TEXT.ear2)}</option>
                                <option value="horn">{text(APP_TEXT.horn2)}</option>
                                <option value="antenna">{text(APP_TEXT.antenna2)}</option>
                                <option value="tail">{text(APP_TEXT.tail2)}</option>
                              </select>
                              {assignment.split_fragment === 0 && assignment.split_x !== undefined && (
                                <span>
                                  {text(APP_TEXT.verticalSplitPositionXPx)}
                                  <input type="number" value={assignment.split_x}
                                    min={beginnerOutlineCandidates?.candidates.find(
                                      (candidate) => candidate.id === assignment.candidate_id)?.bounds.min_x ?? 0}
                                    max={beginnerOutlineCandidates?.candidates.find(
                                      (candidate) => candidate.id === assignment.candidate_id)?.bounds.max_x ?? 0}
                                    onChange={(event) => {
                                      const splitX = Number(event.currentTarget.value)
                                      setBeginnerPartAssignments((items) => items.map((item) =>
                                        item.candidate_id === assignment.candidate_id
                                          && item.source_candidate_ids?.length === 1
                                          ? { ...item, split_x: splitX } : item))
                                    }} />
                                </span>
                              )}
                              <button
                                type="button"
                                disabled={assignment.kind === 'torso'
                                  || beginnerPartAssignments.length <= 2}
                                onClick={() => {
                                  setBeginnerPartAssignments((items) =>
                                    items.filter((item) => item.candidate_id !== assignment.candidate_id))
                                  setExcludedBeginnerPartAssignments((items) => [
                                    ...items.filter((item) => item.candidate_id !== assignment.candidate_id),
                                    assignment,
                                  ])
                                }}
                              >
                                {text(APP_TEXT.excludeAsImageNoise)}
                              </button>
                            </label>
                          ))}
                          {excludedBeginnerPartAssignments.length > 0 && (
                            <section aria-label={text(APP_TEXT.excludedImageCandidates)}>
                              <p>{text(APP_TEXT.restoredCandidatesRemainSemanticallyUnconfirmedAndCannotGenerateADesign)}</p>
                              {excludedBeginnerPartAssignments.map((assignment) => (
                                <button key={assignment.candidate_id} type="button" onClick={() => {
                                  setExcludedBeginnerPartAssignments((items) =>
                                    items.filter((item) => item.candidate_id !== assignment.candidate_id))
                                  setBeginnerPartAssignments((items) => [...items, assignment].sort(
                                    (left, right) => left.candidate_id - right.candidate_id,
                                  ))
                                }}>
                                  {formattedText(APP_TEXT.restoreCandidateIdWithItsOriginalOutlineEvidence, { id: assignment.candidate_id + 1 })}
                                </button>
                              ))}
                            </section>
                          )}
                          <section aria-label={text(APP_TEXT.outlineComponentEditProposal)}>
                            <p>{text(APP_TEXT.splitAndMergeEditsAreNonAuthoritativeProposalsBoundTo)}</p>
                            <button type="button" onClick={() => setBeginnerPartAssignments((items) => {
                              const index = items.findIndex((item) => item.kind !== 'torso'
                                && item.split_fragment === undefined)
                              if (
                                index < 0
                                || items.length >= MAX_BEGINNER_PART_ASSIGNMENTS_V1
                              ) return items
                              const source = items[index]
                              const outline = beginnerOutlineCandidates?.candidates.find(
                                (candidate) => candidate.id === source.candidate_id)
                              if (!outline || outline.bounds.min_x >= outline.bounds.max_x) return items
                              const splitX = Math.floor((outline.bounds.min_x + outline.bounds.max_x + 1) / 2)
                              const split = [
                                { ...source, source_candidate_ids: [source.candidate_id],
                                  split_fragment: 0, split_x: splitX },
                                { ...source, kind: 'tail' as const,
                                  source_candidate_ids: [source.candidate_id],
                                  split_fragment: 1, split_x: splitX },
                              ]
                              return [...items.slice(0, index), ...split, ...items.slice(index + 1)]
                            })}>
                              {text(APP_TEXT.splitFirstPartCandidate)}
                            </button>
                            <button type="button" onClick={() => setBeginnerPartAssignments((items) => {
                              const indexes = items.map((item, index) => ({ item, index }))
                                .filter(({ item }) => item.kind !== 'torso'
                                  && item.split_fragment === undefined).slice(0, 2)
                              if (indexes.length !== 2) return items
                              const first = indexes[0]!
                              const second = indexes[1]!
                              const merged = { ...first.item,
                                candidate_id: Math.min(first.item.candidate_id, second.item.candidate_id),
                                source_candidate_ids: [first.item.candidate_id, second.item.candidate_id]
                                  .sort((left, right) => left - right),
                              }
                              return items.filter((_, index) => index !== first.index && index !== second.index)
                                .concat(merged).sort((left, right) => left.candidate_id - right.candidate_id)
                            })}>
                              {text(APP_TEXT.mergeFirstTwoPartCandidates)}
                            </button>
                          </section>
                          <p>{text(APP_TEXT.theImageProvesOnlyEachCandidateOutlinePartMeaningsCome)}</p>
                          <button type="button" onClick={confirmBeginnerPartAssignments}>
                            {text(APP_TEXT.confirmTargetParts)}
                          </button>
                        </fieldset>
                      )}
                    </section>
                  )}
                  {beginnerRecognitionProposal && (
                    <section aria-labelledby="beginner-recognition-heading">
                      <h3 id="beginner-recognition-heading">
                        {text(APP_TEXT.recognitionProposalPreview)}
                      </h3>
                      <p>
                        {formattedText(APP_TEXT.imageWidthHeightPxPartsPartsSegmentsSkeletonBars, {
                          width: beginnerRecognitionProposal.width,
                          height: beginnerRecognitionProposal.height,
                          parts: beginnerRecognitionProposal.target_parts.reduce(
                            (sum, part) => sum + part.count, 0,
                          ),
                          segments: beginnerRecognitionProposal.skeleton_segments.length,
                        })}
                      </p>
                      <svg
                        viewBox={`0 0 ${beginnerRecognitionProposal.width} ${beginnerRecognitionProposal.height}`}
                        role="img"
                        aria-label={text(APP_TEXT.recognizedShapeBoundsAndSkeleton)}
                      >
                        <rect
                          x={beginnerRecognitionProposal.shape_bounds.min_x}
                          y={beginnerRecognitionProposal.shape_bounds.min_y}
                          width={beginnerRecognitionProposal.shape_bounds.max_x
                            - beginnerRecognitionProposal.shape_bounds.min_x + 1}
                          height={beginnerRecognitionProposal.shape_bounds.max_y
                            - beginnerRecognitionProposal.shape_bounds.min_y + 1}
                          fill="none"
                          stroke="currentColor"
                        />
                        {beginnerRecognitionProposal.skeleton_segments.map((segment) => (
                          <line
                            key={segment.id}
                            x1={segment.start.x_tenths_mm / 10}
                            y1={segment.start.y_tenths_mm / 10}
                            x2={segment.end.x_tenths_mm / 10}
                            y2={segment.end.y_tenths_mm / 10}
                            stroke="currentColor"
                            strokeWidth={Math.max(1, segment.thickness_tenths_mm / 10)}
                          />
                        ))}
                      </svg>
                      <button type="button" onClick={copyBeginnerRecognitionProposal}>
                        {text(APP_TEXT.copyToEditableFields)}
                      </button>
                      {(beginnerRecognitionProposal.generic_body_outline_tenths_mm
                        || beginnerRecognitionProposal.protrusions?.some(
                          (target) => target.local_outline_tenths_mm)) && <p>{formattedText(APP_TEXT.recognizedContoursBodyBodyPointsAndLocalLocalContoursConfirmation, {
                        body: beginnerRecognitionProposal.generic_body_outline_tenths_mm?.length ?? 0,
                        local: beginnerRecognitionProposal.protrusions?.filter(
                          (target) => target.local_outline_tenths_mm).length ?? 0,
                      })}</p>}
                      {beginnerRecognitionProposal.contour_confidence && <p>{formattedText(APP_TEXT.contourConfidenceScore100ReasonsReasons, { score: beginnerRecognitionProposal.contour_confidence.body_score,
                        reasons: beginnerRecognitionProposal.contour_confidence.body_reasons.join(', ') })}</p>}
                      {beginnerRecognitionProposal.skeleton_quality && (
                        <div role="status" aria-label={text(APP_TEXT.skeletonProposalQuality)}>
                          <p>{formattedText(APP_TEXT.skeletonQualityScore100FullyOfflineDistanceAxisApproximationLimit, {
                            score: beginnerRecognitionProposal.skeleton_quality.score,
                            limit: beginnerRecognitionProposal.skeleton_quality.bar_limit,
                          })}</p>
                          {beginnerRecognitionProposal.skeleton_quality.insufficiency_reasons.length > 0 && <p>{formattedText(APP_TEXT.insufficiencyReasonsReasons, { reasons: beginnerRecognitionProposal.skeleton_quality.insufficiency_reasons.join(', ') })}</p>}
                        </div>
                      )}
                      {(beginnerRecognitionProposal.protrusions?.length ?? 0) > 0 && (
                        <fieldset><legend>{text(APP_TEXT.confirmRecognizedProtrusions)}</legend>
                          {(beginnerRecognitionProposal.protrusions ?? []).map((target) => (
                            <label key={target.id}>
                              <input type="checkbox" checked={acceptedRecognitionProtrusionIds.has(target.id)}
                                onChange={(event) => setAcceptedRecognitionProtrusionIds((current) => {
                                  const next = new Set(current)
                                  if (event.target.checked) next.add(target.id); else next.delete(target.id)
                                  return next
                                })} />
                              {formattedText(APP_TEXT.protrusionIdLocalContourPointsPoints, {
                                id: target.id, points: target.local_outline_tenths_mm?.length ?? 0,
                              })}
                            </label>
                          ))}
                        </fieldset>
                      )}
                    </section>
                  )}
                </div>

  )
}
