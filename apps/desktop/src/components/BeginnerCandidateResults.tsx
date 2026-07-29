import { CompleteAnimalBindingList } from './CompleteAnimalBindingList'
import { CompleteInsectBindingList } from './CompleteInsectBindingList'
import { GenericTargetBindingList } from './GenericTargetBindingList'
import { APP_TEXT } from '../lib/appText.ts'
import { isBeginnerApplicableTemplate } from '../lib/beginnerApplicableTemplate.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
  type MessageVariables,
} from '../lib/i18n.ts'
import type { useBeginnerCandidateWorkflow } from '../lib/useBeginnerCandidateWorkflow.ts'

type CandidateWorkflow = ReturnType<typeof useBeginnerCandidateWorkflow>

type BeginnerCandidateResultsProps = Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  coreBusy: boolean
  recoveryBlocking: boolean
  candidateWorkflow: CandidateWorkflow
}>

export function BeginnerCandidateResults({
  locale,
  snapshot: nativeSnapshot,
  coreBusy,
  recoveryBlocking,
  candidateWorkflow,
}: BeginnerCandidateResultsProps) {
  const text = (localized: LocalizedText) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: LocalizedText,
    variables?: MessageVariables,
  ) => formatLocalizedText(locale, localized, variables)
  const {
    beginnerCandidates,
    beginnerCandidateBusy,
    selectedConsensusPair,
    setSelectedConsensusPair,
    requestBeginnerCandidates,
    excludeBeginnerConsensusAsset,
    confirmAndApplyBeginnerPlan,
  } = candidateWorkflow

  return (
    <>
                {beginnerCandidates && (
                  <>
                  <p role="note" className="muted">
                    {text(APP_TEXT.initialDesignTreatsBulgesAsTargetShapeApproximationsAndDoes)}
                  </p>
                  <ol aria-label={text(APP_TEXT.designCandidatesInScoreOrder)}>
                    {beginnerCandidates.candidates.map((candidate) => (
                      <li key={candidate.kind}>
                        <strong>
                          {candidate.rank}. {candidate.kind === 'recommended'
                            ? text(APP_TEXT.recommended)
                            : candidate.kind === 'shape_focused'
                              ? text(APP_TEXT.shapeFocused)
                              : text(APP_TEXT.foldabilityFocused)}
                          {' — '}{candidate.total_score}/100
                        </strong>
                        <span className="muted">
                          {formattedText(APP_TEXT.shapeShapeFoldabilityFoldabilityStepsStepsPaperEfficiencyPaper, {
                            shape: candidate.shape_score,
                            foldability: candidate.foldability_score,
                            steps: candidate.step_count_score,
                            paper: candidate.paper_efficiency_score,
                          })}
                        </span>
                        <span className="muted">{formattedText(APP_TEXT.weightedContributionsShapeShapeFoldabilityFoldabilityStepsStepsPaperEffi, {
                          shape: Math.round(candidate.shape_score
                            * nativeSnapshot.beginner_design_profile.shape_fidelity_weight) / 100,
                          foldability: Math.round(candidate.foldability_score
                            * nativeSnapshot.beginner_design_profile.foldability_weight) / 100,
                          steps: Math.round(candidate.step_count_score
                            * nativeSnapshot.beginner_design_profile.step_count_weight) / 100,
                          paper: Math.round(candidate.paper_efficiency_score
                            * nativeSnapshot.beginner_design_profile.paper_efficiency_weight) / 100,
                        })}</span>
                        <span className="muted">
                          {formattedText(APP_TEXT.targetShapeApproximationTarget100, {
                            target: candidate.target_approximation_score,
                          })}
                        </span>
                      </li>
                    ))}
                  </ol>
                  {beginnerCandidates.requested_candidate_count < 3 && (
                    <button
                      type="button"
                      onClick={() => requestBeginnerCandidates(
                        beginnerCandidates.requested_candidate_count + 1,
                      )}
                      disabled={beginnerCandidateBusy}
                      aria-label={text(APP_TEXT.generateOneAdditionalCandidate)}
                    >
                      {text(APP_TEXT.generateAndCompareAnotherCandidate)}
                    </button>
                  )}
                  {beginnerCandidates.generation_status === 'ready' ? (
                    <div aria-label={text(APP_TEXT.generatedCreasePatternAndInstructionCandidates)}>
                      {beginnerCandidates.multi_reference_fusion && (
                        <p role={beginnerCandidates.multi_reference_fusion.apply_allowed ? 'status' : 'alert'}>
                          {formattedText(APP_TEXT.image3DAgreementAgreement100ExtentErrorError100Result, {
                            agreement: beginnerCandidates.multi_reference_fusion.agreement_score,
                            error: beginnerCandidates.multi_reference_fusion.normalized_extent_error,
                            result: beginnerCandidates.multi_reference_fusion.apply_allowed
                              ? text(APP_TEXT.theBoundedTwoSourceComparisonAgrees)
                              : text(APP_TEXT.imageAndGLBDisagreeCandidateApplyIsBlocked),
                          })}
                        </p>
                      )}
                      {beginnerCandidates.reference_consensus_analysis && (
                        <div aria-label={text(APP_TEXT.referenceConsensus)} role={beginnerCandidates.reference_consensus_analysis.apply_allowed ? 'status' : 'alert'}>
                          <p>{formattedText(APP_TEXT.referenceConsensusScore100PairsPairComparisonsDisagreementsDisagreements, { score: beginnerCandidates.reference_consensus_analysis.agreement_score,
                            pairs: beginnerCandidates.reference_consensus_analysis.pair_count,
                            disagreements: beginnerCandidates.reference_consensus_analysis.disagreement_count })}</p>
                          <table aria-label={text(APP_TEXT.componentAwareReferenceComparisons)}>
                            <thead><tr><th scope="col">{text(APP_TEXT.references)}</th><th scope="col">{text(APP_TEXT.components)}</th><th scope="col">{text(APP_TEXT.extent)}</th><th scope="col">{text(APP_TEXT.branches)}</th><th scope="col">{text(APP_TEXT.result)}</th></tr></thead>
                            <tbody>{beginnerCandidates.reference_consensus_analysis.pairs.slice(0, 6).map((pair) => {
                              const bindings = nativeSnapshot.beginner_design_profile.reference_consensus_v1?.bindings ?? []
                              const left = bindings.findIndex((binding) => binding.asset_id === pair.left_asset_id) + 1
                              const right = bindings.findIndex((binding) => binding.asset_id === pair.right_asset_id) + 1
                              const key = `${pair.left_asset_id}:${pair.right_asset_id}`
                              const reason = pair.disagrees
                                ? [pair.component_error > 1 ? text(APP_TEXT.componentMismatch) : '', pair.normalized_extent_error > 20 ? text(APP_TEXT.extentMismatch) : '', pair.branch_error > 2 ? text(APP_TEXT.branchMismatch) : ''].filter(Boolean).join(', ')
                                : text(APP_TEXT.withinAllThresholds)
                              return <tr key={key} aria-selected={selectedConsensusPair === key}>
                                <th scope="row"><button type="button" aria-pressed={selectedConsensusPair === key}
                                  onClick={() => setSelectedConsensusPair(selectedConsensusPair === key ? null : key)}>
                                  {formattedText(APP_TEXT.referenceLeftReferenceRight, { left, right })}</button></th>
                                <td>{`${pair.left_component_count} / ${pair.right_component_count} (error ${pair.component_error})`}</td>
                                <td>{`${pair.left_normalized_extents.join('×')} / ${pair.right_normalized_extents.join('×')} (error ${pair.normalized_extent_error})`}</td>
                                <td>{`${pair.left_branch_count} / ${pair.right_branch_count} (error ${pair.branch_error})`}</td>
                                <td>{`${pair.agreement_score}/100 — ${reason}`}</td>
                              </tr>
                            })}</tbody>
                          </table>
                          {selectedConsensusPair && (() => {
                            const pair = beginnerCandidates.reference_consensus_analysis?.pairs.find((candidate) => `${candidate.left_asset_id}:${candidate.right_asset_id}` === selectedConsensusPair)
                            return pair ? <p role="status" aria-live="polite">{formattedText(APP_TEXT.readOnlyComponentHighlightALeftExtentLeftBranchesBranchesBRightExtent, { leftExtent: pair.left_normalized_extents.join('×'), leftBranches: pair.left_branch_count,
                              rightExtent: pair.right_normalized_extents.join('×'), rightBranches: pair.right_branch_count })}</p> : null
                          })()}
                          {nativeSnapshot.beginner_design_profile.reference_consensus_v1?.excluded_asset_id && <p role="status">{text(APP_TEXT.oneExplicitlyExcludedReferenceIsOmittedFromThisTable)}</p>}
                          {nativeSnapshot.beginner_design_profile.reference_consensus_v1 && (
                            <fieldset><legend>{text(APP_TEXT.excludeOneOutlier)}</legend>
                              {nativeSnapshot.beginner_design_profile.reference_consensus_v1.bindings.map((binding, index) => (
                                <button type="button" key={binding.asset_id}
                                  disabled={nativeSnapshot.beginner_design_profile.reference_consensus_v1?.excluded_asset_id === binding.asset_id}
                                  onClick={() => excludeBeginnerConsensusAsset(binding.asset_id)}>
                                  {formattedText(APP_TEXT.excludeReferenceIndex, { index: index + 1 })}
                                </button>
                              ))}
                              {nativeSnapshot.beginner_design_profile.reference_consensus_v1.excluded_asset_id && (
                                <button type="button" onClick={() => excludeBeginnerConsensusAsset(null)}>{text(APP_TEXT.includeAllReferences)}</button>
                              )}
                            </fieldset>
                          )}
                        </div>
                      )}
                      {beginnerCandidates.generated_plans.map((plan, index) => {
                        const vertexById = new Map(
                          plan.crease_pattern.vertices.map((vertex) => [vertex.id, vertex]),
                        )
                        const xValues = plan.crease_pattern.vertices.map((vertex) => vertex.position.x)
                        const yValues = plan.crease_pattern.vertices.map((vertex) => vertex.position.y)
                        const minX = Math.min(...xValues)
                        const minY = Math.min(...yValues)
                        const width = Math.max(Math.max(...xValues) - minX, 1)
                        const height = Math.max(Math.max(...yValues) - minY, 1)
                        const applicableKind = (
                          plan.kind === 'diagonal_fold'
                          || isBeginnerApplicableTemplate(plan.kind)
                        ) ? plan.kind : null
                        const assessment = beginnerCandidates.plan_assessments[index]
                        const assessmentReason = assessment?.reason === 'geometry_invalid'
                          ? text(APP_TEXT.geometryValidationFailed)
                          : assessment?.reason === 'global_flat_foldability_proven'
                            ? text(APP_TEXT.globalFlatFoldabilityIsProven)
                            : assessment?.reason === 'global_flat_foldability_impossible'
                              ? text(APP_TEXT.globalFlatFoldabilityIsProvenImpossible)
                              : assessment?.reason === 'global_resource_limit'
                                ? text(APP_TEXT.globalValidationIsIndeterminateBecauseItsResourceLimitWasReached)
                                : assessment?.reason === 'global_timeout' || assessment?.reason === 'deadline_exceeded'
                                  ? text(APP_TEXT.globalValidationIsIndeterminateBecauseItsTimeLimitWasReached)
                                : assessment?.reason === 'global_indeterminate'
                                  ? text(APP_TEXT.globalFlatFoldabilityValidationWasIndeterminate)
                          : assessment?.reason === 'necessary_conditions_violated'
                            ? text(APP_TEXT.localFlatFoldabilityNecessaryConditionsAreViolated)
                            : assessment?.reason === 'local_analysis_blocked'
                              ? text(APP_TEXT.localFlatFoldabilityAnalysisWasBlocked)
                              : assessment?.reason === 'necessary_conditions_satisfied'
                                ? text(APP_TEXT.localFlatFoldabilityNecessaryConditionsAreSatisfied)
                                : text(APP_TEXT.localFlatFoldabilityIsIndeterminateForThisCandidate)
                        return (
                          <article key={plan.kind}>
                            <h4>
                              {text(APP_TEXT.candidate)} {index + 1}
                              {' — '}
                              {beginnerCandidates.candidates[index]?.total_score ?? 0}/100
                            </h4>
                            <svg
                              viewBox={`${minX - 1} ${minY - 1} ${width + 2} ${height + 2}`}
                              role="img"
                              aria-label={text(APP_TEXT.candidateCreasePatternPreview)}
                            >
                              {plan.crease_pattern.edges.map((edge) => {
                                const start = vertexById.get(edge.start)!
                                const end = vertexById.get(edge.end)!
                                return (
                                  <line
                                    key={edge.id}
                                    x1={start.position.x}
                                    y1={start.position.y}
                                    x2={end.position.x}
                                    y2={end.position.y}
                                    stroke="currentColor"
                                    strokeWidth={Math.max(width, height) / 50}
                                    strokeDasharray={edge.kind === 'mountain' ? '4 2' : undefined}
                                  />
                                )
                              })}
                            </svg>
                            <ol aria-label={text(APP_TEXT.candidateFoldingInstructions)}>
                              {plan.instruction_codes.map((code) => (
                                <li key={code}>
                                  {code === 'symmetric_four_leg_base'
                                    ? text(APP_TEXT.createTheSymmetricFourLegBaseFromTheSharedCenter)
                                    : code === 'symmetric_wing_base'
                                      ? text(APP_TEXT.createTheBilateralWingBaseFromTheSharedCenter)
                                      : code === 'symmetric_bird_base'
                                        ? text(APP_TEXT.createTheBilateralBirdWingBase)
                                        : code === 'asymmetric_bird_landmark_base'
                                          ? text(APP_TEXT.createTheAsymmetricBirdBaseBoundToIndividualLandmarks)
                                          : code === 'asymmetric_four_leg_landmark_base'
                                            ? text(APP_TEXT.createTheAsymmetricFourLegBaseBoundToFourIndividual)
                                          : code === 'asymmetric_insect_landmark_base'
                                            ? text(APP_TEXT.bindTenOrderedInsectLandmarksToTheCertifiedFourRay)
                                          : code === 'asymmetric_fish_landmark_base'
                                            ? text(APP_TEXT.bindTheHeadTailAndLeftRightFinsToThe)
                                        : code === 'symmetric_fish_base'
                                          ? text(APP_TEXT.createTheBilateralFishFinBase)
                                          : code === 'symmetric_ear_base'
                                            ? text(APP_TEXT.createTheBilateralLongEarBase)
                                            : code === 'symmetric_horn_base'
                                              ? text(APP_TEXT.createTheBilateralHornBase)
                                              : code === 'symmetric_antenna_base'
                                                ? text(APP_TEXT.createTheBilateralInsectAntennaBase)
                                                : code === 'symmetric_six_leg_base'
                                                  ? (locale === 'ja' ? '左右対称の完全六脚ベース' : 'Symmetric complete six-leg base')
                                                : code === 'center_axis_tail_base'
                                                  ? (locale === 'ja' ? '中心軸から伸びる尾のベース' : 'Center-axis tail base')
                                                : code === 'center_axis_horn_base'
                                                  ? (locale === 'ja' ? '中心軸から伸びる一本角のベース' : 'Center-axis single-horn base')
                                                : code === 'center_axis_antenna_base'
                                                  ? (locale === 'ja' ? '中心軸から伸びる一本触角のベース' : 'Center-axis single-antenna base')
                                                : code === 'composite_tail_ear_base'
                                                  ? (locale === 'ja' ? '単一尾と左右一組の耳の複合ベース' : 'Composite tail and ear base')
                                                : code === 'composite_horn_ear_base'
                                                  ? (locale === 'ja' ? '一本角と左右一組の耳の複合ベース' : 'Composite horn and ear base')
                                                : code === 'composite_horn_tail_base'
                                                  ? (locale === 'ja' ? '一本角と単一尾の複合ベース' : 'Composite horn and tail base')
                                                : code === 'composite_horn_tail_ear_base'
                                                  ? (locale === 'ja' ? '一本角・単一尾・左右一組の耳の複合ベース' : 'Composite horn, tail, and ear base')
                                                : code === 'composite_wing_antenna_base'
                                                  ? (locale === 'ja' ? '左右一組の翅と触角の複合ベース' : 'Composite wing and antenna base')
                                                : code === 'composite_complete_insect_base'
                                                  ? (locale === 'ja' ? '翅・触角・六脚の完全複合昆虫ベース' : 'Complete composite insect base')
                                                : code === 'composite_complete_animal_base'
                                                  ? (locale === 'ja' ? '角・尾・耳・四脚の完全複合動物ベース' : 'Complete composite animal base')
                                                : code === 'composite_complete_winged_animal_base'
                                                  ? (locale === 'ja' ? '角・尾・耳・四脚・翼の完全複合動物ベース' : 'Complete composite winged animal base')
                                                : code === 'composite_generic_target_base'
                                                  ? (locale === 'ja' ? '認識部位から作る上限付き汎用複合ベース' : 'Bounded composite base from recognized parts')
                                                : code === 'symmetric_insect_leg_pair_base'
                                                  ? text(APP_TEXT.createOneBilateralInsectLegPairBase)
                                          : code === 'book_fold_vertical'
                                    ? text(APP_TEXT.foldInHalfOnTheVerticalCenterLine)
                                    : code === 'book_fold_horizontal'
                                      ? text(APP_TEXT.foldInHalfOnTheHorizontalCenterLine)
                                      : text(APP_TEXT.foldOnTheDiagonal)}
                                </li>
                              ))}
                            </ol>
                            <p aria-label={text(APP_TEXT.targetPartsUsedByThisCandidate)}>
                              {plan.target_parts.map((part) => {
                                const label = {
                                  head: APP_TEXT.head,
                                  torso: APP_TEXT.torso,
                                  leg: APP_TEXT.leg,
                                  horn: APP_TEXT.horn,
                                  ear: APP_TEXT.ear,
                                  wing: APP_TEXT.wing,
                                  fin: APP_TEXT.fin,
                                  antenna: APP_TEXT.antenna,
                                  tail: APP_TEXT.tail,
                                }[part.kind]
                                return `${text(label)} × ${part.count}`
                              }).join(' · ')}
                            </p>
                            {(plan.kind === 'composite_complete_animal_base'
                              || plan.kind === 'composite_complete_winged_animal_base') && (
                              <CompleteAnimalBindingList locale={locale}
                                protrusions={nativeSnapshot.beginner_design_profile.generation_constraints.protrusions ?? []} />
                            )}
                            {plan.kind === 'composite_complete_insect_base' && (
                              <CompleteInsectBindingList locale={locale}
                                protrusions={nativeSnapshot.beginner_design_profile.generation_constraints.protrusions ?? []} />
                            )}
                            {plan.kind === 'composite_generic_target_base' && (
                              <GenericTargetBindingList locale={locale}
                                protrusions={nativeSnapshot.beginner_design_profile.generation_constraints.protrusions ?? []} />
                            )}
                            {plan.skeleton_segments.length > 0 && (
                              <svg viewBox="-110 -110 220 220" role="img"
                                aria-label={text(APP_TEXT.stickSkeletonUsedByThisCandidate)}>
                                {plan.skeleton_segments.map((segment) => (
                                  <line
                                    key={segment.id}
                                    x1={segment.start.x_tenths_mm / 10}
                                    y1={segment.start.y_tenths_mm / 10}
                                    x2={segment.end.x_tenths_mm / 10}
                                    y2={segment.end.y_tenths_mm / 10}
                                    stroke="currentColor"
                                    strokeWidth={Math.max(0.5, segment.thickness_tenths_mm / 10)}
                                  />
                                ))}
                              </svg>
                            )}
                            {plan.target_asset && (
                              <p role="note">
                                {text(APP_TEXT.thisCandidateUsesTheSelectedProjectReferenceImageAsTarget)}
                              </p>
                            )}
                            <p className="muted">
                              {text(APP_TEXT.thisIsAReadOnlyCandidateItDoesNotBecome)}
                            </p>
                            <p
                              role={assessment?.apply_allowed === false ? 'alert' : 'status'}
                              aria-label={text(APP_TEXT.candidateValidationResult)}
                            >
                              {assessment?.proof_scope === 'sufficient'
                                ? text(APP_TEXT.sufficientProof)
                                : assessment?.proof_scope === 'necessary'
                                  ? text(APP_TEXT.necessaryConditionValidation)
                                  : text(APP_TEXT.indeterminate)}
                              {': '}{assessmentReason}
                              {assessment?.proof_scope === 'indeterminate' && ` ${text(APP_TEXT.warningApplyingItDoesNotGuaranteeFlatFoldability)}`}
                            </p>
                            {assessment?.shape_approximation_score !== null
                              && assessment?.shape_approximation_score !== undefined && (
                              <p className="muted">
                                {formattedText(APP_TEXT.readOnlyShapeApproximationToReferenceGLBScore100, { score: assessment.shape_approximation_score })}
                                {' '}{assessment.shape_difference_reason === 'certified_flat_surface_v1'
                                  ? text(APP_TEXT.usesActualBboxAreaAndPrincipalAxisFromTheCertified)
                                  : text(APP_TEXT.differenceTheCreaseCandidateHasNoSurfaceMeshSoOnly)}
                              </p>
                            )}
                            {assessment?.component_shape_comparison && (
                              <p className="muted" aria-label={text(APP_TEXT.componentAwareShapeScoreBreakdown)}>
                                {`Components ${assessment.component_shape_comparison.component_count}; `}
                                {`extent ${assessment.component_shape_comparison.extent_score}/100 × 45%; `}
                                {`branches ${assessment.component_shape_comparison.branch_score}/100 × 35%; `}
                                {`bridges ${assessment.component_shape_comparison.bridge_score}/100 × 20%; `}
                                {`matched ${assessment.component_shape_comparison.matched_branch_count}; `}
                                {`bounded work ${assessment.component_shape_comparison.work_units}/64.`}
                              </p>
                            )}
                            {applicableKind && (
                              <button
                                type="button"
                                onClick={() => confirmAndApplyBeginnerPlan(
                                  applicableKind,
                                  plan.crease_pattern.edges[0].id,
                                )}
                                disabled={coreBusy || recoveryBlocking || beginnerCandidateBusy
                                  || !assessment || !assessment.apply_allowed}
                                aria-label={text(APP_TEXT.reviewAndApplyThisBoundedGeneratedCandidate)}
                              >
                                {text(APP_TEXT.reviewAndApplyThisCandidate)}
                              </button>
                            )}
                          </article>
                        )
                      })}
                    </div>
                  ) : (
                    <p role="status">
                      {beginnerCandidates.generation_status === 'missing_target_category'
                        ? text(APP_TEXT.saveAnAnimalOrInsectTargetCategoryFirst)
                        : beginnerCandidates.generation_status === 'missing_required_parts'
                          ? text(APP_TEXT.saveOneHeadAndOneTorsoAsRequiredTargetParts)
                          : beginnerCandidates.generation_status === 'unsupported_animal_template'
                            ? text(APP_TEXT.theAnimalTemplateRequiresOneHeadOneTorsoFourLegs)
                            : beginnerCandidates.generation_status === 'unsupported_insect_template'
                              ? text(APP_TEXT.theInsectTemplateRequiresOneHeadOneTorsoTwoWings)
                              : beginnerCandidates.generation_status === 'missing_target_asset'
                            ? text(APP_TEXT.theReferenceImageWasRemovedOrChangedSelectAnotherUnderlay)
                        : beginnerCandidates.generation_status === 'unsupported_techniques'
                        ? text(APP_TEXT.allowValleyOrMountainFoldsToGeneratePlans)
                        : beginnerCandidates.generation_status === 'resource_limit'
                          ? text(APP_TEXT.theInputExceedsTheGenerationWorkLimit)
                          : text(APP_TEXT.theInitialGeneratorSupportsRectangularSingleSheetPaperOnly)}
                    </p>
                  )}
                  </>
                )}

    </>
  )
}
