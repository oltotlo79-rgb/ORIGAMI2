import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export const STACKED_FOLD_APPLY_CONTRACT_VERSION_V1 = 1 as const

export type StackedFoldApplyModeV1 =
  | 'none'
  | 'certified'
  | 'speculative_unproven'

type FailureClass =
  | 'continuous_path_uncertified'
  | 'target_layer_order_unavailable'

type ApplyContractInput = Readonly<{
  transaction: Readonly<Record<string, unknown>>
  endpointCollision: Readonly<Record<string, unknown>>
  continuousPath: Readonly<Record<string, unknown>>
  flatEndpointLayerOrder: Readonly<Record<string, unknown>>
  certifiedPathGraph: unknown
}>

export function hasValidStackedFoldApplyContractV1(
  input: ApplyContractInput,
): boolean {
  const {
    transaction,
    endpointCollision,
    continuousPath,
    flatEndpointLayerOrder,
  } = input
  if (
    transaction.applyContractVersion !== STACKED_FOLD_APPLY_CONTRACT_VERSION_V1
    || (
      transaction.applyMode !== 'none'
      && transaction.applyMode !== 'certified'
      && transaction.applyMode !== 'speculative_unproven'
    )
    || typeof transaction.speculativeUnprovenAvailable !== 'boolean'
    || typeof transaction.readyForAtomicApply !== 'boolean'
    || typeof transaction.authorizesProjectMutation !== 'boolean'
    || continuousPath.authorizesProjectMutation !== false
    || !Array.isArray(transaction.failureClasses)
  ) return false

  const expectedFailures = expectedFailureClasses(
    continuousPath.continuousClearanceCertified,
    flatEndpointLayerOrder.certified,
  )
  if (
    expectedFailures === null
    || transaction.failureClasses.length !== expectedFailures.length
    || transaction.failureClasses.some(
      (failure, index) => failure !== expectedFailures[index],
    )
  ) return false

  const tokenIsValid = isCanonicalNonNilUuid(transaction.transactionToken)
  const continuousCertified =
    continuousPath.continuousClearanceCertified === true
    && typeof continuousPath.continuousCertificateModelId === 'string'
  const noBlockingObservation =
    endpointCollision.hasBlockingHold === false
    && endpointCollision.penetratingPairCount === 0
    && endpointCollision.indeterminatePairCount === 0
    && continuousPath.firstSampledBlockingAngleDegrees === null

  switch (transaction.applyMode) {
    case 'none':
      return transaction.transactionToken === null
        && transaction.speculativeUnprovenAvailable === false
        && transaction.readyForAtomicApply === false
        && transaction.authorizesProjectMutation === false
    case 'certified':
      return tokenIsValid
        && transaction.speculativeUnprovenAvailable === false
        && transaction.readyForAtomicApply === true
        && transaction.authorizesProjectMutation === true
        && expectedFailures.length === 0
        && continuousCertified
        && flatEndpointLayerOrder.certified === true
        && noBlockingObservation
    case 'speculative_unproven':
      return tokenIsValid
        && transaction.speculativeUnprovenAvailable === true
        && transaction.readyForAtomicApply === false
        && transaction.authorizesProjectMutation === false
        && expectedFailures.length === 1
        && expectedFailures[0] === 'continuous_path_uncertified'
        && continuousPath.continuousClearanceCertified === false
        && continuousPath.continuousCertificateModelId === null
        && Number.isSafeInteger(continuousPath.sampledPoseCount)
        && Number(continuousPath.sampledPoseCount) > 0
        && continuousPath.sampledNonblockingPoseCount
          === continuousPath.sampledPoseCount
        && flatEndpointLayerOrder.certified === true
        && noBlockingObservation
        && input.certifiedPathGraph === null
  }
}

function expectedFailureClasses(
  continuousCertified: unknown,
  layerOrderCertified: unknown,
): readonly FailureClass[] | null {
  if (
    typeof continuousCertified !== 'boolean'
    || typeof layerOrderCertified !== 'boolean'
  ) return null
  const result: FailureClass[] = []
  if (!continuousCertified) result.push('continuous_path_uncertified')
  if (!layerOrderCertified) result.push('target_layer_order_unavailable')
  return result
}
