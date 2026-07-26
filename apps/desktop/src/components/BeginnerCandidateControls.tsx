import { BeginnerGridProgressStatus } from './BeginnerGridProgressStatus'
import { APP_TEXT } from '../lib/appText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
  type MessageVariables,
} from '../lib/i18n.ts'
import type { useBeginnerCandidateWorkflow } from '../lib/useBeginnerCandidateWorkflow.ts'
import type { useBeginnerParameterGridWorkflow } from '../lib/useBeginnerParameterGridWorkflow.ts'

type CandidateWorkflow = ReturnType<typeof useBeginnerCandidateWorkflow>
type GridWorkflow = ReturnType<typeof useBeginnerParameterGridWorkflow>

type BeginnerCandidateControlsProps = Readonly<{
  locale: Locale
  coreBusy: boolean
  recoveryBlocking: boolean
  skeletonTreeStatus: string
  candidateWorkflow: CandidateWorkflow
  gridWorkflow: GridWorkflow
}>

export function BeginnerCandidateControls({
  locale,
  coreBusy,
  recoveryBlocking,
  skeletonTreeStatus,
  candidateWorkflow,
  gridWorkflow,
}: BeginnerCandidateControlsProps) {
  const text = (localized: LocalizedText) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: LocalizedText,
    variables?: MessageVariables,
  ) => formatLocalizedText(locale, localized, variables)
  const {
    beginnerCandidateBusy,
    consensusProgress,
    beginnerSymmetricEstimate,
    beginnerSymmetricScale,
    setBeginnerSymmetricScale,
    beginnerSymmetricSpacing,
    setBeginnerSymmetricSpacing,
    requestBeginnerCandidates,
    cancelConsensusAnalysis,
    requestBeginnerSymmetricEstimate,
    confirmBeginnerSymmetricEstimate,
  } = candidateWorkflow
  const {
    beginnerGrid,
    beginnerGridSelectedPointId,
    setBeginnerGridSelectedPointId,
    beginnerGridBusy,
    beginnerGridProgress,
    beginnerGridButtonRef,
    requestBeginnerGrid,
    cancelBeginnerGrid,
    confirmAndApplyBeginnerGridCandidate,
  } = gridWorkflow

  return (
    <>
                <h3 id="beginner-candidate-heading">
                  {text(APP_TEXT.compareDesignCandidates)}
                </h3>
                <p id="beginner-candidate-description" className="muted">
                  {text(APP_TEXT.scoresUpToThreeCandidatesOnThisDeviceUsingThe)}
                </p>
                <button type="button" onClick={requestBeginnerSymmetricEstimate}>
                  {text(APP_TEXT.estimateSymmetricParameters)}
                </button>
                {beginnerSymmetricEstimate && (
                  <fieldset>
                    <legend>{text(APP_TEXT.adjustReadOnlyEstimate)}</legend>
                    <p>{formattedText(APP_TEXT.countCountScaleScaleSpacingSpacing, { count: beginnerSymmetricEstimate.estimate.protrusion_count,
                      scale: beginnerSymmetricEstimate.estimate.scale_percent,
                      spacing: beginnerSymmetricEstimate.estimate.spacing_percent })}</p>
                    <ol>
                      {beginnerSymmetricEstimate.candidates.map((candidate) => (
                        <li key={candidate.id}>
                          {formattedText(APP_TEXT.scaleScaleSpacingSpacingApproximationScoreComplexityComplexityRequiredCo, { scale: candidate.scale_percent, spacing: candidate.spacing_percent,
                            score: candidate.approximation_score, complexity: candidate.complexity_score,
                            count: candidate.required_protrusion_count })}
                          <button type="button" onClick={() => {
                            setBeginnerSymmetricScale(candidate.scale_percent)
                            setBeginnerSymmetricSpacing(candidate.spacing_percent)
                          }}>
                            {text(APP_TEXT.selectThisCandidate)}
                          </button>
                        </li>
                      ))}
                    </ol>
                    <label>{text(APP_TEXT.scale1045)}
                      <input type="number" min="10" max="45" value={beginnerSymmetricScale}
                        onChange={(event) => setBeginnerSymmetricScale(Number(event.currentTarget.value))} />
                    </label>
                    <label>{text(APP_TEXT.spacing2080)}
                      <input type="number" min="20" max="80" value={beginnerSymmetricSpacing}
                        onChange={(event) => setBeginnerSymmetricSpacing(Number(event.currentTarget.value))} />
                    </label>
                    <button type="button" onClick={confirmBeginnerSymmetricEstimate}>
                      {text(APP_TEXT.confirmDesignParameters)}
                    </button>
                  </fieldset>
                )}
                <button
                  type="button"
                  onClick={() => requestBeginnerCandidates(1)}
                  disabled={coreBusy || recoveryBlocking || beginnerCandidateBusy}
                  aria-describedby="beginner-candidate-description"
                >
                  {beginnerCandidateBusy
                    ? text(APP_TEXT.scoringCandidates)
                    : text(APP_TEXT.scoreCandidates)}
                </button>
                {beginnerCandidateBusy && <div role="status" aria-live="polite">
                  {`Consensus progress: assets ${consensusProgress.processed_assets}/${consensusProgress.total_assets}; pairs ${consensusProgress.processed_pairs}/${consensusProgress.total_pairs}.`}
                  <button type="button" onClick={cancelConsensusAnalysis}>Cancel consensus analysis</button>
                </div>}
                <button ref={beginnerGridButtonRef} type="button" onClick={requestBeginnerGrid}
                  disabled={coreBusy || recoveryBlocking || beginnerGridBusy
                    || skeletonTreeStatus !== 'tree'}>
                  {beginnerGridBusy
                    ? text(APP_TEXT.evaluating27Designs)
                    : text(APP_TEXT.evaluateTop3Of27Designs)}
                </button>
                <BeginnerGridProgressStatus locale={locale} busy={beginnerGridBusy}
                  enumerated={beginnerGridProgress.enumerated}
                  checked={beginnerGridProgress.globalChecked} refined={beginnerGridProgress.refined}
                  onCancel={cancelBeginnerGrid} />
                {beginnerGrid && (
                  <section aria-label={text(APP_TEXT.top3FromThe27DesignSearch)}>
                    <p className="muted">{formattedText(APP_TEXT.countDesignsEvaluatedGridHashHash, { count: beginnerGrid.evaluated_grid_points,
                      hash: beginnerGrid.grid_hash.slice(0, 6).map((byte) => byte.toString(16).padStart(2, '0')).join('') })}</p>
                    <table aria-label={text(APP_TEXT.strictCandidateAuthorityComparison)}>
                      <thead><tr>
                        <th>{text(APP_TEXT.select)}</th>
                        <th>{text(APP_TEXT.creases)}</th>
                        <th>{text(APP_TEXT.steps)}</th>
                        <th>{text(APP_TEXT.localProof)}</th>
                        <th>{text(APP_TEXT.globalProof)}</th>
                        <th>{text(APP_TEXT.pathProof)}</th>
                        <th>{text(APP_TEXT.text3dShape)}</th>
                        <th>{text(APP_TEXT.paperEfficiency)}</th>
                      </tr></thead>
                      <tbody>{beginnerGrid.candidates.map((candidate) => <tr key={candidate.point.id}>
                        <td><input type="radio" name="beginner-grid-authority"
                          aria-label={formattedText(APP_TEXT.selectExactCandidateId, { id: candidate.point.id + 1 })}
                          checked={beginnerGridSelectedPointId === candidate.point.id}
                          onChange={() => setBeginnerGridSelectedPointId(candidate.point.id)} /></td>
                        <td>{candidate.plan.crease_pattern.edges.length}</td>
                        <td>{candidate.plan.instruction_codes.length}</td>
                        <td>{candidate.local_proof_scope}</td>
                        <td>{candidate.global_proof_scope}</td>
                        <td>{candidate.assessment.proof_scope === 'sufficient'
                          ? text(APP_TEXT.certifiedOnApply)
                          : text(APP_TEXT.blocked)}</td>
                        <td>{candidate.assessment.shape_approximation_score
                          ?? text(APP_TEXT.notMeasured)}</td>
                        <td>{candidate.paper_efficiency_score}/100</td>
                      </tr>)}</tbody>
                    </table>
                    <button type="button" disabled={beginnerGridSelectedPointId === null
                      || !beginnerGrid.candidates.some((candidate) => candidate.point.id === beginnerGridSelectedPointId
                        && candidate.assessment.proof_scope === 'sufficient'
                        && candidate.assessment.apply_allowed)}
                      onClick={() => {
                        const selected = beginnerGrid.candidates.find(
                          (candidate) => candidate.point.id === beginnerGridSelectedPointId)
                        if (selected) confirmAndApplyBeginnerGridCandidate(selected)
                      }}>
                      {text(APP_TEXT.revalidateAndApplySelectedCandidate)}
                    </button>
                    <ol>{beginnerGrid.candidates.map((candidate) => (
                      <li key={candidate.point.id}>
                        <strong>{formattedText(APP_TEXT.designIdPrimaryScoreScore1000, { id: candidate.point.id + 1, score: candidate.primary_score })}</strong>
                        <span className="muted">{formattedText(APP_TEXT.strictLocalImprovementsImprovementsIterationsFromStartsStarts, { improvements: candidate.strict_improvements,
                          iterations: candidate.refinement_iterations,
                          starts: candidate.refinement_starts })}</span>
                        <span className="muted">{formattedText(APP_TEXT.scaleScaleSpacingSpacingDetailDetail, { scale: candidate.point.scale_percent, spacing: candidate.point.spacing_percent,
                          detail: candidate.point.detail_level })}</span>
                        <span className="muted">{formattedText(APP_TEXT.localLocalGlobalGlobalComplexityComplexity100, { local: candidate.local_proof_scope, global: candidate.global_proof_scope,
                          complexity: candidate.complexity_score })}</span>
                        <span className="muted">{formattedText(APP_TEXT.paperEfficiencyPaper100, { paper: candidate.paper_efficiency_score })}</span>
                        <span className="muted">{formattedText(APP_TEXT.penaltiesScaleScaleSpacingSpacingDetailDetail, { scale: candidate.scale_deviation_penalty,
                          spacing: candidate.spacing_deviation_penalty,
                          detail: candidate.detail_mismatch_penalty })}</span>
                        <span className="muted">{formattedText(APP_TEXT.outcomeReasonShapeDifferenceShape, { reason: candidate.outcome_reason,
                          shape: candidate.assessment.shape_difference_reason ?? 'none' })}</span>
                        <span className="muted">{formattedText(APP_TEXT.contourPlacementWitnessBodyBodyPointsLocalLocalVerticesVertices, {
                          body: candidate.contour_witness.body_contour_points,
                          local: candidate.contour_witness.local_bindings.length === 0
                            ? 'none'
                            : candidate.contour_witness.local_bindings
                              .map((binding) => `${binding.protrusion_id}:${binding.contour_points}@face${binding.generated_face_id}`)
                              .join(', '),
                          vertices: candidate.contour_witness.witnessed_vertices,
                          creases: candidate.contour_witness.witnessed_creases,
                          error: candidate.contour_witness.max_contour_error_millionths,
                        })}</span>
                        <span className="muted">{formattedText(APP_TEXT.genericFeatureTopologyWitnessFeatures, {
                          features: candidate.contour_witness.generic_feature_bindings.length === 0
                            ? 'none'
                            : candidate.contour_witness.generic_feature_bindings
                              .map((binding) => `${binding.protrusion_id}:${binding.endpoint_count}@feature${binding.generated_feature_id}`
                                + `→skeleton${binding.skeleton_segment_id}.${binding.skeleton_endpoint}`
                                + `#crease-${binding.crease_authority_sha256.slice(0, 4)
                                  .map((byte) => byte.toString(16).padStart(2, '0')).join('')}`)
                              .join(', '),
                        })}</span>
                        {candidate.contour_witness.skeleton_branch_bindings.length > 0 && (
                          <span className="muted">{formattedText(APP_TEXT.confirmedTreeSkeletonBranchesAuthorityDigest, {
                            branches: candidate.contour_witness.skeleton_branch_bindings
                              .map((branch) => `${branch.parent_segment_id ?? 'root'}→${branch.segment_id}`
                                + `[feature ${branch.generated_feature_ids.join(',') || 'none'}]`).join(', '),
                            digest: candidate.contour_witness.skeleton_tree_authority_sha256.slice(0, 4)
                              .map((byte) => byte.toString(16).padStart(2, '0')).join(''),
                          })}</span>
                        )}
                        {candidate.assessment.proof_scope === 'sufficient'
                          && candidate.assessment.reason === 'global_flat_foldability_proven'
                          && candidate.assessment.apply_allowed && (
                          <button type="button" onClick={() => confirmAndApplyBeginnerGridCandidate(candidate)}>
                            {text(APP_TEXT.revalidateAndApplyThisDesign)}
                          </button>
                        )}
                      </li>
                    ))}</ol>
                  </section>
                )}

    </>
  )
}
