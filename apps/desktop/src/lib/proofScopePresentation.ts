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
