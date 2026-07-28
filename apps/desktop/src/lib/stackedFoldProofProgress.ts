import type { ProjectSnapshot } from './coreClient.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import {
  hasValidStackedFoldApplyContractV1,
} from './stackedFoldApplyContract.ts'
import type { StackedFoldReadResponse } from './stackedFoldRead.ts'
import {
  isSafeCount,
  type ProofProgressPanelModel,
  type ProofProgressState,
} from './proofProgressModel.ts'
import type {
  PostApplyProofSchedulerViewStateV1,
} from './postApplyProofSchedulerCoordinator.ts'
import type {
  PostApplyProofStatusV1,
} from './postApplyProofSchedulerClient.ts'
import {
  unprovenHistorySummaryFromSnapshotV1,
} from './speculativeUnprovenWire.ts'

export type StackedFoldProofProgressSource =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'reading' }>
  | Readonly<{ kind: 'ready'; response: StackedFoldReadResponse }>
  | Readonly<{ kind: 'failed'; reason: string }>
  | Readonly<{ kind: 'refresh_failed' }>

export function createStackedFoldProofProgressModel(
  source: StackedFoldProofProgressSource,
  snapshot: Pick<
    ProjectSnapshot,
    | 'project_instance_id'
    | 'project_id'
    | 'revision'
    | 'speculativeUnprovenFolds'
  >,
  postApplyProof: PostApplyProofSchedulerViewStateV1 = Object.freeze({
    kind: 'idle',
  }),
): ProofProgressPanelModel {
  let unprovenHistory = unprovenHistorySummaryFromSnapshotV1(snapshot)
  let status: ProofProgressState | null = null
  let provenPairCount = 0
  let totalPairCount: number | null = null
  let speculativeApplyAvailable = false
  let postApplyNotice: 'starting' | 'unavailable' | null = null
  let proofFailure: ProofProgressPanelModel['proofFailure'] = null

  if (source.kind === 'reading') {
    // This is the pre-Apply safety read, not a persisted post-Apply proof
    // worker. The `proving` state is reserved for a future native scheduler.
    status = null
  } else if (source.kind === 'failed') {
    status = progressStateForFailure(source.reason)
  } else if (source.kind === 'ready') {
    const response = source.response
    totalPairCount = isSafeCount(response.endpointCollision.expectedPairCount)
      ? response.endpointCollision.expectedPairCount
      : null
    if (!responseMatchesSnapshot(response, snapshot)) {
      status = 'stale'
    } else if (!responseHasValidApplyContract(response)) {
      status = 'evidence_insufficient'
    } else if (response.transactionProposal.applyMode === 'certified') {
      status = 'certified'
      provenPairCount = totalPairCount ?? 0
    } else if (
      response.transactionProposal.applyMode === 'speculative_unproven'
    ) {
      status = 'evidence_insufficient'
      speculativeApplyAvailable = true
    } else {
      status = response.endpointCollision.hasBlockingHold
        || response.continuousPath.firstSampledBlockingAngleDegrees !== null
        ? 'blocked'
        : 'evidence_insufficient'
    }
  }

  if (postApplyProof.kind === 'starting') {
    status = 'proving'
    provenPairCount = 0
    totalPairCount = null
    speculativeApplyAvailable = false
    postApplyNotice = 'starting'
  } else if (postApplyProof.kind === 'progress') {
    status = postApplyStatusToPanelStatus(postApplyProof.progress.status)
    provenPairCount = postApplyProof.progress.provenPairCount
    totalPairCount = postApplyProof.progress.totalPairCount
    speculativeApplyAvailable = false
    proofFailure = postApplyProof.progress.proofFailure
  } else if (postApplyProof.kind === 'unavailable') {
    status = 'evidence_insufficient'
    provenPairCount = 0
    totalPairCount = null
    speculativeApplyAvailable = false
    postApplyNotice = 'unavailable'
  }
  if (
    postApplyProof.kind !== 'idle'
    && unprovenHistory.kind === 'absent'
  ) {
    // An absent legacy field is not evidence that the just-applied unproven
    // history count is zero.
    unprovenHistory = Object.freeze({ kind: 'unavailable' })
  }

  return Object.freeze({
    status,
    provenPairCount,
    totalPairCount,
    unprovenHistory,
    speculativeApplyAvailable,
    postApplyNotice,
    proofFailure,
  })
}

function postApplyStatusToPanelStatus(
  status: PostApplyProofStatusV1,
): ProofProgressState {
  switch (status) {
    case 'proving':
    case 'certified':
    case 'blocked':
    case 'stale':
      return status
    case 'unknown_evidence_insufficient':
      return 'evidence_insufficient'
    case 'unknown_resource_limit':
      return 'resource_limit'
    case 'unknown_cancelled':
      return 'cancelled'
    case 'unknown_deadline_reached':
      return 'deadline'
    default:
      return 'evidence_insufficient'
  }
}

export function shouldRenderStackedFoldProofProgress(
  model: ProofProgressPanelModel,
): boolean {
  return model.status !== null
    || model.unprovenHistory.kind === 'unavailable'
    || (
      model.unprovenHistory.kind === 'known'
      && (
        model.unprovenHistory.appliedTotal > 0
        || model.unprovenHistory.unappliedRedoTotal > 0
      )
    )
}

export function speculativeStackedFoldApplyIsCurrent(
  response: StackedFoldReadResponse,
  snapshot: Pick<
    ProjectSnapshot,
    'project_instance_id' | 'project_id' | 'revision'
  >,
  activeToken: string | null,
): boolean {
  return responseMatchesSnapshot(response, snapshot)
    && responseHasValidApplyContract(response)
    && response.transactionProposal.applyMode === 'speculative_unproven'
    && isCanonicalNonNilUuid(activeToken)
    && response.transactionProposal.transactionToken === activeToken
}

function responseMatchesSnapshot(
  response: StackedFoldReadResponse,
  snapshot: Pick<
    ProjectSnapshot,
    'project_instance_id' | 'project_id' | 'revision'
  >,
): boolean {
  return response.binding.projectInstanceId === snapshot.project_instance_id
    && response.binding.projectId === snapshot.project_id
    && response.binding.sourceRevision === snapshot.revision
    && response.transactionProposal.sourceProjectId === snapshot.project_id
    && response.transactionProposal.sourceRevision === snapshot.revision
}

function responseHasValidApplyContract(
  response: StackedFoldReadResponse,
): boolean {
  return hasValidStackedFoldApplyContractV1({
    transaction: response.transactionProposal as unknown as Record<string, unknown>,
    endpointCollision:
      response.endpointCollision as unknown as Record<string, unknown>,
    continuousPath: response.continuousPath as unknown as Record<string, unknown>,
    flatEndpointLayerOrder:
      response.flatEndpointLayerOrder as unknown as Record<string, unknown>,
    certifiedPathGraph: response.certifiedPathGraph,
  })
}

function progressStateForFailure(reason: string): ProofProgressState {
  switch (reason) {
    case 'stale':
      return 'stale'
    case 'cycle_path_resource_limit':
      return 'resource_limit'
    case 'cycle_path_cancelled':
      return 'cancelled'
    case 'cycle_nonclosing':
    case 'cycle_path_collision':
      return 'blocked'
    default:
      return 'evidence_insufficient'
  }
}
