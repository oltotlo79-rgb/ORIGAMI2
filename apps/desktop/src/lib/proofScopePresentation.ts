import type { AssignedLocalSufficiencySummaryResponseV1 } from './coreClient.ts'
import {
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  parseGlobalFlatFoldabilityJobDto,
} from './globalFlatFoldability.ts'

export const PROOF_SCOPE_DIAGNOSTICS_SCHEMA =
  'origami2.proof-scope-summary.v1' as const
export const LOCAL_SUFFICIENCY_CERTIFICATE_MODEL =
  'assigned_single_vertex_unique_blb_crimp_v1' as const
export const PROOF_SCOPE_VISIBLE_VERTEX_LIMIT = 20
export const MAX_PROOF_SCOPE_DIAGNOSTICS_BYTES = 8 * 1024

export type ProofScopeGlobalStatus =
  | 'not_checked'
  | 'in_progress'
  | 'possible'
  | 'impossible'
  | 'unknown'
  | 'unavailable'

export type ProofScopeDiagnostics = Readonly<{
  schema: typeof PROOF_SCOPE_DIAGNOSTICS_SCHEMA
  readOnly: true
  global: Readonly<{
    status: ProofScopeGlobalStatus
    certificateModel: typeof GLOBAL_FLAT_FOLDABILITY_MODEL_ID
    certificateVersion: 1
    targetScope: 'entire_supported_pattern'
    faceCount: number | null
    reason: string | null
    layerOrderModel: string | null
    layerCount: number | null
    maximumPly: number | null
    unproven: readonly string[]
  }>
  local: Readonly<{
    status: 'unavailable' | 'ready'
    certificateModel: typeof LOCAL_SUFFICIENCY_CERTIFICATE_MODEL
    certificateVersion: 1
    targetScope: 'all_vertices_with_assigned_mountain_valley'
    vertexCount: number
    necessaryFailed: number
    sufficientProven: number
    indeterminate: number
    unproven: readonly string[]
  }>
}>

export type ProofScopePresentation = Readonly<{
  diagnostics: ProofScopeDiagnostics
  diagnosticsJson: string
  selectableVertices: readonly Readonly<{
    id: string
    status: 'necessary_failed' | 'sufficient_proven' | 'indeterminate'
  }>[]
  hiddenVertexCount: number
}>

export function createProofScopePresentation(
  rawGlobalJob: unknown,
  localSummary: AssignedLocalSufficiencySummaryResponseV1 | null,
): ProofScopePresentation {
  const job = rawGlobalJob === null
    ? null
    : parseGlobalFlatFoldabilityJobDto(rawGlobalJob)
  const globalStatus: ProofScopeGlobalStatus = job === null
    ? rawGlobalJob === null ? 'not_checked' : 'unavailable'
    : job.state === 'queued' || job.state === 'running'
      ? 'in_progress'
      : job.state === 'completed'
        ? job.result.verdict
        : 'unavailable'
  const globalSummary = job === null
    ? null
    : job.state === 'queued' || job.state === 'running'
      ? job.progress
      : job.state === 'completed'
        ? job.result.summary
        : job.summary
  const completedResult = job?.state === 'completed' ? job.result : null

  const vertices = localSummary?.vertices ?? []
  let necessaryFailed = 0
  let sufficientProven = 0
  let indeterminate = 0
  for (const vertex of vertices) {
    if (vertex.status === 'necessary_failed') necessaryFailed += 1
    else if (vertex.status === 'sufficient_proven') sufficientProven += 1
    else indeterminate += 1
  }
  const diagnostics: ProofScopeDiagnostics = deepFreeze({
    schema: PROOF_SCOPE_DIAGNOSTICS_SCHEMA,
    readOnly: true,
    global: {
      status: globalStatus,
      certificateModel: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      certificateVersion: 1,
      targetScope: 'entire_supported_pattern',
      faceCount: globalSummary?.counts.face_count ?? null,
      reason: completedResult?.verdict === 'unknown'
        ? completedResult.reason
        : completedResult?.verdict === 'impossible'
          ? completedResult.proof.category
          : null,
      layerOrderModel: completedResult?.verdict === 'possible'
        ? completedResult.layer_order.model_id
        : null,
      layerCount: completedResult?.verdict === 'possible'
        ? completedResult.layer_order.layer_count
        : null,
      maximumPly: completedResult?.verdict === 'possible'
        ? completedResult.layer_order.max_ply
        : null,
      unproven: globalUnproven(globalStatus),
    },
    local: {
      status: localSummary === null ? 'unavailable' : 'ready',
      certificateModel: LOCAL_SUFFICIENCY_CERTIFICATE_MODEL,
      certificateVersion: 1,
      targetScope: 'all_vertices_with_assigned_mountain_valley',
      vertexCount: vertices.length,
      necessaryFailed,
      sufficientProven,
      indeterminate,
      unproven: localSummary === null
        ? ['local_summary_unavailable']
        : [
            ...(necessaryFailed > 0 ? ['vertices_failing_necessary_conditions'] : []),
            ...(indeterminate > 0 ? ['vertices_without_sufficiency_proof'] : []),
            'global_flat_foldability',
            'physical_folding_path',
          ],
    },
  })
  const selectableVertices = Object.freeze(
    vertices.slice(0, PROOF_SCOPE_VISIBLE_VERTEX_LIMIT).map((vertex) =>
      Object.freeze({ id: vertex.vertex, status: vertex.status })),
  )
  return Object.freeze({
    diagnostics,
    diagnosticsJson: JSON.stringify(diagnostics, null, 2),
    selectableVertices,
    hiddenVertexCount: Math.max(0, vertices.length - selectableVertices.length),
  })
}

export function parseProofScopeDiagnosticsJson(json: unknown): string | null {
  try {
    if (typeof json !== 'string'
      || new TextEncoder().encode(json).byteLength > MAX_PROOF_SCOPE_DIAGNOSTICS_BYTES) return null
    const value: unknown = JSON.parse(json)
    if (!exactKeys(value, ['schema', 'readOnly', 'global', 'local'])
      || value.schema !== PROOF_SCOPE_DIAGNOSTICS_SCHEMA
      || value.readOnly !== true
      || !exactKeys(value.global, [
        'status', 'certificateModel', 'certificateVersion', 'targetScope',
        'faceCount', 'reason', 'layerOrderModel', 'layerCount', 'maximumPly',
        'unproven',
      ])
      || !['not_checked', 'in_progress', 'possible', 'impossible', 'unknown', 'unavailable']
        .includes(String(value.global.status))
      || value.global.certificateModel !== GLOBAL_FLAT_FOLDABILITY_MODEL_ID
      || value.global.certificateVersion !== 1
      || value.global.targetScope !== 'entire_supported_pattern'
      || !nullableBoundedCount(value.global.faceCount, 2_048)
      || !nullableBoundedCount(value.global.layerCount, 2_048)
      || !nullableBoundedCount(value.global.maximumPly, 2_048)
      || !nullableAllowedString(value.global.reason, [
        'unsupported_topology', 'non_convex_face', 'time_limit_reached',
        'work_limit_reached', 'exact_number_limit_reached',
        'overlap_arrangement_limit_reached', 'constraint_limit_reached',
        'proof_not_completed', 'local_conditions_indeterminate',
        'local_conditions_violated', 'inconsistent_flat_embedding',
        'layer_constraints_contradictory', 'exhaustive_search_no_solution',
      ])
      || !nullableAllowedString(value.global.layerOrderModel, ['facewise_layer_order_v1'])
      || !allowedStringArray(value.global.unproven, [
        'physical_thickness', 'manual_foldability', 'collision_free_folding_path',
        'outside_supported_target_class', 'global_flat_foldability',
        'global_flat_foldability_pending', 'global_flat_foldability_not_checked',
        'global_certificate_unavailable',
      ])
      || !exactKeys(value.local, [
        'status', 'certificateModel', 'certificateVersion', 'targetScope',
        'vertexCount', 'necessaryFailed', 'sufficientProven', 'indeterminate',
        'unproven',
      ])
      || !['unavailable', 'ready'].includes(String(value.local.status))
      || value.local.certificateModel !== LOCAL_SUFFICIENCY_CERTIFICATE_MODEL
      || value.local.certificateVersion !== 1
      || value.local.targetScope !== 'all_vertices_with_assigned_mountain_valley'
      || !boundedCount(value.local.vertexCount, 4_096)
      || !boundedCount(value.local.necessaryFailed, 4_096)
      || !boundedCount(value.local.sufficientProven, 4_096)
      || !boundedCount(value.local.indeterminate, 4_096)
      || Number(value.local.necessaryFailed) + Number(value.local.sufficientProven)
        + Number(value.local.indeterminate) !== Number(value.local.vertexCount)
      || !allowedStringArray(value.local.unproven, [
        'local_summary_unavailable', 'vertices_failing_necessary_conditions',
        'vertices_without_sufficiency_proof', 'global_flat_foldability',
        'physical_folding_path',
      ])
      || JSON.stringify(value, null, 2) !== json) return null
    const status = value.global.status
    if (status === 'possible') {
      if (value.global.reason !== null
        || value.global.layerOrderModel !== 'facewise_layer_order_v1'
        || !boundedCount(value.global.layerCount, 2_048)
        || Number(value.global.layerCount) < 1
        || !boundedCount(value.global.maximumPly, Number(value.global.layerCount))
        || Number(value.global.maximumPly) < 1) return null
    } else if (value.global.layerOrderModel !== null
      || value.global.layerCount !== null
      || value.global.maximumPly !== null) return null
    if ((status === 'unknown' || status === 'impossible')
      ? value.global.reason === null
      : value.global.reason !== null) return null
    return json
  } catch {
    return null
  }
}

function globalUnproven(status: ProofScopeGlobalStatus): readonly string[] {
  switch (status) {
    case 'possible':
      return Object.freeze(['physical_thickness', 'manual_foldability', 'collision_free_folding_path'])
    case 'impossible':
      return Object.freeze(['outside_supported_target_class'])
    case 'unknown':
      return Object.freeze(['global_flat_foldability'])
    case 'in_progress':
      return Object.freeze(['global_flat_foldability_pending'])
    case 'not_checked':
      return Object.freeze(['global_flat_foldability_not_checked'])
    case 'unavailable':
      return Object.freeze(['global_certificate_unavailable'])
  }
}

function deepFreeze<T>(value: T): T {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value as Record<string, unknown>)) {
      deepFreeze(child)
    }
    Object.freeze(value)
  }
  return value
}

function exactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)
    || Object.getPrototypeOf(value) !== Object.prototype) return false
  const ownKeys = Reflect.ownKeys(value)
  return ownKeys.length === keys.length
    && ownKeys.every((key) => typeof key === 'string' && keys.includes(key))
}

function boundedCount(value: unknown, maximum: number) {
  return typeof value === 'number' && Number.isSafeInteger(value)
    && value >= 0 && value <= maximum
}

function nullableBoundedCount(value: unknown, maximum: number) {
  return value === null || boundedCount(value, maximum)
}

function nullableAllowedString(value: unknown, allowed: readonly string[]) {
  return value === null || typeof value === 'string' && allowed.includes(value)
}

function allowedStringArray(value: unknown, allowed: readonly string[]) {
  return Array.isArray(value) && value.length <= allowed.length
    && value.every((item, index) =>
      typeof item === 'string'
      && allowed.includes(item)
      && value.indexOf(item) === index)
}
