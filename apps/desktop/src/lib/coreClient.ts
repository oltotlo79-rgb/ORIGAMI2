import { invoke } from '@tauri-apps/api/core'
import {
  FOLD_ASSIGNMENT_CODES,
  type FoldImportPreview,
  type FoldImportSettings,
} from './foldImport.ts'
import type {
  SvgImportPreview,
  SvgImportSettings,
  SvgImportSettingsDraft,
  SvgImportSettingsValidation,
} from './svgImport.ts'
import type {
  CreasePatternExportFormat,
  CreasePatternExportPreview,
  CreasePatternExportSaveResponse,
} from './creaseExport.ts'
import type {
  InstructionExportBeginResponse,
  InstructionExportFormat,
  InstructionExportProgressResponse,
  InstructionExportPreviewResponse,
  InstructionExportSaveResponse,
} from './instructionExport.ts'
import {
  normalizeStaticMeshExportPreviewResponse,
  normalizeStaticMeshExportSaveResponse,
  type StaticMeshExportFormat,
  type StaticMeshExportPreviewResponse,
  type StaticMeshExportSaveResponse,
} from './staticMeshExport.ts'
import {
  normalizeGeometricConstraintPreflightResponse,
  type GeometricConstraintDocumentV1,
  type GeometricConstraintKindV1,
  type GeometricConstraintPreflightResponseV1,
  type GeometricConstraintPreflightResultV1,
  type GeometricConstraintRecordV1,
} from './geometricConstraints.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import { isExpectedNativeEditSnapshot } from './projectSnapshotBinding.ts'
import {
  isProjectLayerContentKind,
  isProjectLayerName,
  isProjectLayerOpacity,
  MAX_PROJECT_LAYERS,
  normalizeProjectLayerDocument,
  type LayerContentKindV1,
  type ProjectLayerDocumentV1,
} from './projectLayers.ts'
import {
  isStackedFoldReadRequest,
  normalizeStackedFoldReadResponse,
  type StackedFoldReadRequest,
  type StackedFoldReadResponse,
} from './stackedFoldRead.ts'
import {
  isMeshAnimationPreviewRequest,
  isMeshAnimationSaveRequest,
  normalizeMeshAnimationPreviewResponse,
  normalizeMeshAnimationSaveResponse,
  type MeshAnimationPreviewRequest,
  type MeshAnimationPreviewResponse,
  type MeshAnimationSaveRequest,
  type MeshAnimationSaveResponse,
} from './meshAnimationExport.ts'

export type {
  EdgeLayerAssignmentV1,
  LayerContentKindV1,
  LayerRecordV1,
  ProjectLayerDocumentV1,
} from './projectLayers.ts'

export type PatternResponse = {
  requested_edge_count: number
  vertex_count: number
  edge_count: number
  vertices: Array<{
    id: string
    position: { x: number; y: number }
  }>
  edges: Array<{
    id: string
    start: string
    end: string
    kind: 'mountain' | 'valley'
  }>
}

export const MAX_BENCHMARK_EDGE_COUNT = 100_000

export type RgbaColor = {
  red: number
  green: number
  blue: number
  alpha: number
}

export type LengthDisplayUnit =
  | 'mm'
  | 'cm'
  | 'inch'
  | { paper_edge_ratio: { reference_edge: string } }

export type PaperSnapshot = {
  boundary_vertices: string[]
  thickness_mm: number
  length_display_unit: LengthDisplayUnit
  cutting_allowed: boolean
  front: { color: RgbaColor; texture_asset: string | null }
  back: { color: RgbaColor; texture_asset: string | null }
}

export type GeometricConstraintKind = GeometricConstraintKindV1
export type GeometricConstraintRecord = GeometricConstraintRecordV1
export type GeometricConstraintDocument = GeometricConstraintDocumentV1
export type GeometricConstraintPreflightResult = GeometricConstraintPreflightResultV1
export type GeometricConstraintPreflightResponse = GeometricConstraintPreflightResponseV1

export type ProjectSnapshot = {
  project_instance_id: string
  project_id: string
  name: string
  memo: string
  beginner_design_profile: BeginnerDesignProfileV1
  current_path: string | null
  revision: number
  saved_revision: number | null
  is_dirty: boolean
  crease_pattern: {
    vertices: Array<{ id: string; position: { x: number; y: number } }>
    edges: Array<{ id: string; start: string; end: string; kind: string }>
  }
  paper: PaperSnapshot
  can_undo: boolean
  can_redo: boolean
  cutting_allowed: boolean
  instruction_timeline: InstructionTimeline
  geometric_constraints?: GeometricConstraintDocument
  project_layers: ProjectLayerDocumentV1
  element_metadata: ElementMetadataDocumentV1
  annotations?: AnnotationDocumentV1
  underlays?: UnderlayDocumentV1
  numeric_expressions?: {
    rectangular_paper_creation?: NumericExpressionBinding
    undo_stack?: Array<NumericExpressionBinding | null>
    redo_stack?: Array<NumericExpressionBinding | null>
    vertex_coordinates?: Array<VertexCoordinateExpressionBinding>
    vertex_undo_stack?: Array<VertexCoordinateExpressionTransition | null>
    vertex_redo_stack?: Array<VertexCoordinateExpressionTransition | null>
  }
  fold_model_fingerprint: string
}

export type BeginnerDesignProfileV1 = {
  schema_version: 1
  preset: 'balanced' | 'shape_priority' | 'foldability_priority'
  shape_fidelity_weight: number
  foldability_weight: number
  step_count_weight: number
  paper_efficiency_weight: number
  generation_constraints: BeginnerGenerationConstraintsV1
}

export type BeginnerGenerationConstraintsV1 = {
  schema_version: 1
  maximum_steps: number
  detail_level: 'simple' | 'standard' | 'detailed'
  target_category: 'animal' | 'insect' | null
  target_parts: Array<{
    kind: 'head' | 'torso' | 'leg' | 'horn' | 'ear' | 'wing' | 'tail'
    count: number
  }>
  skeleton_segments: Array<{
    id: number
    start: { x_tenths_mm: number; y_tenths_mm: number }
    end: { x_tenths_mm: number; y_tenths_mm: number }
    thickness_tenths_mm: number
  }>
  target_asset: {
    kind: 'reference_image'
    underlay_id: string
    asset_id: string
  } | null
  allowed_techniques: Array<
    | 'valley_fold'
    | 'mountain_fold'
    | 'inside_reverse_fold'
    | 'outside_reverse_fold'
    | 'squash_fold'
    | 'petal_fold'
    | 'sink_fold'
    | 'crimp_fold'
  >
}

const BEGINNER_TECHNIQUES = [
  'valley_fold',
  'mountain_fold',
  'inside_reverse_fold',
  'outside_reverse_fold',
  'squash_fold',
  'petal_fold',
  'sink_fold',
  'crimp_fold',
] as const

function normalizeBeginnerGenerationConstraints(
  value: unknown,
): BeginnerGenerationConstraintsV1 | null {
  const record = exactCoreDataRecord(value, [
    'schema_version',
    'maximum_steps',
    'detail_level',
    'target_category',
    'target_parts',
    'skeleton_segments',
    'target_asset',
    'allowed_techniques',
  ] as const)
  if (
    !record
    || record.schema_version !== 1
    || !Number.isInteger(record.maximum_steps)
    || Number(record.maximum_steps) < 1
    || Number(record.maximum_steps) > 500
    || (
      record.detail_level !== 'simple'
      && record.detail_level !== 'standard'
      && record.detail_level !== 'detailed'
    )
    || (record.target_category !== null
      && record.target_category !== 'animal'
      && record.target_category !== 'insect')
    || !Array.isArray(record.target_parts)
    || record.target_parts.length > 7
    || !Array.isArray(record.skeleton_segments)
    || record.skeleton_segments.length > 64
    || !Array.isArray(record.allowed_techniques)
    || record.allowed_techniques.length < 1
    || record.allowed_techniques.length > 8
    || record.allowed_techniques.some((technique) => !BEGINNER_TECHNIQUES.includes(technique))
    || new Set(record.allowed_techniques).size !== record.allowed_techniques.length
  ) return null
  const partKinds = new Set<string>()
  let partTotal = 0
  const targetParts = record.target_parts.map((part) => {
    const item = exactCoreDataRecord(part, ['kind', 'count'] as const)
    if (
      !item
      || !['head', 'torso', 'leg', 'horn', 'ear', 'wing', 'tail'].includes(String(item.kind))
      || !Number.isInteger(item.count)
      || Number(item.count) < 1
      || Number(item.count) > 8
      || partKinds.has(String(item.kind))
    ) return null
    partKinds.add(String(item.kind))
    partTotal += Number(item.count)
    return { kind: item.kind, count: Number(item.count) }
  })
  if (targetParts.some((part) => part === null)
    || partTotal > 32
    || (targetParts.length > 0 && record.target_category === null)) return null
  const segmentIds = new Set<number>()
  const skeletonSegments = record.skeleton_segments.map((segment) => {
    const item = exactCoreDataRecord(segment, ['id', 'start', 'end', 'thickness_tenths_mm'] as const)
    const start = item && exactCoreDataRecord(item.start, ['x_tenths_mm', 'y_tenths_mm'] as const)
    const end = item && exactCoreDataRecord(item.end, ['x_tenths_mm', 'y_tenths_mm'] as const)
    const coordinates = start && end
      ? [start.x_tenths_mm, start.y_tenths_mm, end.x_tenths_mm, end.y_tenths_mm]
      : []
    if (!item || !start || !end
      || !Number.isInteger(item.id) || Number(item.id) < 0 || Number(item.id) > 65535
      || segmentIds.has(Number(item.id))
      || coordinates.some((coordinate) =>
        !Number.isInteger(coordinate) || Math.abs(Number(coordinate)) > 100_000)
      || (start.x_tenths_mm === end.x_tenths_mm && start.y_tenths_mm === end.y_tenths_mm)
      || !Number.isInteger(item.thickness_tenths_mm)
      || Number(item.thickness_tenths_mm) < 1
      || Number(item.thickness_tenths_mm) > 10_000
    ) return null
    segmentIds.add(Number(item.id))
    return {
      id: Number(item.id),
      start: { x_tenths_mm: Number(start.x_tenths_mm), y_tenths_mm: Number(start.y_tenths_mm) },
      end: { x_tenths_mm: Number(end.x_tenths_mm), y_tenths_mm: Number(end.y_tenths_mm) },
      thickness_tenths_mm: Number(item.thickness_tenths_mm),
    }
  })
  if (skeletonSegments.some((segment) => segment === null)) return null
  let targetAsset: BeginnerGenerationConstraintsV1['target_asset'] = null
  if (record.target_asset !== null) {
    const asset = exactCoreDataRecord(
      record.target_asset,
      ['kind', 'underlay_id', 'asset_id'] as const,
    )
    if (!asset || asset.kind !== 'reference_image'
      || !isCanonicalNonNilUuid(asset.underlay_id)
      || !isCanonicalNonNilUuid(asset.asset_id)) return null
    targetAsset = {
      kind: 'reference_image',
      underlay_id: asset.underlay_id,
      asset_id: asset.asset_id,
    }
  }
  return Object.freeze({
    schema_version: 1,
    maximum_steps: Number(record.maximum_steps),
    detail_level: record.detail_level,
    target_category: record.target_category,
    target_parts: targetParts,
    skeleton_segments: skeletonSegments,
    target_asset: targetAsset,
    allowed_techniques: Object.freeze(record.allowed_techniques.slice()),
  }) as BeginnerGenerationConstraintsV1
}

export type BeginnerCandidateScoreV1 = {
  schema_version: 1
  kind: 'recommended' | 'shape_focused' | 'foldability_focused'
  rank: number
  total_score: number
  shape_score: number
  foldability_score: number
  step_count_score: number
  paper_efficiency_score: number
}

export type BeginnerCandidateResponseV1 = {
  schema_version: 1
  project_instance_id: string
  project_id: string
  revision: number
  requested_candidate_count: number
  bulge_treatment: 'target_shape_approximation'
  elasticity_model: 'not_computed'
  generation_status:
    | 'ready'
    | 'resource_limit'
    | 'unsupported_paper'
    | 'unsupported_techniques'
    | 'missing_target_category'
    | 'missing_required_parts'
    | 'missing_target_asset'
  generated_plans: BeginnerGeneratedPlanV1[]
  candidates: BeginnerCandidateScoreV1[]
}

export type BeginnerGeneratedPlanV1 = {
  schema_version: 1
  kind: 'vertical_book_fold' | 'horizontal_book_fold' | 'diagonal_fold'
  crease_pattern: {
    vertices: Array<{ id: string; position: { x: number; y: number } }>
    edges: Array<{ id: string; start: string; end: string; kind: 'mountain' | 'valley' }>
  }
  instruction_codes: string[]
  target_parts: BeginnerGenerationConstraintsV1['target_parts']
  skeleton_segments: BeginnerGenerationConstraintsV1['skeleton_segments']
  target_asset: BeginnerGenerationConstraintsV1['target_asset']
}

function normalizeBeginnerCandidateResponse(
  value: unknown,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  requestedCandidateCount: number,
): BeginnerCandidateResponseV1 | null {
  const response = exactCoreDataRecord(value, [
    'schema_version',
    'project_instance_id',
    'project_id',
    'revision',
    'requested_candidate_count',
    'bulge_treatment',
    'elasticity_model',
    'generation_status',
    'generated_plans',
    'candidates',
  ] as const)
  if (
    !response
    || response.schema_version !== 1
    || response.project_instance_id !== expectedProjectInstanceId
    || response.project_id !== expectedProjectId
    || response.revision !== expectedRevision
    || response.requested_candidate_count !== requestedCandidateCount
    || response.bulge_treatment !== 'target_shape_approximation'
    || response.elasticity_model !== 'not_computed'
    || !['ready', 'resource_limit', 'unsupported_paper', 'unsupported_techniques', 'missing_target_category', 'missing_required_parts', 'missing_target_asset']
      .includes(String(response.generation_status))
    || !Array.isArray(response.generated_plans)
    || response.generated_plans.length > 3
    || !Array.isArray(response.candidates)
    || response.candidates.length < 1
    || response.candidates.length > 3
    || response.candidates.length !== requestedCandidateCount
  ) return null
  const candidates = response.candidates.map((candidate, index) => {
    const record = exactCoreDataRecord(candidate, [
      'schema_version',
      'kind',
      'rank',
      'total_score',
      'shape_score',
      'foldability_score',
      'step_count_score',
      'paper_efficiency_score',
    ] as const)
    const scores = record && [
      record.total_score,
      record.shape_score,
      record.foldability_score,
      record.step_count_score,
      record.paper_efficiency_score,
    ]
    if (
      !record
      || record.schema_version !== 1
      || (
        record.kind !== 'recommended'
        && record.kind !== 'shape_focused'
        && record.kind !== 'foldability_focused'
      )
      || record.rank !== index + 1
      || !scores
      || scores.some((score) => !Number.isInteger(score) || Number(score) < 0 || Number(score) > 100)
    ) return null
    return Object.freeze({
      schema_version: 1,
      kind: record.kind,
      rank: record.rank,
      total_score: record.total_score,
      shape_score: record.shape_score,
      foldability_score: record.foldability_score,
      step_count_score: record.step_count_score,
      paper_efficiency_score: record.paper_efficiency_score,
    }) as BeginnerCandidateScoreV1
  })
  if (candidates.some((candidate) => candidate === null)) return null
  const generatedPlans = response.generated_plans.map((plan) => {
    const record = exactCoreDataRecord(plan, [
      'schema_version', 'kind', 'crease_pattern', 'instruction_codes', 'target_parts',
      'skeleton_segments',
      'target_asset',
    ] as const)
    const pattern = record && exactCoreDataRecord(record.crease_pattern, ['vertices', 'edges'] as const)
    if (
      !record
      || record.schema_version !== 1
      || !['vertical_book_fold', 'horizontal_book_fold', 'diagonal_fold'].includes(String(record.kind))
      || !pattern
      || !Array.isArray(pattern.vertices)
      || pattern.vertices.length !== 2
      || !Array.isArray(pattern.edges)
      || pattern.edges.length !== 1
      || !Array.isArray(record.instruction_codes)
      || record.instruction_codes.length !== 1
      || !record.instruction_codes.every((code) =>
        ['book_fold_vertical', 'book_fold_horizontal', 'diagonal_fold'].includes(String(code)))
    ) return null
    const normalizedPlanInputs = normalizeBeginnerGenerationConstraints({
      schema_version: 1,
      maximum_steps: 1,
      detail_level: 'simple',
      target_category: 'animal',
      target_parts: record.target_parts,
      skeleton_segments: record.skeleton_segments,
      target_asset: record.target_asset,
      allowed_techniques: ['valley_fold'],
    })
    if (!normalizedPlanInputs) return null
    const vertices = pattern.vertices.map((vertex) => {
      const item = exactCoreDataRecord(vertex, ['id', 'position'] as const)
      const position = item && exactCoreDataRecord(item.position, ['x', 'y'] as const)
      if (!item || !isCanonicalNonNilUuid(item.id) || !position
        || !Number.isFinite(position.x) || !Number.isFinite(position.y)) return null
      return { id: item.id, position: { x: Number(position.x), y: Number(position.y) } }
    })
    const edge = exactCoreDataRecord(pattern.edges[0], ['id', 'start', 'end', 'kind'] as const)
    if (vertices.some((vertex) => vertex === null) || !edge
      || !isCanonicalNonNilUuid(edge.id) || !isCanonicalNonNilUuid(edge.start)
      || !isCanonicalNonNilUuid(edge.end) || !['mountain', 'valley'].includes(String(edge.kind))) return null
    return {
      schema_version: 1,
      kind: record.kind,
      crease_pattern: { vertices, edges: [edge] },
      instruction_codes: record.instruction_codes.slice(),
      target_parts: normalizedPlanInputs.target_parts,
      skeleton_segments: normalizedPlanInputs.skeleton_segments,
      target_asset: normalizedPlanInputs.target_asset,
    } as BeginnerGeneratedPlanV1
  })
  if (generatedPlans.some((plan) => plan === null)
    || (response.generation_status === 'ready') !== (generatedPlans.length > 0)) return null
  const admitted = candidates as BeginnerCandidateScoreV1[]
  if (admitted.some((candidate, index) =>
    index > 0 && admitted[index - 1].total_score < candidate.total_score
  )) return null
  return Object.freeze({
    schema_version: 1,
    project_instance_id: expectedProjectInstanceId,
    project_id: expectedProjectId,
    revision: expectedRevision,
    requested_candidate_count: requestedCandidateCount,
    bulge_treatment: 'target_shape_approximation',
    elasticity_model: 'not_computed',
    generation_status: response.generation_status as BeginnerCandidateResponseV1['generation_status'],
    generated_plans: generatedPlans as BeginnerGeneratedPlanV1[],
    candidates: admitted.slice(),
  }) as BeginnerCandidateResponseV1
}

export function normalizeBeginnerDesignProfile(
  value: unknown,
): BeginnerDesignProfileV1 | null {
  const record = exactCoreDataRecord(value, [
    'schema_version',
    'preset',
    'shape_fidelity_weight',
    'foldability_weight',
    'step_count_weight',
    'paper_efficiency_weight',
    'generation_constraints',
  ] as const)
  if (!record || record.schema_version !== 1 || (
    record.preset !== 'balanced'
    && record.preset !== 'shape_priority'
    && record.preset !== 'foldability_priority'
  )) return null
  const weights = [
    record.shape_fidelity_weight,
    record.foldability_weight,
    record.step_count_weight,
    record.paper_efficiency_weight,
  ].map(Number)
  if (
    weights.some((weight) =>
      !Number.isInteger(weight) || Number(weight) < 0 || Number(weight) > 100)
    || weights.reduce((sum, weight) => sum + weight, 0) !== 100
  ) return null
  const generationConstraints = normalizeBeginnerGenerationConstraints(record.generation_constraints)
  if (!generationConstraints) return null
  return Object.freeze({
    schema_version: 1,
    preset: record.preset,
    shape_fidelity_weight: weights[0],
    foldability_weight: weights[1],
    step_count_weight: weights[2],
    paper_efficiency_weight: weights[3],
    generation_constraints: generationConstraints,
  }) as BeginnerDesignProfileV1
}

function sameBeginnerDesignProfile(
  value: unknown,
  expected: BeginnerDesignProfileV1,
) {
  const profile = normalizeBeginnerDesignProfile(value)
  return profile !== null
    && profile.preset === expected.preset
    && profile.shape_fidelity_weight === expected.shape_fidelity_weight
    && profile.foldability_weight === expected.foldability_weight
    && profile.step_count_weight === expected.step_count_weight
    && profile.paper_efficiency_weight === expected.paper_efficiency_weight
    && JSON.stringify(profile.generation_constraints) === JSON.stringify(expected.generation_constraints)
}

export type AnnotationAnchorV1 =
  | { kind: 'absolute'; position: { x: number; y: number } }
  | { kind: 'vertex'; vertex: string; offset: { x: number; y: number } }

export type AnnotationRecordV1 = {
  id: string
  text: string
  anchor: AnnotationAnchorV1
  style: { color: RgbaColor; font_size_mm: number; bold: boolean; italic: boolean }
  layer: string
}

export type AnnotationDocumentV1 = {
  schema_version: 1
  annotations: AnnotationRecordV1[]
}

export type UnderlayRecordV1 = {
  id: string
  asset: string
  transform: {
    position: { x: number; y: number }
    scale_x: number
    scale_y: number
    rotation_degrees: number
  }
  opacity: number
  layer: string
}

export type UnderlayDocumentV1 = {
  schema_version: 1
  underlays: UnderlayRecordV1[]
}

export type ElementMetadata = {
  name: string
  color: RgbaColor | null
  memo: string
}

export type ElementMetadataDocumentV1 = {
  vertices: readonly { vertex: string; metadata: ElementMetadata }[]
  edges: readonly { edge: string; metadata: ElementMetadata }[]
  faces: readonly { face: string; metadata: ElementMetadata }[]
}

export type ElementMetadataTarget =
  | { kind: 'vertex'; id: string }
  | { kind: 'edge'; id: string }
  | { kind: 'face'; id: string }

export interface NumericExpressionBinding {
      schema_version: 1
      width_source: string
      height_source: string
      adopted_width_mm: number
      adopted_height_mm: number
}

export interface VertexCoordinateExpressionBinding {
  schema_version: 1
  vertex: string
  x_source: string
  y_source: string
  adopted_x_mm: number
  adopted_y_mm: number
  polar_construction?: {
    schema_version: 1
    start_vertex: string
    adopted_start_x_mm: number
    adopted_start_y_mm: number
    length_source: string
    angle_degrees_source: string
    adopted_length_mm: number
    adopted_angle_degrees: number
  }
}

export interface VertexCoordinateExpressionTransition {
  changes: Array<{
    vertex: string
    before: VertexCoordinateExpressionBinding | null
    after: VertexCoordinateExpressionBinding | null
  }>
}

export type ProjectLayerMutationErrorCode =
  | 'invalid_request'
  | 'native_unavailable'
  | 'invalid_response'
  | 'stale_response'

const PROJECT_LAYER_MUTATION_ERROR_MESSAGES:
Readonly<Record<ProjectLayerMutationErrorCode, string>> = Object.freeze({
  invalid_request: 'レイヤー操作の変更条件が正しくありません。',
  native_unavailable: 'レイヤー操作をデスクトップ機能で処理できませんでした。',
  invalid_response: 'レイヤー操作の応答を確認できませんでした。',
  stale_response: '現在とは異なるプロジェクト状態のレイヤー操作応答を拒否しました。',
})

/**
 * Fixed, redacted boundary failure for layer mutations. Native rejection
 * strings and malformed response data are never retained on this error.
 */
export class ProjectLayerMutationError extends Error {
  readonly code: ProjectLayerMutationErrorCode

  constructor(code: ProjectLayerMutationErrorCode) {
    super(PROJECT_LAYER_MUTATION_ERROR_MESSAGES[code])
    this.name = 'ProjectLayerMutationError'
    this.code = code
  }
}

export type InstructionHingeAngle = {
  edge: string
  angle_degrees: number
}

export type InstructionPose = {
  model: 'absolute_hinge_angles_v1' | 'declarative_only_v1'
  source_model_fingerprint: string
  fixed_face: string | null
  hinge_angles: readonly InstructionHingeAngle[]
}

export type InstructionPoint3 = { x: number; y: number; z: number }
export type InstructionVisual = {
  camera: {
    position: InstructionPoint3
    target: InstructionPoint3
    up: InstructionPoint3
  } | null
  arrows: readonly {
    start: InstructionPoint3
    end: InstructionPoint3
    label: string
  }[]
  focus_points: readonly {
    position: InstructionPoint3
    radius: number
    label: string
  }[]
  hand_guides: readonly {
    kind: 'pinch' | 'hold' | 'push'
    position: InstructionPoint3
    direction: InstructionPoint3
    label: string
  }[]
}

export type NamedTechniqueTimelineSourceKindV1 =
  | 'technique'
  | 'parameter'
  | 'precondition'
  | 'operation'

export type NamedTechniqueTimelineProposalStepV1 = Readonly<{
  source_kind: NamedTechniqueTimelineSourceKindV1
  source_id: string
  chunk_index: number
  chunk_count: number
  title: string
  description: string
  caution: string
  duration_ms: number
}>

export type NamedTechniqueTimelineProposalV1 = Readonly<{
  schema_version: 1
  package_id: string
  technique_id: string
  technique_version: number
  steps: readonly NamedTechniqueTimelineProposalStepV1[]
}>

export type NamedTechniqueTimelineClientErrorCode =
  | 'invalid_request'
  | 'native_unavailable'

export class NamedTechniqueTimelineClientError extends Error {
  readonly code: NamedTechniqueTimelineClientErrorCode

  constructor(code: NamedTechniqueTimelineClientErrorCode) {
    super(code === 'invalid_request'
      ? '名前付き折り技法のタイムライン案が正しくありません。'
      : '名前付き折り技法をタイムラインへ追加できませんでした。')
    this.name = 'NamedTechniqueTimelineClientError'
    this.code = code
  }
}

export type InstructionStep = {
  id: string
  title: string
  description: string
  caution: string
  duration_ms: number
  visual: InstructionVisual
  pose: InstructionPose
}

export type InstructionTimeline = {
  steps: readonly InstructionStep[]
}

export type NewProjectSettings = {
  name: string
  widthExpression: string
  heightExpression: string
  thicknessMm: number
  cuttingAllowed: boolean
  frontColor: RgbaColor
  backColor: RgbaColor
}

export type PaperPropertySettings = {
  thicknessMm: number
  frontColor: RgbaColor
  backColor: RgbaColor
  frontTextureAsset: string | null
  backTextureAsset: string | null
  cuttingAllowed: boolean
}

export type ProjectFileResponse = {
  canceled: boolean
  project: ProjectSnapshot
}

export type FoldImportPreviewResponse = {
  canceled: boolean
  preview: FoldImportPreview | null
}

export type SvgImportPreviewResponse = {
  canceled: boolean
  preview: SvgImportPreview | null
}

export type CreasePatternExportPreviewResponse = {
  preview: CreasePatternExportPreview
}

export type EdgeIntersectionResponse = {
  snapshot: ProjectSnapshot
  vertex_id: string
}

export type IntersectionClusterTarget = Readonly<{
  edgeId: string
  relation: 'interior' | 'endpoint'
}>

export type LocalFlatFoldabilityCondition =
  | 'satisfied'
  | 'violated'
  | 'not_applicable'
  | 'indeterminate'

export type LocalFlatFoldabilityReason =
  | 'paper_boundary'
  | 'cut_incident'
  | 'fold_degree_limit'
  | 'no_incident_fold_edges'
  | null

export type LocalFlatFoldabilityVertexSnapshot = {
  vertex: string
  fold_degree: number
  mountain_count: number
  valley_count: number
  verdict: LocalFlatFoldabilityCondition
  reason: LocalFlatFoldabilityReason
  kawasaki: LocalFlatFoldabilityCondition
  maekawa: LocalFlatFoldabilityCondition
}

export type LocalFlatFoldabilityReport = {
  model: 'interior_single_vertex_zero_thickness_v1'
  max_exact_fold_degree: number
  status:
    | 'blocked'
    | 'not_applicable'
    | 'necessary_conditions_satisfied'
    | 'violated'
    | 'indeterminate'
  total_vertices: number
  applicable_vertices: number
  satisfied_vertices: number
  violated_vertices: number
  not_applicable_vertices: number
  indeterminate_vertices: number
  vertices: LocalFlatFoldabilityVertexSnapshot[]
}

export type ValidationSnapshot = {
  project_id: string
  revision: number
  is_valid: boolean
  issues: Array<{
    code: string
    vertices: string[]
    edges: string[]
  }>
  local_flat_foldability: LocalFlatFoldabilityReport
}

export type FoldAssignment = 'mountain' | 'valley'

export type TopologyHalfEdge = {
  edge: string
  origin: string
  destination: string
}

export type TopologyBoundaryWalk = {
  half_edges: TopologyHalfEdge[]
  signed_double_area: number
}

export type TopologyFace = {
  id: string
  /** Canonical SHA-256 digest serialized as exactly 32 bytes. */
  key: number[]
  outer: TopologyBoundaryWalk
  holes?: TopologyBoundaryWalk[]
  seams?: TopologyBoundaryWalk[]
  area: number
}

export type TopologyEdgeIncidence =
  | { kind: 'boundary'; material: string }
  | {
    kind: 'hinge'
    left: string
    right: string
    assignment: FoldAssignment
  }
  | { kind: 'cut'; left: string; right: string }
  | { kind: 'auxiliary_ignored' }

export type TopologyFaceAdjacency = {
  edge: string
  first: string
  second: string
  assignment: FoldAssignment
}

export type TopologyMaterialComponent = {
  key: number[]
  sheet_origin: string
  faces: string[]
}

export type TopologySnapshot = {
  source_revision: number
  faces: TopologyFace[]
  edge_incidence: Array<[string, TopologyEdgeIncidence]>
  hinge_adjacency: TopologyFaceAdjacency[]
  material_components: TopologyMaterialComponent[]
}

export type TopologyIssueKind =
  | { kind: 'duplicate_vertex_id'; vertex: string }
  | { kind: 'duplicate_edge_id'; edge: string }
  | { kind: 'invalid_paper'; issue_count: number }
  | { kind: 'invalid_crease_pattern'; issue_count: number }
  | { kind: 'unsupported_active_edge'; edge: string; edge_kind: string }
  | { kind: 'too_many_active_fold_edges'; edges: string[] }
  | { kind: 'active_edge_outside_paper'; edge: string }
  | { kind: 'disconnected_fold_graph'; edge: string }
  | { kind: 'non_separating_fold'; edge: string }
  | { kind: 'unsupported_fold_graph'; edge: string }
  | { kind: 'invalid_edge_incidence'; edge: string }
  | { kind: 'fold_endpoint_not_on_boundary'; edge: string; vertex: string }
  | { kind: 'unsupported_adjacent_boundary_fold'; edge: string }
  | { kind: 'unsupported_non_convex_fold_sheet'; edge: string; vertex: string }
  | { kind: 'degenerate_fold_face'; edge: string }
  | { kind: 'unrepresentable_face_area' }
  | { kind: 'internal_boundary_resolution' }

export type ProjectTopologyResponse = {
  project_id: string
  revision: number
  simulation_ready: boolean
  snapshot: TopologySnapshot | null
  issues: Array<{
    severity: 'warning' | 'blocks_simulation' | 'fatal'
    kind: TopologyIssueKind
  }>
}

export function isNativeCoreAvailable() {
  return '__TAURI_INTERNALS__' in window
}

export async function generateBenchmarkPattern(edgeCount: number): Promise<PatternResponse> {
  const normalizedEdgeCount = normalizeBenchmarkEdgeCount(edgeCount)
  if (isNativeCoreAvailable()) {
    return invoke<PatternResponse>('generate_benchmark_pattern', { edgeCount: normalizedEdgeCount })
  }

  return createBrowserBenchmarkPattern(normalizedEdgeCount)
}

export function normalizeBenchmarkEdgeCount(edgeCount: number) {
  if (!Number.isFinite(edgeCount)) return 0
  return Math.min(MAX_BENCHMARK_EDGE_COUNT, Math.max(0, Math.trunc(edgeCount)))
}

/**
 * Browser-only development fixture matching the native command's topology,
 * ordering, IDs, coordinates, and crease kinds.
 */
export function createBrowserBenchmarkPattern(edgeCount: number): PatternResponse {
  const normalizedEdgeCount = normalizeBenchmarkEdgeCount(edgeCount)
  if (normalizedEdgeCount === 0) {
    return {
      requested_edge_count: 0,
      vertex_count: 0,
      edge_count: 0,
      vertices: [],
      edges: [],
    }
  }

  let side = Math.max(2, Math.ceil(Math.sqrt(normalizedEdgeCount / 2)))
  while (2 * side * (side - 1) < normalizedEdgeCount) side += 1

  const vertices: PatternResponse['vertices'] = Array.from({ length: side * side }, (_, index) => ({
    id: benchmarkVertexId(index),
    position: { x: index % side, y: Math.floor(index / side) },
  }))
  const edges: PatternResponse['edges'] = []

  outer: for (let y = 0; y < side; y += 1) {
    for (let x = 0; x < side; x += 1) {
      const index = y * side + x
      if (x + 1 < side) {
        edges.push({
          id: benchmarkEdgeId(edges.length),
          start: benchmarkVertexId(index),
          end: benchmarkVertexId(index + 1),
          kind: y % 2 === 0 ? 'mountain' : 'valley',
        })
        if (edges.length === normalizedEdgeCount) break outer
      }
      if (y + 1 < side) {
        edges.push({
          id: benchmarkEdgeId(edges.length),
          start: benchmarkVertexId(index),
          end: benchmarkVertexId(index + side),
          kind: x % 2 === 0 ? 'valley' : 'mountain',
        })
        if (edges.length === normalizedEdgeCount) break outer
      }
    }
  }

  return {
    requested_edge_count: normalizedEdgeCount,
    vertex_count: vertices.length,
    edge_count: edges.length,
    vertices,
    edges,
  }
}

function benchmarkVertexId(index: number) {
  return `benchmark-v-${index}`
}

function benchmarkEdgeId(index: number) {
  return `benchmark-e-${index}`
}

export function getProjectSnapshot() {
  return invoke<ProjectSnapshot>('project_snapshot')
}

export function updateProjectMemo(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  memo: string,
) {
  return invoke<ProjectSnapshot>('update_project_memo', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    memo,
  })
}

export function updateBeginnerDesignProfile(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  profile: BeginnerDesignProfileV1,
) {
  return invoke<ProjectSnapshot>('update_beginner_design_profile', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    profile,
  })
}

export function evaluateBeginnerCandidates(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  requestedCandidateCount: number,
) {
  if (!Number.isInteger(requestedCandidateCount)
    || requestedCandidateCount < 1 || requestedCandidateCount > 3) {
    return Promise.reject(new Error('invalid requested candidate count'))
  }
  return invoke<unknown>('evaluate_beginner_candidates', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    requestedCandidateCount,
  }).then((value) => {
    const response = normalizeBeginnerCandidateResponse(
      value,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
      requestedCandidateCount,
    )
    if (!response) throw new Error('invalid beginner candidate response')
    return response
  })
}

export function applyBeginnerGeneratedPlan(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  expectedProfile: BeginnerDesignProfileV1,
  selectedKind: BeginnerGeneratedPlanV1['kind'],
  expectedCandidateEdgeId: string,
) {
  if (selectedKind !== 'diagonal_fold' || !isCanonicalNonNilUuid(expectedCandidateEdgeId)) {
    return Promise.reject(new Error('unsupported generated plan'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_generated_plan', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    expectedProfile,
    selectedKind,
    expectedCandidateEdgeId,
  })
}

export function validateProject() {
  return invoke<ValidationSnapshot>('validate_project')
}

export function analyzeProjectTopology(expectedProjectId: string, expectedRevision: number) {
  return invoke<ProjectTopologyResponse>('analyze_project_topology', {
    expectedProjectId,
    expectedRevision,
  })
}

export function proposeCurrentStackedFoldRead(
  request: StackedFoldReadRequest,
): Promise<StackedFoldReadResponse> {
  if (!isStackedFoldReadRequest(request)) {
    return Promise.reject(new Error('invalid stacked-fold request'))
  }
  return invoke<unknown>('propose_current_stacked_fold_read', { request }).then((value) => {
    const response = normalizeStackedFoldReadResponse(value, request)
    if (!response) throw new Error('invalid stacked-fold response')
    return response
  }, (error: unknown) => {
    if (error === 'stacked_fold_cycle_nonclosing') {
      throw new StackedFoldReadNativeError('cycle_nonclosing')
    }
    if (error === 'stacked_fold_cycle_path_uncertified') {
      throw new StackedFoldReadNativeError('cycle_path_uncertified')
    }
    throw new StackedFoldReadNativeError('native_failure')
  })
}

export class StackedFoldReadNativeError extends Error {
  readonly reason:
    | 'cycle_nonclosing'
    | 'cycle_path_uncertified'
    | 'native_failure'

  constructor(reason: StackedFoldReadNativeError['reason']) {
    super('stacked-fold read failed')
    this.reason = reason
  }
}

export function cancelStackedFoldTransactionPreview(token: string): Promise<void> {
  if (!isCanonicalNonNilUuid(token)) {
    return Promise.reject(new Error('invalid stacked-fold transaction token'))
  }
  return invoke<void>('cancel_stacked_fold_transaction_preview', { token })
}

export function applyStackedFoldTransaction(token: string): Promise<number> {
  if (!isCanonicalNonNilUuid(token)) {
    return Promise.reject(new Error('invalid stacked-fold transaction token'))
  }
  return invoke<unknown>('apply_stacked_fold_transaction', { token }).then((value) => {
    if (!Number.isSafeInteger(value) || (value as number) < 0) {
      throw new Error('invalid stacked-fold apply response')
    }
    return value as number
  })
}

export function previewInstructionMeshAnimation(
  request: MeshAnimationPreviewRequest,
): Promise<MeshAnimationPreviewResponse> {
  if (!isMeshAnimationPreviewRequest(request)) {
    return Promise.reject(new Error('invalid mesh-animation preview request'))
  }
  return invoke<unknown>('preview_instruction_mesh_animation', { request }).then((value) => {
    const response = normalizeMeshAnimationPreviewResponse(value, request)
    if (!response) throw new Error('invalid mesh-animation preview response')
    return response
  })
}

export function cancelInstructionMeshAnimation(exportId: string): Promise<void> {
  if (!isCanonicalNonNilUuid(exportId)) {
    return Promise.reject(new Error('invalid mesh-animation export id'))
  }
  return invoke<void>('cancel_instruction_mesh_animation', { exportId })
}

export function saveInstructionMeshAnimation(
  request: MeshAnimationSaveRequest,
): Promise<MeshAnimationSaveResponse> {
  if (!isMeshAnimationSaveRequest(request)) {
    return Promise.reject(new Error('invalid mesh-animation save request'))
  }
  return invoke<unknown>('save_instruction_mesh_animation', { request }).then((value) => {
    const response = normalizeMeshAnimationSaveResponse(value)
    if (!response) throw new Error('invalid mesh-animation save response')
    return response
  })
}

export function analyzeGeometricConstraints(
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
) {
  return invoke<unknown>('analyze_geometric_constraints', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  }).then((response) => {
    const normalized = normalizeGeometricConstraintPreflightResponse(response, {
      project_instance_id: expectedProjectInstanceId,
      project_id: expectedProjectId,
      revision: expectedRevision,
    })
    if (!normalized) {
      throw new Error('invalid geometric-constraint preflight response')
    }
    return normalized
  })
}

export function openProject() {
  return invoke<ProjectFileResponse>('open_project')
}

export function saveProject() {
  return invoke<ProjectFileResponse>('save_project')
}

export function saveProjectAs() {
  return invoke<ProjectFileResponse>('save_project_as')
}

export function previewCreasePatternExport(
  expectedProjectId: string,
  expectedRevision: number,
  format: CreasePatternExportFormat,
) {
  return invoke<CreasePatternExportPreviewResponse>('preview_crease_pattern_export', {
    expectedProjectId,
    expectedRevision,
    format,
  })
}

export function saveCreasePatternExport(
  exportId: string,
  expectedProjectId: string,
  expectedRevision: number,
  warningsAcknowledged: boolean,
) {
  return invoke<CreasePatternExportSaveResponse>('save_crease_pattern_export', {
    exportId,
    expectedProjectId,
    expectedRevision,
    warningsAcknowledged,
  })
}

export function cancelCreasePatternExport(exportId: string) {
  return invoke<void>('cancel_crease_pattern_export', { exportId })
}

export function previewStaticMeshExport(
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  format: StaticMeshExportFormat,
) {
  return invoke<unknown>('preview_static_mesh_export', {
    request: {
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
      format,
    },
  }).then((value): StaticMeshExportPreviewResponse => {
    const response = normalizeStaticMeshExportPreviewResponse(value)
    if (!response) throw new Error('invalid static-mesh export preview response')
    return response
  })
}

export function saveStaticMeshExport(
  preview: Readonly<{
    exportId: string
    projectInstanceId: string
    projectId: string
    revision: number
    sourceFingerprint: string
    poseGeneration: string
  }>,
  warningsAcknowledged: boolean,
) {
  return invoke<unknown>('save_static_mesh_export', {
    request: {
      exportId: preview.exportId,
      expectedProjectInstanceId: preview.projectInstanceId,
      expectedProjectId: preview.projectId,
      expectedRevision: preview.revision,
      expectedSourceFingerprint: preview.sourceFingerprint,
      expectedPoseGeneration: preview.poseGeneration,
      warningsAcknowledged,
    },
  }).then((value): StaticMeshExportSaveResponse => {
    const response = normalizeStaticMeshExportSaveResponse(value)
    if (!response) throw new Error('invalid static-mesh export save response')
    return response
  })
}

export function cancelStaticMeshExport(exportId: string) {
  return invoke<void>('cancel_static_mesh_export', { exportId })
}

export function beginInstructionExportGeneration() {
  return invoke<InstructionExportBeginResponse>('begin_instruction_export')
}

export function previewInstructionExport(
  exportId: string,
  expectedProjectId: string,
  expectedRevision: number,
  format: InstructionExportFormat,
) {
  return invoke<InstructionExportPreviewResponse>('preview_instruction_export', {
    exportId,
    expectedProjectId,
    expectedRevision,
    format,
  })
}

export function getInstructionExportProgress(exportId: string) {
  return invoke<InstructionExportProgressResponse>('get_instruction_export_progress', {
    exportId,
  })
}

export function saveInstructionExport(
  exportId: string,
  expectedProjectId: string,
  expectedRevision: number,
  warningsAcknowledged: boolean,
) {
  return invoke<InstructionExportSaveResponse>('save_instruction_export', {
    exportId,
    expectedProjectId,
    expectedRevision,
    warningsAcknowledged,
  })
}

export function cancelInstructionExport(exportId: string) {
  return invoke<void>('cancel_instruction_export', { exportId })
}

export function previewFoldImport() {
  return invoke<FoldImportPreviewResponse>('preview_fold_import')
}

export function applyFoldImport(
  expectedProjectId: string,
  expectedRevision: number,
  settings: FoldImportSettings,
) {
  const assignmentMappings = FOLD_ASSIGNMENT_CODES.flatMap((source) => {
    const target = settings.mappings[source]
    return target ? [{ source, target }] : []
  })
  return invoke<ProjectSnapshot>('apply_fold_import', {
    previewId: settings.importId,
    expectedProjectId,
    expectedRevision,
    name: settings.name,
    millimetersPerUnit: settings.mmPerUnit,
    boundaryCandidateId: settings.boundaryCandidateId,
    assignmentMappings,
  })
}

export function cancelFoldImport(previewId: string) {
  return invoke<void>('cancel_fold_import', { previewId })
}

export function previewSvgImport() {
  return invoke<SvgImportPreviewResponse>('preview_svg_import')
}

export function validateSvgImportSettings(
  expectedProjectId: string,
  expectedRevision: number,
  settings: SvgImportSettingsDraft,
) {
  return invoke<SvgImportSettingsValidation>('validate_svg_import_settings', {
    previewId: settings.importId,
    expectedProjectId,
    expectedRevision,
    millimetersPerUnit: settings.mmPerUnit,
    boundaryCandidateId: settings.boundaryCandidateId,
    styleMappings: svgImportStyleMappings(settings.mappings),
  })
}

export function applySvgImport(
  expectedProjectId: string,
  expectedRevision: number,
  settings: SvgImportSettings,
  replaceDirtyProjectConfirmed: boolean,
) {
  return invoke<ProjectSnapshot>('apply_svg_import', {
    previewId: settings.importId,
    expectedProjectId,
    expectedRevision,
    name: settings.name,
    millimetersPerUnit: settings.mmPerUnit,
    boundaryCandidateId: settings.boundaryCandidateId,
    validationId: settings.validationId,
    boundaryConfirmed: settings.boundaryConfirmed,
    styleMappings: svgImportStyleMappings(settings.mappings),
    warningsAcknowledged: settings.warningsAcknowledged,
    cuttingAllowedConfirmed: settings.cuttingAllowedConfirmed,
    replaceDirtyProjectConfirmed,
  })
}

export function cancelSvgImport(previewId: string) {
  return invoke<void>('cancel_svg_import', { previewId })
}

function svgImportStyleMappings(settings: SvgImportSettingsDraft['mappings']) {
  return Object.entries(settings)
    .filter((entry): entry is [string, NonNullable<(typeof entry)[1]>] => Boolean(entry[1]))
    .map(([groupId, target]) => ({ groupId: Number(groupId), target }))
    .sort((left, right) => left.groupId - right.groupId)
}

export function addInstructionStep(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  title: string,
  description: string,
  caution: string,
  durationMs: number,
  fixedFace: string | null,
  hingeAngles: readonly InstructionHingeAngle[],
) {
  return invoke<ProjectSnapshot>('add_instruction_step', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    title,
    description,
    caution,
    durationMs,
    fixedFace,
    hingeAngles,
  })
}

export function appendNamedTechniqueInstructionSteps(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  proposal: NamedTechniqueTimelineProposalV1,
) {
  if (
    !isCanonicalNonNilUuid(expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(expectedProjectId)
    || !isProjectRevision(expectedRevision)
    || expectedRevision >= Number.MAX_SAFE_INTEGER
    || !isNamedTechniqueTimelineProposalV1(proposal)
  ) {
    return Promise.reject(
      new NamedTechniqueTimelineClientError('invalid_request'),
    )
  }
  let proposalJson: string
  try {
    proposalJson = JSON.stringify(proposal)
  } catch {
    return Promise.reject(
      new NamedTechniqueTimelineClientError('invalid_request'),
    )
  }
  if (new TextEncoder().encode(proposalJson).length > 2 * 1024 * 1024) {
    return Promise.reject(
      new NamedTechniqueTimelineClientError('invalid_request'),
    )
  }
  return invoke<ProjectSnapshot>('append_named_technique_instruction_steps', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    proposalJson,
  }).catch(() => {
    throw new NamedTechniqueTimelineClientError('native_unavailable')
  })
}

export function updateInstructionStepMetadata(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  stepId: string,
  title: string,
  description: string,
  caution: string,
  durationMs: number,
  visual: InstructionVisual,
) {
  return invoke<ProjectSnapshot>('update_instruction_step_metadata', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    stepId,
    title,
    description,
    caution,
    durationMs,
    visual,
  })
}

export function replaceInstructionStepPose(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  stepId: string,
  fixedFace: string | null,
  hingeAngles: readonly InstructionHingeAngle[],
) {
  return invoke<ProjectSnapshot>('replace_instruction_step_pose', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    stepId,
    fixedFace,
    hingeAngles,
  })
}

export function removeInstructionStep(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  stepId: string,
) {
  return invoke<ProjectSnapshot>('remove_instruction_step', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    stepId,
  })
}

export function moveInstructionStep(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  stepId: string,
  targetIndex: number,
) {
  return invoke<ProjectSnapshot>('move_instruction_step', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    stepId,
    targetIndex,
  })
}

export function newProject(
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  settings: NewProjectSettings,
) {
  return invoke<ProjectSnapshot>('new_project', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    name: settings.name,
    widthExpression: settings.widthExpression,
    heightExpression: settings.heightExpression,
    thicknessMm: settings.thicknessMm,
    cuttingAllowed: settings.cuttingAllowed,
    frontColor: settings.frontColor,
    backColor: settings.backColor,
  })
}

export function addVertex(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  x: number,
  y: number,
  xExpression = String(x),
  yExpression = String(y),
) {
  return invoke<ProjectSnapshot>('add_vertex', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    x,
    y,
    xExpression,
    yExpression,
  })
}

export function addEdge(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  start: string,
  end: string,
  kind: 'mountain' | 'valley' | 'auxiliary' | 'cut',
) {
  return invoke<ProjectSnapshot>('add_edge', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    start,
    end,
    kind,
  })
}

export function addConnectedVertex(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  start: string,
  x: number,
  y: number,
  lengthExpression: string,
  angleDegreesExpression: string,
  lengthMm: number,
  angleDegrees: number,
  kind: 'mountain' | 'valley' | 'auxiliary' | 'cut',
) {
  return invoke<ProjectSnapshot>('add_connected_vertex', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    start,
    x,
    y,
    lengthExpression,
    angleDegreesExpression,
    lengthMm,
    angleDegrees,
    kind,
  })
}

export function moveVertex(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
  x: number,
  y: number,
  xExpression = String(x),
  yExpression = String(y),
) {
  return invoke<ProjectSnapshot>('move_vertex', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
    x,
    y,
    xExpression,
    yExpression,
  })
}

export function moveEdge(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
  deltaXExpression: string,
  deltaYExpression: string,
  deltaXMm: number,
  deltaYMm: number,
) {
  return invoke<ProjectSnapshot>('move_edge', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
    deltaXExpression,
    deltaYExpression,
    deltaXMm,
    deltaYMm,
  })
}

export function mirrorEdgeLeftRight(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
  axisXExpression: string,
  axisXMm: number,
) {
  return invoke<ProjectSnapshot>('mirror_edge_left_right', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
    axisXExpression,
    axisXMm,
  })
}

export function rotateEdgeAboutPoint(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
  centerXExpression: string,
  centerYExpression: string,
  angleDegreesExpression: string,
  centerXMm: number,
  centerYMm: number,
  angleDegrees: number,
) {
  return invoke<ProjectSnapshot>('rotate_edge_about_point', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
    centerXExpression,
    centerYExpression,
    angleDegreesExpression,
    centerXMm,
    centerYMm,
    angleDegrees,
  })
}

export function moveVertices(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  vertices: string[],
  deltaXExpression: string,
  deltaYExpression: string,
  deltaXMm: number,
  deltaYMm: number,
) {
  return invoke<ProjectSnapshot>('move_vertices', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    vertices,
    deltaXExpression,
    deltaYExpression,
    deltaXMm,
    deltaYMm,
  })
}

export type MirrorSelectionRequest = {
  vertices: string[]
  edges: string[]
  axis: {
    start: { x: number; y: number }
    end: { x: number; y: number }
  }
  mode: 'move' | 'duplicate'
  new_vertices: string[]
  new_edges: string[]
}

export type MirrorSelectionPreflight = {
  allowed: boolean
  mode: 'move' | 'duplicate'
  vertex_count: number
  edge_count: number
  issue: string | null
}

export function preflightMirrorSelection(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  request: MirrorSelectionRequest,
) {
  return invoke<MirrorSelectionPreflight>('preflight_mirror_selection', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    request,
  })
}

export function applyMirrorSelection(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  request: MirrorSelectionRequest,
) {
  return invoke<ProjectSnapshot>('apply_mirror_selection', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    request,
  })
}

export type GeometricConstraintSolvePreview = {
  token: string
  revision: number
  iterations: number
  maximumResidual: number
  rank: number
  degreesOfFreedom: number
  equationCount: number
  conditionEstimate: number
  systemClassification: 'under_constrained' | 'over_constrained' | 'well_constrained'
  changedVertices: Array<{ vertexId: string; x: number; y: number }>
}

export function previewGeometricConstraintEdgeSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  drivingEdge: string,
  startXMm: number,
  startYMm: number,
  endXMm: number,
  endYMm: number,
) {
  return invoke<GeometricConstraintSolvePreview>('preview_geometric_constraint_edge_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    drivingEdge,
    startXMm,
    startYMm,
    endXMm,
    endYMm,
  })
}

export function previewGeometricConstraintExpressionSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<GeometricConstraintSolvePreview>('preview_geometric_constraint_expression_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
}

export function previewGeometricConstraintSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  drivingVertex: string,
  xMm: number,
  yMm: number,
) {
  return invoke<GeometricConstraintSolvePreview>('preview_geometric_constraint_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    drivingVertex,
    xMm,
    yMm,
  })
}

export function applyGeometricConstraintSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  token: string,
) {
  return invoke<ProjectSnapshot>('apply_geometric_constraint_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    token,
  })
}

export function removeVertex(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
) {
  return invoke<ProjectSnapshot>('remove_vertex', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
  })
}

export function removeBoundaryVertex(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  vertex: string,
) {
  return invoke<ProjectSnapshot>('remove_boundary_vertex', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    vertex,
  })
}

export function removeEdge(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
) {
  return invoke<ProjectSnapshot>('remove_edge', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
  })
}

export function createProjectLayer(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  baseSnapshot: ProjectSnapshot,
  name: string,
  contentKind: LayerContentKindV1,
) {
  if (
    !isProjectLayerMutationBinding(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerName(name)
    || !isProjectLayerContentKind(contentKind)
  ) return rejectProjectLayerMutation('invalid_request')

  return invoke<unknown>('create_project_layer', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    name,
    contentKind,
  }).then(
    (value) => admitProjectLayerMutationSnapshot(
      value,
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    ),
    () => {
      throw new ProjectLayerMutationError('native_unavailable')
    },
  )
}

export function renameProjectLayer(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  baseSnapshot: ProjectSnapshot,
  layer: string,
  name: string,
) {
  if (
    !isProjectLayerMutationBinding(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isCanonicalNonNilUuid(layer)
    || !isProjectLayerName(name)
  ) return rejectProjectLayerMutation('invalid_request')

  return invoke<unknown>('rename_project_layer', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    layer,
    name,
  }).then(
    (value) => admitProjectLayerMutationSnapshot(
      value,
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    ),
    () => {
      throw new ProjectLayerMutationError('native_unavailable')
    },
  )
}

export function updateProjectLayerPresentation(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  baseSnapshot: ProjectSnapshot,
  layer: string,
  visible: boolean,
  locked: boolean,
  opacity: number,
) {
  if (
    !isProjectLayerMutationBinding(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isCanonicalNonNilUuid(layer)
    || typeof visible !== 'boolean'
    || typeof locked !== 'boolean'
    || !isProjectLayerOpacity(opacity)
  ) return rejectProjectLayerMutation('invalid_request')

  return invoke<unknown>('update_project_layer_presentation', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    layer,
    presentation: {
      visible,
      locked,
      opacity,
    },
  }).then(
    (value) => admitProjectLayerMutationSnapshot(
      value,
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    ),
    () => {
      throw new ProjectLayerMutationError('native_unavailable')
    },
  )
}

export function moveProjectLayer(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  baseSnapshot: ProjectSnapshot,
  layer: string,
  targetIndex: number,
) {
  if (
    !isProjectLayerMutationBinding(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isCanonicalNonNilUuid(layer)
    || !Number.isSafeInteger(targetIndex)
    || targetIndex < 0
    || targetIndex >= MAX_PROJECT_LAYERS
  ) return rejectProjectLayerMutation('invalid_request')

  return invoke<unknown>('move_project_layer', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    layer,
    targetIndex,
  }).then(
    (value) => admitProjectLayerMutationSnapshot(
      value,
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    ),
    () => {
      throw new ProjectLayerMutationError('native_unavailable')
    },
  )
}

export function deleteProjectLayer(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  baseSnapshot: ProjectSnapshot,
  layer: string,
) {
  if (
    !isProjectLayerMutationBinding(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isCanonicalNonNilUuid(layer)
  ) return rejectProjectLayerMutation('invalid_request')

  return invoke<unknown>('delete_project_layer', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    layer,
  }).then(
    (value) => admitProjectLayerMutationSnapshot(
      value,
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    ),
    () => {
      throw new ProjectLayerMutationError('native_unavailable')
    },
  )
}

export function assignEdgeToProjectLayer(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  baseSnapshot: ProjectSnapshot,
  edge: string,
  layer: string,
) {
  if (
    !isProjectLayerMutationBinding(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    || !isCanonicalNonNilUuid(edge)
    || !isCanonicalNonNilUuid(layer)
  ) return rejectProjectLayerMutation('invalid_request')

  return invoke<unknown>('assign_edge_to_project_layer', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    edge,
    layer,
  }).then(
    (value) => admitProjectLayerMutationSnapshot(
      value,
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    ),
    () => {
      throw new ProjectLayerMutationError('native_unavailable')
    },
  )
}

export function addEdgeOrientationConstraint(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  edge: string,
  orientation: 'horizontal' | 'vertical',
) {
  return invoke<ProjectSnapshot>('add_edge_orientation_constraint', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    edge,
    orientation,
  })
}

export function addGeometricConstraint(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  constraint: GeometricConstraintKind,
) {
  return invoke<ProjectSnapshot>('add_geometric_constraint', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    constraint,
  })
}

export function removeGeometricConstraint(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  constraint: string,
) {
  return invoke<ProjectSnapshot>('remove_geometric_constraint', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    constraint,
  })
}

export function addAnnotation(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  record: AnnotationRecordV1,
) {
  return invoke<ProjectSnapshot>('add_annotation', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    record,
  })
}

export function updateAnnotation(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  record: AnnotationRecordV1,
) {
  return invoke<ProjectSnapshot>('update_annotation', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    record,
  })
}

export function removeAnnotation(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
) {
  return invoke<ProjectSnapshot>('remove_annotation', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
  })
}

export function addUnderlay(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  record: UnderlayRecordV1,
) {
  return invoke<ProjectSnapshot>('add_underlay', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    record,
  })
}

export function updateUnderlay(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  record: UnderlayRecordV1,
) {
  return invoke<ProjectSnapshot>('update_underlay', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    record,
  })
}

export function removeUnderlay(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  id: string,
) {
  return invoke<ProjectSnapshot>('remove_underlay', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    id,
  })
}

export function importUnderlayImage(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  draft: Omit<UnderlayRecordV1, 'asset'>,
) {
  return invoke<ProjectSnapshot>('import_underlay_image', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    draft,
  })
}

export async function readUnderlayAssetDataUrl(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  asset: string,
) {
  if (!isCanonicalNonNilUuid(asset)) throw new Error('invalid underlay asset')
  const value = await invoke<unknown>('read_underlay_asset_data_url', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    asset,
  })
  const maximumLength = Math.ceil(16 * 1024 * 1024 / 3) * 4 + 32
  if (
    typeof value !== 'string'
    || value.length > maximumLength
    || !/^data:image\/(?:png|jpeg);base64,[A-Za-z0-9+/]+={0,2}$/u.test(value)
  ) throw new Error('invalid underlay asset response')
  return value
}

export function undo(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<ProjectSnapshot>('undo', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
}

export function redo(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<ProjectSnapshot>('redo', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
}

export function setCuttingAllowed(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  allowed: boolean,
) {
  return invoke<ProjectSnapshot>('set_cutting_allowed', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    allowed,
  })
}

export function updatePaperProperties(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  settings: PaperPropertySettings,
) {
  return invoke<ProjectSnapshot>('update_paper_properties', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    thicknessMm: settings.thicknessMm,
    frontColor: settings.frontColor,
    backColor: settings.backColor,
    frontTextureAsset: settings.frontTextureAsset,
    backTextureAsset: settings.backTextureAsset,
    cuttingAllowed: settings.cuttingAllowed,
  })
}

export function importFrontPaperTexture(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<ProjectSnapshot>('import_front_paper_texture', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
}

export function importBackPaperTexture(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<ProjectSnapshot>('import_back_paper_texture', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
}

export function setElementMetadata(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  target: ElementMetadataTarget,
  metadata: ElementMetadata | null,
) {
  return invoke<ProjectSnapshot>('set_element_metadata', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    target,
    metadata,
  })
}

export function setLengthDisplayUnit(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  unit: LengthDisplayUnit,
) {
  return invoke<ProjectSnapshot>('set_length_display_unit', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    unit,
  })
}

export function resizeRectangularPaper(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  widthExpression: string,
  heightExpression: string,
  widthMm: number,
  heightMm: number,
) {
  return invoke<ProjectSnapshot>('resize_rectangular_paper', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    widthExpression,
    heightExpression,
    widthMm,
    heightMm,
  })
}

export function splitBoundaryEdge(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  edge: string,
  fraction: number,
) {
  return invoke<ProjectSnapshot>('split_boundary_edge', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    edge,
    fraction,
  })
}

export function splitEdge(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  edge: string,
  fraction: number,
) {
  return invoke<ProjectSnapshot>('split_edge', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    edge,
    fraction,
  })
}

export function connectEdgeIntersection(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  firstEdge: string,
  secondEdge: string,
) {
  return invoke<EdgeIntersectionResponse>('connect_edge_intersection', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    firstEdge,
    secondEdge,
  })
}

export function connectIntersectionCluster(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  targets: readonly IntersectionClusterTarget[],
  junctionVertexId?: string,
) {
  return invoke<EdgeIntersectionResponse>('connect_intersection_cluster', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    targets,
    junctionVertexId: junctionVertexId ?? null,
  })
}

export function connectTJunction(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  firstEdge: string,
  secondEdge: string,
) {
  return invoke<EdgeIntersectionResponse>('connect_t_junction', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    firstEdge,
    secondEdge,
  })
}

const PROJECT_LAYER_MUTATION_SNAPSHOT_KEYS = [
  'project_instance_id',
  'project_id',
  'name',
  'memo',
  'beginner_design_profile',
  'current_path',
  'revision',
  'saved_revision',
  'is_dirty',
  'paper',
  'crease_pattern',
  'instruction_timeline',
  'numeric_expressions',
  'geometric_constraints',
  'project_layers',
  'element_metadata',
  'fold_model_fingerprint',
  'can_undo',
  'can_redo',
  'cutting_allowed',
] as const

function normalizeProjectLayerMutationBaseSnapshot(
  value: unknown,
): ProjectSnapshot | null {
  const record = exactCoreDataRecord(
    value,
    PROJECT_LAYER_MUTATION_SNAPSHOT_KEYS,
  )
  if (
    !record
    || !isCanonicalNonNilUuid(record.project_instance_id)
    || !isCanonicalNonNilUuid(record.project_id)
    || typeof record.name !== 'string'
    || typeof record.memo !== 'string'
    || (
      record.current_path !== null
      && typeof record.current_path !== 'string'
    )
    || !isProjectRevision(record.revision)
    || (
      record.saved_revision !== null
      && !isProjectRevision(record.saved_revision)
    )
    || typeof record.is_dirty !== 'boolean'
    || !isCoreDataRecord(record.paper)
    || !isCoreDataRecord(record.instruction_timeline)
    || !isCoreDataRecord(record.numeric_expressions)
    || !isCoreDataRecord(record.geometric_constraints)
    || !isCoreDataRecord(record.element_metadata)
    || typeof record.fold_model_fingerprint !== 'string'
    || !/^[0-9a-f]{64}$/u.test(record.fold_model_fingerprint)
    || typeof record.can_undo !== 'boolean'
    || typeof record.can_redo !== 'boolean'
    || typeof record.cutting_allowed !== 'boolean'
  ) return null

  const beginnerDesignProfile = normalizeBeginnerDesignProfile(
    record.beginner_design_profile,
  )
  const creasePattern = exactCoreDataRecord(
    record.crease_pattern,
    ['vertices', 'edges'] as const,
  )
  if (
    !beginnerDesignProfile
    || !creasePattern
    || !Array.isArray(creasePattern.vertices)
    || !Array.isArray(creasePattern.edges)
  ) return null
  const projectLayers = normalizeProjectLayerDocument(
    record.project_layers,
    creasePattern.edges as readonly Readonly<{ id: string }>[],
  )
  if (!projectLayers) return null

  return Object.freeze({
    project_instance_id: record.project_instance_id,
    project_id: record.project_id,
    name: record.name,
    memo: record.memo,
    beginner_design_profile: beginnerDesignProfile,
    current_path: record.current_path,
    revision: record.revision,
    saved_revision: record.saved_revision,
    is_dirty: record.is_dirty,
    paper: record.paper as ProjectSnapshot['paper'],
    crease_pattern:
      record.crease_pattern as ProjectSnapshot['crease_pattern'],
    instruction_timeline:
      record.instruction_timeline as ProjectSnapshot['instruction_timeline'],
    numeric_expressions:
      record.numeric_expressions as ProjectSnapshot['numeric_expressions'],
    geometric_constraints:
      record.geometric_constraints as ProjectSnapshot['geometric_constraints'],
    project_layers: projectLayers,
    element_metadata:
      record.element_metadata as ProjectSnapshot['element_metadata'],
    fold_model_fingerprint: record.fold_model_fingerprint,
    can_undo: record.can_undo,
    can_redo: record.can_redo,
    cutting_allowed: record.cutting_allowed,
  })
}

/**
 * Admits only the fields a layer command may change and merges them into the
 * already-admitted current snapshot. Unverified response geometry, paper,
 * timeline, constraints, and expression objects are deliberately ignored.
 */
export function normalizeProjectLayerMutationSnapshot(
  value: unknown,
  baseSnapshot: ProjectSnapshot,
): ProjectSnapshot | null {
  const base = normalizeProjectLayerMutationBaseSnapshot(baseSnapshot)
  const record = exactCoreDataRecord(
    value,
    PROJECT_LAYER_MUTATION_SNAPSHOT_KEYS,
  )
  if (
    !base
    || !record
    || record.project_instance_id !== base.project_instance_id
    || record.project_id !== base.project_id
    || record.name !== base.name
    || record.memo !== base.memo
    || !sameBeginnerDesignProfile(
      record.beginner_design_profile,
      base.beginner_design_profile,
    )
    || record.current_path !== base.current_path
    || !isProjectRevision(record.revision)
    || record.saved_revision !== base.saved_revision
    || typeof record.is_dirty !== 'boolean'
    || record.fold_model_fingerprint !== base.fold_model_fingerprint
    || typeof record.can_undo !== 'boolean'
    || typeof record.can_redo !== 'boolean'
    || record.cutting_allowed !== base.cutting_allowed
  ) return null

  const projectLayers = normalizeProjectLayerDocument(
    record.project_layers,
    base.crease_pattern.edges,
  )
  if (!projectLayers) return null

  return Object.freeze({
    project_instance_id: base.project_instance_id,
    project_id: base.project_id,
    name: base.name,
    memo: base.memo,
    beginner_design_profile: base.beginner_design_profile,
    current_path: base.current_path,
    revision: record.revision,
    saved_revision: base.saved_revision,
    is_dirty: record.is_dirty,
    paper: base.paper,
    crease_pattern: base.crease_pattern,
    instruction_timeline: base.instruction_timeline,
    numeric_expressions: base.numeric_expressions,
    geometric_constraints: base.geometric_constraints,
    project_layers: projectLayers,
    element_metadata: base.element_metadata,
    fold_model_fingerprint: base.fold_model_fingerprint,
    can_undo: record.can_undo,
    can_redo: record.can_redo,
    cutting_allowed: base.cutting_allowed,
  })
}

export function admitProjectLayerMutationSnapshot(
  value: unknown,
  baseSnapshot: ProjectSnapshot,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  previousRevision: number,
): ProjectSnapshot {
  if (
    !isProjectLayerMutationBaseSnapshot(
      baseSnapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      previousRevision,
    )
  ) throw new ProjectLayerMutationError('invalid_request')
  if (
    isStaleProjectLayerMutationResponse(
      value,
      expectedProjectInstanceId,
      expectedProjectId,
      previousRevision,
    )
  ) throw new ProjectLayerMutationError('stale_response')
  const snapshot = normalizeProjectLayerMutationSnapshot(value, baseSnapshot)
  if (!snapshot) throw new ProjectLayerMutationError('invalid_response')
  if (
    !isExpectedNativeEditSnapshot(
      snapshot,
      expectedProjectInstanceId,
      expectedProjectId,
      previousRevision,
    )
  ) throw new ProjectLayerMutationError('stale_response')
  return snapshot
}

function isStaleProjectLayerMutationResponse(
  value: unknown,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  previousRevision: number,
) {
  const record = snapshotCoreDataRecord(value)
  if (
    !record
    || !isCanonicalNonNilUuid(record.project_instance_id)
    || !isCanonicalNonNilUuid(record.project_id)
    || !isProjectRevision(record.revision)
  ) return false
  return record.project_instance_id !== expectedProjectInstanceId
    || record.project_id !== expectedProjectId
    || record.revision !== previousRevision + 1
}

function isProjectLayerMutationBaseSnapshot(
  value: unknown,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
): value is ProjectSnapshot {
  const snapshot = normalizeProjectLayerMutationBaseSnapshot(value)
  return snapshot !== null
    && snapshot.project_instance_id === expectedProjectInstanceId
    && snapshot.project_id === expectedProjectId
    && snapshot.revision === expectedRevision
}

function rejectProjectLayerMutation(
  code: ProjectLayerMutationErrorCode,
): Promise<never> {
  return Promise.reject(new ProjectLayerMutationError(code))
}

function isProjectLayerMutationBinding(
  expectedProjectInstanceId: unknown,
  expectedProjectId: unknown,
  expectedRevision: unknown,
): boolean {
  return isCanonicalNonNilUuid(expectedProjectInstanceId)
    && isCanonicalNonNilUuid(expectedProjectId)
    && isProjectRevision(expectedRevision)
    && expectedRevision < Number.MAX_SAFE_INTEGER
}

function isProjectRevision(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
    && !Object.is(value, -0)
}

function isNamedTechniqueTimelineProposalV1(
  value: unknown,
): value is NamedTechniqueTimelineProposalV1 {
  try {
    const record = exactCoreDataRecord(value, [
      'schema_version',
      'package_id',
      'technique_id',
      'technique_version',
      'steps',
    ] as const)
    if (
      !record
      || record.schema_version !== 1
      || !isNamedTechniqueIdentifier(record.package_id)
      || !isNamedTechniqueIdentifier(record.technique_id)
      || !Number.isSafeInteger(record.technique_version)
      || (record.technique_version as number) < 1
      || (record.technique_version as number) > 1_000_000
    ) return false
    const rawSteps = snapshotCoreDataArray(record.steps, 512)
    if (!rawSteps || rawSteps.length === 0) return false

    const rank = Object.freeze({
      technique: 0,
      parameter: 1,
      precondition: 2,
      operation: 3,
    }) satisfies Readonly<Record<NamedTechniqueTimelineSourceKindV1, number>>
    let previous:
      | Readonly<{
          kind: NamedTechniqueTimelineSourceKindV1
          id: string
          chunkIndex: number
          chunkCount: number
        }>
      | null = null
    const seen = new Set<string>()
    for (const rawStep of rawSteps) {
      const step = exactCoreDataRecord(rawStep, [
        'source_kind',
        'source_id',
        'chunk_index',
        'chunk_count',
        'title',
        'description',
        'caution',
        'duration_ms',
      ] as const)
      if (
        !step
        || typeof step.source_kind !== 'string'
        || !Object.hasOwn(rank, step.source_kind)
        || !isNamedTechniqueIdentifier(step.source_id)
        || !Number.isSafeInteger(step.chunk_index)
        || !Number.isSafeInteger(step.chunk_count)
        || (step.chunk_count as number) < 1
        || (step.chunk_count as number) > 512
        || (step.chunk_index as number) < 1
        || (step.chunk_index as number) > (step.chunk_count as number)
        || !isInstructionProposalTitle(step.title)
        || !isInstructionProposalText(step.description, 4_000)
        || !isInstructionProposalText(step.caution, 2_000)
        || !Number.isSafeInteger(step.duration_ms)
        || (step.duration_ms as number) < 100
        || (step.duration_ms as number) > 600_000
      ) return false
      const kind = step.source_kind as NamedTechniqueTimelineSourceKindV1
      const sourceId = step.source_id as string
      const chunkIndex = step.chunk_index as number
      const chunkCount = step.chunk_count as number
      if (
        (previous === null && kind !== 'technique')
        || (kind === 'technique' && sourceId !== record.technique_id)
      ) return false
      if (previous !== null && rank[kind] < rank[previous.kind]) return false
      if (
        previous !== null
        && previous.kind === kind
        && previous.id === sourceId
      ) {
        if (chunkIndex !== previous.chunkIndex + 1) return false
      } else {
        if (
          chunkIndex !== 1
          || (previous && previous.chunkIndex !== previous.chunkCount)
        ) return false
        const sourceKey = `${kind}\0${sourceId}`
        if (seen.has(sourceKey)) return false
        seen.add(sourceKey)
      }
      previous = { kind, id: sourceId, chunkIndex, chunkCount }
    }
    return previous !== null && previous.chunkIndex === previous.chunkCount
  } catch {
    return false
  }
}

function isNamedTechniqueIdentifier(value: unknown): value is string {
  return typeof value === 'string'
    && new TextEncoder().encode(value).length <= 96
    && /^[a-z](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u.test(value)
}

function isInstructionProposalTitle(value: unknown): value is string {
  return typeof value === 'string'
    && value.trim().length > 0
    && [...value].length <= 120
    && [...value].every((character) => {
      const code = character.codePointAt(0)
      return code !== undefined
        && !(code <= 0x1f || (code >= 0x7f && code <= 0x9f))
    })
}

function isInstructionProposalText(
  value: unknown,
  maximum: number,
): value is string {
  return typeof value === 'string'
    && [...value].length <= maximum
    && [...value].every((character) => {
      const code = character.codePointAt(0)
      return code !== undefined
        && (
          !(code <= 0x1f || (code >= 0x7f && code <= 0x9f))
          || character === '\n'
          || character === '\t'
        )
    })
}

function isCoreDataRecord(value: unknown): value is Record<string, unknown> {
  return snapshotCoreDataRecord(value) !== null
}

function exactCoreDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = snapshotCoreDataRecord(value)
  if (!record) return null
  const actualKeys = Object.keys(record)
  return actualKeys.length === expectedKeys.length
    && expectedKeys.every((key) => Object.hasOwn(record, key))
    ? record as Readonly<Record<Keys[number], unknown>>
    : null
}

function snapshotCoreDataRecord(
  value: unknown,
): Record<string, unknown> | null {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
    ) return null
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of Reflect.ownKeys(descriptors)) {
      if (typeof key !== 'string') return null
      const descriptor = descriptors[key]
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch {
    return null
  }
}

function snapshotCoreDataArray(
  value: unknown,
  maximumLength: number,
): readonly unknown[] | null {
  try {
    if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
      return null
    }
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const lengthDescriptor = Reflect.getOwnPropertyDescriptor(value, 'length')
    const lengthValue = lengthDescriptor && 'value' in lengthDescriptor
      ? lengthDescriptor.value
      : null
    if (
      typeof lengthValue !== 'number'
      || !Number.isSafeInteger(lengthValue)
      || lengthValue < 0
      || lengthValue > maximumLength
    ) return null
    const length = lengthValue
    const keys = Reflect.ownKeys(descriptors)
    if (
      keys.length !== length + 1
      || keys.some((key) =>
        typeof key !== 'string'
        || (
          key !== 'length'
          && (
            !/^(?:0|[1-9][0-9]*)$/u.test(key)
            || Number(key) >= length
          )
        ))
    ) return null
    const snapshot: unknown[] = []
    for (let index = 0; index < length; index += 1) {
      const descriptor = descriptors[String(index)]
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot.push(descriptor.value)
    }
    return Object.freeze(snapshot)
  } catch {
    return null
  }
}
