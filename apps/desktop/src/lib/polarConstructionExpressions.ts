import type { VertexCoordinateExpressionBinding } from './coreClient.ts'

/**
 * Identifies a legacy polar construction whose creator-runtime endpoint bits
 * remain authoritative because no deterministic replay model was recorded.
 */
export function isUnverifiedLegacyV1PolarConstructionBinding(
  binding: VertexCoordinateExpressionBinding | undefined,
): boolean {
  return binding?.schema_version === 1
    && binding.transcendental_model_id === undefined
    && binding.polar_construction !== undefined
}
