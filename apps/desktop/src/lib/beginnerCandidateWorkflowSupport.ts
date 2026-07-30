import { listen } from '@tauri-apps/api/event'

import {
  applyBeginnerGeneratedPlan,
  applyBeginnerSymmetricParameters,
  appendGenericTreeInstructionProposal,
  beginnerGeneratedPlanAssessmentAllowsApplyV1,
  cancelReferenceConsensus,
  evaluateBeginnerCandidates,
  getBeginnerSymmetricParameterEstimate,
  updateBeginnerDesignProfile,
  updateBeginnerReferenceConsensus,
  type BeginnerCandidateResponseV1,
  type ProjectSnapshot,
} from './coreClient.ts'
import { matchesBeginnerProjectBinding } from './beginnerWorkflowSupport.ts'

export type ConsensusSelection = Readonly<{
  kind: 'image' | 'reference_model'
  asset_id: string
}>

export type ConsensusProgress = Readonly<{
  processed_assets: number
  total_assets: number
  processed_pairs: number
  total_pairs: number
}>

export type ConsensusProgressListener = (
  payload: Readonly<Record<string, unknown>>,
) => void

export type BeginnerCandidateRequestStatus =
  | 'idle'
  | 'running'
  | 'ready'
  | 'empty'
  | 'cancelled'
  | 'failed'

export type CandidateTransport = Readonly<{
  evaluate: typeof evaluateBeginnerCandidates
  cancelConsensus: typeof cancelReferenceConsensus
  estimateSymmetric: typeof getBeginnerSymmetricParameterEstimate
  applySymmetric: typeof applyBeginnerSymmetricParameters
  applyPlan: typeof applyBeginnerGeneratedPlan
  appendInstructions: typeof appendGenericTreeInstructionProposal
  updateProfile: typeof updateBeginnerDesignProfile
  updateConsensus: typeof updateBeginnerReferenceConsensus
}>

export const EMPTY_CONSENSUS_PROGRESS: ConsensusProgress = Object.freeze({
  processed_assets: 0,
  total_assets: 0,
  processed_pairs: 0,
  total_pairs: 0,
})

export const DEFAULT_CANDIDATE_TRANSPORT: CandidateTransport = Object.freeze({
  evaluate: evaluateBeginnerCandidates,
  cancelConsensus: cancelReferenceConsensus,
  estimateSymmetric: getBeginnerSymmetricParameterEstimate,
  applySymmetric: applyBeginnerSymmetricParameters,
  applyPlan: applyBeginnerGeneratedPlan,
  appendInstructions: appendGenericTreeInstructionProposal,
  updateProfile: updateBeginnerDesignProfile,
  updateConsensus: updateBeginnerReferenceConsensus,
})

export function subscribeConsensusProgressByDefault(
  listener: ConsensusProgressListener,
) {
  return listen<Record<string, unknown>>(
    'reference-consensus-progress-v1',
    (event) => listener(event.payload),
  )
}

export function beginnerCandidatePlanApplyAuthorityIsLiveV1(
  response: BeginnerCandidateResponseV1 | null,
  authority: BeginnerCandidateResponseV1 | null,
  current: ProjectSnapshot | null,
  kind: Parameters<typeof applyBeginnerGeneratedPlan>[4],
  expectedCandidateEdgeId: string,
  blocked: boolean,
): boolean {
  if (
    !response
    || response !== authority
    || !current
    || blocked
    || !matchesBeginnerProjectBinding(response, current)
  ) return false
  const planIndex = response.generated_plans.findIndex(
    (candidate) => (
      candidate.kind === kind
      && candidate.crease_pattern.edges[0]?.id === expectedCandidateEdgeId
    ),
  )
  const assessment = response.plan_assessments[planIndex]
  return assessment !== undefined
    && beginnerGeneratedPlanAssessmentAllowsApplyV1(assessment)
}
