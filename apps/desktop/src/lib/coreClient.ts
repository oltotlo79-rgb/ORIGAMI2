import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
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
  type GeometricConstraintSatisfactionEvidenceKindV1,
  type GeometricConstraintSemanticMusV1,
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
  isCycleScheduleRequestV1,
  normalizeLiveHingeRegistryV1,
  normalizeStackedFoldReadRequest,
  normalizeStackedFoldReadResponse,
  type LiveHingeRegistryRequestV1,
  type LiveHingeRegistryResponseV1,
  type CycleScheduleRequestV1,
  type StackedFoldReadRequest,
  type StackedFoldReadResponse,
} from './stackedFoldRead.ts'
import {
  normalizeGeometricConstraintSolvePreview,
  type GeometricConstraintSolvePreview,
} from './geometricConstraintSolvePreview.ts'
import { snapshotStackedFoldReadWireValue } from './stackedFoldReadWireSnapshot.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from './deterministicTranscendentalModel.ts'
import {
  normalizeBoundaryLengthAuthorityV1,
  type BoundaryLengthAuthorityV1,
} from './boundaryLengthAuthority.ts'
import type {
  UnprovenHistoryStatusCountsView,
} from './proofProgressModel.ts'
import {
  unprovenHistorySummaryFromSnapshotV1,
} from './speculativeUnprovenWire.ts'
import { resolveCompleteAnimalBindings } from './completeAnimalBindings.ts'
import {
  beginnerGeneratedPlanInstructionsAreCanonicalV1,
  beginnerGeneratedPlanSizeIsAdmissibleV1,
  beginnerGeneratedPlanTargetPartsAreCompatibleV1,
  beginnerTargetPartRecordCountIsAdmissibleV1,
  MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1,
  MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
  MAX_BEGINNER_GENERIC_PLAN_EDGES_V1,
  MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1,
  MAX_BEGINNER_TARGET_PART_RECORDS_V1,
  MIN_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1,
  type BeginnerGeneratedPlanInstructionContextV1,
  type BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanContract.ts'
import {
  normalizeBeginnerGridEvaluationResponseV1,
} from './beginnerGridResponse.ts'
import {
  beginnerGeneratedPlanTopologyMatchesProfileV1,
} from './beginnerGeneratedPlanTopologyContract.ts'
import {
  beginnerExpectedTargetApproximationScoreV1,
  beginnerReferenceConsensusPairDigestV1,
} from './beginnerCandidateScoreContract.ts'
export {
  beginnerGeneratedPlanInstructionsAreCanonicalV1,
  beginnerGeneratedPlanSizeIsAdmissibleV1,
  beginnerGeneratedPlanTargetPartsAreCompatibleV1,
  beginnerGenericFeatureBindingIdentityIsCanonicalV1,
  beginnerTargetPartRecordCountIsAdmissibleV1,
  MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
  MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1,
  MAX_BEGINNER_GENERIC_PLAN_EDGES_V1,
  MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1,
  MAX_BEGINNER_SPECIALIZED_PLAN_EDGES_V1,
  MAX_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1,
  MAX_BEGINNER_TARGET_PART_RECORDS_V1,
  MIN_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1,
  type BeginnerGeneratedPlanInstructionContextV1,
  type BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanContract.ts'
export {
  applySpeculativeStackedFoldTransaction,
  normalizeSpeculativeStackedFoldApplyRequestV1,
  type SpeculativeStackedFoldApplyRequestV1,
} from './speculativeStackedFoldClient.ts'

export type CurrentCyclePosePreviewRequestV1 = Readonly<{
  progressRequestId?: string
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  cycleScheduleV1: CycleScheduleRequestV1 | Readonly<{
    version: 2
    entries: readonly []
    endpointDenominator?: 1 | 2 | 4 | 8 | 16
  }>
}>

export type CurrentCyclePosePreviewResponseV1 = Readonly<{
  version: 1
  transactionToken: string
  sourceRevision: number
  targetRevision: number
  closureLeafCount: number
  closureMaxDepth: number
  checkedHingeCount: number
  totalHingeCount: number
  continuousPathCertified: true
  continuousLayerTransportModelId:
    | 'general_multi_face_positive_thickness_cell_transport_v1'
    | 'blockwise_positive_layer_authority_v1'
    | 'common_articulation_continuous_layer_path_authority_v1'
    | null
  continuousLayerTransitionCount: number
  continuousLayerPairOrderCount: number
  continuousLayerTargetOrderSha256: string | null
  sourceLayerOrder: readonly Readonly<{ lowerFace: string; upperFace: string }>[]
  targetLayerOrder: readonly Readonly<{ lowerFace: string; upperFace: string }>[]
  authorizesProjectMutation: false
}>

export type CurrentCyclePoseProgressV1 = Readonly<{
  version: 1
  requestId: string
  status: 'running' | 'certified' | 'cancelled' | 'failed'
  completedWork: number
  totalWork: 2
  authorizesProjectMutation: false
}>
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
export {
  normalizeGeometricConstraintSolvePreview,
} from './geometricConstraintSolvePreview.ts'
export type {
  GeometricConstraintSolvePreview,
} from './geometricConstraintSolvePreview.ts'

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
export type GeometricConstraintDocument = GeometricConstraintDocumentV1
export type GeometricConstraintPreflightResult = GeometricConstraintPreflightResultV1
export type GeometricConstraintPreflightResponse = GeometricConstraintPreflightResponseV1
export type GeometricConstraintSatisfactionEvidenceKind =
  GeometricConstraintSatisfactionEvidenceKindV1
export type GeometricConstraintSemanticMus = GeometricConstraintSemanticMusV1

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
  boundary_length_authority_v1?: unknown
  reference_model_assets?: Array<{ asset_id: string; sha256: number[] }>
  speculativeUnprovenFolds?: unknown
}

export type ProjectOccGuard = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
}>

const INVALID_PROJECT_OCC_GUARD_FIELD = Symbol('invalid project OCC guard field')

function ownDataField(
  value: unknown,
  key: PropertyKey,
): unknown | typeof INVALID_PROJECT_OCC_GUARD_FIELD {
  if (
    value === null
    || (typeof value !== 'object' && typeof value !== 'function')
  ) return INVALID_PROJECT_OCC_GUARD_FIELD
  try {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    return descriptor && 'value' in descriptor
      ? descriptor.value
      : INVALID_PROJECT_OCC_GUARD_FIELD
  } catch {
    return INVALID_PROJECT_OCC_GUARD_FIELD
  }
}

function projectOccGuardField(
  guard: unknown,
  key: keyof ProjectOccGuard,
): unknown | typeof INVALID_PROJECT_OCC_GUARD_FIELD {
  return ownDataField(guard, key)
}

export function matchesProjectOccGuard(
  guard: ProjectOccGuard,
  project: Readonly<{
    project_instance_id: unknown
    project_id: unknown
    revision: unknown
  }>,
): boolean {
  const expectedProjectInstanceId = projectOccGuardField(
    guard,
    'expectedProjectInstanceId',
  )
  if (
    expectedProjectInstanceId === INVALID_PROJECT_OCC_GUARD_FIELD
    || ownDataField(project, 'project_instance_id') !== expectedProjectInstanceId
  ) return false
  const expectedProjectId = projectOccGuardField(guard, 'expectedProjectId')
  if (
    expectedProjectId === INVALID_PROJECT_OCC_GUARD_FIELD
    || ownDataField(project, 'project_id') !== expectedProjectId
  ) return false
  const expectedRevision = projectOccGuardField(guard, 'expectedRevision')
  return expectedRevision !== INVALID_PROJECT_OCC_GUARD_FIELD
    && ownDataField(project, 'revision') === expectedRevision
}

export type BeginnerDesignProfileV1 = {
  schema_version: 1
  preset: 'balanced' | 'shape_priority' | 'foldability_priority'
  shape_fidelity_weight: number
  foldability_weight: number
  step_count_weight: number
  paper_efficiency_weight: number
  generation_constraints: BeginnerGenerationConstraintsV1
  generation_provenance?: Readonly<{
    schema_version: 1; topology_authority_sha256: ReadonlyArray<number>
    fold_path_certificate_sha256?: ReadonlyArray<number>
    document_authority_sha256?: ReadonlyArray<number>; confidence_score: number
    confidence_reasons: ReadonlyArray<string>; explicit_override: boolean; source_asset_fingerprint: string
    semantic_landmark_provenance?: Readonly<{
      schema_version: 1
      ordered_bindings: ReadonlyArray<Readonly<{
        ordinal: number; role: string; physical_ray: number
      }>>
      physical_ray_group_sha256: ReadonlyArray<ReadonlyArray<number>>
    }>
    generic_tree?: Readonly<{
      schema_version: 1; source: 'image_silhouette' | 'glb_geometry' | 'manual_skeleton'
      target_category?: 'custom_object'
      asset_content_sha256?: ReadonlyArray<number>; tree_topology_sha256: ReadonlyArray<number>
      normalized_length_ratios: ReadonlyArray<number>; orientation: 'horizontal' | 'vertical'
      generator_version: 1; authorizes_apply: false
      instruction_proposal?: Readonly<{ schema_version: 1; topology_sha256: ReadonlyArray<number>
        generator_version: 1; authorizes_apply: false; physical_motion_proof: false
        steps: ReadonlyArray<Readonly<{ canonical_crease_id: string; tree_depth: number
          assignment: 'mountain' | 'valley'; target_branch: string; fixed_side: 'root' | 'leaf'; caution: string }>> }>
    }>
    reference_consensus?: Readonly<{
      schema_version: 1; source_revision: number
      bindings: NonNullable<BeginnerDesignProfileV1['reference_consensus_v1']>['bindings']
      excluded_asset_id?: string; pair_digests_sha256: ReadonlyArray<ReadonlyArray<number>>
      summary: Readonly<{ schema_version: 1; model: 'component_extent_branch_v1'; source_count: number
        excluded_count: number; agreement_score: number; component_subscore: number; extent_subscore: number; branch_subscore: number }>
    }>
    reference_consensus_summary?: Readonly<{ schema_version: 1; model: 'component_extent_branch_v1'; source_count: number
      excluded_count: number; agreement_score: number; component_subscore: number; extent_subscore: number; branch_subscore: number }>
  }>
  reference_surface_landmarks_tenths_mm?: ReadonlyArray<readonly [number, number, number]>
  outline_edit_authority?: Readonly<{
    schema_version: 1; source_asset_id: string; source_sha256: ReadonlyArray<number>
    edits: ReadonlyArray<Readonly<Record<string, unknown>>>
  }>
  archived_reference_model_asset_ids?: ReadonlyArray<string>
  reference_consensus_v1?: Readonly<{
    schema_version: 1
    bindings: ReadonlyArray<Readonly<{
      kind: 'image' | 'reference_model'; asset_id: string; sha256: ReadonlyArray<number>; quality: number
    }>>
    excluded_asset_id?: string
  }>
}

export type BeginnerGenerationConstraintsV1 = {
  schema_version: 1
  maximum_steps: number
  detail_level: 'simple' | 'standard' | 'detailed'
  generic_body_size_tenths_mm?: [number, number]
  generic_body_outline_tenths_mm?: Array<[number, number]>
  generic_body_outline_mode?: 'symmetric' | 'general'
  target_category: 'animal' | 'insect' | 'custom_object' | null
  custom_object_display_name?: string
  target_parts: Array<{
    kind: 'head' | 'torso' | 'leg' | 'horn' | 'ear' | 'wing' | 'fin' | 'antenna' | 'tail'
    count: number
  }>
  skeleton_segments: Array<{
    id: number
    start: { x_tenths_mm: number; y_tenths_mm: number }
    end: { x_tenths_mm: number; y_tenths_mm: number }
    thickness_tenths_mm: number
  }>
  component_bridge_override?: {
    schema_version: 1; source_asset_sha256: number[]; component_count: number; reviewed: boolean
    bridges: Array<{ id: number; start_component_id: number; end_component_id: number; accepted: boolean }>
  }
  silhouette_thresholds?: { schema_version: 1; alpha: number; luma: number; polarity: 'dark_on_light' | 'light_on_dark' | 'alpha_only' }
  silhouette_crop_roi?: { schema_version: 1; x_millionths: number; y_millionths: number; width_millionths: number; height_millionths: number }
  silhouette_orientation_degrees?: 0 | 90 | 180 | 270
  silhouette_mirror?: { schema_version: 1; mirror_x: boolean; mirror_y: boolean }
  protrusions?: Array<{
    id: number
    count: number
    length_tenths_mm: number
    thickness_tenths_mm: number
    root_width_tenths_mm?: number
    tip_width_tenths_mm?: number
    local_outline_tenths_mm?: Array<[number, number]>
    position_tenths_mm: [number, number, number]
    direction_milli: [number, number, number]
    symmetry: 'none' | 'bilateral' | 'radial'
    curvature_degrees: number
    joint: 'fixed' | 'hinge' | 'ball'
    motion_degrees: [number, number]
    side: 'front' | 'back' | 'either'
    priority: number
  }>
  bulge_targets?: Array<{
    id: number
    face_ids: string[]
    range_min_tenths_mm: [number, number, number]
    range_max_tenths_mm: [number, number, number]
    direction_milli: [number, number, number]
    amount_tenths_mm: number
    source_fold_model_fingerprint: string
    reference_surface_binding?: {
      asset_id: string
      range_id: number
      protrusion_id: number
      triangle_indices: number[]
      range_digest_sha256: number[]
    }
  }>
  target_asset: {
    kind: 'reference_image'
    underlay_id: string
    asset_id: string
  } | {
    kind: 'reference_model'
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

export type BeginnerRecognitionProposalV1 = {
  schema_version: 1
  format: 'marker_png_v1' | 'silhouette_png_v1'
  source_underlay_id: string
  source_asset_id: string
  source_sha256: readonly number[]
  width: number
  height: number
  shape_bounds: {
    min_x: number
    min_y: number
    max_x: number
    max_y: number
  }
  target_parts: BeginnerGenerationConstraintsV1['target_parts']
  skeleton_segments: BeginnerGenerationConstraintsV1['skeleton_segments']
  generic_body_outline_tenths_mm?: Array<[number, number]>
  generic_body_outline_mode?: 'symmetric' | 'general'
  protrusions?: BeginnerGenerationConstraintsV1['protrusions']
  contour_confidence?: Readonly<{
    body_score: number; body_reasons: ReadonlyArray<string>
    local_scores: ReadonlyArray<Readonly<{ protrusion_id: number; score: number; reasons: ReadonlyArray<string> }>>
    explicit_override_required: boolean
  }>
  skeleton_quality?: Readonly<{
    score: number
    reasons: ReadonlyArray<string>
    insufficiency_reasons: ReadonlyArray<string>
    distance_metric: 'manhattan_pixel_v1' | 'aabb_squared_distance_v1'
    bar_limit: 16 | 32
  }>
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

function isBoundedIntegerTuple(
  value: unknown,
  length: number,
  absoluteMaximum: number,
): value is number[] {
  const snapshot = snapshotCoreDataArray(value, length)
  return snapshot?.length === length
    && snapshot.every((item) =>
      Number.isInteger(item)
      && Math.abs(Number(item)) <= absoluteMaximum)
}

function isI32Tuple(
  value: unknown,
  length: number,
): value is number[] {
  const snapshot = snapshotCoreDataArray(value, length)
  return snapshot?.length === length
    && snapshot.every((item) =>
      Number.isInteger(item)
      && Number(item) >= -2_147_483_648
      && Number(item) <= 2_147_483_647)
}

function snapshotSha256Bytes(value: unknown): ReadonlyArray<number> | null {
  const snapshot = snapshotCoreDataArray(value, 32)
  if (
    snapshot?.length !== 32
    || snapshot.some((byte) =>
      !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255)
  ) return null
  return Object.freeze(snapshot.map(Number))
}

function isNonEmptyUtf8StringWithin(
  value: unknown,
  maximumBytes: number,
): value is string {
  if (typeof value !== 'string' || value.length === 0) return false
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false
      index += 1
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return false
    }
  }
  try {
    return new TextEncoder().encode(value).byteLength <= maximumBytes
  } catch {
    return false
  }
}

function compareUtf8Strings(left: string, right: string): number {
  const leftBytes = new TextEncoder().encode(left)
  const rightBytes = new TextEncoder().encode(right)
  const sharedLength = Math.min(leftBytes.length, rightBytes.length)
  for (let index = 0; index < sharedLength; index += 1) {
    if (leftBytes[index] !== rightBytes[index]) {
      return Number(leftBytes[index]) - Number(rightBytes[index])
    }
  }
  return leftBytes.length - rightBytes.length
}

function isCanonicalGenericBodyOutline(
  value: unknown, mode: 'symmetric' | 'symmetric_ccw' | 'general', minimum = 4, maximum = 16,
  coordinateMaximum = 100_000,
): value is Array<[number, number]> {
  const snapshot = snapshotCoreDataArray(value, maximum)
  if (!snapshot || snapshot.length < minimum
    || snapshot.some((point) =>
      !isBoundedIntegerTuple(point, 2, coordinateMaximum))) return false
  const points = snapshot as Array<[number, number]>
  const keys = points.map(([x, y]) => `${x},${y}`)
  if (new Set(keys).size !== points.length
    || keys[0] !== [...keys].sort((left, right) => {
      const [lx, ly] = left.split(',').map(Number)
      const [rx, ry] = right.split(',').map(Number)
      return lx - rx || ly - ry
    })[0]
    || (mode !== 'general' && points.some(([x, y]) => !keys.includes(`${-x},${y}`)))) return false
  const area = points.reduce((sum, [x, y], index) => {
    const next = points[(index + 1) % points.length]!
    return sum + x * next[1] - next[0] * y
  }, 0)
  if (!Number.isSafeInteger(area) || (mode === 'symmetric' ? area >= 0 : area <= 0)) return false
  const orient = (a: [number, number], b: [number, number], c: [number, number]) =>
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
  for (let first = 0; first < points.length; first += 1) {
    const firstEnd = (first + 1) % points.length
    for (let second = first + 1; second < points.length; second += 1) {
      const secondEnd = (second + 1) % points.length
      if (first === secondEnd || firstEnd === second) continue
      const values = [
        orient(points[first]!, points[firstEnd]!, points[second]!),
        orient(points[first]!, points[firstEnd]!, points[secondEnd]!),
        orient(points[second]!, points[secondEnd]!, points[first]!),
        orient(points[second]!, points[secondEnd]!, points[firstEnd]!),
      ]
      if (values.some((item) => item === 0)
        || (Math.sign(values[0]!) !== Math.sign(values[1]!)
          && Math.sign(values[2]!) !== Math.sign(values[3]!))) return false
    }
  }
  return true
}

export function normalizeCustomObjectDisplayName(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim().normalize('NFC')
  const scalarCount = Array.from(normalized).length
  if (scalarCount < 1 || scalarCount > 64 || /[\\/\p{Cc}\u202A-\u202E\u2066-\u2069]/u.test(normalized)) return null
  return normalized
}

function isCustomObjectDisplayName(value: unknown): value is string {
  return typeof value === 'string' && normalizeCustomObjectDisplayName(value) === value
}

export function normalizeBeginnerGenerationConstraints(
  value: unknown,
  options: Readonly<{ requireCanonicalGenericIds?: boolean }> = {},
): BeginnerGenerationConstraintsV1 | null {
  const currentKeys = [
    'schema_version',
    'maximum_steps',
    'detail_level',
    'generic_body_size_tenths_mm',
    'generic_body_outline_tenths_mm',
    'generic_body_outline_mode',
    'target_category', 'custom_object_display_name',
    'target_parts',
    'skeleton_segments',
    'component_bridge_override',
    'silhouette_thresholds',
    'silhouette_crop_roi',
    'silhouette_orientation_degrees',
    'silhouette_mirror',
    'protrusions',
    'bulge_targets',
    'target_asset',
    'allowed_techniques',
  ] as const
  const requiredKeys = currentKeys.filter(
    (key) => key !== 'generic_body_size_tenths_mm'
      && key !== 'generic_body_outline_tenths_mm' && key !== 'generic_body_outline_mode'
      && key !== 'custom_object_display_name'
      && key !== 'component_bridge_override'
      && key !== 'silhouette_thresholds'
      && key !== 'silhouette_crop_roi'
      && key !== 'silhouette_orientation_degrees'
      && key !== 'silhouette_mirror'
      && key !== 'protrusions' && key !== 'bulge_targets',
  )
  const snapshot = snapshotCoreDataRecord(value)
  if (!snapshot) return null
  const hadProtrusions = Object.hasOwn(snapshot, 'protrusions')
  const hadBulgeTargets = Object.hasOwn(snapshot, 'bulge_targets')
  const actualKeys = Object.keys(snapshot)
  if (actualKeys.some((key) => !currentKeys.includes(key as typeof currentKeys[number]))
    || requiredKeys.some((key) => !Object.hasOwn(snapshot, key))) {
    return null
  }
  const targetPartsInput = snapshotCoreDataArray(
    snapshot.target_parts,
    MAX_BEGINNER_TARGET_PART_RECORDS_V1,
  )
  const skeletonSegmentsInput = snapshotCoreDataArray(
    snapshot.skeleton_segments,
    64,
  )
  const protrusionsInput = snapshotCoreDataArray(
    Object.hasOwn(snapshot, 'protrusions') ? snapshot.protrusions : [],
    32,
  )
  const bulgeTargetsInput = snapshotCoreDataArray(
    Object.hasOwn(snapshot, 'bulge_targets') ? snapshot.bulge_targets : [],
    32,
  )
  const allowedTechniquesInput = snapshotCoreDataArray(
    snapshot.allowed_techniques,
    8,
  )
  if (
    !targetPartsInput
    || !skeletonSegmentsInput
    || !protrusionsInput
    || !bulgeTargetsInput
    || !allowedTechniquesInput
  ) return null
  const record: Record<string, unknown> = {
    ...snapshot,
    target_parts: targetPartsInput,
    skeleton_segments: skeletonSegmentsInput,
    protrusions: protrusionsInput,
    bulge_targets: bulgeTargetsInput,
    allowed_techniques: allowedTechniquesInput,
  }
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
      && record.target_category !== 'insect'
      && record.target_category !== 'custom_object')
    || (record.custom_object_display_name !== undefined
      && (record.target_category !== 'custom_object'
        || !isCustomObjectDisplayName(record.custom_object_display_name)))
    || !Array.isArray(record.target_parts)
    || !beginnerTargetPartRecordCountIsAdmissibleV1(record.target_parts)
    || (record.generic_body_size_tenths_mm !== undefined
      && (!isBoundedIntegerTuple(record.generic_body_size_tenths_mm, 2, 1_000_000)
        || record.generic_body_size_tenths_mm.some((axis) => axis < 1)))
    || (record.generic_body_outline_tenths_mm !== undefined
      && !isCanonicalGenericBodyOutline(record.generic_body_outline_tenths_mm,
        record.generic_body_outline_mode === 'general' ? 'general' : 'symmetric'))
    || (record.generic_body_outline_mode !== undefined
      && record.generic_body_outline_mode !== 'symmetric'
      && record.generic_body_outline_mode !== 'general')
    || !Array.isArray(record.skeleton_segments)
    || record.skeleton_segments.length > 64
    || !Array.isArray(record.protrusions)
    || record.protrusions.length > 32
    || !Array.isArray(record.bulge_targets) || record.bulge_targets.length > 32
    || !Array.isArray(record.allowed_techniques)
    || record.allowed_techniques.length < 1
    || record.allowed_techniques.length > 8
    || record.allowed_techniques.some((technique) => !BEGINNER_TECHNIQUES.includes(technique))
    || new Set(record.allowed_techniques).size !== record.allowed_techniques.length
  ) return null
  let partTotal = 0
  const targetParts = record.target_parts.map((part) => {
    const item = exactCoreDataRecord(part, ['kind', 'count'] as const)
    if (
      !item
      || !['head', 'torso', 'leg', 'horn', 'ear', 'wing', 'fin', 'antenna', 'tail'].includes(String(item.kind))
      || !Number.isInteger(item.count)
      || Number(item.count) < 1
      || Number(item.count) > 8
    ) return null
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
  const protrusionIds = new Set<number>()
  const protrusions = record.protrusions.map((value) => {
    const oldKeys = [
      'id', 'count', 'length_tenths_mm', 'thickness_tenths_mm',
      'position_tenths_mm', 'direction_milli', 'symmetry', 'curvature_degrees',
      'joint', 'motion_degrees', 'side', 'priority',
    ] as const
    const newKeys = [...oldKeys, 'root_width_tenths_mm', 'tip_width_tenths_mm',
      'local_outline_tenths_mm'] as const
    const snapshot = snapshotCoreDataRecord(value)
    const item = snapshot && Object.keys(snapshot).every((key) => newKeys.includes(key as typeof newKeys[number]))
      && oldKeys.every((key) => Object.hasOwn(snapshot, key)) ? snapshot : null
    const position = item
      ? snapshotCoreDataArray(item.position_tenths_mm, 3)
      : null
    const direction = item
      ? snapshotCoreDataArray(item.direction_milli, 3)
      : null
    const motion = item
      ? snapshotCoreDataArray(item.motion_degrees, 2)
      : null
    const hasLocalOutline =
      item?.local_outline_tenths_mm !== undefined
    const localOutlineInputs = hasLocalOutline
      ? snapshotCoreDataArray(item.local_outline_tenths_mm, 8)
      : null
    const localOutline = localOutlineInputs?.map((point) => {
      const coordinates = snapshotCoreDataArray(point, 2)
      return coordinates?.length === 2
        ? [Number(coordinates[0]), Number(coordinates[1])] as [
            number,
            number,
          ]
        : null
    }) ?? null
    if (!item || !Number.isInteger(item.id) || Number(item.id) < 0
      || Number(item.id) > 65_535
      || protrusionIds.has(Number(item.id))
      || !Number.isInteger(item.count) || Number(item.count) < 1 || Number(item.count) > 8
      || !Number.isInteger(item.length_tenths_mm) || Number(item.length_tenths_mm) < 1
      || Number(item.length_tenths_mm) > 1_000_000
      || !Number.isInteger(item.thickness_tenths_mm) || Number(item.thickness_tenths_mm) < 1
      || Number(item.thickness_tenths_mm) > 10_000
      || (item.root_width_tenths_mm !== undefined
        && (!Number.isInteger(item.root_width_tenths_mm)
          || Number(item.root_width_tenths_mm) < 1 || Number(item.root_width_tenths_mm) > 10_000))
      || (item.tip_width_tenths_mm !== undefined
        && (!Number.isInteger(item.tip_width_tenths_mm)
          || Number(item.tip_width_tenths_mm) < 1 || Number(item.tip_width_tenths_mm) > 10_000))
      || (hasLocalOutline
        && (
          localOutlineInputs === null
          || localOutline === null
          || localOutline.some((point) => point === null)
          || !isCanonicalGenericBodyOutline(localOutline,
          item.symmetry === 'bilateral' ? 'symmetric_ccw' : 'general', 3, 8, 10_000))
        )
      || !isBoundedIntegerTuple(position, 3, 100_000)
      || !isBoundedIntegerTuple(direction, 3, 1_000)
      || direction.every((axis) => axis === 0)
      || !['none', 'bilateral', 'radial'].includes(String(item.symmetry))
      || !Number.isInteger(item.curvature_degrees) || Math.abs(Number(item.curvature_degrees)) > 360
      || !['fixed', 'hinge', 'ball'].includes(String(item.joint))
      || !isBoundedIntegerTuple(motion, 2, 360)
      || motion[0] > motion[1]
      || !['front', 'back', 'either'].includes(String(item.side))
      || !Number.isInteger(item.priority) || Number(item.priority) < 1 || Number(item.priority) > 100
    ) return null
    protrusionIds.add(Number(item.id))
    return Object.freeze({
      id: Number(item.id),
      count: Number(item.count),
      length_tenths_mm: Number(item.length_tenths_mm),
      thickness_tenths_mm: Number(item.thickness_tenths_mm),
      ...(item.root_width_tenths_mm === undefined ? {} : {
        root_width_tenths_mm: Number(item.root_width_tenths_mm),
      }),
      ...(item.tip_width_tenths_mm === undefined ? {} : {
        tip_width_tenths_mm: Number(item.tip_width_tenths_mm),
      }),
      ...(localOutline === null ? {} : {
        local_outline_tenths_mm: Object.freeze(
          localOutline.map((point) => Object.freeze(point!)),
        ),
      }),
      position_tenths_mm: Object.freeze(position.map(Number)),
      direction_milli: Object.freeze(direction.map(Number)),
      symmetry: item.symmetry,
      curvature_degrees: Number(item.curvature_degrees),
      joint: item.joint,
      motion_degrees: Object.freeze(motion.map(Number)),
      side: item.side,
      priority: Number(item.priority),
    }) as NonNullable<
      BeginnerGenerationConstraintsV1['protrusions']
    >[number]
  })
  if (protrusions.some((target) => target === null)) return null
  const validProtrusions =
    protrusions as NonNullable<BeginnerGenerationConstraintsV1['protrusions']>
  if (options.requireCanonicalGenericIds
    && (skeletonSegments.some((segment, index) => index > 0
      && Number(skeletonSegments[index - 1]?.id) >= Number(segment?.id))
      || validProtrusions.some((target, index) => index > 0
        && Number(validProtrusions[index - 1]?.id) >= Number(target.id)))) {
    return null
  }
  const completeAnimal = record.target_category === 'animal'
    && targetParts.some((part) => part?.kind === 'horn' && part.count === 1)
    && targetParts.some((part) => part?.kind === 'tail' && part.count === 1)
    && targetParts.some((part) => part?.kind === 'ear' && part.count === 2)
    && targetParts.some((part) => part?.kind === 'leg' && part.count === 4)
  const animalWingParts = targetParts.filter((part) => part?.kind === 'wing')
  const completeAnimalHasWings = animalWingParts.length === 1 && animalWingParts[0]?.count === 2
  if (completeAnimal && (animalWingParts.length > 1
    || (animalWingParts.length === 1 && !completeAnimalHasWings)
    || (validProtrusions.length > 0
      && resolveCompleteAnimalBindings(validProtrusions, completeAnimalHasWings) === null))) return null
  const bulgeIds = new Set<number>()
  const bulgeTargets = record.bulge_targets.map((value) => {
    const item = exactCoreDataRecord(value, [
      'id', 'face_ids', 'range_min_tenths_mm', 'range_max_tenths_mm',
      'direction_milli', 'amount_tenths_mm', 'source_fold_model_fingerprint',
      'reference_surface_binding',
    ] as const)
    const faceIds = item
      ? snapshotCoreDataArray(item.face_ids, 32)
      : null
    const minimum = item
      ? snapshotCoreDataArray(item.range_min_tenths_mm, 3)
      : null
    const maximum = item
      ? snapshotCoreDataArray(item.range_max_tenths_mm, 3)
      : null
    const direction = item
      ? snapshotCoreDataArray(item.direction_milli, 3)
      : null
    if (!item || !Number.isInteger(item.id) || Number(item.id) < 0 || bulgeIds.has(Number(item.id))
      || !faceIds || faceIds.length < 1
      || faceIds.some((id) => !isCanonicalNonNilUuid(id))
      || new Set(faceIds).size !== faceIds.length
      || !isBoundedIntegerTuple(minimum, 3, 100_000)
      || !isBoundedIntegerTuple(maximum, 3, 100_000)
      || !isBoundedIntegerTuple(direction, 3, 1_000)
      || !Number.isInteger(item.amount_tenths_mm) || Number(item.amount_tenths_mm) < 1
      || Number(item.amount_tenths_mm) > 1_000_000
      || typeof item.source_fold_model_fingerprint !== 'string'
      || !/^[0-9a-f]{64}$/u.test(item.source_fold_model_fingerprint)) return null
    const surface = item.reference_surface_binding === undefined ? null
      : exactCoreDataRecord(item.reference_surface_binding, [
          'asset_id', 'range_id', 'protrusion_id', 'triangle_indices', 'range_digest_sha256',
        ] as const)
    const surfaceTriangleIndices = surface
      ? snapshotCoreDataArray(surface.triangle_indices, 40_000)
      : null
    const surfaceRangeDigest = surface
      ? snapshotSha256Bytes(surface.range_digest_sha256)
      : null
    if (item.reference_surface_binding !== undefined && (!surface
      || !isCanonicalNonNilUuid(surface.asset_id)
      || !Number.isInteger(surface.range_id) || Number(surface.range_id) < 1
      || !Number.isInteger(surface.protrusion_id) || Number(surface.protrusion_id) < 1
      || !surfaceTriangleIndices
      || surfaceTriangleIndices.length < 1
      || surfaceTriangleIndices.some((triangle) =>
        !Number.isInteger(triangle) || Number(triangle) < 0)
      || new Set(surfaceTriangleIndices).size
        !== surfaceTriangleIndices.length
      || !surfaceRangeDigest)) return null
    if (minimum.some((value, index) => value > maximum[index])
      || minimum.every((value, index) => value === maximum[index])
      || direction.every((axis) => axis === 0)) return null
    bulgeIds.add(Number(item.id))
    return Object.freeze({
      id: Number(item.id),
      face_ids: Object.freeze(faceIds.map(String)),
      range_min_tenths_mm: Object.freeze(minimum.map(Number)),
      range_max_tenths_mm: Object.freeze(maximum.map(Number)),
      direction_milli: Object.freeze(direction.map(Number)),
      amount_tenths_mm: Number(item.amount_tenths_mm),
      source_fold_model_fingerprint:
        String(item.source_fold_model_fingerprint),
      ...(surface === null ? {} : {
        reference_surface_binding: Object.freeze({
          asset_id: String(surface.asset_id),
          range_id: Number(surface.range_id),
          protrusion_id: Number(surface.protrusion_id),
          triangle_indices: Object.freeze(
            surfaceTriangleIndices!.map(Number),
          ),
          range_digest_sha256: surfaceRangeDigest!,
        }),
      }),
    }) as NonNullable<
      BeginnerGenerationConstraintsV1['bulge_targets']
    >[number]
  })
  if (bulgeTargets.some((target) => target === null)) return null
  let targetAsset: BeginnerGenerationConstraintsV1['target_asset'] = null
  if (record.target_asset !== null) {
    const candidate = isCoreDataRecord(record.target_asset) ? record.target_asset : null
    if (candidate?.kind === 'reference_image') {
      const asset = exactCoreDataRecord(candidate, ['kind', 'underlay_id', 'asset_id'] as const)
      if (!asset || !isCanonicalNonNilUuid(asset.underlay_id)
        || !isCanonicalNonNilUuid(asset.asset_id)) return null
      targetAsset = Object.freeze({
        kind: 'reference_image',
        underlay_id: asset.underlay_id,
        asset_id: asset.asset_id,
      })
    } else {
      const asset = exactCoreDataRecord(candidate, ['kind', 'asset_id'] as const)
      if (!asset || asset.kind !== 'reference_model'
        || !isCanonicalNonNilUuid(asset.asset_id)) return null
      targetAsset = Object.freeze({
        kind: 'reference_model',
        asset_id: asset.asset_id,
      })
    }
  }
  let componentBridgeOverride: BeginnerGenerationConstraintsV1['component_bridge_override']
  if (record.component_bridge_override !== undefined) {
    const document = exactCoreDataRecord(record.component_bridge_override, [
      'schema_version', 'source_asset_sha256', 'component_count', 'reviewed', 'bridges',
    ] as const)
    const sourceAssetDigest = document
      ? snapshotSha256Bytes(document.source_asset_sha256)
      : null
    if (!document || document.schema_version !== 1 || !sourceAssetDigest
      || !Number.isInteger(document.component_count) || Number(document.component_count) < 2 || Number(document.component_count) > 8
      || typeof document.reviewed !== 'boolean' || !Array.isArray(document.bridges) || document.bridges.length > 7) return null
    const bridges = document.bridges.map((value, index) => {
      const bridge = exactCoreDataRecord(value, ['id', 'start_component_id', 'end_component_id', 'accepted'] as const)
      if (!bridge || bridge.id !== index || !Number.isInteger(bridge.start_component_id) || !Number.isInteger(bridge.end_component_id)
        || Number(bridge.start_component_id) < 0 || Number(bridge.end_component_id) < 0
        || Number(bridge.start_component_id) >= Number(document.component_count)
        || Number(bridge.end_component_id) >= Number(document.component_count)
        || bridge.start_component_id === bridge.end_component_id || typeof bridge.accepted !== 'boolean') return null
      return { id: index, start_component_id: Number(bridge.start_component_id), end_component_id: Number(bridge.end_component_id), accepted: bridge.accepted }
    })
    if (bridges.some((bridge) => bridge === null)) return null
    componentBridgeOverride = {
      schema_version: 1,
      source_asset_sha256: sourceAssetDigest.slice(),
      component_count: Number(document.component_count),
      reviewed: document.reviewed,
      bridges: bridges as NonNullable<
        typeof componentBridgeOverride
      >['bridges'],
    }
  }
  let silhouetteThresholds: BeginnerGenerationConstraintsV1['silhouette_thresholds']
  if (record.silhouette_thresholds !== undefined) {
    const thresholds = snapshotCoreDataRecord(record.silhouette_thresholds)
    if (!thresholds || thresholds.schema_version !== 1
      || Object.keys(thresholds).some((key) => !['schema_version', 'alpha', 'luma', 'polarity'].includes(key))
      || !Number.isInteger(thresholds.alpha) || Number(thresholds.alpha) < 0 || Number(thresholds.alpha) > 255
      || !Number.isInteger(thresholds.luma) || Number(thresholds.luma) < 0 || Number(thresholds.luma) > 255
      || !['dark_on_light', 'light_on_dark', 'alpha_only'].includes(String(thresholds.polarity ?? 'dark_on_light'))) return null
    silhouetteThresholds = Object.freeze({ schema_version: 1, alpha: Number(thresholds.alpha), luma: Number(thresholds.luma), polarity: (thresholds.polarity ?? 'dark_on_light') as 'dark_on_light' | 'light_on_dark' | 'alpha_only' })
  }
  let silhouetteCropRoi: BeginnerGenerationConstraintsV1['silhouette_crop_roi']
  if (record.silhouette_crop_roi !== undefined) {
    const roi = exactCoreDataRecord(record.silhouette_crop_roi, ['schema_version', 'x_millionths', 'y_millionths', 'width_millionths', 'height_millionths'] as const)
    const values = roi && [roi.x_millionths, roi.y_millionths, roi.width_millionths, roi.height_millionths]
    if (!roi || roi.schema_version !== 1 || !values || values.some((value) => !Number.isInteger(value) || Number(value) < 0 || Number(value) > 1_000_000)
      || Number(roi.width_millionths) < 1 || Number(roi.height_millionths) < 1
      || Number(roi.x_millionths) + Number(roi.width_millionths) > 1_000_000
      || Number(roi.y_millionths) + Number(roi.height_millionths) > 1_000_000) return null
    silhouetteCropRoi = Object.freeze({ schema_version: 1, x_millionths: Number(roi.x_millionths), y_millionths: Number(roi.y_millionths), width_millionths: Number(roi.width_millionths), height_millionths: Number(roi.height_millionths) })
  }
  const silhouetteOrientation = record.silhouette_orientation_degrees
  if (silhouetteOrientation !== undefined && ![0, 90, 180, 270].includes(Number(silhouetteOrientation))) return null
  let silhouetteMirror: BeginnerGenerationConstraintsV1['silhouette_mirror']
  if (record.silhouette_mirror !== undefined) {
    const mirror = exactCoreDataRecord(record.silhouette_mirror, ['schema_version', 'mirror_x', 'mirror_y'] as const)
    if (!mirror || mirror.schema_version !== 1 || typeof mirror.mirror_x !== 'boolean'
      || typeof mirror.mirror_y !== 'boolean') return null
    silhouetteMirror = Object.freeze({ schema_version: 1, mirror_x: mirror.mirror_x, mirror_y: mirror.mirror_y })
  }
  return Object.freeze({
    schema_version: 1,
    maximum_steps: Number(record.maximum_steps),
    detail_level: record.detail_level,
    ...(record.generic_body_size_tenths_mm === undefined ? {} : {
      generic_body_size_tenths_mm: Object.freeze(
        (record.generic_body_size_tenths_mm as number[]).map(Number),
      ),
    }),
    ...(record.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: Object.freeze(
        (record.generic_body_outline_tenths_mm as Array<[number, number]>)
          .map((point) => Object.freeze([...point] as [number, number])),
      ),
    }),
    generic_body_outline_mode: record.generic_body_outline_mode === 'general' ? 'general' : 'symmetric',
    target_category: record.target_category,
    ...(record.custom_object_display_name === undefined ? {} : {
      custom_object_display_name: record.custom_object_display_name,
    }),
    target_parts: Object.freeze(
      targetParts.map((part) => Object.freeze(part!)),
    ),
    skeleton_segments: Object.freeze(
      skeletonSegments.map((segment) => Object.freeze({
        ...segment!,
        start: Object.freeze({ ...segment!.start }),
        end: Object.freeze({ ...segment!.end }),
      })),
    ),
    ...(componentBridgeOverride ? {
      component_bridge_override: Object.freeze({
        ...componentBridgeOverride,
        source_asset_sha256: Object.freeze(
          componentBridgeOverride.source_asset_sha256.slice(),
        ),
        bridges: Object.freeze(
          componentBridgeOverride.bridges.map((bridge) =>
            Object.freeze({ ...bridge })),
        ),
      }),
    } : {}),
    ...(silhouetteThresholds ? { silhouette_thresholds: silhouetteThresholds } : {}),
    ...(silhouetteCropRoi ? { silhouette_crop_roi: silhouetteCropRoi } : {}),
    ...(silhouetteOrientation === undefined ? {} : { silhouette_orientation_degrees: Number(silhouetteOrientation) as 0 | 90 | 180 | 270 }),
    ...(silhouetteMirror ? { silhouette_mirror: silhouetteMirror } : {}),
    ...(hadProtrusions ? {
      protrusions: Object.freeze(validProtrusions.slice()),
    } : {}),
    ...(hadBulgeTargets ? {
      bulge_targets: Object.freeze(bulgeTargets.slice()),
    } : {}),
    target_asset: targetAsset,
    allowed_techniques: Object.freeze(record.allowed_techniques.slice()),
  }) as BeginnerGenerationConstraintsV1
}

function normalizeBeginnerRecognitionProposal(
  value: unknown,
  expectedUnderlayId: string,
  expectedAssetId: string,
  expectedFormat: BeginnerRecognitionProposalV1['format'] = 'marker_png_v1',
): BeginnerRecognitionProposalV1 | null {
  const requiredKeys = [
    'schema_version', 'format', 'source_underlay_id', 'source_asset_id',
    'source_sha256', 'width', 'height', 'shape_bounds', 'target_parts',
    'skeleton_segments',
  ] as const
  const optionalKeys = ['generic_body_outline_tenths_mm', 'generic_body_outline_mode', 'protrusions', 'contour_confidence', 'skeleton_quality'] as const
  const record = snapshotCoreDataRecord(value)
  const sourceSha256 = record
    ? snapshotSha256Bytes(record.source_sha256)
    : null
  if (!record || requiredKeys.some((key) => !Object.hasOwn(record, key))
    || Object.keys(record).some((key) => ![...requiredKeys, ...optionalKeys].includes(key as never))) return null
  if (!record || record.schema_version !== 1 || record.format !== expectedFormat
    || record.source_underlay_id !== expectedUnderlayId
    || record.source_asset_id !== expectedAssetId
    || !sourceSha256
    || !Number.isInteger(record.width) || Number(record.width) < 1 || Number(record.width) > 4096
    || !Number.isInteger(record.height) || Number(record.height) < 1 || Number(record.height) > 4096
    || Number(record.width) * Number(record.height) > 4_000_000) return null
  const bounds = exactCoreDataRecord(record.shape_bounds, ['min_x', 'min_y', 'max_x', 'max_y'] as const)
  if (!bounds) return null
  const coordinates = [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
  if (coordinates.some((coordinate) => !Number.isInteger(coordinate))
    || Number(bounds.min_x) < 0 || Number(bounds.min_y) < 0
    || Number(bounds.max_x) < Number(bounds.min_x)
    || Number(bounds.max_y) < Number(bounds.min_y)
    || Number(bounds.max_x) >= Number(record.width)
    || Number(bounds.max_y) >= Number(record.height)) return null
  const constraints = normalizeBeginnerGenerationConstraints({
    schema_version: 1,
    maximum_steps: 1,
    detail_level: 'simple',
    target_category: 'animal',
    target_parts: record.target_parts,
    skeleton_segments: record.skeleton_segments,
    protrusions: record.protrusions ?? [],
    ...(record.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: record.generic_body_outline_tenths_mm,
    }),
    ...(record.generic_body_outline_mode === undefined ? {} : {
      generic_body_outline_mode: record.generic_body_outline_mode,
    }),
    bulge_targets: [],
    target_asset: null,
    allowed_techniques: ['valley_fold'],
  })
  if (!constraints) return null
  if (expectedFormat === 'silhouette_png_v1' && constraints.skeleton_segments.length > 32) return null
  const confidence = record.contour_confidence === undefined ? null : exactCoreDataRecord(
    record.contour_confidence, ['body_score', 'body_reasons', 'local_scores', 'explicit_override_required'] as const)
  const localConfidence = confidence && Array.isArray(confidence.local_scores)
    ? confidence.local_scores.map((item) => exactCoreDataRecord(item, ['protrusion_id', 'score', 'reasons'] as const)) : []
  const validReasons = (value: unknown) => Array.isArray(value) && value.length > 0
    && value.every((reason) => ['dominant_component', 'bounded_simplification_error', 'bounded_curvature',
      'asymmetric_extremity', 'bilateral_symmetry', 'low_component_ratio'].includes(String(reason)))
  if (record.contour_confidence !== undefined && (!confidence
    || !Number.isInteger(confidence.body_score) || Number(confidence.body_score) < 0 || Number(confidence.body_score) > 100
    || !validReasons(confidence.body_reasons) || typeof confidence.explicit_override_required !== 'boolean'
    || localConfidence.length !== (confidence.local_scores as unknown[]).length
    || localConfidence.some((item) => !item || !Number.isInteger(item.protrusion_id)
      || !Number.isInteger(item.score) || Number(item.score) < 0 || Number(item.score) > 100 || !validReasons(item.reasons)))) return null
  const validatedLocalConfidence = localConfidence as ReadonlyArray<NonNullable<(typeof localConfidence)[number]>>
  const skeletonQuality = record.skeleton_quality === undefined ? null : exactCoreDataRecord(
    record.skeleton_quality, ['score', 'reasons', 'insufficiency_reasons', 'distance_metric', 'bar_limit'] as const)
  const validSkeletonReasons = (value: unknown, allowEmpty = false) => Array.isArray(value)
    && (allowEmpty || value.length > 0) && value.length <= 8
    && value.every((reason) => typeof reason === 'string' && [
      'offline_manhattan_distance_ridges', 'deterministic_axis_spans',
      'per_component_medial_axis_v1', 'inferred_aabb_kruskal_mst_bridges',
      'no_branch_evidence', 'bar_limit_reached', 'component_bridges_are_estimated',
    ].includes(reason))
  if (record.skeleton_quality !== undefined && (!skeletonQuality
    || !Number.isInteger(skeletonQuality.score) || Number(skeletonQuality.score) < 0 || Number(skeletonQuality.score) > 100
    || !validSkeletonReasons(skeletonQuality.reasons)
    || !validSkeletonReasons(skeletonQuality.insufficiency_reasons, true)
    || !['manhattan_pixel_v1', 'aabb_squared_distance_v1'].includes(String(skeletonQuality.distance_metric))
    || ![16, 32].includes(Number(skeletonQuality.bar_limit)))) return null
  return Object.freeze({
    schema_version: 1,
    format: expectedFormat,
    source_underlay_id: expectedUnderlayId,
    source_asset_id: expectedAssetId,
    source_sha256: sourceSha256,
    width: Number(record.width),
    height: Number(record.height),
    shape_bounds: Object.freeze({
      min_x: Number(bounds.min_x), min_y: Number(bounds.min_y),
      max_x: Number(bounds.max_x), max_y: Number(bounds.max_y),
    }),
    target_parts: constraints.target_parts,
    skeleton_segments: constraints.skeleton_segments,
    ...(constraints.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: constraints.generic_body_outline_tenths_mm,
    }),
    ...(record.generic_body_outline_mode === undefined ? {} : {
      generic_body_outline_mode: constraints.generic_body_outline_mode,
    }),
    ...(record.protrusions === undefined ? {} : { protrusions: constraints.protrusions }),
    ...(confidence === null ? {} : { contour_confidence: Object.freeze({
      body_score: Number(confidence.body_score), body_reasons: Object.freeze((confidence.body_reasons as string[]).slice()),
      local_scores: Object.freeze(validatedLocalConfidence.map((item) => Object.freeze({ protrusion_id: Number(item.protrusion_id),
        score: Number(item.score), reasons: Object.freeze((item.reasons as string[]).slice()) }))),
      explicit_override_required: confidence.explicit_override_required as boolean,
    }) }),
    ...(skeletonQuality === null ? {} : { skeleton_quality: Object.freeze({
      score: Number(skeletonQuality.score),
      reasons: Object.freeze((skeletonQuality.reasons as string[]).slice()),
      insufficiency_reasons: Object.freeze((skeletonQuality.insufficiency_reasons as string[]).slice()),
      distance_metric: skeletonQuality.distance_metric as 'manhattan_pixel_v1' | 'aabb_squared_distance_v1',
      bar_limit: skeletonQuality.bar_limit as 16 | 32,
    }) }),
  })
}

export type BeginnerCandidateScoreV1 = {
  schema_version: 1
  kind: 'recommended' | 'shape_focused' | 'foldability_focused'
  rank: number
  total_score: number
  shape_score: number
  target_approximation_score: number
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
    | 'unsupported_animal_template'
    | 'unsupported_insect_template'
  generated_plans: BeginnerGeneratedPlanV1[]
  plan_assessments: BeginnerGeneratedPlanAssessmentV1[]
  candidates: BeginnerCandidateScoreV1[]
  multi_reference_fusion: null | {
    revision: number; image_sha256: readonly number[]; reference_sha256: readonly number[]; source_count: 2
    image_component_count: number; reference_component_count: number
    image_branch_count: number; reference_branch_count: number
    normalized_extent_error: number; agreement_score: number; apply_allowed: boolean
    reason: 'image_glb_agreement_v1' | 'image_glb_disagreement_v1'
  }
  reference_consensus_analysis: null | {
    schema_version: 1; revision: number; source_count: number; excluded_asset_id: string | null
    pair_count: number; disagreement_count: number; agreement_score: number; apply_allowed: boolean
    reason: 'reference_consensus_agreement_v1' | 'reference_consensus_multiple_disagreements_v1'
    pairs: ReadonlyArray<Readonly<{ left_asset_id: string; right_asset_id: string; component_error: number
      normalized_extent_error: number; branch_error: number; agreement_score: number
      disagrees: boolean; pair_digest_sha256: readonly number[]; left_component_count: number
      right_component_count: number; left_normalized_extents: readonly [number, number]
      right_normalized_extents: readonly [number, number]; left_branch_count: number; right_branch_count: number }>>
  }
}

export type BeginnerGeneratedPlanAssessmentV1 = {
  kind: BeginnerGeneratedPlanV1['kind']
  expected_candidate_edge_id: string
  proof_scope: 'necessary' | 'sufficient' | 'indeterminate'
  apply_allowed: boolean
  shape_approximation_score: number | null
  shape_difference_reason:
    | 'crease_preview_has_no_surface_mesh'
    | 'certified_flat_surface_v1'
    | 'component_aware_quantized_shape_v1'
    | 'bounded_folded_pose_landmarks_v1'
    | null
  component_shape_comparison: {
    component_count: number
    matched_branch_count: number
    work_units: number
    extent_score: number
    branch_score: number
    bridge_score: number
    extent_weight: 45
    branch_weight: 35
    bridge_weight: 20
  } | null
  reason:
    | 'geometry_invalid'
    | 'folded_pose_simulation_failed'
    | 'fold_path_certificate_unavailable'
    | 'manufacturability_missing_vertex'
    | 'manufacturability_minimum_crease_spacing'
    | 'manufacturability_minimum_face_area'
    | 'manufacturability_paper_boundary_margin'
    | 'necessary_conditions_satisfied'
    | 'necessary_conditions_violated'
    | 'local_analysis_blocked'
    | 'local_theorem_not_applicable'
    | 'local_analysis_indeterminate'
    | 'native_fold_path_certified'
    | 'global_flat_foldability_proven'
    | 'global_flat_foldability_impossible'
    | 'global_resource_limit'
    | 'global_timeout'
    | 'deadline_exceeded'
    | 'global_indeterminate'
    | 'multi_reference_disagreement'
}

export function beginnerGeneratedPlanAssessmentAllowsApplyV1(
  assessment: Readonly<Pick<
    BeginnerGeneratedPlanAssessmentV1,
    'proof_scope' | 'apply_allowed'
  >>,
): boolean {
  return assessment.proof_scope === 'sufficient'
    && assessment.apply_allowed
}

const ANIMAL_SPECIALIZED_TARGET_PART_KINDS_V1: ReadonlyArray<
  BeginnerGeneratedPlanKindV1
> = Object.freeze([
  'composite_complete_winged_animal_base',
  'composite_complete_animal_base',
  'composite_horn_tail_ear_base',
  'composite_tail_ear_base',
  'composite_horn_ear_base',
  'composite_horn_tail_base',
  'symmetric_four_leg_base',
  'asymmetric_four_leg_landmark_base',
  'symmetric_bird_base',
  'asymmetric_bird_landmark_base',
  'asymmetric_fish_landmark_base',
  'symmetric_fish_base',
  'symmetric_ear_base',
  'symmetric_horn_base',
  'center_axis_tail_base',
  'center_axis_horn_base',
])

const INSECT_SPECIALIZED_TARGET_PART_KINDS_V1: ReadonlyArray<
  BeginnerGeneratedPlanKindV1
> = Object.freeze([
  'composite_complete_insect_base',
  'composite_wing_antenna_base',
  'asymmetric_insect_landmark_base',
  'symmetric_wing_base',
  'symmetric_antenna_base',
  'center_axis_antenna_base',
  'symmetric_insect_leg_pair_base',
  'symmetric_six_leg_base',
])

const ANIMAL_FOLD_VARIANT_KINDS_V1: ReadonlyArray<
  BeginnerGeneratedPlanKindV1
> = Object.freeze([
  'vertical_book_fold',
  'horizontal_book_fold',
])

const INSECT_FOLD_VARIANT_KINDS_V1: ReadonlyArray<
  BeginnerGeneratedPlanKindV1
> = Object.freeze([
  'diagonal_fold',
  'vertical_book_fold',
])

function sameBeginnerTargetPartsInOrderV1(
  actual: BeginnerGenerationConstraintsV1['target_parts'],
  expected: BeginnerGenerationConstraintsV1['target_parts'],
): boolean {
  return actual.length === expected.length
    && actual.every((part, index) =>
      part.kind === expected[index]?.kind
      && part.count === expected[index]?.count)
}

function sameBeginnerTargetAssetV1(
  actual: BeginnerGenerationConstraintsV1['target_asset'],
  expected: BeginnerGenerationConstraintsV1['target_asset'],
): boolean {
  if (actual === null || expected === null) return actual === expected
  return actual.kind === expected.kind
    && actual.asset_id === expected.asset_id
    && (
      actual.kind !== 'reference_image'
      || (
        expected.kind === 'reference_image'
        && actual.underlay_id === expected.underlay_id
      )
    )
}

function sameBeginnerSkeletonSegmentsInOrderV1(
  actual: BeginnerGenerationConstraintsV1['skeleton_segments'],
  expected: BeginnerGenerationConstraintsV1['skeleton_segments'],
): boolean {
  return actual.length === expected.length
    && actual.every((segment, index) => {
      const expectedSegment = expected[index]
      return segment.id === expectedSegment?.id
        && segment.thickness_tenths_mm
          === expectedSegment.thickness_tenths_mm
        && segment.start.x_tenths_mm
          === expectedSegment.start.x_tenths_mm
        && segment.start.y_tenths_mm
          === expectedSegment.start.y_tenths_mm
        && segment.end.x_tenths_mm
          === expectedSegment.end.x_tenths_mm
        && segment.end.y_tenths_mm
          === expectedSegment.end.y_tenths_mm
    })
}

function canonicalBeginnerGenericSkeletonSegmentsV1(
  segments: BeginnerGenerationConstraintsV1['skeleton_segments'],
): BeginnerGenerationConstraintsV1['skeleton_segments'] {
  return segments
    .map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
    .sort((left, right) => left.id - right.id)
    .map((segment) => {
      const start = [
        segment.start.x_tenths_mm,
        segment.start.y_tenths_mm,
      ] as const
      const end = [
        segment.end.x_tenths_mm,
        segment.end.y_tenths_mm,
      ] as const
      return end[0] < start[0]
        || (end[0] === start[0] && end[1] < start[1])
        ? { ...segment, start: segment.end, end: segment.start }
        : segment
    })
}

function expectedProfileSelectsGenericCandidatePlanV1(
  expectedProfile: BeginnerDesignProfileV1,
): boolean {
  const constraints = expectedProfile.generation_constraints
  const category = constraints.target_category
  if (
    category !== 'animal'
    && category !== 'insect'
    && category !== 'custom_object'
  ) return false
  const parts = constraints.target_parts
  if (new Set(parts.map((part) => part.kind)).size !== parts.length) {
    return false
  }
  const semanticEndpointCount = parts.reduce(
    (sum, part) =>
      part.kind === 'head' || part.kind === 'torso'
        ? sum
        : sum + part.count,
    0,
  )
  const protrusions = constraints.protrusions ?? []
  const physicalEndpointCount = protrusions.reduce(
    (sum, protrusion) => sum + protrusion.count,
    0,
  )
  const physicalBindingsAreBounded = protrusions.length >= 1
    && protrusions.length <= MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1
    && physicalEndpointCount >= 1
    && physicalEndpointCount <= 32
    && protrusions.every((protrusion, index) =>
      index === 0 || protrusions[index - 1]!.id < protrusion.id)
  if (!physicalBindingsAreBounded) return false
  if (category === 'custom_object') {
    return semanticEndpointCount === 0
      || semanticEndpointCount === physicalEndpointCount
  }
  const hasExactBody = parts.filter((part) =>
    part.kind === 'head' && part.count === 1).length === 1
    && parts.filter((part) =>
      part.kind === 'torso' && part.count === 1).length === 1
  const specializedKinds = category === 'animal'
    ? ANIMAL_SPECIALIZED_TARGET_PART_KINDS_V1
    : INSECT_SPECIALIZED_TARGET_PART_KINDS_V1
  return hasExactBody
    && semanticEndpointCount >= MIN_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
    && semanticEndpointCount <= MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
    && semanticEndpointCount === physicalEndpointCount
    && !specializedKinds.some((kind) =>
      beginnerGeneratedPlanTargetPartsAreCompatibleV1(kind, parts))
}

const BEGINNER_SPECIALIZED_BASE_PHYSICAL_EDGE_COUNT_V1 =
  Object.freeze({
    symmetric_four_leg_base: 4,
    asymmetric_four_leg_landmark_base: 4,
    symmetric_wing_base: 4,
    symmetric_bird_base: 4,
    asymmetric_bird_landmark_base: 4,
    asymmetric_insect_landmark_base: 4,
    asymmetric_fish_landmark_base: 4,
    symmetric_fish_base: 4,
    symmetric_ear_base: 4,
    symmetric_horn_base: 4,
    symmetric_antenna_base: 4,
    symmetric_insect_leg_pair_base: 4,
    symmetric_six_leg_base: 12,
    center_axis_tail_base: 1,
    center_axis_horn_base: 1,
    center_axis_antenna_base: 1,
    composite_tail_ear_base: 5,
    composite_horn_ear_base: 5,
    composite_horn_tail_base: 2,
    composite_horn_tail_ear_base: 6,
    composite_wing_antenna_base: 8,
    composite_complete_insect_base: 20,
    composite_complete_animal_base: 10,
    composite_complete_winged_animal_base: 10,
  } satisfies Partial<Record<BeginnerGeneratedPlanKindV1, number>>)

const BEGINNER_SPECIALIZED_RADIAL_SUPPORT_KINDS_V1 =
  new Set<BeginnerGeneratedPlanKindV1>([
    'symmetric_four_leg_base',
    'symmetric_six_leg_base',
    'composite_horn_tail_ear_base',
    'composite_wing_antenna_base',
    'composite_complete_insect_base',
    'composite_complete_animal_base',
    'composite_complete_winged_animal_base',
  ])

function beginnerPlanRadialSupportAddedV1(
  plan: BeginnerGeneratedPlanV1,
): number | null | undefined {
  const supportCodes = plan.instruction_codes.filter((code) =>
    code.startsWith('bounded_radial_corner_support_v1:'))
  if (supportCodes.length === 0) return null
  if (supportCodes.length !== 1) return undefined
  const match =
    /^bounded_radial_corner_support_v1:added=([0-5]):covered=4$/u
      .exec(supportCodes[0]!)
  return match ? Number(match[1]) : undefined
}

function physicalBeginnerPlanEdgeCountV1(
  plan: BeginnerGeneratedPlanV1,
): number {
  return plan.crease_pattern.edges.filter((edge) =>
    edge.kind === 'mountain' || edge.kind === 'valley').length
}

function genericCandidatePhysicalEdgesMatchProfileV1(
  plan: BeginnerGeneratedPlanV1,
  expectedProfile: BeginnerDesignProfileV1,
): boolean {
  const physicalEndpointCount = (
    expectedProfile.generation_constraints.protrusions ?? []
  ).reduce((sum, protrusion) => sum + protrusion.count, 0)
  const supportAdded = beginnerPlanRadialSupportAddedV1(plan)
  const supportIsRequired =
    physicalEndpointCount === 2
    || physicalEndpointCount === 4
    || (physicalEndpointCount >= 6 && physicalEndpointCount % 2 === 0)
    || [3, 5, 7, 9, 11, 13].includes(physicalEndpointCount)
  return supportAdded !== undefined
    && (supportAdded !== null) === supportIsRequired
    && (
      physicalEndpointCount === 2
        ? supportAdded === 4
        : physicalEndpointCount === 4
          ? supportAdded === 2 || supportAdded === 4
          : true
    )
    && (
      physicalEndpointCount !== 4
      || (
        physicalEndpointCount + Number(supportAdded) >= 6
        && (physicalEndpointCount + Number(supportAdded)) % 2 === 0
      )
    )
    && physicalBeginnerPlanEdgeCountV1(plan)
      === physicalEndpointCount + (supportAdded ?? 0)
}

function specializedCandidatePhysicalEdgesMatchKindV1(
  plan: BeginnerGeneratedPlanV1,
): boolean {
  const basePhysicalEdgeCount =
    BEGINNER_SPECIALIZED_BASE_PHYSICAL_EDGE_COUNT_V1[
      plan.kind as keyof typeof BEGINNER_SPECIALIZED_BASE_PHYSICAL_EDGE_COUNT_V1
    ]
  if (basePhysicalEdgeCount === undefined) return false
  const supportAdded = beginnerPlanRadialSupportAddedV1(plan)
  const supportIsRequired =
    BEGINNER_SPECIALIZED_RADIAL_SUPPORT_KINDS_V1.has(plan.kind)
  return supportAdded !== undefined
    && (supportAdded === null || supportAdded <= 4)
    && (supportAdded !== null) === supportIsRequired
    && (
      plan.kind !== 'symmetric_four_leg_base'
      || supportAdded === 2
      || supportAdded === 4
    )
    && physicalBeginnerPlanEdgeCountV1(plan)
      === basePhysicalEdgeCount + (supportAdded ?? 0)
}

function protrusionLocalOutlinesFitSkeletonV1(
  constraints: BeginnerGenerationConstraintsV1,
): boolean {
  const outlined = (constraints.protrusions ?? []).filter(
    (protrusion) => protrusion.local_outline_tenths_mm !== undefined,
  )
  if (outlined.length === 0) return true
  const coordinates = constraints.skeleton_segments.flatMap((segment) => [
    segment.start,
    segment.end,
  ])
  if (coordinates.length === 0) return false
  const minimumX = Math.min(...coordinates.map((point) =>
    point.x_tenths_mm))
  const maximumX = Math.max(...coordinates.map((point) =>
    point.x_tenths_mm))
  const minimumY = Math.min(...coordinates.map((point) =>
    point.y_tenths_mm))
  const maximumY = Math.max(...coordinates.map((point) =>
    point.y_tenths_mm))
  return outlined.every((protrusion) =>
    protrusion.local_outline_tenths_mm?.every(([localX, localY]) => {
      const x = protrusion.position_tenths_mm[0] + localX
      const y = protrusion.position_tenths_mm[1] + localY
      return x >= minimumX && x <= maximumX
        && y >= minimumY && y <= maximumY
    }) === true)
}

function hasExactOrderedAsymmetricLandmarksV1(
  constraints: BeginnerGenerationConstraintsV1,
  requiredCount: number,
  minimumSkeletonSegments: number,
): boolean {
  const protrusions = constraints.protrusions ?? []
  return constraints.skeleton_segments.length >= minimumSkeletonSegments
    && protrusions.length === requiredCount
    && protrusions.every((protrusion, index) =>
      (index === 0 || protrusions[index - 1]!.id < protrusion.id)
      && protrusion.count === 1
      && protrusion.symmetry === 'none'
      && protrusion.direction_milli.some((coordinate) => coordinate !== 0))
    && protrusionLocalOutlinesFitSkeletonV1(constraints)
}

function expectedSpecializedPrimaryKindV1(
  expectedProfile: BeginnerDesignProfileV1,
): BeginnerGeneratedPlanKindV1 | null {
  const constraints = expectedProfile.generation_constraints
  const parts = constraints.target_parts
  const categoryKinds = constraints.target_category === 'animal'
    ? ANIMAL_SPECIALIZED_TARGET_PART_KINDS_V1
    : constraints.target_category === 'insect'
      ? INSECT_SPECIALIZED_TARGET_PART_KINDS_V1
      : []
  const compatible = categoryKinds.filter((kind) =>
    beginnerGeneratedPlanTargetPartsAreCompatibleV1(kind, parts))
  if (compatible.length === 0) return null
  if (compatible.includes('symmetric_four_leg_base')) {
    return hasExactOrderedAsymmetricLandmarksV1(constraints, 4, 3)
      ? 'asymmetric_four_leg_landmark_base'
      : 'symmetric_four_leg_base'
  }
  if (compatible.includes('symmetric_bird_base')) {
    return hasExactOrderedAsymmetricLandmarksV1(constraints, 2, 2)
      ? 'asymmetric_bird_landmark_base'
      : 'symmetric_bird_base'
  }
  if (compatible.includes('asymmetric_insect_landmark_base')) {
    return hasExactOrderedAsymmetricLandmarksV1(constraints, 7, 0)
      ? 'asymmetric_insect_landmark_base'
      : null
  }
  if (compatible.includes('asymmetric_fish_landmark_base')) {
    return hasExactOrderedAsymmetricLandmarksV1(constraints, 3, 0)
      ? 'asymmetric_fish_landmark_base'
      : null
  }
  return compatible[0] ?? null
}

function expectedProfileAuthorizesCandidatePlanV1(
  plan: BeginnerGeneratedPlanV1,
  expectedProfile: BeginnerDesignProfileV1,
  planIndex: number,
  instructionContext: BeginnerGeneratedPlanInstructionContextV1,
): boolean {
  const constraints = expectedProfile.generation_constraints
  if (
    !sameBeginnerTargetPartsInOrderV1(
      plan.target_parts,
      constraints.target_parts,
    )
    || !sameBeginnerTargetAssetV1(
      plan.target_asset,
      constraints.target_asset,
    )
  ) return false
  const primaryIsGeneric =
    expectedProfileSelectsGenericCandidatePlanV1(expectedProfile)
  const expectedSkeleton = primaryIsGeneric
    ? canonicalBeginnerGenericSkeletonSegmentsV1(
        constraints.skeleton_segments,
      )
    : constraints.skeleton_segments
  if (!sameBeginnerSkeletonSegmentsInOrderV1(
    plan.skeleton_segments,
    expectedSkeleton,
  )) return false
  if (planIndex > 0) {
    const foldVariants = constraints.target_category === 'animal'
      ? ANIMAL_FOLD_VARIANT_KINDS_V1
      : constraints.target_category === 'insect'
        ? INSECT_FOLD_VARIANT_KINDS_V1
        : []
    return plan.kind === foldVariants[planIndex - 1]
      && (
        instructionContext === 'grid'
        || beginnerGeneratedPlanTopologyMatchesProfileV1(
          plan,
          expectedProfile,
          planIndex,
        )
      )
  }
  if (plan.kind === 'composite_generic_target_base') {
    return primaryIsGeneric
      && genericCandidatePhysicalEdgesMatchProfileV1(plan, expectedProfile)
      && (
        instructionContext === 'grid'
        || beginnerGeneratedPlanTopologyMatchesProfileV1(
          plan,
          expectedProfile,
          planIndex,
        )
      )
  }
  return plan.kind === expectedSpecializedPrimaryKindV1(expectedProfile)
    && specializedCandidatePhysicalEdgesMatchKindV1(plan)
    && (
      instructionContext === 'grid'
      || beginnerGeneratedPlanTopologyMatchesProfileV1(
        plan,
        expectedProfile,
        planIndex,
      )
    )
}

export type BeginnerGeneratedPlanV1 = {
  schema_version: 1
  kind: BeginnerGeneratedPlanKindV1
  crease_pattern: {
    vertices: Array<{ id: string; position: { x: number; y: number } }>
    edges: Array<{
      id: string
      start: string
      end: string
      kind: 'mountain' | 'valley' | 'auxiliary'
    }>
  }
  instruction_codes: string[]
  target_parts: BeginnerGenerationConstraintsV1['target_parts']
  skeleton_segments: BeginnerGenerationConstraintsV1['skeleton_segments']
  target_asset: BeginnerGenerationConstraintsV1['target_asset']
  semantic_landmark_provenance?: {
    schema_version: 1
    ordered_bindings: ReadonlyArray<Readonly<{
      ordinal: number
      role: string
      physical_ray: number
    }>>
    physical_ray_group_sha256: ReadonlyArray<ReadonlyArray<number>>
  }
}

const BEGINNER_ASYMMETRIC_INSECT_SEMANTIC_ROLES_V1 =
  Object.freeze([
    'head', 'tail', 'wing_left', 'wing_right', 'leg_front_left',
    'leg_front_right', 'leg_middle_left', 'leg_middle_right',
    'leg_rear_left', 'leg_rear_right',
  ])

const BEGINNER_ASYMMETRIC_FISH_SEMANTIC_ROLES_V1 =
  Object.freeze(['head', 'tail', 'fin_left', 'fin_right'])

const BEGINNER_ASYMMETRIC_INSECT_RAY_DIGESTS_V1 =
  Object.freeze([
    Object.freeze([213, 100, 5, 8, 192, 66, 152, 160, 194, 233, 1, 213, 122, 93, 223, 98, 40, 90, 120, 82, 11, 67, 162, 155, 111, 87, 115, 210, 17, 24, 20, 214]),
    Object.freeze([129, 45, 3, 220, 103, 100, 168, 77, 239, 198, 183, 47, 163, 199, 110, 178, 201, 166, 66, 26, 155, 17, 241, 21, 87, 84, 107, 98, 136, 35, 51, 92]),
    Object.freeze([23, 164, 6, 77, 87, 18, 29, 42, 246, 60, 210, 220, 59, 34, 167, 44, 157, 174, 12, 81, 10, 0, 226, 138, 153, 54, 51, 73, 94, 193, 23, 250]),
    Object.freeze([229, 127, 126, 18, 52, 160, 111, 196, 175, 230, 97, 142, 9, 79, 197, 232, 238, 88, 70, 214, 0, 195, 94, 118, 124, 163, 45, 91, 174, 243, 198, 219]),
  ])

const BEGINNER_ASYMMETRIC_FISH_RAY_DIGESTS_V1 =
  Object.freeze([
    Object.freeze([75, 41, 210, 152, 136, 151, 46, 106, 24, 123, 23, 184, 30, 114, 42, 135, 137, 104, 245, 152, 132, 24, 91, 70, 94, 24, 236, 17, 27, 2, 50, 160]),
    Object.freeze([161, 248, 204, 0, 96, 167, 32, 29, 69, 192, 109, 11, 216, 173, 136, 184, 254, 168, 75, 149, 4, 228, 224, 106, 4, 131, 187, 25, 183, 13, 1, 159]),
    Object.freeze([202, 241, 97, 235, 226, 126, 156, 158, 161, 24, 8, 56, 7, 121, 174, 191, 34, 49, 180, 97, 195, 114, 200, 217, 150, 23, 163, 150, 142, 77, 176, 173]),
    Object.freeze([244, 237, 179, 47, 153, 216, 77, 228, 12, 216, 247, 224, 124, 44, 111, 86, 85, 226, 67, 79, 22, 1, 187, 119, 64, 146, 75, 8, 53, 62, 112, 224]),
  ])

function beginnerSemanticContractV1(
  kindOrBindingCount: BeginnerGeneratedPlanV1['kind'] | number,
): Readonly<{
  roles: ReadonlyArray<string>
  rayDigests: ReadonlyArray<ReadonlyArray<number>>
}> | null {
  if (
    kindOrBindingCount === 'asymmetric_insect_landmark_base'
    || kindOrBindingCount === 10
  ) {
    return {
      roles: BEGINNER_ASYMMETRIC_INSECT_SEMANTIC_ROLES_V1,
      rayDigests: BEGINNER_ASYMMETRIC_INSECT_RAY_DIGESTS_V1,
    }
  }
  if (
    kindOrBindingCount === 'asymmetric_fish_landmark_base'
    || kindOrBindingCount === 4
  ) {
    return {
      roles: BEGINNER_ASYMMETRIC_FISH_SEMANTIC_ROLES_V1,
      rayDigests: BEGINNER_ASYMMETRIC_FISH_RAY_DIGESTS_V1,
    }
  }
  return null
}

function sameBeginnerDigestBytesV1(
  actual: ReadonlyArray<number> | null | undefined,
  expected: ReadonlyArray<number>,
): boolean {
  return actual?.length === expected.length
    && actual.every((byte, index) => byte === expected[index])
}

const BEGINNER_ASSESSMENT_OUTCOME_TUPLES_V1 =
  Object.freeze({
    geometry_invalid: ['necessary', false],
    folded_pose_simulation_failed: ['indeterminate', false],
    fold_path_certificate_unavailable: ['necessary', false],
    manufacturability_missing_vertex: ['necessary', false],
    manufacturability_minimum_crease_spacing: ['necessary', false],
    manufacturability_minimum_face_area: ['necessary', false],
    manufacturability_paper_boundary_margin: ['necessary', false],
    necessary_conditions_satisfied: ['necessary', true],
    necessary_conditions_violated: ['necessary', false],
    local_analysis_blocked: ['necessary', false],
    local_theorem_not_applicable: ['indeterminate', true],
    local_analysis_indeterminate: ['indeterminate', true],
    native_fold_path_certified: ['sufficient', true],
    global_flat_foldability_proven: ['sufficient', true],
    global_flat_foldability_impossible: ['necessary', false],
    global_resource_limit: ['indeterminate', true],
    global_timeout: ['indeterminate', true],
    deadline_exceeded: ['indeterminate', false],
    global_indeterminate: ['indeterminate', true],
    multi_reference_disagreement: ['indeterminate', false],
  } as const)

function beginnerAssessmentOutcomeTupleIsCanonicalV1(
  reason: unknown,
  proofScope: unknown,
  applyAllowed: unknown,
): boolean {
  if (
    typeof reason !== 'string'
    || !Object.hasOwn(BEGINNER_ASSESSMENT_OUTCOME_TUPLES_V1, reason)
  ) return false
  const tuple = BEGINNER_ASSESSMENT_OUTCOME_TUPLES_V1[
    reason as keyof typeof BEGINNER_ASSESSMENT_OUTCOME_TUPLES_V1
  ]
  return proofScope === tuple[0] && applyAllowed === tuple[1]
}

function beginnerReferenceDescriptorBranchIsCanonicalV1(
  kind: unknown,
  componentCount: unknown,
  branchCount: unknown,
): boolean {
  if (
    !Number.isInteger(componentCount)
    || !Number.isInteger(branchCount)
  ) return false
  const components = Number(componentCount)
  const branches = Number(branchCount)
  if (kind === 'image') {
    return components >= 1
      && components <= 16
      && branches === components * 2 - 1
  }
  if (kind === 'reference_model') {
    return components >= 1
      && components <= 8
      && branches === (components === 1 ? 3 : components * 2 - 1)
  }
  return false
}

function normalizeBeginnerCandidateResponse(
  value: unknown,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  requestedCandidateCount: number,
  instructionContext: BeginnerGeneratedPlanInstructionContextV1,
  expectedProfile: BeginnerDesignProfileV1 | null = null,
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
    'plan_assessments',
    'candidates',
    'multi_reference_fusion',
    'reference_consensus_analysis',
  ] as const)
  const fusionInputIsNull = response?.multi_reference_fusion === null
  const fusion = fusionInputIsNull ? null
    : exactCoreDataRecord(response?.multi_reference_fusion, [
      'revision', 'image_sha256', 'reference_sha256', 'source_count', 'image_component_count',
      'reference_component_count', 'image_branch_count', 'reference_branch_count',
      'normalized_extent_error', 'agreement_score', 'apply_allowed', 'reason',
    ] as const)
  const fusionImageSha256 = fusion
    ? snapshotSha256Bytes(fusion.image_sha256)
    : null
  const fusionReferenceSha256 = fusion
    ? snapshotSha256Bytes(fusion.reference_sha256)
    : null
  const consensusInputIsNull = response?.reference_consensus_analysis === null
  const consensus = consensusInputIsNull ? null
    : exactCoreDataRecord(response?.reference_consensus_analysis, [
      'schema_version', 'revision', 'source_count', 'excluded_asset_id', 'pair_count',
      'disagreement_count', 'agreement_score', 'apply_allowed', 'reason', 'pairs',
    ] as const)
  const generatedPlanInputs = snapshotCoreDataArray(
    response?.generated_plans,
    3,
  )
  const planAssessmentInputs = snapshotCoreDataArray(
    response?.plan_assessments,
    3,
  )
  const candidateInputs = snapshotCoreDataArray(response?.candidates, 3)
  const consensusPairInputs = consensus === null
    ? []
    : snapshotCoreDataArray(consensus?.pairs, 6)
  const consensusPairs = consensusPairInputs ? consensusPairInputs.map((raw) =>
    exactCoreDataRecord(raw, ['left_asset_id', 'right_asset_id', 'component_error',
      'normalized_extent_error', 'branch_error', 'agreement_score', 'disagrees', 'pair_digest_sha256',
      'left_component_count', 'right_component_count', 'left_normalized_extents',
      'right_normalized_extents', 'left_branch_count', 'right_branch_count'] as const)) : []
  const consensusPairDigests = consensusPairs.map((pair) =>
    pair ? snapshotSha256Bytes(pair.pair_digest_sha256) : null)
  const consensusPairLeftExtents = consensusPairs.map((pair) =>
    pair ? snapshotCoreDataArray(pair.left_normalized_extents, 2) : null)
  const consensusPairRightExtents = consensusPairs.map((pair) =>
    pair ? snapshotCoreDataArray(pair.right_normalized_extents, 2) : null)
  if (
    !response
    || response.schema_version !== 1
    || !matchesProjectOccGuard({
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    }, response as Readonly<{
      project_instance_id: string
      project_id: string
      revision: number
    }>)
    || response.requested_candidate_count !== requestedCandidateCount
    || response.bulge_treatment !== 'target_shape_approximation'
    || response.elasticity_model !== 'not_computed'
    || !['ready', 'resource_limit', 'unsupported_paper', 'unsupported_techniques', 'missing_target_category', 'missing_required_parts', 'missing_target_asset', 'unsupported_animal_template', 'unsupported_insect_template']
      .includes(String(response.generation_status))
    || !generatedPlanInputs
    || !planAssessmentInputs
    || planAssessmentInputs.length !== generatedPlanInputs.length
    || !candidateInputs
    || candidateInputs.length < 1
    || candidateInputs.length !== requestedCandidateCount
    || (!fusionInputIsNull && !fusion)
    || (fusion !== null && (fusion.revision !== expectedRevision || fusion.source_count !== 2
      || expectedProfile?.generation_constraints.target_asset?.kind
        !== 'reference_model'
      || !fusionImageSha256 || !fusionReferenceSha256
      || !Number.isInteger(fusion.image_component_count)
      || Number(fusion.image_component_count) < 1
      || Number(fusion.image_component_count) > 8
      || !Number.isInteger(fusion.reference_component_count)
      || Number(fusion.reference_component_count) < 1
      || Number(fusion.reference_component_count) > 8
      || !Number.isInteger(fusion.image_branch_count)
      || Number(fusion.image_branch_count) < 1
      || Number(fusion.image_branch_count) > 15
      || !Number.isInteger(fusion.reference_branch_count)
      || Number(fusion.reference_branch_count) < 1
      || Number(fusion.reference_branch_count) > 15
      || Number(fusion.image_branch_count)
        !== Number(fusion.image_component_count) * 2 - 1
      || Number(fusion.reference_branch_count) !== (
        Number(fusion.reference_component_count) === 1
          ? 3
          : Number(fusion.reference_component_count) * 2 - 1
      )
      || !Number.isInteger(fusion.normalized_extent_error) || Number(fusion.normalized_extent_error) < 0 || Number(fusion.normalized_extent_error) > 100
      || !Number.isInteger(fusion.agreement_score) || Number(fusion.agreement_score) < 0 || Number(fusion.agreement_score) > 100
      || Number(fusion.agreement_score) !== Math.max(
        0,
        100 - Math.min(
          100,
          Number(fusion.normalized_extent_error) * 2
            + Math.abs(
              Number(fusion.image_component_count)
                - Number(fusion.reference_component_count),
            ) * 20
            + Math.abs(
              Number(fusion.image_branch_count)
                - Number(fusion.reference_branch_count),
            ) * 10,
        ),
      )
      || typeof fusion.apply_allowed !== 'boolean'
      || fusion.apply_allowed !== (
        Number(fusion.normalized_extent_error) <= 20
        && Math.abs(
          Number(fusion.image_component_count)
            - Number(fusion.reference_component_count),
        ) <= 1
        && Math.abs(
          Number(fusion.image_branch_count)
            - Number(fusion.reference_branch_count),
        ) <= 2
      )
      || !['image_glb_agreement_v1', 'image_glb_disagreement_v1'].includes(String(fusion.reason))
      || (fusion.reason === 'image_glb_agreement_v1') !== fusion.apply_allowed))
    || (!consensusInputIsNull && !consensus)
    || (consensus !== null && (consensus.schema_version !== 1 || consensus.revision !== expectedRevision
      || !Number.isInteger(consensus.source_count) || Number(consensus.source_count) < 2 || Number(consensus.source_count) > 4
      || (consensus.excluded_asset_id !== null && !isCanonicalNonNilUuid(consensus.excluded_asset_id))
      || !Number.isInteger(consensus.pair_count) || Number(consensus.pair_count) < 1 || Number(consensus.pair_count) > 6
      || !consensusPairInputs
      || consensusPairs.length !== consensus.pair_count || !Number.isInteger(consensus.disagreement_count)
      || Number(consensus.disagreement_count) < 0 || Number(consensus.disagreement_count) > Number(consensus.pair_count)
      || !Number.isInteger(consensus.agreement_score) || Number(consensus.agreement_score) < 0 || Number(consensus.agreement_score) > 100
      || typeof consensus.apply_allowed !== 'boolean'
      || !['reference_consensus_agreement_v1', 'reference_consensus_multiple_disagreements_v1'].includes(String(consensus.reason))
      || (Number(consensus.disagreement_count) < 2) !== consensus.apply_allowed
      || consensusPairs.some((pair, index) => !pair || !isCanonicalNonNilUuid(pair.left_asset_id) || !isCanonicalNonNilUuid(pair.right_asset_id)
        || pair.left_asset_id === pair.right_asset_id || !consensusPairDigests[index]
        || [pair.component_error, pair.normalized_extent_error, pair.branch_error, pair.agreement_score]
          .some((metric) => !Number.isInteger(metric) || Number(metric) < 0 || Number(metric) > 100)
        || [pair.left_component_count, pair.right_component_count]
          .some((metric) =>
            !Number.isInteger(metric)
            || Number(metric) < 1
            || Number(metric) > 16)
        || [pair.left_branch_count, pair.right_branch_count]
          .some((metric) =>
            !Number.isInteger(metric)
            || Number(metric) < 1
            || Number(metric) > 31)
        || consensusPairLeftExtents[index]?.length !== 2
        || consensusPairLeftExtents[index]?.some((extent) =>
          !Number.isInteger(extent)
          || Number(extent) < 0
          || Number(extent) > 100)
        || consensusPairRightExtents[index]?.length !== 2
        || consensusPairRightExtents[index]?.some((extent) =>
          !Number.isInteger(extent)
          || Number(extent) < 0
          || Number(extent) > 100)
        || Math.max(
          ...consensusPairLeftExtents[index]!.map(Number),
        ) !== 100
        || Math.max(
          ...consensusPairRightExtents[index]!.map(Number),
        ) !== 100
        || Number(pair.component_error) !== Math.abs(
          Number(pair.left_component_count)
            - Number(pair.right_component_count),
        )
        || Number(pair.branch_error) !== Math.abs(
          Number(pair.left_branch_count)
            - Number(pair.right_branch_count),
        )
        || Number(pair.normalized_extent_error) !== Math.max(
          Math.abs(
            Number(consensusPairLeftExtents[index]![0])
              - Number(consensusPairRightExtents[index]![0]),
          ),
          Math.abs(
            Number(consensusPairLeftExtents[index]![1])
              - Number(consensusPairRightExtents[index]![1]),
          ),
        )
        || Number(pair.agreement_score) !== Math.max(
          0,
          100 - Math.min(
            100,
            Number(pair.normalized_extent_error) * 2
              + Number(pair.component_error) * 20
              + Number(pair.branch_error) * 10,
          ),
        )
      || typeof pair.disagrees !== 'boolean'
        || pair.disagrees !== (
          Number(pair.component_error) > 1
          || Number(pair.branch_error) > 2
          || Number(pair.normalized_extent_error) > 20
        ))))
    || (response.generation_status === 'missing_target_asset'
      && (fusion !== null || consensus !== null))
  ) return null
  if (consensus !== null) {
    const expectedConsensus =
      expectedProfile?.reference_consensus_v1
    const expectedExcludedAssetId =
      expectedConsensus?.excluded_asset_id ?? null
    const activeBindings = (expectedConsensus?.bindings ?? []).filter(
      (binding) => binding.asset_id !== expectedExcludedAssetId,
    )
    const expectedPairs: Array<readonly [string, string]> = []
    for (let left = 0; left < activeBindings.length; left += 1) {
      for (
        let right = left + 1;
        right < activeBindings.length;
        right += 1
      ) {
        expectedPairs.push([
          activeBindings[left]!.asset_id,
          activeBindings[right]!.asset_id,
        ])
      }
    }
    const disagreementCount = consensusPairs.reduce(
      (count, pair) => count + Number(pair?.disagrees === true),
      0,
    )
    const agreementScore = consensusPairs.reduce(
      (sum, pair) => sum + Number(pair?.agreement_score),
      0,
    ) / consensusPairs.length
    const applyAllowed = disagreementCount < 2
    if (
      !expectedConsensus
      || activeBindings.length < 2
      || consensus.excluded_asset_id !== expectedExcludedAssetId
      || consensus.source_count !== activeBindings.length
      || consensus.pair_count !== expectedPairs.length
      || consensusPairs.length !== expectedPairs.length
      || consensusPairs.some((pair, index) => {
        const leftBinding = activeBindings.find((binding) =>
          binding.asset_id === pair?.left_asset_id)
        const rightBinding = activeBindings.find((binding) =>
          binding.asset_id === pair?.right_asset_id)
        const expectedDigest = pair && leftBinding && rightBinding
          ? beginnerReferenceConsensusPairDigestV1(
              leftBinding.asset_id,
              leftBinding.sha256,
              rightBinding.asset_id,
              rightBinding.sha256,
              {
                componentError: Number(pair.component_error),
                normalizedExtentError:
                  Number(pair.normalized_extent_error),
                branchError: Number(pair.branch_error),
                agreementScore: Number(pair.agreement_score),
                disagrees: pair.disagrees === true,
              },
            )
          : null
        return pair?.left_asset_id !== expectedPairs[index]?.[0]
          || pair?.right_asset_id !== expectedPairs[index]?.[1]
          || !beginnerReferenceDescriptorBranchIsCanonicalV1(
            leftBinding?.kind,
            pair?.left_component_count,
            pair?.left_branch_count,
          )
          || !beginnerReferenceDescriptorBranchIsCanonicalV1(
            rightBinding?.kind,
            pair?.right_component_count,
            pair?.right_branch_count,
          )
          || !expectedDigest
          || !consensusPairDigests[index]?.every(
            (byte, digestIndex) => byte === expectedDigest[digestIndex],
          )
      })
      || consensus.disagreement_count !== disagreementCount
      || consensus.agreement_score !== Math.floor(agreementScore)
      || consensus.apply_allowed !== applyAllowed
      || consensus.reason !== (
        applyAllowed
          ? 'reference_consensus_agreement_v1'
          : 'reference_consensus_multiple_disagreements_v1'
      )
    ) return null
  }
  const candidates = candidateInputs.map((candidate, index) => {
    const record = exactCoreDataRecord(candidate, [
      'schema_version',
      'kind',
      'rank',
      'total_score',
      'shape_score',
      'target_approximation_score',
      'foldability_score',
      'step_count_score',
      'paper_efficiency_score',
    ] as const)
    const scores = record && [
      record.total_score,
      record.shape_score,
      record.target_approximation_score,
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
      target_approximation_score: record.target_approximation_score,
      foldability_score: record.foldability_score,
      step_count_score: record.step_count_score,
      paper_efficiency_score: record.paper_efficiency_score,
    }) as BeginnerCandidateScoreV1
  })
  if (candidates.some((candidate) => candidate === null)) return null
  const generatedPlans = generatedPlanInputs.map((plan) => {
    const basePlanKeys = [
      'schema_version', 'kind', 'crease_pattern', 'instruction_codes', 'target_parts',
      'skeleton_segments', 'target_asset',
    ] as const
    const recordWithoutSemantic = exactCoreDataRecord(plan, basePlanKeys)
    const recordWithSemantic = exactCoreDataRecord(plan, [
      ...basePlanKeys,
      'semantic_landmark_provenance',
    ] as const)
    const record = recordWithSemantic ?? recordWithoutSemantic
    const semanticInput =
      recordWithSemantic?.semantic_landmark_provenance
    const pattern = record && exactCoreDataRecord(record.crease_pattern, ['vertices', 'edges'] as const)
    const verticesInput = pattern
      ? snapshotCoreDataArray(
          pattern.vertices,
          MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1,
        )
      : null
    const edgesInput = pattern
      ? snapshotCoreDataArray(
          pattern.edges,
          MAX_BEGINNER_GENERIC_PLAN_EDGES_V1,
        )
      : null
    const instructionCodes = record
      ? snapshotCoreDataArray(record.instruction_codes, 4)
      : null
    const targetPartsInput = record
      ? snapshotCoreDataArray(
          record.target_parts,
          MAX_BEGINNER_TARGET_PART_RECORDS_V1,
        )
      : null
    const skeletonSegmentsInput = record
      ? snapshotCoreDataArray(record.skeleton_segments, 64)
      : null
    if (
      !record
      || record.schema_version !== 1
      || typeof record.kind !== 'string'
      || !pattern
      || !verticesInput
      || !edgesInput
      || !instructionCodes
      || instructionCodes.length < 1
      || !targetPartsInput
      || !skeletonSegmentsInput
      || (recordWithSemantic !== null
        && semanticInput === undefined)
    ) return null
    const kind = record.kind as BeginnerGeneratedPlanV1['kind']
    if (!beginnerGeneratedPlanSizeIsAdmissibleV1(
      kind,
      verticesInput.length,
      edgesInput.length,
    )) return null
    const normalizedPlanInputs = normalizeBeginnerGenerationConstraints(
      {
        schema_version: 1,
        maximum_steps: 1,
        detail_level: 'simple',
        target_category: record.kind === 'composite_generic_target_base'
          ? 'custom_object'
          : record.kind === 'asymmetric_insect_landmark_base' ? 'insect' : 'animal',
        target_parts: targetPartsInput,
        skeleton_segments: skeletonSegmentsInput,
        target_asset: record.target_asset,
        allowed_techniques: ['valley_fold'],
      },
      {
        requireCanonicalGenericIds:
          record.kind === 'composite_generic_target_base',
      },
    )
    if (!normalizedPlanInputs) return null
    if (!beginnerGeneratedPlanTargetPartsAreCompatibleV1(
      kind,
      normalizedPlanInputs.target_parts,
    )) return null
    if (!beginnerGeneratedPlanInstructionsAreCanonicalV1(
      kind,
      instructionCodes as string[],
      normalizedPlanInputs.skeleton_segments,
      instructionContext,
    )) return null
    const semantic = semanticInput === undefined ? null
      : exactCoreDataRecord(semanticInput, [
        'schema_version', 'ordered_bindings', 'physical_ray_group_sha256',
      ] as const)
    const semanticContract = beginnerSemanticContractV1(kind)
    const semanticRoles = semanticContract?.roles ?? null
    const semanticBindingInputs = semantic
      ? snapshotCoreDataArray(semantic.ordered_bindings, 10)
      : null
    const semanticBindings = semanticBindingInputs
      ? semanticBindingInputs.map((value, index) => {
        const binding = exactCoreDataRecord(value, ['ordinal', 'role', 'physical_ray'] as const)
        return binding && binding.ordinal === index && binding.role === semanticRoles?.[index]
          && binding.physical_ray === index % 4
          ? { ordinal: index, role: String(binding.role), physical_ray: Number(binding.physical_ray) }
          : null
      }) : null
    const rayDigestInputs = semantic
      ? snapshotCoreDataArray(semantic.physical_ray_group_sha256, 4)
      : null
    const rayDigests = rayDigestInputs
      ? rayDigestInputs.map(snapshotSha256Bytes)
      : null
    if ((semanticRoles !== null) !== (semantic !== null)
      || (semantic && (semantic.schema_version !== 1 || !semanticBindings
        || semanticBindings.length !== semanticRoles?.length
        || semanticBindings.some((binding) => binding === null)
        || rayDigests?.length !== 4
        || rayDigests.some((digest, index) =>
          !sameBeginnerDigestBytesV1(
            digest,
            semanticContract?.rayDigests[index] ?? [],
          ))))) return null
    const vertices = verticesInput.map((vertex) => {
      const item = exactCoreDataRecord(vertex, ['id', 'position'] as const)
      const position = item && exactCoreDataRecord(item.position, ['x', 'y'] as const)
      if (!item || !isCanonicalNonNilUuid(item.id) || !position
        || !Number.isFinite(position.x) || !Number.isFinite(position.y)) return null
      return { id: item.id, position: { x: Number(position.x), y: Number(position.y) } }
    })
    if (vertices.some((vertex) => vertex === null)) return null
    const admittedVertices = vertices as BeginnerGeneratedPlanV1['crease_pattern']['vertices']
    const vertexIds = new Set(admittedVertices.map((vertex) => vertex.id))
    if (vertexIds.size !== admittedVertices.length) return null
    const edges = edgesInput.map((value) => {
      const edge = exactCoreDataRecord(value, ['id', 'start', 'end', 'kind'] as const)
      if (!edge
        || !isCanonicalNonNilUuid(edge.id) || !isCanonicalNonNilUuid(edge.start)
        || !isCanonicalNonNilUuid(edge.end) || edge.start === edge.end
        || !vertexIds.has(edge.start) || !vertexIds.has(edge.end)
        || !['mountain', 'valley', 'auxiliary'].includes(String(edge.kind))) {
        return null
      }
      return {
        id: edge.id,
        start: edge.start,
        end: edge.end,
        kind: edge.kind,
      } as BeginnerGeneratedPlanV1['crease_pattern']['edges'][number]
    })
    if (edges.some((edge) => edge === null)) return null
    const admittedEdges = edges as BeginnerGeneratedPlanV1['crease_pattern']['edges']
    if (
      new Set(admittedEdges.map((edge) => edge.id)).size
        !== admittedEdges.length
      || admittedVertices.some((vertex) =>
        !admittedEdges.some((edge) =>
          edge.start === vertex.id || edge.end === vertex.id))
    ) return null
    return {
      schema_version: 1,
      kind: record.kind,
      crease_pattern: { vertices: admittedVertices, edges: admittedEdges },
      instruction_codes: instructionCodes.slice() as string[],
      target_parts: normalizedPlanInputs.target_parts,
      skeleton_segments: normalizedPlanInputs.skeleton_segments,
      target_asset: normalizedPlanInputs.target_asset,
      ...(semantic && semanticBindings && rayDigests ? { semantic_landmark_provenance: Object.freeze({
        schema_version: 1 as const,
        ordered_bindings: Object.freeze(semanticBindings.map((binding) =>
          Object.freeze({ ...binding! }))),
        physical_ray_group_sha256: Object.freeze(rayDigests.map(
          (digest) => Object.freeze(Array.from(digest!)),
        )),
      }) } : {}),
    } as BeginnerGeneratedPlanV1
  })
  if (generatedPlans.some((plan) => plan === null)
    || (response.generation_status === 'ready') !== (generatedPlans.length > 0)) return null
  const admittedPlans = generatedPlans as BeginnerGeneratedPlanV1[]
  if (instructionContext === 'candidate' && (
    expectedProfile === null
    || (
      response.generation_status === 'ready'
      && admittedPlans.length !== (
        expectedProfile.generation_constraints.target_category
          === 'custom_object'
          ? 1
          : requestedCandidateCount
      )
    )
    || admittedPlans.some((plan, planIndex) =>
      !expectedProfileAuthorizesCandidatePlanV1(
        plan,
        expectedProfile,
        planIndex,
        instructionContext,
      ))
  )) return null
  if (instructionContext === 'grid' && (
    expectedProfile === null
    || admittedPlans.some((plan) =>
      !expectedProfileAuthorizesCandidatePlanV1(
        plan,
        expectedProfile,
        0,
        instructionContext,
      ))
  )) return null
  const planAssessments = planAssessmentInputs.map((assessment, index) => {
    const record = exactCoreDataRecord(assessment, [
      'kind', 'expected_candidate_edge_id', 'proof_scope', 'apply_allowed', 'reason',
      'shape_approximation_score', 'shape_difference_reason',
      'component_shape_comparison',
    ] as const)
    const componentComparison = record && record.component_shape_comparison === null ? null
      : exactCoreDataRecord(record?.component_shape_comparison, [
        'component_count', 'matched_branch_count', 'work_units', 'extent_score', 'branch_score',
        'bridge_score', 'extent_weight', 'branch_weight', 'bridge_weight',
      ] as const)
    const componentScores = componentComparison && [componentComparison.extent_score,
      componentComparison.branch_score, componentComparison.bridge_score]
    const plan = admittedPlans[index]
    const skeletonBranchCount = plan?.skeleton_segments.length ?? 0
    const componentCount = Number(componentComparison?.component_count)
    const matchedBranchCount = Math.min(
      componentCount,
      skeletonBranchCount,
    )
    const expectedComponentWork =
      matchedBranchCount * skeletonBranchCount
      - matchedBranchCount * (matchedBranchCount - 1) / 2
    const targetBridgeCount = componentCount - 1
    const candidateBridgeCount = Math.max(
      0,
      skeletonBranchCount - componentCount,
    )
    const expectedBridgeScore = 100 - Math.floor(
      Math.abs(targetBridgeCount - candidateBridgeCount) * 100
        / Math.max(targetBridgeCount, candidateBridgeCount, 1),
    )
    if (!record || !plan
      || record.kind !== plan.kind
      || record.expected_candidate_edge_id !== plan.crease_pattern.edges[0]?.id
      || !isCanonicalNonNilUuid(record.expected_candidate_edge_id)
      || !['necessary', 'sufficient', 'indeterminate'].includes(String(record.proof_scope))
      || typeof record.apply_allowed !== 'boolean'
      || (record.shape_approximation_score !== null
        && (!Number.isInteger(record.shape_approximation_score)
          || Number(record.shape_approximation_score) < 0
          || Number(record.shape_approximation_score) > 100))
      || ![
        null,
        'crease_preview_has_no_surface_mesh',
        'certified_flat_surface_v1',
        'component_aware_quantized_shape_v1',
        'bounded_folded_pose_landmarks_v1',
      ].includes(
        record.shape_difference_reason as null | string,
      )
      || (componentComparison !== null && (!componentComparison
        || !Number.isInteger(componentComparison.component_count)
        || Number(componentComparison.component_count) < 2 || Number(componentComparison.component_count) > 8
        || skeletonBranchCount < 1
        || skeletonBranchCount > 16
        || !Number.isInteger(componentComparison.matched_branch_count)
        || Number(componentComparison.matched_branch_count)
          !== matchedBranchCount
        || !Number.isInteger(componentComparison.work_units)
        || Number(componentComparison.work_units)
          !== expectedComponentWork
        || expectedComponentWork < 1
        || expectedComponentWork > 64
        || !componentScores || componentScores.some((score) => !Number.isInteger(score) || Number(score) < 0 || Number(score) > 100)
        || Number(componentComparison.bridge_score)
          !== expectedBridgeScore
        || componentComparison.extent_weight !== 45 || componentComparison.branch_weight !== 35
        || componentComparison.bridge_weight !== 20))
      || ((record.shape_difference_reason === 'component_aware_quantized_shape_v1') !== (componentComparison !== null))
      || ((record.shape_approximation_score === null)
        !== (record.shape_difference_reason === null))
      || (
        expectedProfile?.generation_constraints.target_asset?.kind
          !== 'reference_model'
        && (
          record.shape_approximation_score !== null
          || record.shape_difference_reason !== null
          || componentComparison !== null
        )
      )
      || !beginnerAssessmentOutcomeTupleIsCanonicalV1(
        record.reason,
        record.proof_scope,
        record.apply_allowed,
      )
      || (
        record.reason === 'folded_pose_simulation_failed'
        && (
          record.shape_difference_reason
            !== 'bounded_folded_pose_landmarks_v1'
          || componentComparison !== null
        )
      )
      || (
        record.shape_difference_reason === 'certified_flat_surface_v1'
        && ![
          'global_flat_foldability_proven',
          'fold_path_certificate_unavailable',
          'multi_reference_disagreement',
        ].includes(String(record.reason))
      )
      || (componentComparison !== null
        && record.shape_approximation_score !== Math.floor(
          (
            Number(componentComparison.extent_score) * 45
            + Number(componentComparison.branch_score) * 35
            + Number(componentComparison.bridge_score) * 20
          ) / 100,
        ))
    ) return null
    return Object.freeze({
      kind: record.kind,
      expected_candidate_edge_id: record.expected_candidate_edge_id,
      proof_scope: record.proof_scope,
      apply_allowed: record.apply_allowed,
      reason: record.reason,
      shape_approximation_score: record.shape_approximation_score,
      shape_difference_reason: record.shape_difference_reason,
      component_shape_comparison: componentComparison,
    }) as BeginnerGeneratedPlanAssessmentV1
  })
  if (planAssessments.some((assessment) => assessment === null)) return null
  const admitted = candidates as BeginnerCandidateScoreV1[]
  const scoreKindPriority = Object.freeze({
    recommended: 0,
    shape_focused: 1,
    foldability_focused: 2,
  } as const)
  const expectedScorePrimaryKind = admittedPlans[0]?.kind ?? (
    expectedProfile
      && expectedProfileSelectsGenericCandidatePlanV1(expectedProfile)
      ? 'composite_generic_target_base'
      : expectedProfile
        ? expectedSpecializedPrimaryKindV1(expectedProfile)
        : null
  )
  const expectedTargetApproximationScore =
    expectedProfile && expectedScorePrimaryKind
      ? beginnerExpectedTargetApproximationScoreV1(
          expectedProfile,
          expectedScorePrimaryKind,
        )
      : 0
  const multiReferenceBlocksApply =
    fusion?.apply_allowed === false
    || consensus?.apply_allowed === false
  const candidateScoresAreCanonical =
    instructionContext !== 'candidate'
    || (
      expectedProfile !== null
      && admitted.length >= 1
      && admitted.every((candidate) =>
        candidate.target_approximation_score
          === expectedTargetApproximationScore
        && candidate.step_count_score
          === admitted[0]!.step_count_score
        && candidate.paper_efficiency_score
          === admitted[0]!.paper_efficiency_score
        && candidate.paper_efficiency_score >= 50
        && candidate.total_score === Math.floor(
          (
            candidate.shape_score
              * expectedProfile.shape_fidelity_weight
            + candidate.foldability_score
              * expectedProfile.foldability_weight
            + candidate.step_count_score
              * expectedProfile.step_count_weight
            + candidate.paper_efficiency_score
              * expectedProfile.paper_efficiency_weight
          ) / 100,
        )
        && candidate.foldability_score === (
          candidate.kind === 'recommended'
            ? candidate.step_count_score
            : candidate.kind === 'shape_focused'
              ? Math.max(0, candidate.step_count_score - 10)
              : Math.min(100, candidate.step_count_score + 15)
        ))
      && Array.from({ length: 101 }, (_, base) => base).some(
        (base) => admitted.every((candidate) =>
          candidate.shape_score === (
            candidate.kind === 'recommended'
              ? base
              : candidate.kind === 'shape_focused'
                ? Math.min(100, base + 15)
                : Math.max(0, base - 10)
          )),
      )
    )
  if (
    !candidateScoresAreCanonical
    || new Set(admitted.map((candidate) => candidate.kind)).size
      !== admitted.length
    || admitted.some((candidate, index) => {
      const previous = admitted[index - 1]
      return index > 0 && (
        previous!.total_score < candidate.total_score
        || (
          previous!.total_score === candidate.total_score
          && scoreKindPriority[previous!.kind]
            >= scoreKindPriority[candidate.kind]
        )
      )
    })
    || planAssessments.some((assessment) =>
      (assessment?.reason === 'multi_reference_disagreement')
        !== multiReferenceBlocksApply)
  ) return null
  return Object.freeze({
    schema_version: 1,
    project_instance_id: expectedProjectInstanceId,
    project_id: expectedProjectId,
    revision: expectedRevision,
    requested_candidate_count: requestedCandidateCount,
    bulge_treatment: 'target_shape_approximation',
    elasticity_model: 'not_computed',
    generation_status: response.generation_status as BeginnerCandidateResponseV1['generation_status'],
    generated_plans: admittedPlans,
    plan_assessments: planAssessments as BeginnerGeneratedPlanAssessmentV1[],
    candidates: admitted.slice(),
    multi_reference_fusion: fusion === null ? null : Object.freeze({
      revision: Number(fusion.revision),
      image_sha256: Object.freeze(Array.from(fusionImageSha256!)),
      reference_sha256: Object.freeze(Array.from(fusionReferenceSha256!)),
      source_count: 2 as const,
      image_component_count: Number(fusion.image_component_count),
      reference_component_count: Number(fusion.reference_component_count),
      image_branch_count: Number(fusion.image_branch_count),
      reference_branch_count: Number(fusion.reference_branch_count),
      normalized_extent_error: Number(fusion.normalized_extent_error),
      agreement_score: Number(fusion.agreement_score),
      apply_allowed: fusion.apply_allowed as boolean,
      reason: fusion.reason as 'image_glb_agreement_v1' | 'image_glb_disagreement_v1',
    }) as BeginnerCandidateResponseV1['multi_reference_fusion'],
    reference_consensus_analysis: consensus === null ? null : Object.freeze({
      schema_version: 1 as const,
      revision: Number(consensus.revision),
      source_count: Number(consensus.source_count),
      excluded_asset_id: consensus.excluded_asset_id as string | null,
      pair_count: Number(consensus.pair_count),
      disagreement_count: Number(consensus.disagreement_count),
      agreement_score: Number(consensus.agreement_score),
      apply_allowed: consensus.apply_allowed as boolean,
      reason: consensus.reason as
        | 'reference_consensus_agreement_v1'
        | 'reference_consensus_multiple_disagreements_v1',
      pairs: Object.freeze(consensusPairs.map((pair, index) => Object.freeze({
        left_asset_id: String(pair!.left_asset_id),
        right_asset_id: String(pair!.right_asset_id),
        component_error: Number(pair!.component_error),
        normalized_extent_error: Number(pair!.normalized_extent_error),
        branch_error: Number(pair!.branch_error),
        agreement_score: Number(pair!.agreement_score),
        disagrees: pair!.disagrees as boolean,
        pair_digest_sha256: Object.freeze(
          Array.from(consensusPairDigests[index]!),
        ),
        left_component_count: Number(pair!.left_component_count),
        right_component_count: Number(pair!.right_component_count),
        left_normalized_extents: Object.freeze([
          Number(consensusPairLeftExtents[index]![0]),
          Number(consensusPairLeftExtents[index]![1]),
        ] as [number, number]),
        right_normalized_extents: Object.freeze([
          Number(consensusPairRightExtents[index]![0]),
          Number(consensusPairRightExtents[index]![1]),
        ] as [number, number]),
        left_branch_count: Number(pair!.left_branch_count),
        right_branch_count: Number(pair!.right_branch_count),
      }))),
    }) as BeginnerCandidateResponseV1['reference_consensus_analysis'],
  }) as BeginnerCandidateResponseV1
}

export function normalizeBeginnerDesignProfile(
  value: unknown,
): BeginnerDesignProfileV1 | null {
  const requiredKeys = [
    'schema_version',
    'preset',
    'shape_fidelity_weight',
    'foldability_weight',
    'step_count_weight',
    'paper_efficiency_weight',
    'generation_constraints',
  ] as const
  const optionalKeys = [
    'generation_provenance',
    'reference_surface_landmarks_tenths_mm',
    'outline_edit_authority',
    'archived_reference_model_asset_ids',
    'reference_consensus_v1',
  ] as const
  const record = snapshotCoreDataRecord(value)
  if (!record || requiredKeys.some((key) => !Object.hasOwn(record, key))
    || optionalKeys.some((key) =>
      Object.hasOwn(record, key) && record[key] === undefined)
    || Object.keys(record).some((key) =>
      ![...requiredKeys, ...optionalKeys].includes(key as never))) return null
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
  const provenance = record.generation_provenance === undefined ? null
    : coreDataRecordWithOptionalKeys(
        record.generation_provenance,
        ['schema_version', 'topology_authority_sha256', 'confidence_score',
          'confidence_reasons', 'explicit_override', 'source_asset_fingerprint'] as const,
        ['fold_path_certificate_sha256', 'document_authority_sha256',
          'semantic_landmark_provenance', 'generic_tree', 'reference_consensus',
          'reference_consensus_summary'] as const,
      )
  const consensusSummaryKeys = [
    'schema_version', 'model', 'source_count', 'excluded_count',
    'agreement_score', 'component_subscore', 'extent_subscore',
    'branch_subscore',
  ] as const
  const exportedConsensusSummary =
    provenance?.reference_consensus_summary === undefined
      ? null
      : exactCoreDataRecord(
          provenance.reference_consensus_summary,
          consensusSummaryKeys,
        )
  const provenanceConsensus = provenance?.reference_consensus === undefined ? null
    : coreDataRecordWithOptionalKeys(
        provenance.reference_consensus,
        ['schema_version', 'source_revision', 'bindings',
          'pair_digests_sha256', 'summary'] as const,
        ['excluded_asset_id'] as const,
      )
  const provenanceConsensusSummary = provenanceConsensus === null
    ? null
    : exactCoreDataRecord(provenanceConsensus.summary, consensusSummaryKeys)
  const provenanceConsensusBindingInputs = provenanceConsensus === null
    ? null
    : snapshotCoreDataArray(provenanceConsensus.bindings, 4)
  const normalizedProvenanceConsensusBindings =
    provenanceConsensusBindingInputs === null
      ? null
      : provenanceConsensusBindingInputs.map((raw) => {
          const binding = exactCoreDataRecord(
            raw,
            ['kind', 'asset_id', 'sha256', 'quality'] as const,
          )
          const digest = binding ? snapshotSha256Bytes(binding.sha256) : null
          if (
            !binding
            || (
              binding.kind !== 'image'
              && binding.kind !== 'reference_model'
            )
            || !isCanonicalNonNilUuid(binding.asset_id)
            || !digest
            || !Number.isInteger(binding.quality)
            || Number(binding.quality) < 0
            || Number(binding.quality) > 100
          ) return null
          return Object.freeze({
            kind: binding.kind as 'image' | 'reference_model',
            asset_id: String(binding.asset_id),
            sha256: digest,
            quality: Number(binding.quality),
          })
        })
  const provenanceConsensusPairDigestInputs = provenanceConsensus === null
    ? null
    : snapshotCoreDataArray(provenanceConsensus.pair_digests_sha256, 6)
  const normalizedProvenanceConsensusPairDigests =
    provenanceConsensusPairDigestInputs === null
      ? null
      : provenanceConsensusPairDigestInputs.map(snapshotSha256Bytes)
  const genericTree = provenance?.generic_tree === undefined ? null
    : coreDataRecordWithOptionalKeys(
        provenance.generic_tree,
        ['schema_version', 'source', 'tree_topology_sha256',
          'normalized_length_ratios', 'orientation', 'generator_version',
          'authorizes_apply'] as const,
        ['target_category', 'asset_content_sha256', 'instruction_proposal'] as const,
      )
  const treeProposal = genericTree?.instruction_proposal === undefined ? null : exactCoreDataRecord(
    genericTree.instruction_proposal, ['schema_version', 'topology_sha256', 'generator_version', 'authorizes_apply',
      'physical_motion_proof', 'steps'] as const)
  const genericTreeRatios = genericTree === null
    ? null
    : snapshotCoreDataArray(genericTree.normalized_length_ratios, 16)
  const genericTreeAssetDigest =
    genericTree?.asset_content_sha256 === undefined
      ? null
      : snapshotSha256Bytes(genericTree.asset_content_sha256)
  const genericTreeTopologyDigest = genericTree === null
    ? null
    : snapshotSha256Bytes(genericTree.tree_topology_sha256)
  const treeProposalTopologyDigest = treeProposal === null
    ? null
    : snapshotSha256Bytes(treeProposal.topology_sha256)
  const treeProposalStepInputs = treeProposal === null
    ? null
    : snapshotCoreDataArray(treeProposal.steps, 16)
  const semantic = provenance?.semantic_landmark_provenance === undefined
    ? null
    : exactCoreDataRecord(
        provenance.semantic_landmark_provenance,
        ['schema_version', 'ordered_bindings', 'physical_ray_group_sha256'] as const,
      )
  const semanticBindings = semantic === null
    ? null
    : snapshotCoreDataArray(semantic.ordered_bindings, 10)
  const semanticRayDigestInputs = semantic === null
    ? null
    : snapshotCoreDataArray(semantic.physical_ray_group_sha256, 4)
  const semanticRayDigests = semanticRayDigestInputs === null
    ? null
    : semanticRayDigestInputs.map(snapshotSha256Bytes)
  const confidenceReasons = provenance === null
    ? null
    : snapshotCoreDataArray(provenance.confidence_reasons, 8)
  const topologyAuthorityDigest = provenance === null
    ? null
    : snapshotSha256Bytes(provenance.topology_authority_sha256)
  const foldPathCertificateDigest =
    provenance?.fold_path_certificate_sha256 === undefined
      ? null
      : snapshotSha256Bytes(provenance.fold_path_certificate_sha256)
  const documentAuthorityDigest =
    provenance?.document_authority_sha256 === undefined
      ? null
      : snapshotSha256Bytes(provenance.document_authority_sha256)
  const semanticContract = beginnerSemanticContractV1(
    semanticBindings?.length ?? -1,
  )
  const semanticRoles = semanticContract?.roles ?? null
  const normalizedSemanticBindings = semanticBindings === null
    ? null
    : semanticBindings.map((raw, index) => {
        const binding = exactCoreDataRecord(
          raw,
          ['ordinal', 'role', 'physical_ray'] as const,
        )
        if (!binding
          || binding.ordinal !== index
          || binding.role !== semanticRoles?.[index]
          || binding.physical_ray !== index % 4) return null
        return Object.freeze({
          ordinal: index,
          role: String(binding.role),
          physical_ray: Number(binding.physical_ray),
        })
      })
  const normalizedTreeProposalSteps = treeProposalStepInputs === null
    ? null
    : treeProposalStepInputs.map((rawStep, index, all) => {
        const step = exactCoreDataRecord(
          rawStep,
          [
            'canonical_crease_id', 'tree_depth', 'assignment',
            'target_branch', 'fixed_side', 'caution',
          ] as const,
        )
        const previous = index === 0
          ? null
          : exactCoreDataRecord(
              all[index - 1],
              [
                'canonical_crease_id', 'tree_depth', 'assignment',
                'target_branch', 'fixed_side', 'caution',
              ] as const,
            )
        if (
          !step
          || !isNonEmptyUtf8StringWithin(step.canonical_crease_id, 64)
          || !Number.isInteger(step.tree_depth)
          || Number(step.tree_depth) < 0
          || Number(step.tree_depth) > 255
          || (
            step.assignment !== 'mountain'
            && step.assignment !== 'valley'
          )
          || !isNonEmptyUtf8StringWithin(step.target_branch, 96)
          || (step.fixed_side !== 'root' && step.fixed_side !== 'leaf')
          || !isNonEmptyUtf8StringWithin(step.caution, 256)
          || (
            index > 0
            && (
              previous === null
              || !Number.isInteger(previous.tree_depth)
              || !isNonEmptyUtf8StringWithin(
                previous.canonical_crease_id,
                64,
              )
              || Number(previous.tree_depth) > Number(step.tree_depth)
              || (
                previous.tree_depth === step.tree_depth
                && compareUtf8Strings(
                  previous.canonical_crease_id,
                  step.canonical_crease_id,
                ) >= 0
              )
            )
          )
        ) return null
        return Object.freeze({
          canonical_crease_id: String(step.canonical_crease_id),
          tree_depth: Number(step.tree_depth),
          assignment: step.assignment as 'mountain' | 'valley',
          target_branch: String(step.target_branch),
          fixed_side: step.fixed_side as 'root' | 'leaf',
          caution: String(step.caution),
        })
      })
  const exportedConsensusMatchesEmbedded =
    exportedConsensusSummary === null
    || provenanceConsensusSummary === null
    || consensusSummaryKeys.every((key) =>
      exportedConsensusSummary[key] === provenanceConsensusSummary[key])
  const semanticIsValid = provenance?.semantic_landmark_provenance === undefined
    ? true
    : semantic !== null
      && semantic.schema_version === 1
      && semanticRoles !== null
      && normalizedSemanticBindings !== null
      && normalizedSemanticBindings.every((binding) => binding !== null)
      && semanticRayDigests?.length === 4
      && semanticRayDigests.every((digest, index) =>
        sameBeginnerDigestBytesV1(
          digest,
          semanticContract?.rayDigests[index] ?? [],
        ))
  if (
    record.generation_provenance !== undefined
    && (
      !provenance
      || provenance.schema_version !== 1
      || !topologyAuthorityDigest
      || (
        provenance.fold_path_certificate_sha256 !== undefined
        && !foldPathCertificateDigest
      )
      || (
        provenance.document_authority_sha256 !== undefined
        && !documentAuthorityDigest
      )
      || !semanticIsValid
      || !Number.isInteger(provenance.confidence_score)
      || Number(provenance.confidence_score) < 0
      || Number(provenance.confidence_score) > 100
      || !confidenceReasons
      || confidenceReasons.some((reason) =>
        !isNonEmptyUtf8StringWithin(reason, 64))
      || typeof provenance.explicit_override !== 'boolean'
      || !isNonEmptyUtf8StringWithin(
        provenance.source_asset_fingerprint,
        128,
      )
      || (
        provenance.reference_consensus !== undefined
        && (
          !provenanceConsensus
          || provenanceConsensus.schema_version !== 1
          || !Number.isSafeInteger(provenanceConsensus.source_revision)
          || Number(provenanceConsensus.source_revision) < 0
          || provenanceConsensusBindingInputs === null
          || provenanceConsensusBindingInputs.length < 2
          || normalizedProvenanceConsensusBindings === null
          || normalizedProvenanceConsensusBindings.some(
            (binding) => binding === null,
          )
          || new Set(
            normalizedProvenanceConsensusBindings.map(
              (binding) => binding?.asset_id,
            ),
          ).size !== normalizedProvenanceConsensusBindings.length
          || (
            provenanceConsensus.excluded_asset_id !== undefined
            && (
              !isCanonicalNonNilUuid(
                provenanceConsensus.excluded_asset_id,
              )
              || !normalizedProvenanceConsensusBindings.some(
                (binding) =>
                  binding?.asset_id
                    === provenanceConsensus.excluded_asset_id,
              )
            )
          )
          || provenanceConsensusPairDigestInputs === null
          || provenanceConsensusPairDigestInputs.length < 1
          || normalizedProvenanceConsensusPairDigests === null
          || normalizedProvenanceConsensusPairDigests.some(
            (digest) => digest === null,
          )
          || !provenanceConsensusSummary
          || provenanceConsensusSummary.schema_version !== 1
          || provenanceConsensusSummary.model
            !== 'component_extent_branch_v1'
          || provenanceConsensusSummary.source_count
            !== provenanceConsensusBindingInputs.length
          || provenanceConsensusSummary.excluded_count
            !== (
              provenanceConsensus.excluded_asset_id === undefined ? 0 : 1
            )
          || [
            provenanceConsensusSummary.agreement_score,
            provenanceConsensusSummary.component_subscore,
            provenanceConsensusSummary.extent_subscore,
            provenanceConsensusSummary.branch_subscore,
          ].some((score) =>
            !Number.isInteger(score)
            || Number(score) < 0
            || Number(score) > 100)
        )
      )
      || (
        provenance.reference_consensus_summary !== undefined
        && (
          !exportedConsensusSummary
          || exportedConsensusSummary.schema_version !== 1
          || exportedConsensusSummary.model
            !== 'component_extent_branch_v1'
          || !Number.isInteger(exportedConsensusSummary.source_count)
          || Number(exportedConsensusSummary.source_count) < 2
          || Number(exportedConsensusSummary.source_count) > 4
          || ![0, 1].includes(
            Number(exportedConsensusSummary.excluded_count),
          )
          || ![
            exportedConsensusSummary.agreement_score,
            exportedConsensusSummary.component_subscore,
            exportedConsensusSummary.extent_subscore,
            exportedConsensusSummary.branch_subscore,
          ].every((score) =>
            Number.isInteger(score)
            && Number(score) >= 0
            && Number(score) <= 100)
          || !exportedConsensusMatchesEmbedded
        )
      )
      || (
        provenance.generic_tree !== undefined
        && (
          !genericTree
          || genericTree.schema_version !== 1
          || (
            genericTree.source !== 'image_silhouette'
            && genericTree.source !== 'glb_geometry'
            && genericTree.source !== 'manual_skeleton'
          )
          || (
            genericTree.target_category !== undefined
            && genericTree.target_category !== 'custom_object'
          )
          || (
            genericTree.asset_content_sha256 !== undefined
            && !genericTreeAssetDigest
          )
          || !genericTreeTopologyDigest
          || genericTreeRatios === null
          || genericTreeRatios.length < 1
          || genericTreeRatios.some((ratio) =>
            !Number.isSafeInteger(ratio)
            || Number(ratio) < 1_000_000
            || Number(ratio) > 4_294_967_295)
          || (
            genericTree.orientation !== 'horizontal'
            && genericTree.orientation !== 'vertical'
          )
          || genericTree.generator_version !== 1
          || genericTree.authorizes_apply !== false
          || (
            genericTree.instruction_proposal !== undefined
            && (
              !treeProposal
              || treeProposal.schema_version !== 1
              || !treeProposalTopologyDigest
              || !genericTreeTopologyDigest.every(
                (byte, index) =>
                  byte === treeProposalTopologyDigest[index],
              )
              || treeProposal.generator_version !== 1
              || treeProposal.authorizes_apply !== false
              || treeProposal.physical_motion_proof !== false
              || treeProposalStepInputs === null
              || treeProposalStepInputs.length < 1
              || normalizedTreeProposalSteps === null
              || normalizedTreeProposalSteps.some((step) => step === null)
            )
          )
        )
      )
    )
  ) return null
  const landmarkInputs =
    record.reference_surface_landmarks_tenths_mm === undefined
      ? null
      : snapshotCoreDataArray(
          record.reference_surface_landmarks_tenths_mm,
          256,
        )
  const normalizedLandmarks = landmarkInputs === null
    ? null
    : landmarkInputs.map((point) => {
        const coordinates = snapshotCoreDataArray(point, 3)
        if (
          coordinates?.length !== 3
          || coordinates.some((coordinate) =>
            !Number.isInteger(coordinate)
            || Number(coordinate) < -2_147_483_648
            || Number(coordinate) > 2_147_483_647)
        ) return null
        return Object.freeze(
          coordinates.map(Number),
        ) as readonly [number, number, number]
      })
  if (
    record.reference_surface_landmarks_tenths_mm !== undefined
    && (
      landmarkInputs === null
      || landmarkInputs.length < 1
      || normalizedLandmarks === null
      || normalizedLandmarks.some((point) => point === null)
    )
  ) return null
  const outlineAuthority = record.outline_edit_authority === undefined ? null
    : exactCoreDataRecord(record.outline_edit_authority, [
        'schema_version', 'source_asset_id', 'source_sha256', 'edits',
      ] as const)
  const outlineSourceDigest = outlineAuthority === null
    ? null
    : snapshotSha256Bytes(outlineAuthority.source_sha256)
  const outlineEditInputs = outlineAuthority === null
    ? null
    : snapshotCoreDataArray(outlineAuthority.edits, 8)
  if (
    record.outline_edit_authority !== undefined
    && (
      !outlineAuthority
      || outlineAuthority.schema_version !== 1
      || !isCanonicalNonNilUuid(outlineAuthority.source_asset_id)
      || !outlineSourceDigest
      || outlineEditInputs === null
      || outlineEditInputs.length < 1
    )
  ) return null
  const outlineEdits = outlineEditInputs === null ? [] : outlineEditInputs.map((edit) => {
    const kind = snapshotCoreDataRecord(edit)?.kind
    const record = kind === 'split_vertical'
      ? exactCoreDataRecord(edit, ['kind', 'source_candidate_id', 'split_x', 'fragment_kinds'] as const)
      : kind === 'merge'
        ? exactCoreDataRecord(edit, ['kind', 'source_candidate_ids', 'merged_kind'] as const) : null
    const data = record as Readonly<Record<string, unknown>> | null
    const validPartKind = (value: unknown) => typeof value === 'string'
      && ['head', 'torso', 'leg', 'horn', 'ear', 'wing', 'fin', 'antenna', 'tail'].includes(value)
    const validCandidateId = (value: unknown) => Number.isInteger(value)
      && Number(value) >= 0 && Number(value) <= 255
    const fragmentKinds = kind === 'split_vertical'
      ? snapshotCoreDataArray(data?.fragment_kinds, 2)
      : null
    const sourceCandidateIds = kind === 'merge'
      ? snapshotCoreDataArray(data?.source_candidate_ids, 2)
      : null
    if (
      !data
      || (
        kind === 'split_vertical'
        && (
          !validCandidateId(data.source_candidate_id)
          || !Number.isSafeInteger(data.split_x)
          || Number(data.split_x) < 0
          || Number(data.split_x) > 4_294_967_295
          || fragmentKinds?.length !== 2
          || !fragmentKinds.every(validPartKind)
          || fragmentKinds[0] === fragmentKinds[1]
        )
      )
      || (
        kind === 'merge'
        && (
          sourceCandidateIds?.length !== 2
          || !validCandidateId(sourceCandidateIds[0])
          || !validCandidateId(sourceCandidateIds[1])
          || Number(sourceCandidateIds[0]) >= Number(sourceCandidateIds[1])
          || !validPartKind(data.merged_kind)
        )
      )
    ) return null
    return kind === 'split_vertical'
      ? Object.freeze({
          kind,
          source_candidate_id: Number(data.source_candidate_id),
          split_x: Number(data.split_x),
          fragment_kinds: Object.freeze(fragmentKinds!.slice()),
        })
      : Object.freeze({
          kind,
          source_candidate_ids: Object.freeze(sourceCandidateIds!.map(Number)),
          merged_kind: String(data.merged_kind),
        })
  })
  if (outlineEdits.some((edit) => edit === null)) return null
  const archivedAssetInputs =
    record.archived_reference_model_asset_ids === undefined
      ? Object.freeze([] as unknown[])
      : snapshotCoreDataArray(record.archived_reference_model_asset_ids, 8)
  if (
    !archivedAssetInputs
    || archivedAssetInputs.some((id) => !isCanonicalNonNilUuid(id))
    || new Set(archivedAssetInputs).size !== archivedAssetInputs.length
  ) return null
  const archivedAssets = archivedAssetInputs.map(String)
  const consensus = record.reference_consensus_v1 === undefined ? null
    : coreDataRecordWithOptionalKeys(
        record.reference_consensus_v1,
        ['schema_version', 'bindings'] as const,
        ['excluded_asset_id'] as const,
      )
  const consensusBindingInputs = consensus === null
    ? null
    : snapshotCoreDataArray(consensus.bindings, 4)
  if (
    record.reference_consensus_v1 !== undefined
    && (
      !consensus
      || consensus.schema_version !== 1
      || consensusBindingInputs === null
      || consensusBindingInputs.length < 2
    )
  ) return null
  const normalizedConsensusBindings = consensusBindingInputs === null ? [] : consensusBindingInputs.map((raw) => {
    const binding = exactCoreDataRecord(raw, ['kind', 'asset_id', 'sha256', 'quality'] as const)
    const digest = binding ? snapshotSha256Bytes(binding.sha256) : null
    if (!binding
      || (binding.kind !== 'image' && binding.kind !== 'reference_model')
      || !isCanonicalNonNilUuid(binding.asset_id) || !digest
      || !Number.isInteger(binding.quality) || Number(binding.quality) < 0 || Number(binding.quality) > 100) return null
    return Object.freeze({ kind: binding.kind as 'image' | 'reference_model', asset_id: String(binding.asset_id),
      sha256: digest, quality: Number(binding.quality) })
  })
  if (normalizedConsensusBindings.some((binding) => binding === null)
    || new Set(normalizedConsensusBindings.map((binding) => binding?.asset_id)).size !== normalizedConsensusBindings.length
    || (consensus?.excluded_asset_id !== undefined && (!isCanonicalNonNilUuid(consensus.excluded_asset_id)
      || !normalizedConsensusBindings.some((binding) => binding?.asset_id === consensus.excluded_asset_id)))) return null
  return Object.freeze({
    schema_version: 1,
    preset: record.preset,
    shape_fidelity_weight: weights[0],
    foldability_weight: weights[1],
    step_count_weight: weights[2],
    paper_efficiency_weight: weights[3],
    generation_constraints: generationConstraints,
    ...(normalizedLandmarks === null ? {} : {
      reference_surface_landmarks_tenths_mm: Object.freeze(
        normalizedLandmarks.map((point) => point!),
      ),
    }),
    ...(outlineAuthority === null ? {} : { outline_edit_authority: Object.freeze({
      schema_version: 1 as const,
      source_asset_id: String(outlineAuthority.source_asset_id),
      source_sha256: outlineSourceDigest as ReadonlyArray<number>,
      edits: Object.freeze(outlineEdits as ReadonlyArray<Readonly<Record<string, unknown>>>),
    }) }),
    ...(archivedAssets.length === 0 ? {} : {
      archived_reference_model_asset_ids: Object.freeze(archivedAssets.slice() as string[]),
    }),
    ...(consensus === null ? {} : { reference_consensus_v1: Object.freeze({
      schema_version: 1 as const,
      bindings: Object.freeze(normalizedConsensusBindings as NonNullable<BeginnerDesignProfileV1['reference_consensus_v1']>['bindings']),
      ...(consensus.excluded_asset_id === undefined ? {} : { excluded_asset_id: String(consensus.excluded_asset_id) }),
    }) }),
    ...(provenance === null ? {} : { generation_provenance: Object.freeze({
      schema_version: 1 as const,
      topology_authority_sha256:
        topologyAuthorityDigest as ReadonlyArray<number>,
      ...(provenance.fold_path_certificate_sha256 === undefined ? {} : {
        fold_path_certificate_sha256:
          foldPathCertificateDigest as ReadonlyArray<number>,
      }),
      ...(provenance.document_authority_sha256 === undefined ? {} : {
        document_authority_sha256:
          documentAuthorityDigest as ReadonlyArray<number>,
      }),
      confidence_score: Number(provenance.confidence_score),
      confidence_reasons: Object.freeze(
        confidenceReasons!.map(String),
      ),
      explicit_override: provenance.explicit_override as boolean,
      source_asset_fingerprint: provenance.source_asset_fingerprint as string,
      ...(semantic === null ? {} : {
        semantic_landmark_provenance: Object.freeze({
          schema_version: 1 as const,
          ordered_bindings: Object.freeze(
            normalizedSemanticBindings!.map((binding) => binding!),
          ),
          physical_ray_group_sha256: Object.freeze(
            semanticRayDigests!.map((digest) => digest!),
          ),
        }),
      }),
      ...(exportedConsensusSummary === null ? {} : { reference_consensus_summary: Object.freeze({ ...exportedConsensusSummary }) }),
      ...(provenanceConsensus === null ? {} : { reference_consensus: Object.freeze({
        schema_version: 1 as const, source_revision: Number(provenanceConsensus.source_revision),
        bindings: Object.freeze(
          normalizedProvenanceConsensusBindings!.map((binding) => binding!),
        ),
        ...(provenanceConsensus.excluded_asset_id === undefined ? {} : { excluded_asset_id: String(provenanceConsensus.excluded_asset_id) }),
        pair_digests_sha256: Object.freeze(
          normalizedProvenanceConsensusPairDigests!.map((digest) => digest!),
        ),
        summary: Object.freeze({ ...provenanceConsensusSummary! }),
      }) }),
      ...(genericTree === null ? {} : { generic_tree: Object.freeze({
        schema_version: 1 as const,
        ...(genericTree.target_category === undefined ? {} : { target_category: 'custom_object' as const }),
        source: genericTree.source as 'image_silhouette' | 'glb_geometry' | 'manual_skeleton',
        ...(genericTree.asset_content_sha256 === undefined ? {} : {
          asset_content_sha256:
            genericTreeAssetDigest as ReadonlyArray<number>,
        }),
        tree_topology_sha256:
          genericTreeTopologyDigest as ReadonlyArray<number>,
        normalized_length_ratios: Object.freeze(
          genericTreeRatios!.map(Number),
        ),
        orientation: genericTree.orientation as 'horizontal' | 'vertical', generator_version: 1 as const,
        authorizes_apply: false as const,
        ...(treeProposal === null ? {} : { instruction_proposal: Object.freeze({
          schema_version: 1 as const,
          topology_sha256:
            treeProposalTopologyDigest as ReadonlyArray<number>,
          generator_version: 1 as const, authorizes_apply: false as const, physical_motion_proof: false as const,
          steps: Object.freeze(
            normalizedTreeProposalSteps!.map((step) => step!),
          ),
        }) }),
      }) }),
    }) }),
  }) as BeginnerDesignProfileV1
}

function sameBeginnerReferenceConsensusV1(
  actual: BeginnerDesignProfileV1['reference_consensus_v1'],
  expected: BeginnerDesignProfileV1['reference_consensus_v1'],
): boolean {
  if (actual === undefined || expected === undefined) {
    return actual === expected
  }
  return actual.schema_version === expected.schema_version
    && actual.excluded_asset_id === expected.excluded_asset_id
    && actual.bindings.length === expected.bindings.length
    && actual.bindings.every((binding, index) => {
      const expectedBinding = expected.bindings[index]
      return expectedBinding !== undefined
        && binding.kind === expectedBinding.kind
        && binding.asset_id === expectedBinding.asset_id
        && binding.quality === expectedBinding.quality
        && binding.sha256.length === expectedBinding.sha256.length
        && binding.sha256.every(
          (byte, digestIndex) =>
            byte === expectedBinding.sha256[digestIndex],
        )
    })
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
    && JSON.stringify(profile.generation_provenance) === JSON.stringify(expected.generation_provenance)
    && JSON.stringify(profile.reference_surface_landmarks_tenths_mm)
      === JSON.stringify(expected.reference_surface_landmarks_tenths_mm)
    && JSON.stringify(profile.outline_edit_authority)
      === JSON.stringify(expected.outline_edit_authority)
    && JSON.stringify(profile.archived_reference_model_asset_ids ?? [])
      === JSON.stringify(expected.archived_reference_model_asset_ids ?? [])
    && sameBeginnerReferenceConsensusV1(
      profile.reference_consensus_v1,
      expected.reference_consensus_v1,
    )
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

export { DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1 }

type VertexCoordinateExpressionBindingBase = Readonly<{
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
}>

export type VertexCoordinateExpressionBinding =
  VertexCoordinateExpressionBindingBase & (
    | Readonly<{
      schema_version: 1
      transcendental_model_id?: never
    }>
    | Readonly<{
      schema_version: 2
      transcendental_model_id: typeof DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    }>
  )

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
export type PathCertificateReferenceV1 = Readonly<{
  version: 1
  model_id: 'bounded_certified_pose_graph_path_reference_v1'
  binding_sha256: readonly number[]
  source_pose_sha256: readonly number[]
  target_pose_sha256: readonly number[]
  source_model_binding_sha256: readonly number[]
  transition_count: number
}>
export type InstructionVisual = {
  named_technique_compiler_v1?: Readonly<{
    version: 1
    model_id: 'certified_named_technique_compiler_metadata_v1'
    technique_kind: BasicFoldTimelinePreviewRequestV1['techniqueKind']
    segment_index: number
    segment_count: number
    compiler_output_sha256: readonly number[]
  }> | null
  cycle_layer_order_proof_v1?: Readonly<{
    version: 1
    model_id: 'native_continuous_layer_transport_certificate_v1'
    target_order_sha256: readonly number[]
    transition_count: number
    pairs: readonly Readonly<{ lower_face: string; upper_face: string }>[]
  }> | null
  path_certificate_reference_v1?: PathCertificateReferenceV1 | null
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
    kind: 'pinch' | 'hold' | 'push' | 'regrip'
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

export type AssignedLocalSufficiencyResponseV1 = Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
  result:
    | Readonly<{
      status: 'proven'
      model_id: 'assigned_single_vertex_unique_blb_crimp_v1'
      vertex: string
      reduction_steps: number
      reductions: readonly Readonly<{ first_crease: string; second_crease: string }>[]
    }>
    | Readonly<{
      status: 'indeterminate'
      vertex: string
      reason:
        | 'vertex_unavailable'
        | 'necessary_conditions_not_satisfied'
        | 'reduction_theorem_not_applicable'
        | 'resource_limit'
    }>
  authorizesProjectMutation: false
}>

export type AssignedLocalSufficiencySummaryResponseV1 = Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprint: string
  vertices: readonly (
    | Readonly<{ status: 'necessary_failed'; vertex: string }>
    | Readonly<{
      status: 'sufficient_proven'
      vertex: string
      model_id: 'assigned_single_vertex_unique_blb_crimp_v1'
      reduction_steps: number
    }>
    | Readonly<{
      status: 'indeterminate'
      vertex: string
      reason: 'vertex_unavailable' | 'reduction_theorem_not_applicable' | 'resource_limit' | 'cancelled'
    }>
  )[]
  totalReductionSteps: number
  authorizesProjectMutation: false
}>

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

export type EffectiveCutReadOnlyRequestV1 = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  expectedFoldModelFingerprint: string
  requestedComponentKeys: readonly (readonly number[])[]
}>

export type EffectiveCutReadOnlyResponseV1 = Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprint: string
  effectiveSnapshotFingerprint: readonly number[]
  geometryModelId: 'effective_cut_collision_geometry_v1'
  geometryFingerprint: readonly number[]
  pairObservationModelId: 'effective_cut_source_flat_pair_observation_v1'
  pairObservationFingerprint: readonly number[]
  multiHingeGapModelId: 'effective_cut_multi_hinge_union_gap_diagnostic_v1'
  multiHingeGapFingerprint: readonly number[]
  sourceFlatPairCount: number
  separatedPairs: number
  touchingPairs: number
  sharedHingeCorridorObservedPairs: number
  sharedVertexCorridorObservedPairs: number
  penetratingPairs: number
  indeterminatePairs: number
  multiHingePairs: number
  multiHingeUnionCorridorUnprovedPairs: number
  authorizesProjectMutation: false
  authorizesPersistence: false
  authorizesSimulationAdmission: false
  authorizesPairClassification: false
  authorizesCollisionFreeClassification: false
  authorizesPoseSolving: false
  authorizesMaterialRemoval: false
}>

export type EffectiveCutCandidateListRequestV1 = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  expectedFoldModelFingerprint: string
}>

export type EffectiveCutCandidateV1 = Readonly<{
  componentKey: readonly number[]
  ownsOriginalBoundary: false
  faceCount: number
  areaSquareMm: number
  closureComponentCount: number
  closureFaceCount: number
  nestedDependencyCount: number
}>

export type EffectiveCutCandidateListResponseV1 = Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprint: string
  modelId: 'cut_material_component_selection_diagnostic_v1'
  diagnosticFingerprint: readonly number[]
  totalComponentCount: number
  boundaryComponentCount: 1
  candidates: readonly EffectiveCutCandidateV1[]
  authorizesProjectMutation: false
  authorizesPersistence: false
  authorizesSimulationAdmission: false
  authorizesMaterialRemoval: false
}>

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

export function updateBeginnerReferenceConsensus(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  selections: ReadonlyArray<Readonly<{ kind: 'image' | 'reference_model'; asset_id: string }>>,
) {
  if (selections.length < 2 || selections.length > 4
    || new Set(selections.map((selection) => selection.asset_id)).size !== selections.length
    || selections.some((selection) => !['image', 'reference_model'].includes(selection.kind)
      || !isCanonicalNonNilUuid(selection.asset_id))) {
    return Promise.reject(new Error('invalid reference consensus selection'))
  }
  return invoke<ProjectSnapshot>('update_beginner_reference_consensus', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    selections: selections.map((selection) => ({ kind: selection.kind, asset_id: selection.asset_id })),
  })
}

export function importBeginnerReferenceModel(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<ProjectSnapshot>('import_beginner_reference_model', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
}

export function activateBeginnerReferenceModelAsset(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
  assetId: string,
) {
  if (!isCanonicalNonNilUuid(assetId)) return Promise.reject(new Error('invalid reference model asset'))
  return invoke<ProjectSnapshot>('activate_beginner_reference_model_asset', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, assetId,
  })
}

export function archiveBeginnerReferenceModelAsset(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
  assetId: string, archived: boolean,
) {
  if (!isCanonicalNonNilUuid(assetId)) return Promise.reject(new Error('invalid reference model asset'))
  return invoke<ProjectSnapshot>('archive_beginner_reference_model_asset', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, assetId, archived,
  })
}

export type BeginnerReferenceModelGeometry = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
  asset_id: string
  positions: ReadonlyArray<readonly [number, number, number]>
  triangle_indices: ReadonlyArray<readonly [number, number, number]>
  material_color: readonly [number, number, number, number]
}>

export async function getBeginnerReferenceModelGeometry(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
): Promise<BeginnerReferenceModelGeometry> {
  const value = await invoke<unknown>('get_beginner_reference_model_geometry', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
  const record = exactCoreDataRecord(value, [
    'project_instance_id', 'project_id', 'revision', 'asset_id',
    'positions', 'triangle_indices', 'material_color',
  ] as const)
  if (!record
    || !matchesProjectOccGuard({
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    }, record as Readonly<{
      project_instance_id: string
      project_id: string
      revision: number
    }>)
    || !isCanonicalNonNilUuid(record.asset_id)
    || !Array.isArray(record.positions) || record.positions.length < 1
    || record.positions.length > 20_000
    || !Array.isArray(record.triangle_indices) || record.triangle_indices.length < 1
    || record.triangle_indices.length > 40_000
    || !isBoundedIntegerTuple(record.material_color, 4, 255)
    || record.material_color.some((channel) => channel < 0)) {
    throw new Error('invalid reference model geometry')
  }
  const positions = record.positions.map((position) => {
    if (!Array.isArray(position) || position.length !== 3
      || position.some((coordinate) => typeof coordinate !== 'number'
        || !Number.isFinite(coordinate) || Math.abs(coordinate) > 1_000_000)) {
      throw new Error('invalid reference model geometry')
    }
    return Object.freeze([position[0], position[1], position[2]] as const)
  })
  const triangleIndices = record.triangle_indices.map((triangle) => {
    if (!Array.isArray(triangle) || triangle.length !== 3
      || triangle.some((index) => !Number.isInteger(index)
        || index < 0 || index >= positions.length)) {
      throw new Error('invalid reference model geometry')
    }
    return Object.freeze([triangle[0], triangle[1], triangle[2]] as const)
  })
  return Object.freeze({
    project_instance_id: expectedProjectInstanceId,
    project_id: expectedProjectId,
    revision: expectedRevision,
    asset_id: record.asset_id,
    positions: Object.freeze(positions),
    triangle_indices: Object.freeze(triangleIndices),
    material_color: Object.freeze(record.material_color.slice()) as unknown as
      readonly [number, number, number, number],
  })
}

export type BeginnerReferenceModelSuggestionV1 = Readonly<{
  asset_id: string
  source_asset_sha256: readonly number[]
  bbox_min_tenths_mm: readonly [number, number, number]
  bbox_max_tenths_mm: readonly [number, number, number]
  dominant_normal_milli: readonly [number, number, number]
  surface_area_milli: number
  surface_landmarks_tenths_mm: readonly (readonly [number, number, number])[]
  surface_ranges: readonly Readonly<{
    id: number
    triangle_indices: readonly number[]
    range_min_tenths_mm: readonly [number, number, number]
    range_max_tenths_mm: readonly [number, number, number]
    digest_sha256: readonly number[]
  }>[]
  protrusions: readonly NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number][]
  general_protrusion_candidates: readonly NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number][]
  stick_bars: readonly Readonly<{ id: number; start_tenths_mm: readonly [number, number, number]; end_tenths_mm: readonly [number, number, number]; thickness_tenths_mm: number }>[]
  component_count: number
  inferred_component_bridges: boolean
  principal_axis_extents_tenths_mm: readonly [number, number, number]
  quality_score: number
  quality_reasons: readonly string[]
  insufficiency_reasons: readonly string[]
  generic_body_outline_tenths_mm?: readonly (readonly [number, number])[]
  generic_body_outline_mode?: 'symmetric' | 'general'
  pair_bindings: readonly Readonly<{ pair_index: number; protrusion_id: number; center_y_tenths_mm: number }>[]
  method: 'bounded_bbox_area_normal_v1'
  suggested_part_kind: 'wing' | 'fin' | 'ear' | 'horn' | 'antenna' | 'leg' | 'tail' | null
}>

export async function suggestBeginnerReferenceModelFeatures(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
): Promise<BeginnerReferenceModelSuggestionV1> {
  const value = await invoke<unknown>('suggest_beginner_reference_model_features', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
  })
  const response = exactCoreDataRecord(value, [
    'project_instance_id', 'project_id', 'revision', 'source_asset_sha256', 'suggestion',
  ] as const)
  const sourceAssetSha256 = response
    ? snapshotSha256Bytes(response.source_asset_sha256)
    : null
  const suggestionKeys = [
    'asset_id', 'bbox_min_tenths_mm', 'bbox_max_tenths_mm', 'dominant_normal_milli',
    'surface_area_milli', 'surface_landmarks_tenths_mm', 'surface_ranges', 'protrusions',
    'general_protrusion_candidates', 'stick_bars', 'component_count', 'inferred_component_bridges', 'principal_axis_extents_tenths_mm',
    'quality_score', 'quality_reasons', 'insufficiency_reasons', 'pair_bindings', 'method', 'suggested_part_kind',
  ] as const
  const suggestion = snapshotCoreDataRecord(response?.suggestion)
  if (!suggestion || suggestionKeys.some((key) => !Object.hasOwn(suggestion, key))
    || Object.keys(suggestion).some((key) => ![...suggestionKeys,
      'generic_body_outline_tenths_mm', 'generic_body_outline_mode'].includes(key))) {
    throw new Error('invalid reference model suggestion')
  }
  const bboxMinimum = snapshotCoreDataArray(
    suggestion.bbox_min_tenths_mm,
    3,
  )
  const bboxMaximum = snapshotCoreDataArray(
    suggestion.bbox_max_tenths_mm,
    3,
  )
  const dominantNormal = snapshotCoreDataArray(
    suggestion.dominant_normal_milli,
    3,
  )
  const landmarkInputs = snapshotCoreDataArray(
    suggestion.surface_landmarks_tenths_mm,
    256,
  )
  const surfaceLandmarks = landmarkInputs?.map((point) => {
    const coordinates = snapshotCoreDataArray(point, 3)
    return coordinates?.length === 3
      && isI32Tuple(coordinates, 3)
      ? Object.freeze(coordinates.map(Number))
      : null
  }) ?? []
  const surfaceRangeInputs = snapshotCoreDataArray(
    suggestion.surface_ranges,
    8,
  )
  if (!response || !matchesProjectOccGuard({
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  }, response)
    || !sourceAssetSha256
    || !suggestion || !isCanonicalNonNilUuid(suggestion.asset_id)
    || suggestion.method !== 'bounded_bbox_area_normal_v1'
    || ![null, 'wing', 'fin', 'ear', 'horn', 'antenna', 'leg', 'tail'].includes(suggestion.suggested_part_kind as null | string)
    || !isI32Tuple(bboxMinimum, 3)
    || !isI32Tuple(bboxMaximum, 3)
    || !isBoundedIntegerTuple(dominantNormal, 3, 1000)
    || !Number.isSafeInteger(suggestion.surface_area_milli)
    || Number(suggestion.surface_area_milli) < 0
    || !landmarkInputs
    || surfaceLandmarks.length < 1
    || surfaceLandmarks.some((point) => point === null)) {
    throw new Error('invalid reference model suggestion')
  }
  if (!surfaceRangeInputs || surfaceRangeInputs.length < 1) {
    throw new Error('invalid reference model suggestion')
  }
  const surfaceRanges = surfaceRangeInputs.map((value, index) => {
    const range = exactCoreDataRecord(value, [
      'id', 'triangle_indices', 'range_min_tenths_mm', 'range_max_tenths_mm', 'digest_sha256',
    ] as const)
    const triangleIndices = range
      ? snapshotCoreDataArray(range.triangle_indices, 40_000)
      : null
    const rangeMinimum = range
      ? snapshotCoreDataArray(range.range_min_tenths_mm, 3)
      : null
    const rangeMaximum = range
      ? snapshotCoreDataArray(range.range_max_tenths_mm, 3)
      : null
    const digestSha256 = range
      ? snapshotSha256Bytes(range.digest_sha256)
      : null
    if (!range || range.id !== index + 1
      || !triangleIndices || triangleIndices.length < 1
      || triangleIndices.some((triangle) =>
        !Number.isInteger(triangle) || Number(triangle) < 0)
      || !isI32Tuple(rangeMinimum, 3)
      || !isI32Tuple(rangeMaximum, 3)
      || !digestSha256) throw new Error('invalid reference model suggestion')
    return Object.freeze({
      id: index + 1,
      triangle_indices: Object.freeze(triangleIndices.map(Number)),
      range_min_tenths_mm: Object.freeze(rangeMinimum.map(Number)),
      range_max_tenths_mm: Object.freeze(rangeMaximum.map(Number)),
      digest_sha256: digestSha256,
    })
  })
  const constraints = normalizeBeginnerGenerationConstraints({
    schema_version: 1, maximum_steps: 1, detail_level: 'simple', target_category: 'animal',
    target_parts: [], skeleton_segments: [], protrusions: suggestion.protrusions,
    ...(suggestion.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: suggestion.generic_body_outline_tenths_mm,
    }),
    ...(suggestion.generic_body_outline_mode === undefined ? {} : {
      generic_body_outline_mode: suggestion.generic_body_outline_mode,
    }),
    bulge_targets: [], target_asset: null, allowed_techniques: ['valley_fold'],
  })
  const generalConstraints = normalizeBeginnerGenerationConstraints({
    schema_version: 1, maximum_steps: 1, detail_level: 'simple', target_category: 'animal',
    target_parts: [], skeleton_segments: [], protrusions: suggestion.general_protrusion_candidates,
    bulge_targets: [], target_asset: null, allowed_techniques: ['valley_fold'],
  })
  const generalProtrusions = generalConstraints?.protrusions ?? []
  const stickBarInputs = snapshotCoreDataArray(suggestion.stick_bars, 3)
  const stickBars = stickBarInputs ? stickBarInputs.map((value, index) => {
    const bar = exactCoreDataRecord(value, ['id', 'start_tenths_mm', 'end_tenths_mm', 'thickness_tenths_mm'] as const)
    const start = bar
      ? snapshotCoreDataArray(bar.start_tenths_mm, 3)
      : null
    const end = bar
      ? snapshotCoreDataArray(bar.end_tenths_mm, 3)
      : null
    if (!bar || bar.id !== index
      || !isI32Tuple(start, 3)
      || !isI32Tuple(end, 3)
      || !Number.isInteger(bar.thickness_tenths_mm) || Number(bar.thickness_tenths_mm) < 1
      || Number(bar.thickness_tenths_mm) > 65_535) return null
    return Object.freeze({
      id: index,
      start_tenths_mm: Object.freeze(start.map(Number)),
      end_tenths_mm: Object.freeze(end.map(Number)),
      thickness_tenths_mm: Number(bar.thickness_tenths_mm),
    })
  }) : []
  const protrusions = constraints?.protrusions ?? []
  const bilateralProtrusions = protrusions.filter((target) => target.symmetry === 'bilateral')
  const principalAxisExtents = snapshotCoreDataArray(
    suggestion.principal_axis_extents_tenths_mm,
    3,
  )
  const qualityReasons = snapshotCoreDataArray(
    suggestion.quality_reasons,
    8,
  )
  const insufficiencyReasons = snapshotCoreDataArray(
    suggestion.insufficiency_reasons,
    8,
  )
  const pairBindingInputs = snapshotCoreDataArray(
    suggestion.pair_bindings,
    8,
  )
  const pairBindings = pairBindingInputs?.map((binding, index) => {
    const record = exactCoreDataRecord(binding, [
      'pair_index', 'protrusion_id', 'center_y_tenths_mm',
    ] as const)
    return record
      && record.pair_index === index
      && record.protrusion_id === bilateralProtrusions[index]?.id
      && record.center_y_tenths_mm
        === bilateralProtrusions[index]?.position_tenths_mm[1]
      ? Object.freeze({
          pair_index: index,
          protrusion_id: Number(record.protrusion_id),
          center_y_tenths_mm: Number(record.center_y_tenths_mm),
        })
      : null
  }) ?? []
  // Native may generalize an explicitly authored generic target to at most
  // eight bounded features. Geometry supplies measurements only; semantic
  // kinds remain the user's current target_parts and apply still requires
  // exact live-suggestion revalidation plus confirmation.
  if (!constraints || !generalConstraints || generalProtrusions.length < 1
    || generalProtrusions.length > 32 || !stickBarInputs
    || stickBars.length !== 3 || stickBars.some((bar) => !bar)
    || !isBoundedIntegerTuple(principalAxisExtents, 3, 2_147_483_647)
    || principalAxisExtents.some((extent) => Number(extent) < 1)
    || !Number.isInteger(suggestion.quality_score) || Number(suggestion.quality_score) < 0 || Number(suggestion.quality_score) > 100
    || !qualityReasons || qualityReasons.length < 1
    || qualityReasons.some((reason) => !['strict_glb_vertex_index_bounds', 'deterministic_aabb_principal_axes'].includes(String(reason)))
    || !Number.isInteger(suggestion.component_count) || Number(suggestion.component_count) < 1 || Number(suggestion.component_count) > 8
    || typeof suggestion.inferred_component_bridges !== 'boolean'
    || (suggestion.inferred_component_bridges !== (Number(suggestion.component_count) > 1))
    || !insufficiencyReasons
    || insufficiencyReasons.some((reason) => !['insufficient_distinct_vertices', 'protrusion_candidate_limit_reached', 'component_bridges_are_estimated'].includes(String(reason)))
    || protrusions.length < 1 || protrusions.length > 8
    || !pairBindingInputs
    || pairBindings.length !== bilateralProtrusions.length
    || pairBindings.some((binding) => binding === null)) {
    throw new Error('invalid reference model suggestion')
  }
  return Object.freeze({
    asset_id: String(suggestion.asset_id),
    source_asset_sha256: sourceAssetSha256,
    bbox_min_tenths_mm: Object.freeze(bboxMinimum.map(Number)),
    bbox_max_tenths_mm: Object.freeze(bboxMaximum.map(Number)),
    dominant_normal_milli: Object.freeze(dominantNormal.map(Number)),
    surface_area_milli: Number(suggestion.surface_area_milli),
    surface_ranges: Object.freeze(surfaceRanges),
    surface_landmarks_tenths_mm: Object.freeze(
      surfaceLandmarks.map((point) => point!),
    ),
    ...(constraints.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: constraints.generic_body_outline_tenths_mm,
      generic_body_outline_mode: constraints.generic_body_outline_mode,
    }),
    protrusions: Object.freeze(protrusions.slice()),
    general_protrusion_candidates: Object.freeze(generalProtrusions.slice()),
    stick_bars: Object.freeze(stickBars as NonNullable<(typeof stickBars)[number]>[]),
    component_count: Number(suggestion.component_count),
    inferred_component_bridges: suggestion.inferred_component_bridges as boolean,
    principal_axis_extents_tenths_mm:
      Object.freeze(principalAxisExtents.map(Number)),
    quality_score: Number(suggestion.quality_score),
    quality_reasons: Object.freeze(qualityReasons.map(String)),
    insufficiency_reasons: Object.freeze(insufficiencyReasons.map(String)),
    pair_bindings: Object.freeze(pairBindings.map((binding) => binding!)),
    method: 'bounded_bbox_area_normal_v1' as const,
    suggested_part_kind: suggestion.suggested_part_kind as
      BeginnerReferenceModelSuggestionV1['suggested_part_kind'],
  }) as BeginnerReferenceModelSuggestionV1
}

export function applyBeginnerReferenceModelFeatures(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
  expectedSuggestion: BeginnerReferenceModelSuggestionV1,
  surfaceAssignments: readonly Readonly<{ range_id: number, protrusion_id: number }>[],
  surfaceEdits: readonly Readonly<{
    range_id: number, base_digest_sha256: readonly number[], triangle_indices: readonly number[]
    bulge_direction_milli: readonly [number, number, number], bulge_amount_tenths_mm: number
  }>[],
) {
  const assignmentInputs = snapshotCoreDataArray(surfaceAssignments, 8)
  const assignments = assignmentInputs?.map((value) => {
    const item = exactCoreDataRecord(
      value,
      ['range_id', 'protrusion_id'] as const,
    )
    return item
      && Number.isInteger(item.range_id)
      && Number(item.range_id) >= 1
      && Number(item.range_id) <= 8
      && Number.isInteger(item.protrusion_id)
      && Number(item.protrusion_id) >= 1
      && Number(item.protrusion_id) <= 65_535
      ? {
          range_id: Number(item.range_id),
          protrusion_id: Number(item.protrusion_id),
        }
      : null
  }) ?? []
  if (!assignmentInputs || assignments.length < 2
    || assignments.some((item) => item === null)
    || new Set(assignments.map((item) => item?.range_id)).size
      !== assignments.length
    || new Set(assignments.map((item) => item?.protrusion_id)).size
      !== assignments.length) {
    return Promise.reject(new Error('invalid reference model surface selection'))
  }
  const editInputs = snapshotCoreDataArray(surfaceEdits, 8)
  const edits = editInputs?.map((value) => {
    const item = exactCoreDataRecord(value, [
      'range_id', 'base_digest_sha256', 'triangle_indices',
      'bulge_direction_milli', 'bulge_amount_tenths_mm',
    ] as const)
    const digest = item
      ? snapshotSha256Bytes(item.base_digest_sha256)
      : null
    const triangles = item
      ? snapshotCoreDataArray(item.triangle_indices, 40_000)
      : null
    const direction = item
      ? snapshotCoreDataArray(item.bulge_direction_milli, 3)
      : null
    if (
      !item
      || !Number.isInteger(item.range_id)
      || Number(item.range_id) < 1
      || Number(item.range_id) > 8
      || !digest
      || !triangles
      || triangles.length < 1
      || new Set(triangles).size !== triangles.length
      || triangles.some((triangle) =>
        !Number.isInteger(triangle) || Number(triangle) < 0)
      || !isBoundedIntegerTuple(direction, 3, 1_000)
      || direction.every((axis) => axis === 0)
      || !Number.isInteger(item.bulge_amount_tenths_mm)
      || Number(item.bulge_amount_tenths_mm) < 1
      || Number(item.bulge_amount_tenths_mm) > 1_000_000
    ) return null
    return {
      range_id: Number(item.range_id),
      base_digest_sha256: Array.from(digest),
      triangle_indices: triangles.map(Number),
      bulge_direction_milli: direction.map(Number),
      bulge_amount_tenths_mm: Number(item.bulge_amount_tenths_mm),
    }
  }) ?? []
  if (!editInputs || edits.length !== assignments.length
    || edits.some((item) => item === null)
    || new Set(edits.map((item) => item?.range_id)).size !== edits.length
    || edits.some((edit) => !assignments.some((assignment) =>
      assignment?.range_id === edit?.range_id))) {
    return Promise.reject(new Error('invalid reference model surface edit'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_reference_model_features', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    expectedSuggestion,
    surfaceAssignments: assignments,
    surfaceEdits: edits,
    confirmed: true,
  })
}

export function evaluateBeginnerCandidates(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  requestedCandidateCount: number,
  requestGenerationId: string,
  expectedProfile: BeginnerDesignProfileV1,
) {
  if (!isCanonicalNonNilUuid(requestGenerationId)) {
    return Promise.reject(new Error('invalid candidate generation'))
  }
  if (!Number.isInteger(requestedCandidateCount)
    || requestedCandidateCount < 1 || requestedCandidateCount > 3) {
    return Promise.reject(new Error('invalid requested candidate count'))
  }
  const normalizedExpectedProfile =
    normalizeBeginnerDesignProfile(expectedProfile)
  if (!normalizedExpectedProfile) {
    return Promise.reject(new Error('invalid expected beginner profile'))
  }
  return invoke<unknown>('evaluate_beginner_candidates', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    requestedCandidateCount,
    requestGenerationId,
  }).then((value) => {
    const response = normalizeBeginnerCandidateResponse(
      value,
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
      requestedCandidateCount,
      'candidate',
      normalizedExpectedProfile,
    )
    if (!response) throw new Error('invalid beginner candidate response')
    return response
  })
}

export function cancelReferenceConsensus(requestGenerationId: string) {
  if (!isCanonicalNonNilUuid(requestGenerationId)) return Promise.reject(new Error('invalid consensus generation'))
  return invoke<void>('cancel_reference_consensus', { requestGenerationId })
}

export type BeginnerParameterGridPointV1 = Readonly<{
  id: number
  scale_percent: number
  spacing_percent: number
  detail_level: 'simple' | 'standard' | 'detailed'
}>

export type BeginnerContourPlacementWitnessV1 = Readonly<{
  body_contour_points: number
  local_bindings: ReadonlyArray<Readonly<{
    protrusion_id: number, contour_points: number, generated_face_id: number,
    vertex_start: number, crease_start: number,
  }>>
  generic_feature_bindings: ReadonlyArray<Readonly<{
    protrusion_id: number, generated_feature_id: number,
    endpoint_count: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8,
    crease_start: number, crease_authority_sha256: ReadonlyArray<number>,
    skeleton_segment_id: number, skeleton_endpoint: 'start' | 'end',
    mount_distance_squared_tenths_mm: number,
  }>>
  skeleton_branch_bindings: ReadonlyArray<Readonly<{
    segment_id: number, parent_segment_id: number | null,
    parent_endpoint: 'start' | 'end' | null, child_endpoint: 'start' | 'end' | null,
    generated_feature_ids: ReadonlyArray<number>,
  }>>
  skeleton_tree_authority_sha256: ReadonlyArray<number>
  witnessed_vertices: number
  witnessed_creases: number
  topology_authority_hash: ReadonlyArray<number>
  max_contour_error_millionths: number
}>

export type BeginnerGridEvaluationResponse = Readonly<{
  request_generation_id: string
  authority_token: string
  project_instance_id: string
  project_id: string
  revision: number
  grid_hash: ReadonlyArray<number>
  evaluated_grid_points: 27
  global_checked_candidates: 3
  refinement_iterations: number
  candidates: ReadonlyArray<Readonly<{
    point: BeginnerParameterGridPointV1
    primary_score: number
    plan: BeginnerGeneratedPlanV1
    assessment: BeginnerGeneratedPlanAssessmentV1
    local_proof_scope: 'necessary'
    global_proof_scope: 'necessary' | 'sufficient' | 'indeterminate'
    complexity_score: number
    paper_efficiency_score: number
    scale_deviation_penalty: number
    spacing_deviation_penalty: number
    detail_mismatch_penalty: number
    outcome_reason: BeginnerGeneratedPlanAssessmentV1['reason']
    contour_witness: BeginnerContourPlacementWitnessV1
    refinement_iterations: number
    strict_improvements: number
    refinement_starts: number
  }>>
}>

export async function evaluateBeginnerParameterGrid(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
  requestGenerationId: string,
  expectedProfile: BeginnerDesignProfileV1,
): Promise<BeginnerGridEvaluationResponse> {
  if (!isCanonicalNonNilUuid(requestGenerationId)) {
    throw new Error('invalid beginner grid generation')
  }
  const normalizedExpectedProfile =
    normalizeBeginnerDesignProfile(expectedProfile)
  if (!normalizedExpectedProfile) {
    throw new Error('invalid expected beginner profile')
  }
  const value = await invoke<unknown>('evaluate_beginner_parameter_grid', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, requestGenerationId,
  })
  return normalizeBeginnerGridEvaluationResponseV1(value, {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    requestGenerationId,
    expectedProfile: normalizedExpectedProfile,
  }, normalizeBeginnerCandidateResponse)
}

export async function getBeginnerParameterGridProgress(requestGenerationId: string) {
  const value = await invoke<unknown>('get_beginner_parameter_grid_progress', { requestGenerationId })
  const record = exactCoreDataRecord(value, ['request_generation_id', 'enumerated_grid_points', 'global_checked_candidates', 'refinement_iterations', 'terminal_state'] as const)
  if (!record || record.request_generation_id !== requestGenerationId
    || !Number.isInteger(record.enumerated_grid_points) || Number(record.enumerated_grid_points) < 0 || Number(record.enumerated_grid_points) > 27
    || !Number.isInteger(record.global_checked_candidates) || Number(record.global_checked_candidates) < 0 || Number(record.global_checked_candidates) > 3
    || !Number.isInteger(record.refinement_iterations) || Number(record.refinement_iterations) < 0 || Number(record.refinement_iterations) > 24
    || !['running', 'completed', 'cancelled', 'failed'].includes(String(record.terminal_state))) {
    throw new Error('invalid beginner grid progress')
  }
  return Object.freeze({ request_generation_id: requestGenerationId,
    enumerated_grid_points: Number(record.enumerated_grid_points),
    global_checked_candidates: Number(record.global_checked_candidates),
    refinement_iterations: Number(record.refinement_iterations),
    terminal_state: record.terminal_state as 'running' | 'completed' | 'cancelled' | 'failed' })
}

export function cancelBeginnerParameterGrid(requestGenerationId: string) {
  return invoke<void>('cancel_beginner_parameter_grid', { requestGenerationId })
}

export function applyBeginnerParameterGridCandidate(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
  grid: BeginnerGridEvaluationResponse,
  expectedProfile: BeginnerDesignProfileV1,
  candidate: BeginnerGridEvaluationResponse['candidates'][number],
) {
  if (expectedProjectId !== grid.project_id || expectedRevision !== grid.revision
    || expectedProjectInstanceId !== grid.project_instance_id
    || !isCanonicalNonNilUuid(grid.request_generation_id)
    || !isCanonicalNonNilUuid(grid.authority_token)
    || !grid.candidates.includes(candidate)
    || !beginnerGeneratedPlanAssessmentAllowsApplyV1(
      candidate.assessment,
    )) {
    return Promise.reject(new Error('grid candidate lacks a live sufficient proof'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_parameter_grid_candidate', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    requestGenerationId: grid.request_generation_id,
    authorityToken: grid.authority_token,
    expectedProfile,
    expectedGridHash: grid.grid_hash,
    selectedPoint: candidate.point,
    expectedCandidateEdgeId: candidate.assessment.expected_candidate_edge_id,
    expectedTopologyAuthorityHash: candidate.contour_witness.topology_authority_hash,
    confirmed: true,
  })
}

export type BeginnerSymmetricParameterEstimateResponse = Readonly<{
  project_instance_id: string; project_id: string; revision: number
  estimate: Readonly<{ protrusion_count: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14; scale_percent: number; spacing_percent: number }>
  candidates: ReadonlyArray<Readonly<{ id: number; scale_percent: number; spacing_percent: number
    approximation_score: number; complexity_score: number; required_protrusion_count: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 }>>
}>

const BEGINNER_SUPPORTED_PROTRUSION_COUNTS_V1 =
  Object.freeze([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] as const)

function isBeginnerSupportedProtrusionCountV1(value: unknown): boolean {
  return Number.isInteger(value)
    && (BEGINNER_SUPPORTED_PROTRUSION_COUNTS_V1 as readonly number[])
      .includes(Number(value))
}

export async function getBeginnerSymmetricParameterEstimate(
  projectId: string, revision: number, projectInstanceId: string,
): Promise<BeginnerSymmetricParameterEstimateResponse> {
  const value = await invoke<unknown>('get_beginner_symmetric_parameter_estimate', {
    expectedProjectInstanceId: projectInstanceId, expectedProjectId: projectId, expectedRevision: revision,
  })
  const record = exactCoreDataRecord(value, ['project_instance_id', 'project_id', 'revision', 'estimate', 'candidates'] as const)
  const estimate = exactCoreDataRecord(record?.estimate, ['protrusion_count', 'scale_percent', 'spacing_percent'] as const)
  if (!record || !matchesProjectOccGuard({
    expectedProjectInstanceId: projectInstanceId,
    expectedProjectId: projectId,
    expectedRevision: revision,
  }, record) || !estimate
    || !isBeginnerSupportedProtrusionCountV1(estimate.protrusion_count)
    || !Number.isInteger(estimate.scale_percent) || Number(estimate.scale_percent) < 10 || Number(estimate.scale_percent) > 45
    || !Number.isInteger(estimate.spacing_percent) || Number(estimate.spacing_percent) < 20 || Number(estimate.spacing_percent) > 80
    || !Array.isArray(record.candidates) || record.candidates.length !== 3) {
    throw new Error('invalid symmetric parameter estimate')
  }
  const candidates = record.candidates.map((value, index) => {
    const item = exactCoreDataRecord(value, ['id', 'scale_percent', 'spacing_percent', 'approximation_score', 'complexity_score', 'required_protrusion_count'] as const)
    if (!item || item.id !== index
      || !isBeginnerSupportedProtrusionCountV1(item.required_protrusion_count)
      || Number(item.required_protrusion_count) !== Number(estimate.protrusion_count)
      || !Number.isInteger(item.scale_percent) || Number(item.scale_percent) < 10 || Number(item.scale_percent) > 45
      || !Number.isInteger(item.spacing_percent) || Number(item.spacing_percent) < 20 || Number(item.spacing_percent) > 80
      || !Number.isInteger(item.approximation_score) || Number(item.approximation_score) < 0 || Number(item.approximation_score) > 100
      || !Number.isInteger(item.complexity_score) || Number(item.complexity_score) < 0 || Number(item.complexity_score) > 255) throw new Error('invalid symmetric parameter candidates')
    return Object.freeze(item)
  })
  return Object.freeze({ ...record, estimate: Object.freeze(estimate), candidates: Object.freeze(candidates) }) as BeginnerSymmetricParameterEstimateResponse
}

export function applyBeginnerSymmetricParameters(
  expectedProjectId: string, expectedRevision: number, expectedProjectInstanceId: string,
  expectedEstimate: BeginnerSymmetricParameterEstimateResponse['estimate'],
  scalePercent: number, spacingPercent: number,
) {
  return invoke<ProjectSnapshot>('apply_beginner_symmetric_parameters', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, expectedEstimate,
    scalePercent, spacingPercent, confirmed: true,
  })
}

export function recognizeBeginnerTarget(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  underlayId: string,
  assetId: string,
) {
  if (!isCanonicalNonNilUuid(expectedProjectId)
    || !isCanonicalNonNilUuid(expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(underlayId)
    || !isCanonicalNonNilUuid(assetId)
    || !Number.isSafeInteger(expectedRevision) || expectedRevision < 0) {
    return Promise.reject(new Error('invalid beginner recognition request'))
  }
  const request = {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    underlayId,
    assetId,
  }
  return invoke<unknown>('recognize_beginner_target', { request }).then((value) => {
    const proposal = normalizeBeginnerRecognitionProposal(value, underlayId, assetId)
    if (!proposal) throw new Error('invalid beginner recognition response')
    return proposal
  })
}

export class BeginnerRecognitionError extends Error {
  readonly reason:
    | 'ambiguous_silhouette'
    | 'unsupported_silhouette'
    | 'resource_limit'
    | 'native_failure'

  constructor(reason: BeginnerRecognitionError['reason']) {
    super('beginner recognition failed')
    this.reason = reason
  }
}

export function recognizeBeginnerSilhouette(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  underlayId: string,
  assetId: string,
  thresholds: { alpha: number; luma: number; polarity: 'dark_on_light' | 'light_on_dark' | 'alpha_only'; crop_roi?: BeginnerGenerationConstraintsV1['silhouette_crop_roi']; orientation_degrees?: 0 | 90 | 180 | 270; mirror?: BeginnerGenerationConstraintsV1['silhouette_mirror'] } = { alpha: 128, luma: 127, polarity: 'dark_on_light' },
) {
  if (!isCanonicalNonNilUuid(expectedProjectId)
    || !isCanonicalNonNilUuid(expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(underlayId)
    || !isCanonicalNonNilUuid(assetId)
    || !Number.isSafeInteger(expectedRevision) || expectedRevision < 0
    || !Number.isInteger(thresholds.alpha) || thresholds.alpha < 0 || thresholds.alpha > 255
    || !Number.isInteger(thresholds.luma) || thresholds.luma < 0 || thresholds.luma > 255
    || !['dark_on_light', 'light_on_dark', 'alpha_only'].includes(thresholds.polarity)) {
    return Promise.reject(new BeginnerRecognitionError('native_failure'))
  }
  const request = {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, underlayId, assetId,
    alphaThreshold: thresholds.alpha, lumaThreshold: thresholds.luma,
    polarity: thresholds.polarity,
    cropRoi: thresholds.crop_roi,
    orientationDegrees: thresholds.orientation_degrees,
    mirror: thresholds.mirror,
  }
  return invoke<unknown>('recognize_beginner_silhouette', { request }).then((value) => {
    const proposal = normalizeBeginnerRecognitionProposal(
      value, underlayId, assetId, 'silhouette_png_v1',
    )
    if (!proposal) throw new BeginnerRecognitionError('native_failure')
    return proposal
  }, (error: unknown) => {
    if (error === 'recognition_ambiguous_silhouette') {
      throw new BeginnerRecognitionError('ambiguous_silhouette')
    }
    if (error === 'recognition_unsupported_silhouette') {
      throw new BeginnerRecognitionError('unsupported_silhouette')
    }
    if (error === 'recognition_resource_limit') {
      throw new BeginnerRecognitionError('resource_limit')
    }
    throw new BeginnerRecognitionError('native_failure')
  })
}

export type BeginnerOutlineCandidatesResponse = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
  underlay_id: string
  asset_id: string
  source_sha256: readonly number[]
  candidates: ReadonlyArray<Readonly<{
    id: number
    bounds: Readonly<{ min_x: number; min_y: number; max_x: number; max_y: number }>
    area_pixels: number
    confidence_reason: 'solid_component' | 'small_component'
  }>>
}>

export async function recognizeBeginnerOutlineCandidates(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  underlayId: string,
  assetId: string,
): Promise<BeginnerOutlineCandidatesResponse> {
  const request = {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, underlayId, assetId,
  }
  const value = await invoke<unknown>('recognize_beginner_outline_candidates', { request })
  const record = exactCoreDataRecord(value, [
    'project_instance_id', 'project_id', 'revision', 'underlay_id', 'asset_id', 'source_sha256', 'candidates',
  ] as const)
  const sourceSha256 = record
    ? snapshotSha256Bytes(record.source_sha256)
    : null
  if (!record || !matchesProjectOccGuard({
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  }, record)
    || record.underlay_id !== underlayId || record.asset_id !== assetId
    || !sourceSha256
    || !Array.isArray(record.candidates) || record.candidates.length > 16) {
    throw new BeginnerRecognitionError('native_failure')
  }
  const candidates = record.candidates.map((value, index) => {
    const candidate = exactCoreDataRecord(value, [
      'id', 'bounds', 'area_pixels', 'confidence_reason',
    ] as const)
    const bounds = exactCoreDataRecord(candidate?.bounds, ['min_x', 'min_y', 'max_x', 'max_y'] as const)
    if (!candidate || candidate.id !== index || !bounds
      || !Number.isSafeInteger(candidate.area_pixels) || Number(candidate.area_pixels) < 4
      || !['solid_component', 'small_component'].includes(String(candidate.confidence_reason))
      || [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .some((coordinate) => !Number.isSafeInteger(coordinate) || Number(coordinate) < 0)
      || Number(bounds.min_x) > Number(bounds.max_x)
      || Number(bounds.min_y) > Number(bounds.max_y)) {
      throw new BeginnerRecognitionError('native_failure')
    }
    return Object.freeze({
      id: index,
      bounds: Object.freeze({
        min_x: Number(bounds.min_x), min_y: Number(bounds.min_y),
        max_x: Number(bounds.max_x), max_y: Number(bounds.max_y),
      }),
      area_pixels: Number(candidate.area_pixels),
      confidence_reason: candidate.confidence_reason as 'solid_component' | 'small_component',
    })
  })
  return Object.freeze({
    project_instance_id: expectedProjectInstanceId,
    project_id: expectedProjectId,
    revision: expectedRevision,
    underlay_id: underlayId,
    asset_id: assetId,
    source_sha256: sourceSha256,
    candidates: Object.freeze(candidates),
  })
}

export function applyBeginnerOutlineCandidate(
  proposal: BeginnerOutlineCandidatesResponse,
  candidate: BeginnerOutlineCandidatesResponse['candidates'][number],
  confirmed: boolean,
) {
  if (!confirmed || !proposal.candidates.includes(candidate)) {
    return Promise.reject(new BeginnerRecognitionError('native_failure'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_outline_candidate', {
    request: {
      expectedProjectInstanceId: proposal.project_instance_id,
      expectedProjectId: proposal.project_id,
      expectedRevision: proposal.revision,
      underlayId: proposal.underlay_id,
      assetId: proposal.asset_id,
      candidate,
      confirmed: true,
    },
  })
}

export type BeginnerPartSuggestionsResponse = Readonly<{
  project_instance_id: string; project_id: string; revision: number
  underlay_id: string; asset_id: string; selected_outline_id: number
  suggestions: ReadonlyArray<Readonly<{
    candidate_id: number
    suggested_kind: 'torso' | 'head' | 'leg' | 'wing'
    confidence_reason: 'selected_primary_outline' | 'largest_secondary_outline' | 'small_secondary_outline' | 'bilateral_secondary_pair'
  }>>
}>

export const MAX_BEGINNER_PART_ASSIGNMENTS_V1 = 16

export async function recognizeBeginnerPartSuggestions(
  proposal: BeginnerOutlineCandidatesResponse,
  candidate: BeginnerOutlineCandidatesResponse['candidates'][number],
): Promise<BeginnerPartSuggestionsResponse> {
  const value = await invoke<unknown>('recognize_beginner_part_suggestions', { request: {
    expectedProjectInstanceId: proposal.project_instance_id, expectedProjectId: proposal.project_id,
    expectedRevision: proposal.revision, underlayId: proposal.underlay_id, assetId: proposal.asset_id,
    candidate, confirmed: false,
  } })
  const record = exactCoreDataRecord(value, ['project_instance_id', 'project_id', 'revision', 'underlay_id', 'asset_id', 'selected_outline_id', 'suggestions'] as const)
  if (!record || !matchesProjectOccGuard({
    expectedProjectInstanceId: proposal.project_instance_id,
    expectedProjectId: proposal.project_id,
    expectedRevision: proposal.revision,
  }, record)
    || record.underlay_id !== proposal.underlay_id || record.asset_id !== proposal.asset_id
    || record.selected_outline_id !== candidate.id || !Array.isArray(record.suggestions)
    || record.suggestions.length < 2
    || record.suggestions.length > MAX_BEGINNER_PART_ASSIGNMENTS_V1) {
    throw new BeginnerRecognitionError('native_failure')
  }
  const suggestions = record.suggestions.map((value) => {
    const item = exactCoreDataRecord(value, ['candidate_id', 'suggested_kind', 'confidence_reason'] as const)
    if (!item || !Number.isInteger(item.candidate_id)
      || !['torso', 'head', 'leg', 'wing'].includes(String(item.suggested_kind))
      || !['selected_primary_outline', 'largest_secondary_outline', 'small_secondary_outline', 'bilateral_secondary_pair'].includes(String(item.confidence_reason))) {
      throw new BeginnerRecognitionError('native_failure')
    }
    return Object.freeze(item) as BeginnerPartSuggestionsResponse['suggestions'][number]
  })
  return Object.freeze({ ...record, suggestions: Object.freeze(suggestions) }) as BeginnerPartSuggestionsResponse
}

export function applyBeginnerPartAssignments(
  outline: BeginnerOutlineCandidatesResponse,
  selectedOutline: BeginnerOutlineCandidatesResponse['candidates'][number],
  assignments: ReadonlyArray<{
    candidate_id: number
    kind: BeginnerGenerationConstraintsV1['target_parts'][number]['kind']
    source_candidate_ids?: number[]
    split_fragment?: number
    split_x?: number
  }>,
) {
  if (assignments.length < 1
    || assignments.length > MAX_BEGINNER_PART_ASSIGNMENTS_V1
    || assignments.some((assignment) => assignment.source_candidate_ids
    && (assignment.source_candidate_ids.length < 1 || assignment.source_candidate_ids.length > 2
      || new Set(assignment.source_candidate_ids).size !== assignment.source_candidate_ids.length
      || assignment.source_candidate_ids.some((id) => !Number.isInteger(id)
        || id < 0 || id >= MAX_BEGINNER_PART_ASSIGNMENTS_V1)))
    || assignments.some((assignment) => assignment.split_fragment !== undefined
      && assignment.split_fragment !== 0 && assignment.split_fragment !== 1)
    || assignments.some((assignment) => assignment.split_x !== undefined
      && (!Number.isSafeInteger(assignment.split_x) || assignment.split_x < 0))) {
    return Promise.reject(new BeginnerRecognitionError('native_failure'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_part_assignments', { request: {
    expectedProjectInstanceId: outline.project_instance_id, expectedProjectId: outline.project_id,
    expectedRevision: outline.revision, underlayId: outline.underlay_id, assetId: outline.asset_id,
    sourceSha256: [...outline.source_sha256], selectedOutline, assignments, confirmed: true,
  } })
}

export function applyBeginnerGeneratedPlan(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  expectedProfile: BeginnerDesignProfileV1,
  selectedKind: BeginnerGeneratedPlanV1['kind'],
  expectedCandidateEdgeId: string,
) {
  if (![
    'diagonal_fold',
    'symmetric_four_leg_base',
    'symmetric_wing_base',
    'symmetric_bird_base',
    'asymmetric_bird_landmark_base',
    'asymmetric_four_leg_landmark_base',
    'asymmetric_insect_landmark_base',
    'asymmetric_fish_landmark_base',
    'symmetric_fish_base',
    'symmetric_ear_base',
    'symmetric_horn_base',
    'symmetric_antenna_base',
    'symmetric_insect_leg_pair_base',
    'symmetric_six_leg_base',
    'center_axis_tail_base',
    'center_axis_horn_base',
    'center_axis_antenna_base',
    'composite_tail_ear_base',
    'composite_horn_ear_base',
    'composite_horn_tail_base',
    'composite_horn_tail_ear_base',
    'composite_wing_antenna_base',
    'composite_complete_insect_base',
    'composite_complete_animal_base',
    'composite_complete_winged_animal_base',
    'composite_generic_target_base',
  ].includes(selectedKind) || !isCanonicalNonNilUuid(expectedCandidateEdgeId)) {
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

export function proveCurrentAssignedLocalSufficiencyV1(
  request: Readonly<{
    expectedProjectInstanceId: string
    expectedProjectId: string
    expectedRevision: number
    vertex: string
  }>,
): Promise<AssignedLocalSufficiencyResponseV1> {
  return invoke<unknown>('prove_current_assigned_local_sufficiency_v1', { request }).then((value) => {
    const normalized = normalizeAssignedLocalSufficiencyResponseV1(value, request)
    if (!normalized) throw new Error('invalid local sufficiency response')
    return normalized
  })
}

export function normalizeAssignedLocalSufficiencyResponseV1(
  value: unknown,
  request: Readonly<{
    expectedProjectInstanceId: string
    expectedProjectId: string
    expectedRevision: number
    vertex: string
  }>,
): AssignedLocalSufficiencyResponseV1 | null {
    const record = (candidate: unknown): candidate is Record<string, unknown> =>
      typeof candidate === 'object' && candidate !== null && !Array.isArray(candidate)
    if (!record(value) || !record(value.result)) return null
    const result = value.result
    const uuid = (candidate: unknown) =>
      typeof candidate === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u.test(candidate)
    const exactTop = Object.keys(value).sort().join(',') ===
      ['authorizesProjectMutation', 'projectId', 'projectInstanceId', 'result', 'revision', 'version'].sort().join(',')
    const binding = value.version === 1
      && value.projectInstanceId === request.expectedProjectInstanceId
      && value.projectId === request.expectedProjectId
      && value.revision === request.expectedRevision
      && value.authorizesProjectMutation === false
    const valid = result.status === 'proven'
      ? Object.keys(result).sort().join(',') === ['model_id', 'reduction_steps', 'reductions', 'status', 'vertex'].sort().join(',')
        && result.model_id === 'assigned_single_vertex_unique_blb_crimp_v1'
        && result.vertex === request.vertex
        && Number.isSafeInteger(result.reduction_steps)
        && Number(result.reduction_steps) >= 0
        && Array.isArray(result.reductions)
        && result.reductions.length === result.reduction_steps
        && result.reductions.length <= 128
        && result.reductions.every((step) =>
          record(step)
          && Object.keys(step).sort().join(',') === 'first_crease,second_crease'
          && uuid(step.first_crease)
          && uuid(step.second_crease)
          && step.first_crease !== step.second_crease)
      : result.status === 'indeterminate'
        && Object.keys(result).sort().join(',') === 'reason,status,vertex'
        && result.vertex === request.vertex
        && ['vertex_unavailable', 'necessary_conditions_not_satisfied', 'reduction_theorem_not_applicable', 'resource_limit'].includes(String(result.reason))
    if (!exactTop || !binding || !valid) return null
    return value as AssignedLocalSufficiencyResponseV1
}

export function summarizeCurrentAssignedLocalSufficiencyV1(request: Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  expectedFoldModelFingerprint: string
}>): Promise<AssignedLocalSufficiencySummaryResponseV1> {
  return invoke<unknown>('summarize_current_assigned_local_sufficiency_v1', { request })
    .catch((error) => {
      throw new AssignedLocalSufficiencySummaryError(
        String(error).includes('Another native pose analysis is already running.')
          ? 'busy'
          : 'native_failure',
      )
    })
    .then((value) => {
      const normalized = normalizeAssignedLocalSufficiencySummaryResponseV1(value, request)
      if (!normalized) throw new Error('invalid local sufficiency summary response')
      return normalized
    })
}

export class AssignedLocalSufficiencySummaryError extends Error {
  readonly reason: 'busy' | 'native_failure'

  constructor(reason: 'busy' | 'native_failure') {
    super(reason)
    this.name = 'AssignedLocalSufficiencySummaryError'
    this.reason = reason
  }
}

export function cancelCurrentAssignedLocalSufficiencySummaryV1(): Promise<void> {
  return invoke('cancel_current_assigned_local_sufficiency_summary_v1')
}

export function normalizeAssignedLocalSufficiencySummaryResponseV1(
  value: unknown,
  request: Readonly<{
    expectedProjectInstanceId: string
    expectedProjectId: string
    expectedRevision: number
    expectedFoldModelFingerprint: string
  }>,
): AssignedLocalSufficiencySummaryResponseV1 | null {
  const record = (candidate: unknown): candidate is Record<string, unknown> =>
    typeof candidate === 'object' && candidate !== null && !Array.isArray(candidate)
  const uuid = (candidate: unknown): candidate is string =>
    typeof candidate === 'string'
    && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u.test(candidate)
  if (!record(value)
    || Object.keys(value).sort().join(',') !== [
      'authorizesProjectMutation', 'foldModelFingerprint', 'projectId',
      'projectInstanceId', 'revision', 'totalReductionSteps', 'version', 'vertices',
    ].sort().join(',')
    || value.version !== 1
    || value.projectInstanceId !== request.expectedProjectInstanceId
    || value.projectId !== request.expectedProjectId
    || value.revision !== request.expectedRevision
    || value.foldModelFingerprint !== request.expectedFoldModelFingerprint
    || value.authorizesProjectMutation !== false
    || !Array.isArray(value.vertices) || value.vertices.length > 4096
    || !Number.isSafeInteger(value.totalReductionSteps)
    || Number(value.totalReductionSteps) < 0
    || Number(value.totalReductionSteps) > 16_384) return null
  const seen = new Set<string>()
  let reductions = 0
  for (const item of value.vertices) {
    if (!record(item) || !uuid(item.vertex) || seen.has(item.vertex)) return null
    seen.add(item.vertex)
    if (item.status === 'necessary_failed') {
      if (Object.keys(item).sort().join(',') !== 'status,vertex') return null
    } else if (item.status === 'sufficient_proven') {
      if (Object.keys(item).sort().join(',') !== 'model_id,reduction_steps,status,vertex'
        || item.model_id !== 'assigned_single_vertex_unique_blb_crimp_v1'
        || !Number.isSafeInteger(item.reduction_steps)
        || Number(item.reduction_steps) < 0) return null
      reductions += Number(item.reduction_steps)
    } else if (item.status === 'indeterminate') {
      if (Object.keys(item).sort().join(',') !== 'reason,status,vertex'
        || !['vertex_unavailable', 'reduction_theorem_not_applicable', 'resource_limit', 'cancelled']
          .includes(String(item.reason))) return null
    } else return null
  }
  if (reductions !== value.totalReductionSteps) return null
  return value as AssignedLocalSufficiencySummaryResponseV1
}

export function analyzeProjectTopology(expectedProjectId: string, expectedRevision: number) {
  return invoke<ProjectTopologyResponse>('analyze_project_topology', {
    expectedProjectId,
    expectedRevision,
  })
}

const EFFECTIVE_CUT_RESPONSE_KEYS = [
  'version', 'projectInstanceId', 'projectId', 'revision', 'foldModelFingerprint',
  'effectiveSnapshotFingerprint', 'geometryModelId', 'geometryFingerprint',
  'pairObservationModelId', 'pairObservationFingerprint', 'multiHingeGapModelId',
  'multiHingeGapFingerprint', 'sourceFlatPairCount', 'separatedPairs', 'touchingPairs',
  'sharedHingeCorridorObservedPairs', 'sharedVertexCorridorObservedPairs',
  'penetratingPairs', 'indeterminatePairs', 'multiHingePairs',
  'multiHingeUnionCorridorUnprovedPairs', 'authorizesProjectMutation',
  'authorizesPersistence', 'authorizesSimulationAdmission', 'authorizesPairClassification',
  'authorizesCollisionFreeClassification', 'authorizesPoseSolving',
  'authorizesMaterialRemoval',
] as const

const EFFECTIVE_CUT_CANDIDATE_RESPONSE_KEYS = [
  'version', 'projectInstanceId', 'projectId', 'revision', 'foldModelFingerprint',
  'modelId', 'diagnosticFingerprint', 'totalComponentCount', 'boundaryComponentCount',
  'candidates', 'authorizesProjectMutation', 'authorizesPersistence',
  'authorizesSimulationAdmission', 'authorizesMaterialRemoval',
] as const

function isSha256Bytes(value: unknown): value is readonly number[] {
  return Array.isArray(value)
    && value.length === 32
    && value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
}

function isCanonicalSha256Hex(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)
}

function isSafeCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0
}

export function isEffectiveCutReadOnlyRequestV1(
  value: unknown,
): value is EffectiveCutReadOnlyRequestV1 {
  const request = exactCoreDataRecord(value, [
    'expectedProjectInstanceId', 'expectedProjectId', 'expectedRevision',
    'expectedFoldModelFingerprint', 'requestedComponentKeys',
  ] as const)
  if (!request) return false
  const keys = request.requestedComponentKeys
  return isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    && isCanonicalNonNilUuid(request.expectedProjectId)
    && isSafeCount(request.expectedRevision)
    && isCanonicalSha256Hex(request.expectedFoldModelFingerprint)
    && Array.isArray(keys)
    && keys.length > 0
    && keys.length <= 64
    && keys.every(isSha256Bytes)
    && keys.slice(1).every((key, index) => {
      const previous = keys[index]
      for (let byte = 0; byte < 32; byte += 1) {
        if (previous[byte] < key[byte]) return true
        if (previous[byte] > key[byte]) return false
      }
      return false
    })
}

export function normalizeEffectiveCutCandidateListResponseV1(
  value: unknown,
  request: EffectiveCutCandidateListRequestV1,
): EffectiveCutCandidateListResponseV1 | null {
  if (!isEffectiveCutCandidateListRequestV1(request)) return null
  const record = exactCoreDataRecord(value, EFFECTIVE_CUT_CANDIDATE_RESPONSE_KEYS)
  if (!record
    || record.version !== 1
    || record.projectInstanceId !== request.expectedProjectInstanceId
    || record.projectId !== request.expectedProjectId
    || record.revision !== request.expectedRevision
    || record.foldModelFingerprint !== request.expectedFoldModelFingerprint
    || record.modelId !== 'cut_material_component_selection_diagnostic_v1'
    || !isSha256Bytes(record.diagnosticFingerprint)
    || !isSafeCount(record.totalComponentCount)
    || record.totalComponentCount < 2
    || record.totalComponentCount > 64
    || record.boundaryComponentCount !== 1
    || !Array.isArray(record.candidates)
    || record.candidates.length + 1 !== record.totalComponentCount
    || record.authorizesProjectMutation !== false
    || record.authorizesPersistence !== false
    || record.authorizesSimulationAdmission !== false
    || record.authorizesMaterialRemoval !== false
  ) return null
  const candidates: EffectiveCutCandidateV1[] = []
  for (const value of record.candidates) {
    const candidate = exactCoreDataRecord(value, [
      'componentKey', 'ownsOriginalBoundary', 'faceCount', 'areaSquareMm',
      'closureComponentCount', 'closureFaceCount', 'nestedDependencyCount',
    ] as const)
    if (!candidate
      || !isSha256Bytes(candidate.componentKey)
      || candidate.ownsOriginalBoundary !== false
      || !isSafeCount(candidate.faceCount)
      || candidate.faceCount < 1
      || candidate.faceCount > 50_000
      || typeof candidate.areaSquareMm !== 'number'
      || !Number.isFinite(candidate.areaSquareMm)
      || candidate.areaSquareMm < 0
      || !isSafeCount(candidate.closureComponentCount)
      || candidate.closureComponentCount < 1
      || candidate.closureComponentCount > 64
      || !isSafeCount(candidate.closureFaceCount)
      || candidate.closureFaceCount < candidate.faceCount
      || candidate.closureFaceCount > 50_000
      || candidate.nestedDependencyCount !== candidate.closureComponentCount - 1
    ) return null
    candidates.push(Object.freeze({
      ...candidate,
      componentKey: Object.freeze([...candidate.componentKey]),
    }) as EffectiveCutCandidateV1)
  }
  if (candidates.slice(1).some((candidate, index) => {
    const previous = candidates[index].componentKey
    for (let byte = 0; byte < 32; byte += 1) {
      if (previous[byte] < candidate.componentKey[byte]) return false
      if (previous[byte] > candidate.componentKey[byte]) return true
    }
    return true
  })) return null
  return Object.freeze({
    ...record,
    diagnosticFingerprint: Object.freeze([...record.diagnosticFingerprint]),
    candidates: Object.freeze(candidates),
  }) as EffectiveCutCandidateListResponseV1
}

export function isEffectiveCutCandidateListRequestV1(
  value: unknown,
): value is EffectiveCutCandidateListRequestV1 {
  const request = exactCoreDataRecord(value, [
    'expectedProjectInstanceId', 'expectedProjectId', 'expectedRevision',
    'expectedFoldModelFingerprint',
  ] as const)
  return request !== null
    && isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    && isCanonicalNonNilUuid(request.expectedProjectId)
    && isSafeCount(request.expectedRevision)
    && isCanonicalSha256Hex(request.expectedFoldModelFingerprint)
}

export function listEffectiveCutCandidatesV1(
  request: EffectiveCutCandidateListRequestV1,
): Promise<EffectiveCutCandidateListResponseV1> {
  if (!isEffectiveCutCandidateListRequestV1(request)) {
    return Promise.reject(new Error('invalid effective-cut candidate request'))
  }
  const snapshot = { ...request }
  return invoke<unknown>('list_effective_cut_candidates_v1', { request: snapshot }).then((value) => {
    const response = normalizeEffectiveCutCandidateListResponseV1(value, snapshot)
    if (!response) throw new Error('invalid effective-cut candidate response')
    return response
  })
}

export function normalizeEffectiveCutReadOnlyResponseV1(
  value: unknown,
  request: EffectiveCutReadOnlyRequestV1,
): EffectiveCutReadOnlyResponseV1 | null {
  if (!isEffectiveCutReadOnlyRequestV1(request)) return null
  const record = exactCoreDataRecord(value, EFFECTIVE_CUT_RESPONSE_KEYS)
  if (!record
    || record.version !== 1
    || record.projectInstanceId !== request.expectedProjectInstanceId
    || record.projectId !== request.expectedProjectId
    || record.revision !== request.expectedRevision
    || record.foldModelFingerprint !== request.expectedFoldModelFingerprint
    || !isCanonicalNonNilUuid(record.projectInstanceId)
    || !isCanonicalNonNilUuid(record.projectId)
    || !isCanonicalSha256Hex(record.foldModelFingerprint)
    || !isSha256Bytes(record.effectiveSnapshotFingerprint)
    || record.geometryModelId !== 'effective_cut_collision_geometry_v1'
    || !isSha256Bytes(record.geometryFingerprint)
    || record.pairObservationModelId !== 'effective_cut_source_flat_pair_observation_v1'
    || !isSha256Bytes(record.pairObservationFingerprint)
    || record.multiHingeGapModelId !== 'effective_cut_multi_hinge_union_gap_diagnostic_v1'
    || !isSha256Bytes(record.multiHingeGapFingerprint)
  ) return null
  const counts = [
    record.sourceFlatPairCount, record.separatedPairs, record.touchingPairs,
    record.sharedHingeCorridorObservedPairs, record.sharedVertexCorridorObservedPairs,
    record.penetratingPairs, record.indeterminatePairs, record.multiHingePairs,
    record.multiHingeUnionCorridorUnprovedPairs,
  ]
  if (!counts.every(isSafeCount)
    || Number(record.sourceFlatPairCount) > 50_000
    || Number(record.multiHingePairs) > 64
    || counts.slice(1, 7).some((count) => count > Number(record.sourceFlatPairCount))
    || counts.slice(1, 7).reduce((sum, count) => sum + count, 0) !== counts[0]
    || record.multiHingePairs !== record.multiHingeUnionCorridorUnprovedPairs
    || record.authorizesProjectMutation !== false
    || record.authorizesPersistence !== false
    || record.authorizesSimulationAdmission !== false
    || record.authorizesPairClassification !== false
    || record.authorizesCollisionFreeClassification !== false
    || record.authorizesPoseSolving !== false
    || record.authorizesMaterialRemoval !== false
  ) return null
  return Object.freeze({
    ...record,
    effectiveSnapshotFingerprint: Object.freeze([...record.effectiveSnapshotFingerprint]),
    geometryFingerprint: Object.freeze([...record.geometryFingerprint]),
    pairObservationFingerprint: Object.freeze([...record.pairObservationFingerprint]),
    multiHingeGapFingerprint: Object.freeze([...record.multiHingeGapFingerprint]),
  }) as EffectiveCutReadOnlyResponseV1
}

export function inspectEffectiveCutReadOnlyV1(
  request: EffectiveCutReadOnlyRequestV1,
): Promise<EffectiveCutReadOnlyResponseV1> {
  if (!isEffectiveCutReadOnlyRequestV1(request)) {
    return Promise.reject(new Error('invalid effective-cut read-only request'))
  }
  const snapshot: EffectiveCutReadOnlyRequestV1 = {
    expectedProjectInstanceId: request.expectedProjectInstanceId,
    expectedProjectId: request.expectedProjectId,
    expectedRevision: request.expectedRevision,
    expectedFoldModelFingerprint: request.expectedFoldModelFingerprint,
    requestedComponentKeys: request.requestedComponentKeys.map((key) => [...key]),
  }
  return invoke<unknown>('inspect_effective_cut_read_only_v1', { request: snapshot }).then((value) => {
    const response = normalizeEffectiveCutReadOnlyResponseV1(value, snapshot)
    if (!response) throw new Error('invalid effective-cut read-only response')
    return response
  })
}

export function proposeCurrentStackedFoldRead(
  request: StackedFoldReadRequest,
): Promise<StackedFoldReadResponse> {
  const snapshot = normalizeStackedFoldReadRequest(request)
  if (!snapshot) {
    return Promise.reject(new Error('invalid stacked-fold request'))
  }
  return invoke<unknown>('propose_current_stacked_fold_read', {
    request: snapshot,
  }).then((value) => {
    const response = normalizeStackedFoldReadResponse(value, snapshot)
    if (!response) throw new Error('invalid stacked-fold response')
    return response
  }, (error: unknown) => {
    if (error === 'stacked_fold_cycle_nonclosing') {
      throw new StackedFoldReadNativeError('cycle_nonclosing')
    }
    if (error === 'stacked_fold_cycle_path_uncertified') {
      throw new StackedFoldReadNativeError('cycle_path_uncertified')
    }
    if (error === 'stacked_fold_cycle_path_unsupported') {
      throw new StackedFoldReadNativeError('cycle_path_unsupported')
    }
    if (error === 'stacked_fold_cycle_path_resource_limit') {
      throw new StackedFoldReadNativeError('cycle_path_resource_limit')
    }
    if (error === 'stacked_fold_cycle_path_no_certified_path') {
      throw new StackedFoldReadNativeError('cycle_path_no_certified_path')
    }
    if (error === 'stacked_fold_cycle_path_cancelled') {
      throw new StackedFoldReadNativeError('cycle_path_cancelled')
    }
    if (error === 'stacked_fold_cycle_path_collision') {
      throw new StackedFoldReadNativeError('cycle_path_collision')
    }
    throw new StackedFoldReadNativeError('native_failure')
  })
}

export type EvenCycleCandidatesRequestV1 = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  maxPairTests: number
}>

export type EvenCycleCandidatesResponseV1 = Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
  status: 'ready' | 'none' | 'resource_limit' | 'unsupported'
  reason: string
  candidates: readonly Readonly<{
    version: 1
    edges: readonly [string, string]
    reason: 'same_assignment_geometrically_opposite'
  }>[]
  kawasakiEndpoints: readonly Readonly<{
    version: 1
    endpointDenominator: 1 | 2 | 4 | 8 | 16
    closureStatus: 'certified'
    collisionStatus: 'certified' | 'uncertified'
    authorizesApply: false
  }>[]
  authorizesProjectMutation: false
}>

export function readEvenCycleCandidatesV1(
  request: EvenCycleCandidatesRequestV1,
): Promise<EvenCycleCandidatesResponseV1> {
  if (!isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !Number.isSafeInteger(request.maxPairTests) || request.maxPairTests < 0
    || request.maxPairTests > 120) {
    return Promise.reject(new Error('invalid even-cycle candidate request'))
  }
  return invoke<unknown>('read_even_cycle_candidates_v1', { request }).then((value) => {
    if (!isCoreDataRecord(value)
      || Object.keys(value).sort().join(',') !== 'authorizesProjectMutation,candidates,kawasakiEndpoints,projectId,projectInstanceId,reason,revision,status,version'
      || value.version !== 1
      || value.projectInstanceId !== request.expectedProjectInstanceId
      || value.projectId !== request.expectedProjectId
      || value.revision !== request.expectedRevision
      || !['ready', 'none', 'resource_limit', 'unsupported'].includes(String(value.status))
      || typeof value.reason !== 'string'
      || value.authorizesProjectMutation !== false
      || !Array.isArray(value.candidates) || value.candidates.length > 8
      || !Array.isArray(value.kawasakiEndpoints) || value.kawasakiEndpoints.length > 5) throw new Error('invalid even-cycle candidate response')
    const seen = new Set<string>()
    for (const candidate of value.candidates) {
      if (!isCoreDataRecord(candidate) || candidate.version !== 1
        || Object.keys(candidate).sort().join(',') !== 'edges,reason,version'
        || candidate.reason !== 'same_assignment_geometrically_opposite'
        || !Array.isArray(candidate.edges) || candidate.edges.length !== 2
        || !candidate.edges.every(isCanonicalNonNilUuid)
        || String(candidate.edges[0]).localeCompare(String(candidate.edges[1])) >= 0
        || seen.has(candidate.edges.join(':'))) {
        throw new Error('invalid even-cycle candidate response')
      }
      seen.add(candidate.edges.join(':'))
    }
    for (const endpoint of value.kawasakiEndpoints) {
      if (!isCoreDataRecord(endpoint)
        || Object.keys(endpoint).sort().join(',') !== 'authorizesApply,closureStatus,collisionStatus,endpointDenominator,version'
        || endpoint.version !== 1
        || ![1, 2, 4, 8, 16].includes(Number(endpoint.endpointDenominator))
        || endpoint.closureStatus !== 'certified'
        || !['certified', 'uncertified'].includes(String(endpoint.collisionStatus))
        || endpoint.authorizesApply !== false) throw new Error('invalid Kawasaki endpoint response')
    }
    if ((value.status === 'ready') !== (value.candidates.length > 0)) {
      throw new Error('invalid even-cycle candidate response')
    }
    return value as EvenCycleCandidatesResponseV1
  })
}

export type DyadicPoseGraphReadResponseV1 = Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
  status: 'certified' | 'no_path' | 'resource_limit' | 'cancelled' | 'unsupported'
  reason: 'proof_complete' | 'no_certified_path' | 'bounded_resource_limit' | 'cancelled' | 'unsupported_geometry'
  stateCount: number
  transitionCount: number
  exploredStateCount: number
  evaluatedTransitionCount: number
  certifiedTransitionCount: number
  certificateBindingSha256: string | null
  positiveThicknessTransitionCount: number
  positiveThicknessCertified: boolean
  positiveThicknessBindingSha256: string | null
  layerTransportTransitionCount: number
  layerTransportCertified: boolean
  layerTransportBindingSha256: string | null
  mutationCandidateReady: boolean
  authorizesProjectMutation: false
}>

export function readBoundedDyadicPoseGraphV1(request: Readonly<{
  progressRequestId?: string
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  targetAngles: readonly Readonly<{ edge: string; angleDegrees: number }>[]
  maxStates: number
  maxTransitions: number
  levelCount: 3 | 5 | 9
  cycleScheduleV1?: CycleScheduleRequestV1
}>): Promise<DyadicPoseGraphReadResponseV1> {
  if ((request.progressRequestId !== undefined
      && (!/^[\x21-\x7e]{1,128}$/.test(request.progressRequestId)))
    || !isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !Number.isSafeInteger(request.maxStates) || request.maxStates < 1 || request.maxStates > 2187
    || !Number.isSafeInteger(request.maxTransitions) || request.maxTransitions < 1 || request.maxTransitions > 20412
    || ![3, 5, 9].includes(request.levelCount)
    || !Array.isArray(request.targetAngles) || request.targetAngles.length === 0 || request.targetAngles.length > 64
    || request.targetAngles.some((entry, index, entries) =>
      !isCanonicalNonNilUuid(entry.edge)
      || !Number.isFinite(entry.angleDegrees) || entry.angleDegrees < 0 || entry.angleDegrees > 180
      || (index > 0 && entries[index - 1]!.edge >= entry.edge))) {
    return Promise.reject(new Error('invalid dyadic pose graph request'))
  }
  return invoke<unknown>('read_bounded_dyadic_pose_graph_v1', { request }).then((value) => {
    if (!isCoreDataRecord(value)
      || Object.keys(value).sort().join(',') !== 'authorizesProjectMutation,certificateBindingSha256,certifiedTransitionCount,evaluatedTransitionCount,exploredStateCount,layerTransportBindingSha256,layerTransportCertified,layerTransportTransitionCount,mutationCandidateReady,positiveThicknessBindingSha256,positiveThicknessCertified,positiveThicknessTransitionCount,projectId,projectInstanceId,reason,revision,stateCount,status,transitionCount,version'
      || value.version !== 1
      || value.projectInstanceId !== request.expectedProjectInstanceId
      || value.projectId !== request.expectedProjectId
      || value.revision !== request.expectedRevision
      || !['certified', 'no_path', 'resource_limit', 'cancelled', 'unsupported'].includes(String(value.status))
      || !['proof_complete', 'no_certified_path', 'bounded_resource_limit', 'cancelled', 'unsupported_geometry'].includes(String(value.reason))
      || (value.reason === 'proof_complete' && value.status !== 'certified')
      || (value.reason === 'unsupported_geometry') !== (value.status === 'unsupported')
      || ![value.stateCount, value.transitionCount, value.exploredStateCount, value.evaluatedTransitionCount, value.certifiedTransitionCount, value.positiveThicknessTransitionCount, value.layerTransportTransitionCount]
        .every((count) => Number.isSafeInteger(count) && Number(count) >= 0)
      || Number(value.stateCount) > request.maxStates
      || Number(value.exploredStateCount) > request.maxStates
      || Number(value.transitionCount) > request.maxTransitions
      || Number(value.evaluatedTransitionCount) > request.maxTransitions
      || Number(value.certifiedTransitionCount) > Number(value.evaluatedTransitionCount)
      || (value.status === 'certified') !== (typeof value.certificateBindingSha256 === 'string' && /^[0-9a-f]{64}$/.test(value.certificateBindingSha256))
      || Number(value.positiveThicknessTransitionCount) > Number(value.certifiedTransitionCount)
      || Number(value.layerTransportTransitionCount) > Number(value.certifiedTransitionCount)
      || value.positiveThicknessCertified !== (Number(value.certifiedTransitionCount) > 0 && Number(value.positiveThicknessTransitionCount) === Number(value.certifiedTransitionCount) && typeof value.positiveThicknessBindingSha256 === 'string' && /^[0-9a-f]{64}$/.test(value.positiveThicknessBindingSha256))
      || value.layerTransportCertified !== (Number(value.certifiedTransitionCount) > 0 && Number(value.layerTransportTransitionCount) === Number(value.certifiedTransitionCount) && typeof value.layerTransportBindingSha256 === 'string' && /^[0-9a-f]{64}$/.test(value.layerTransportBindingSha256))
      || (value.positiveThicknessCertified === false && value.positiveThicknessBindingSha256 !== null)
      || (value.layerTransportCertified === false && value.layerTransportBindingSha256 !== null)
      || value.mutationCandidateReady !== (value.positiveThicknessCertified === true && value.layerTransportCertified === true)
      || value.authorizesProjectMutation !== false) throw new Error('invalid dyadic pose graph response')
    return value as DyadicPoseGraphReadResponseV1
  })
}

export type DyadicPathPreviewResponseV1 = Readonly<{
  version: 1
  previewToken: string
  projectInstanceId: string
  projectId: string
  revision: number
  targetBindingSha256: string
  pathBindingSha256: string
  positiveThicknessBindingSha256: string
  layerTransportBindingSha256: string
  authorizesProjectMutation: false
}>

export function mintDyadicPosePathPreviewV1(request: Readonly<{
  progressRequestId: string
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  targetAngles: readonly Readonly<{ edge: string; angleDegrees: number }>[]
  maxStates: number
  maxTransitions: number
  levelCount: 3 | 5 | 9
  cycleScheduleV1?: CycleScheduleRequestV1
  expectedPathBindingSha256: string
  expectedPositiveThicknessBindingSha256: string
  expectedLayerTransportBindingSha256: string
}>): Promise<DyadicPathPreviewResponseV1> {
  const hash = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)
  if (!/^[\x21-\x7e]{1,128}$/.test(request.progressRequestId)
    || !isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !Number.isSafeInteger(request.maxStates) || request.maxStates < 1 || request.maxStates > 2187
    || !Number.isSafeInteger(request.maxTransitions) || request.maxTransitions < 1 || request.maxTransitions > 20412
    || ![3, 5, 9].includes(request.levelCount)
    || !Array.isArray(request.targetAngles) || request.targetAngles.length === 0 || request.targetAngles.length > 64
    || request.targetAngles.some((entry, index, entries) =>
      !isCanonicalNonNilUuid(entry.edge)
      || !Number.isFinite(entry.angleDegrees) || entry.angleDegrees < 0 || entry.angleDegrees > 180
      || (index > 0 && entries[index - 1]!.edge >= entry.edge))
    || !hash(request.expectedPathBindingSha256)
    || !hash(request.expectedPositiveThicknessBindingSha256)
    || !hash(request.expectedLayerTransportBindingSha256)) return Promise.reject(new Error('invalid dyadic preview request'))
  return invoke<unknown>('mint_dyadic_pose_path_preview_v1', { request }).then((value) => {
    if (!isCoreDataRecord(value)
      || Object.keys(value).sort().join(',') !== 'authorizesProjectMutation,layerTransportBindingSha256,pathBindingSha256,positiveThicknessBindingSha256,previewToken,projectId,projectInstanceId,revision,targetBindingSha256,version'
      || value.version !== 1
      || !isCanonicalNonNilUuid(value.previewToken)
      || value.projectInstanceId !== request.expectedProjectInstanceId
      || value.projectId !== request.expectedProjectId
      || value.revision !== request.expectedRevision
      || !hash(value.targetBindingSha256)
      || value.pathBindingSha256 !== request.expectedPathBindingSha256
      || value.positiveThicknessBindingSha256 !== request.expectedPositiveThicknessBindingSha256
      || value.layerTransportBindingSha256 !== request.expectedLayerTransportBindingSha256
      || value.authorizesProjectMutation !== false) throw new Error('invalid dyadic preview response')
    return value as DyadicPathPreviewResponseV1
  })
}

export function applyDyadicPosePathPreviewV1(request: Readonly<{
  previewToken: string
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  expectedTargetBindingSha256: string
  expectedPathBindingSha256: string
  expectedPositiveThicknessBindingSha256: string
  expectedLayerTransportBindingSha256: string
}>): Promise<number> {
  const hash = (value: unknown) => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)
  if (!isCanonicalNonNilUuid(request.previewToken)
    || !isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !hash(request.expectedTargetBindingSha256)
    || !hash(request.expectedPathBindingSha256)
    || !hash(request.expectedPositiveThicknessBindingSha256)
    || !hash(request.expectedLayerTransportBindingSha256)) return Promise.reject(new Error('invalid dyadic apply request'))
  return invoke<unknown>('apply_dyadic_pose_path_preview_v1', { request }).then((revision) => {
    if (!Number.isSafeInteger(revision) || Number(revision) !== request.expectedRevision + 1) {
      throw new Error('invalid dyadic apply response')
    }
    return Number(revision)
  })
}

export function cancelDyadicPosePathPreviewV1(previewToken: string): Promise<void> {
  if (!isCanonicalNonNilUuid(previewToken)) {
    return Promise.reject(new Error('invalid dyadic preview token'))
  }
  return invoke<void>('cancel_dyadic_pose_path_preview_v1', {
    request: { previewToken },
  })
}

export function proposeCurrentCyclePoseV1(
  request: CurrentCyclePosePreviewRequestV1,
): Promise<CurrentCyclePosePreviewResponseV1> {
  const wire = snapshotStackedFoldReadWireValue(request)
  if (
    !wire
    || typeof wire.value !== 'object'
    || wire.value === null
    || Array.isArray(wire.value)
  ) return Promise.reject(new Error('invalid current-cycle preview request'))
  const snapshot = wire.value as CurrentCyclePosePreviewRequestV1
  const keys = Object.keys(snapshot).sort().join(',')
  if (
    keys !== 'cycleScheduleV1,expectedProjectId,expectedProjectInstanceId,expectedRevision' &&
    keys !== 'cycleScheduleV1,expectedProjectId,expectedProjectInstanceId,expectedRevision,progressRequestId'
  ) return Promise.reject(new Error('invalid current-cycle preview request'))
  const schedule: unknown = snapshot.cycleScheduleV1
  const scheduleRecord =
    typeof schedule === 'object' && schedule !== null && !Array.isArray(schedule)
      ? schedule as Record<string, unknown>
      : null
  const scheduleKeys = scheduleRecord
    ? Object.keys(scheduleRecord).sort().join(',')
    : ''
  const validSchedule = isCycleScheduleRequestV1(schedule)
    || (
      scheduleRecord !== null
      && scheduleRecord.version === 2
      && Array.isArray(scheduleRecord.entries)
      && scheduleRecord.entries.length === 0
      && (
        scheduleKeys === 'entries,version'
        || (
          scheduleKeys === 'endpointDenominator,entries,version'
          && typeof scheduleRecord.endpointDenominator === 'number'
          && [1, 2, 4, 8, 16].includes(scheduleRecord.endpointDenominator)
        )
      )
    )
  if (
    !isCanonicalNonNilUuid(snapshot.expectedProjectInstanceId) ||
    !isCanonicalNonNilUuid(snapshot.expectedProjectId) ||
    !Number.isSafeInteger(snapshot.expectedRevision) ||
    Object.is(snapshot.expectedRevision, -0) ||
    snapshot.expectedRevision < 0 ||
    !validSchedule ||
    (snapshot.progressRequestId !== undefined &&
      !/^[\x21-\x7e]{1,128}$/.test(snapshot.progressRequestId))
  ) return Promise.reject(new Error('invalid current-cycle preview request'))
  return invoke<unknown>('propose_current_cycle_pose_v1', {
    request: snapshot,
  }).then((payload) =>
    normalizeCurrentCyclePosePreviewResponseV1(payload, snapshot.expectedRevision))
}

const MAX_CURRENT_CYCLE_LAYER_ORDER_PAIRS_V1 = 50_000

export function normalizeCurrentCyclePosePreviewResponseV1(
  payload: unknown,
  expectedRevision: number,
): CurrentCyclePosePreviewResponseV1 {
    const wire = snapshotStackedFoldReadWireValue(payload)
    if (
      !wire
      || typeof wire.value !== 'object'
      || wire.value === null
      || Array.isArray(wire.value)
    ) {
      throw new Error('invalid current-cycle preview response')
    }
    const value = wire.value as Record<string, unknown>
    const continuousLayerTransitionCount =
      value.continuousLayerTransitionCount
    const sourceLayerOrder = value.sourceLayerOrder
    const targetLayerOrder = value.targetLayerOrder
    if (
      Object.keys(value).sort().join(',') !==
        'authorizesProjectMutation,checkedHingeCount,closureLeafCount,closureMaxDepth,continuousLayerPairOrderCount,continuousLayerTargetOrderSha256,continuousLayerTransitionCount,continuousLayerTransportModelId,continuousPathCertified,sourceLayerOrder,sourceRevision,targetLayerOrder,targetRevision,totalHingeCount,transactionToken,version' ||
      value.version !== 1 ||
      !isCanonicalNonNilUuid(value.transactionToken) ||
      value.sourceRevision !== expectedRevision ||
      value.targetRevision !== expectedRevision + 1 ||
      !Number.isSafeInteger(value.closureLeafCount) ||
      Number(value.closureLeafCount) <= 0 ||
      Number(value.closureLeafCount) > 65_536 ||
      !Number.isSafeInteger(value.closureMaxDepth) ||
      Number(value.closureMaxDepth) < 0 ||
      Number(value.closureMaxDepth) > 16 ||
      !Number.isSafeInteger(value.checkedHingeCount) ||
      !Number.isSafeInteger(value.totalHingeCount) ||
      Number(value.checkedHingeCount) <= 0 ||
      value.checkedHingeCount !== value.totalHingeCount ||
      Number(value.totalHingeCount) > 128 ||
      value.continuousPathCertified !== true ||
      (value.continuousLayerTransportModelId !== null &&
        value.continuousLayerTransportModelId !== 'general_multi_face_positive_thickness_cell_transport_v1' &&
        value.continuousLayerTransportModelId !== 'blockwise_positive_layer_authority_v1' &&
        value.continuousLayerTransportModelId !== 'common_articulation_continuous_layer_path_authority_v1') ||
      typeof continuousLayerTransitionCount !== 'number' ||
      !Number.isSafeInteger(continuousLayerTransitionCount) ||
      continuousLayerTransitionCount < 0 ||
      !Number.isSafeInteger(value.continuousLayerPairOrderCount) ||
      Number(value.continuousLayerPairOrderCount) < 0 ||
      (value.continuousLayerTargetOrderSha256 !== null &&
        (typeof value.continuousLayerTargetOrderSha256 !== 'string' ||
          !/^[0-9a-f]{64}$/.test(value.continuousLayerTargetOrderSha256))) ||
      !Array.isArray(sourceLayerOrder) ||
      !Array.isArray(targetLayerOrder) ||
      sourceLayerOrder.length > MAX_CURRENT_CYCLE_LAYER_ORDER_PAIRS_V1 ||
      targetLayerOrder.length > MAX_CURRENT_CYCLE_LAYER_ORDER_PAIRS_V1 ||
      !isLayerOrderPairsV1(sourceLayerOrder) ||
      !isLayerOrderPairsV1(targetLayerOrder) ||
      JSON.stringify(sourceLayerOrder) !== JSON.stringify(targetLayerOrder) ||
      (value.continuousLayerTransportModelId === null
        ? continuousLayerTransitionCount !== 0 ||
          value.continuousLayerPairOrderCount !== 0 ||
          value.continuousLayerTargetOrderSha256 !== null ||
          sourceLayerOrder.length !== 0 ||
          targetLayerOrder.length !== 0
        : continuousLayerTransitionCount <= 0 ||
          value.continuousLayerPairOrderCount !== sourceLayerOrder.length ||
          value.continuousLayerTargetOrderSha256 === null) ||
      value.authorizesProjectMutation !== false
    ) throw new Error('invalid current-cycle preview response')
    return value as CurrentCyclePosePreviewResponseV1
}

function isLayerOrderPairsV1(value: unknown): boolean {
  if (!Array.isArray(value)) return false
  const identities = new Set<string>()
  return value.every((pair) => {
    if (typeof pair !== 'object' || pair === null || Array.isArray(pair)) return false
    const record = pair as Record<string, unknown>
    if (Object.keys(record).sort().join(',') !== 'lowerFace,upperFace' ||
      !isCanonicalNonNilUuid(record.lowerFace) || !isCanonicalNonNilUuid(record.upperFace) ||
      record.lowerFace === record.upperFace) return false
    const identity = `${record.lowerFace}:${record.upperFace}`
    if (identities.has(identity)) return false
    identities.add(identity)
    return true
  })
}

export function listenCurrentCyclePoseProgressV1(
  onProgress: (progress: CurrentCyclePoseProgressV1) => void,
): Promise<UnlistenFn> {
  return listen<unknown>('current-cycle-pose-progress-v1', ({ payload }) => {
    const value = exactCoreDataRecord(payload, [
      'version',
      'requestId',
      'status',
      'completedWork',
      'totalWork',
      'authorizesProjectMutation',
    ] as const)
    if (
      !value ||
      value.version !== 1 ||
      typeof value.requestId !== 'string' ||
      !/^[\x21-\x7e]{1,128}$/.test(value.requestId) ||
      !['running', 'certified', 'cancelled', 'failed'].includes(String(value.status)) ||
      !Number.isSafeInteger(value.completedWork) ||
      Object.is(value.completedWork, -0) ||
      Number(value.completedWork) < 0 ||
      Number(value.completedWork) > 2 || value.totalWork !== 2 ||
      (value.status === 'running'
        ? Number(value.completedWork) >= 2
        : Number(value.completedWork) !== 2) ||
      value.authorizesProjectMutation !== false
    ) return
    onProgress(Object.freeze({
      version: 1,
      requestId: value.requestId,
      status: value.status as CurrentCyclePoseProgressV1['status'],
      completedWork: Number(value.completedWork),
      totalWork: 2,
      authorizesProjectMutation: false,
    }))
  })
}

export type StackedFoldReadProgressV1 = Readonly<{
  version: 1
  requestId: string
  exploredStateCount: number
  evaluatedTransitionCount: number
  stateLimit: 32
  transitionLimit: 64
  authorizesProjectMutation: false
}>

export function listenStackedFoldReadProgressV1(
  onProgress: (progress: StackedFoldReadProgressV1) => void,
): Promise<UnlistenFn> {
  return listen<unknown>('stacked-fold-read-progress-v1', ({ payload }) => {
    const value = exactCoreDataRecord(payload, [
      'version',
      'requestId',
      'exploredStateCount',
      'evaluatedTransitionCount',
      'stateLimit',
      'transitionLimit',
      'authorizesProjectMutation',
    ] as const)
    if (
      !value ||
      value.version !== 1 ||
      typeof value.requestId !== 'string' ||
      !/^[\x21-\x7e]{1,128}$/.test(value.requestId) ||
      !Number.isSafeInteger(value.exploredStateCount) ||
      Object.is(value.exploredStateCount, -0) ||
      Number(value.exploredStateCount) < 0 ||
      Number(value.exploredStateCount) > 32 ||
      !Number.isSafeInteger(value.evaluatedTransitionCount) ||
      Object.is(value.evaluatedTransitionCount, -0) ||
      Number(value.evaluatedTransitionCount) < 0 ||
      Number(value.evaluatedTransitionCount) > 64 ||
      value.stateLimit !== 32 ||
      value.transitionLimit !== 64 ||
      value.authorizesProjectMutation !== false
    ) return
    onProgress(Object.freeze({
      version: 1,
      requestId: value.requestId,
      exploredStateCount: Number(value.exploredStateCount),
      evaluatedTransitionCount: Number(value.evaluatedTransitionCount),
      stateLimit: 32,
      transitionLimit: 64,
      authorizesProjectMutation: false,
    }))
  })
}

export function readLiveHingeRegistryV1(
  request: LiveHingeRegistryRequestV1,
): Promise<LiveHingeRegistryResponseV1> {
  return invoke<unknown>('read_live_hinge_registry_v1', { request }).then((value) => {
    const response = normalizeLiveHingeRegistryV1(value, request)
    if (!response) throw new Error('invalid live hinge registry response')
    return response
  })
}

export class StackedFoldReadNativeError extends Error {
  readonly reason:
    | 'cycle_nonclosing'
    | 'cycle_path_uncertified'
    | 'cycle_path_unsupported'
    | 'cycle_path_resource_limit'
    | 'cycle_path_no_certified_path'
    | 'cycle_path_cancelled'
    | 'cycle_path_collision'
    | 'native_failure'

  constructor(reason: StackedFoldReadNativeError['reason']) {
    super('stacked-fold read failed')
    this.reason = reason
  }
}

export type BasicFoldTimelinePreviewRequestV1 = Readonly<{
  token: string
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  expectedSourceModelFingerprint: string
  foldEdge: string
  assignment: 'mountain' | 'valley'
  techniqueKind: 'mountain' | 'valley' | 'squash' | 'crimp' | 'inside_reverse' | 'outside_reverse' | 'sink' | 'accordion' | 'layer_selective'
  techniqueDocument: unknown
  techniqueId: string
}>

export type BasicFoldTimelinePreviewResponseV1 = Readonly<{
  schemaVersion: 1
  transactionToken: string
  projectInstanceId: string
  projectId: string
  revision: number
  sourceModelFingerprint: string
  fixedFace: string
  foldEdge: string
  assignment: 'mountain' | 'valley'
  techniqueKind: 'mountain' | 'valley' | 'squash' | 'crimp' | 'inside_reverse' | 'outside_reverse' | 'sink' | 'accordion' | 'layer_selective'
  previewBindingSha256: string
  timeline: InstructionTimeline
}>

export function previewNamedBasicFoldTimeline(
  request: BasicFoldTimelinePreviewRequestV1,
): Promise<BasicFoldTimelinePreviewResponseV1> {
  if (!isCanonicalNonNilUuid(request.token)
    || !isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !isCanonicalNonNilUuid(request.foldEdge)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !/^[0-9a-f]{64}$/u.test(request.expectedSourceModelFingerprint)
    || (request.assignment !== 'mountain' && request.assignment !== 'valley')
    || !['mountain', 'valley', 'squash', 'crimp', 'inside_reverse', 'outside_reverse', 'sink', 'accordion', 'layer_selective'].includes(request.techniqueKind)
    || typeof request.techniqueId !== 'string') {
    return Promise.reject(new Error('invalid basic-fold timeline preview request'))
  }
  let techniqueDocumentJson: string
  try { techniqueDocumentJson = JSON.stringify(request.techniqueDocument) } catch {
    return Promise.reject(new Error('invalid basic-fold technique document'))
  }
  const { techniqueDocument: _, ...native } = request
  return invoke<unknown>('preview_named_basic_fold_timeline', {
    ...native,
    techniqueDocumentJson,
  }).then((value) => {
    if (!value || typeof value !== 'object') throw new Error('invalid basic-fold preview response')
    const response = value as Record<string, unknown>
    if (Object.keys(response).sort().join(',') !== 'assignment,fixedFace,foldEdge,previewBindingSha256,projectId,projectInstanceId,revision,schemaVersion,sourceModelFingerprint,techniqueKind,timeline,transactionToken'
      || response.schemaVersion !== 1 || response.transactionToken !== request.token
      || response.projectInstanceId !== request.expectedProjectInstanceId
      || response.projectId !== request.expectedProjectId || response.revision !== request.expectedRevision
      || response.sourceModelFingerprint !== request.expectedSourceModelFingerprint
      || !isCanonicalNonNilUuid(response.fixedFace) || response.foldEdge !== request.foldEdge
      || response.assignment !== request.assignment || response.techniqueKind !== request.techniqueKind
      || typeof response.previewBindingSha256 !== 'string'
      || !/^[0-9a-f]{64}$/u.test(response.previewBindingSha256) || !response.timeline
      || typeof response.timeline !== 'object' || !Array.isArray((response.timeline as { steps?: unknown }).steps)) {
      throw new Error('invalid basic-fold preview response')
    }
    return response as BasicFoldTimelinePreviewResponseV1
  })
}

export function cancelStackedFoldTransactionPreview(token: string): Promise<void> {
  if (!isCanonicalNonNilUuid(token)) {
    return Promise.reject(new Error('invalid stacked-fold transaction token'))
  }
  return invoke<void>('cancel_stacked_fold_transaction_preview', { token })
}

export function cancelCurrentStackedFoldReadV1(): Promise<void> {
  return invoke('cancel_current_stacked_fold_read_v1')
}

export function cancelCurrentStackedFoldReadRequestV1(requestId: string): Promise<void> {
  if (!/^[\x21-\x7e]{1,128}$/.test(requestId)) {
    return Promise.reject(new Error('invalid stacked-fold read request ID'))
  }
  return invoke('cancel_current_stacked_fold_read_request_v1', { requestId })
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

export function applyNamedBookFoldTransaction(
  token: string,
  techniqueDocument: unknown,
  techniqueId: string,
  preview: BasicFoldTimelinePreviewResponseV1,
): Promise<number> {
  if (!isCanonicalNonNilUuid(token) || typeof techniqueId !== 'string'
    || preview.transactionToken !== token || !/^[0-9a-f]{64}$/u.test(preview.previewBindingSha256)) {
    return Promise.reject(new Error('invalid named book-fold request'))
  }
  let techniqueDocumentJson: string
  try {
    techniqueDocumentJson = JSON.stringify(techniqueDocument)
  } catch {
    return Promise.reject(new Error('invalid named book-fold document'))
  }
  if (new TextEncoder().encode(techniqueDocumentJson).length > 2 * 1024 * 1024) {
    return Promise.reject(new Error('named book-fold document is too large'))
  }
  return invoke<unknown>('apply_named_book_fold_transaction', {
    token,
    expectedProjectInstanceId: preview.projectInstanceId,
    expectedProjectId: preview.projectId,
    expectedRevision: preview.revision,
    expectedSourceModelFingerprint: preview.sourceModelFingerprint,
    foldEdge: preview.foldEdge,
    assignment: preview.assignment,
    techniqueKind: preview.techniqueKind,
    expectedPreviewBindingSha256: preview.previewBindingSha256,
    techniqueDocumentJson,
    techniqueId,
  }).then((value) => {
    if (!Number.isSafeInteger(value) || (value as number) < 0) {
      throw new Error('invalid named book-fold apply response')
    }
    return value as number
  })
}

export function applyNamedReverseFoldTransaction(
  token: string,
  techniqueDocument: unknown,
  techniqueId: string,
): Promise<number> {
  if (!isCanonicalNonNilUuid(token) || typeof techniqueId !== 'string') {
    return Promise.reject(new Error('invalid named reverse-fold request'))
  }
  let techniqueDocumentJson: string
  try {
    techniqueDocumentJson = JSON.stringify(techniqueDocument)
  } catch {
    return Promise.reject(new Error('invalid named reverse-fold document'))
  }
  if (new TextEncoder().encode(techniqueDocumentJson).length > 2 * 1024 * 1024) {
    return Promise.reject(new Error('named reverse-fold document is too large'))
  }
  return invoke<unknown>('apply_named_reverse_fold_transaction', {
    token, techniqueDocumentJson, techniqueId,
  }).then((value) => {
    if (!Number.isSafeInteger(value) || (value as number) < 0) {
      throw new Error('invalid named reverse-fold apply response')
    }
    return value as number
  })
}

export function applyNamedAccordionFoldTransaction(
  token: string, techniqueDocument: unknown, techniqueId: string,
): Promise<number> {
  if (!isCanonicalNonNilUuid(token) || typeof techniqueId !== 'string') {
    return Promise.reject(new Error('invalid accordion-fold request'))
  }
  let techniqueDocumentJson: string
  try { techniqueDocumentJson = JSON.stringify(techniqueDocument) } catch {
    return Promise.reject(new Error('invalid accordion-fold document'))
  }
  if (new TextEncoder().encode(techniqueDocumentJson).length > 2 * 1024 * 1024) {
    return Promise.reject(new Error('accordion-fold document is too large'))
  }
  return invoke<unknown>('apply_named_accordion_fold_transaction', {
    token, techniqueDocumentJson, techniqueId,
  }).then((value) => {
    if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error('invalid accordion apply response')
    return value as number
  })
}

export function applyNamedSinkFoldTransaction(
  token: string, techniqueDocument: unknown, techniqueId: string,
): Promise<number> {
  if (!isCanonicalNonNilUuid(token) || typeof techniqueId !== 'string') {
    return Promise.reject(new Error('invalid sink-fold request'))
  }
  let techniqueDocumentJson: string
  try { techniqueDocumentJson = JSON.stringify(techniqueDocument) } catch {
    return Promise.reject(new Error('invalid sink-fold document'))
  }
  if (new TextEncoder().encode(techniqueDocumentJson).length > 2 * 1024 * 1024) {
    return Promise.reject(new Error('sink-fold document is too large'))
  }
  return invoke<unknown>('apply_named_sink_fold_transaction', {
    token, techniqueDocumentJson, techniqueId,
  }).then((value) => {
    if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error('invalid sink apply response')
    return value as number
  })
}

export function applyNamedLayerSelectiveTransaction(
  token: string, techniqueDocument: unknown, techniqueId: string,
): Promise<number> {
  if (!isCanonicalNonNilUuid(token)) return Promise.reject(new Error('invalid layer request'))
  let techniqueDocumentJson: string
  try { techniqueDocumentJson = JSON.stringify(techniqueDocument) } catch {
    return Promise.reject(new Error('invalid layer document'))
  }
  return invoke<unknown>('apply_named_layer_selective_transaction', {
    token, techniqueDocumentJson, techniqueId,
  }).then((value) => {
    if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error('invalid layer response')
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
  requestGenerationId: string,
) {
  if (
    !isCanonicalNonNilUuid(expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(expectedProjectId)
    || !Number.isSafeInteger(expectedRevision)
    || Object.is(expectedRevision, -0)
    || expectedRevision < 0
    || !isCanonicalNonNilUuid(requestGenerationId)
  ) {
    return Promise.reject(new Error('invalid geometric-constraint analysis request'))
  }
  return invoke<unknown>('analyze_geometric_constraints', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    requestGenerationId,
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

export function cancelGeometricConstraintAnalysis(
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  requestGenerationId: string,
): Promise<boolean> {
  if (
    !isCanonicalNonNilUuid(expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(expectedProjectId)
    || !Number.isSafeInteger(expectedRevision)
    || Object.is(expectedRevision, -0)
    || expectedRevision < 0
    || !isCanonicalNonNilUuid(requestGenerationId)
  ) {
    return Promise.reject(new Error('invalid geometric-constraint cancellation request'))
  }
  return invoke<unknown>('cancel_geometric_constraint_analysis', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    requestGenerationId,
  }).then((cancelled) => {
    if (typeof cancelled !== 'boolean') {
      throw new Error('invalid geometric-constraint cancellation response')
    }
    return cancelled
  })
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

export function duplicateInstructionStep(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  stepId: string,
) {
  return invoke<ProjectSnapshot>('duplicate_instruction_step', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    stepId,
  })
}

export function appendNamedTechniqueInstructionSteps(
  guard: ProjectOccGuard,
  proposal: NamedTechniqueTimelineProposalV1,
) {
  const expectedProjectInstanceId = projectOccGuardField(
    guard,
    'expectedProjectInstanceId',
  )
  if (
    expectedProjectInstanceId === INVALID_PROJECT_OCC_GUARD_FIELD
    || !isCanonicalNonNilUuid(expectedProjectInstanceId)
  ) {
    return Promise.reject(
      new NamedTechniqueTimelineClientError('invalid_request'),
    )
  }
  const expectedProjectId = projectOccGuardField(guard, 'expectedProjectId')
  if (
    expectedProjectId === INVALID_PROJECT_OCC_GUARD_FIELD
    || !isCanonicalNonNilUuid(expectedProjectId)
  ) {
    return Promise.reject(
      new NamedTechniqueTimelineClientError('invalid_request'),
    )
  }
  const expectedRevision = projectOccGuardField(guard, 'expectedRevision')
  if (
    expectedRevision === INVALID_PROJECT_OCC_GUARD_FIELD
    || !isProjectRevision(expectedRevision)
    || expectedRevision >= Number.MAX_SAFE_INTEGER
  ) {
    return Promise.reject(
      new NamedTechniqueTimelineClientError('invalid_request'),
    )
  }
  if (!isNamedTechniqueTimelineProposalV1(proposal)) {
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

export function appendGenericTreeInstructionProposal(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  expectedTopologySha256: ReadonlyArray<number>,
) {
  const topologySha256 = snapshotSha256Bytes(expectedTopologySha256)
  if (!isCanonicalNonNilUuid(expectedProjectInstanceId) || !isCanonicalNonNilUuid(expectedProjectId)
    || !isProjectRevision(expectedRevision) || !topologySha256) {
    return Promise.reject(new Error('invalid_generic_tree_instruction_request'))
  }
  return invoke<ProjectSnapshot>('append_generic_tree_instruction_proposal', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    expectedTopologySha256: Array.from(topologySha256), confirmed: true,
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

export function splitInstructionStep(expectedProjectId: string, expectedRevision: number,
  expectedProjectInstanceId: string, stepId: string) {
  return invoke<ProjectSnapshot>('split_instruction_step', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, stepId,
  })
}

export function mergeAdjacentInstructionSteps(expectedProjectId: string, expectedRevision: number,
  expectedProjectInstanceId: string, firstStepId: string, secondStepId: string) {
  return invoke<ProjectSnapshot>('merge_adjacent_instruction_steps', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, firstStepId, secondStepId,
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
  targetLayer?: string,
) {
  return invoke<ProjectSnapshot>('add_edge', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    start,
    end,
    kind,
    targetLayer,
  })
}

export function addRayToFirstTarget(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  start: string,
  angleMicrodegrees: number,
  kind: 'mountain' | 'valley' | 'auxiliary' | 'cut',
  targetLayer?: string,
) {
  if (!Number.isSafeInteger(angleMicrodegrees) || angleMicrodegrees < 0 || angleMicrodegrees >= 360_000_000) {
    return Promise.reject(new Error('Angle must be an exact microdegree value from 0° up to 360° (exclusive).'))
  }
  return invoke<ProjectSnapshot>('add_ray_to_first_target', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    start,
    angleMicrodegrees,
    kind,
    targetLayer,
  })
}

export function addConnectedVertex(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  start: string,
  lengthExpression: string,
  angleDegreesExpression: string,
  kind: 'mountain' | 'valley' | 'auxiliary' | 'cut',
  targetLayer?: string,
) {
  return invoke<ProjectSnapshot>('add_connected_vertex', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    start,
    lengthExpression,
    angleDegreesExpression,
    kind,
    targetLayer,
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

export type LinearArrayRequest = {
  vertices: string[]
  edges: string[]
  additional_copies: number
  delta: { x: number; y: number }
}

export type LinearArrayPreview = {
  version: 1
  project_instance_id: string
  project_id: string
  revision: number
  request_sha256: string
  source_vertex_count: number
  source_edge_count: number
  additional_copies: number
  generated_vertex_count: number
  generated_edge_seed_count: number
  authorizes_project_mutation: false
}

export function previewLinearArray(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  request: LinearArrayRequest,
) {
  return invoke<LinearArrayPreview>('preview_linear_array', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, request,
  })
}

export function confirmLinearArray(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  request: LinearArrayRequest,
  expectedRequestSha256: string,
) {
  return invoke<ProjectSnapshot>('confirm_linear_array', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, request, expectedRequestSha256,
  })
}
export type RadialArrayRequest = {
  center: string; vertices: string[]; edges: string[]
  additional_copies: number; angle_microdegrees: number
}
export type RadialArrayPreview = {
  version: 1; project_instance_id: string; project_id: string; revision: number
  request_sha256: string; source_vertex_count: number; source_edge_count: number
  additional_copies: number; angle_microdegrees: number
  authorizes_project_mutation: false
}
export function previewRadialArray(expectedProjectId: string, expectedRevision: number,
  expectedProjectInstanceId: string, request: RadialArrayRequest) {
  return invoke<RadialArrayPreview>('preview_radial_array', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, request,
  })
}
export function confirmRadialArray(expectedProjectId: string, expectedRevision: number,
  expectedProjectInstanceId: string, request: RadialArrayRequest,
  expectedRequestSha256: string) {
  return invoke<ProjectSnapshot>('confirm_radial_array', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    request, expectedRequestSha256,
  })
}

function requireGeometricConstraintSolvePreview(
  value: unknown,
): GeometricConstraintSolvePreview {
  const parsed = normalizeGeometricConstraintSolvePreview(value)
  if (!parsed) throw new Error('invalid geometric constraint solve preview response')
  return parsed
}

export async function previewGeometricConstraintEdgeSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  drivingEdge: string,
  startXMm: number,
  startYMm: number,
  endXMm: number,
  endYMm: number,
) {
  const value = await invoke<unknown>('preview_geometric_constraint_edge_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    drivingEdge,
    startXMm,
    startYMm,
    endXMm,
    endYMm,
  })
  return requireGeometricConstraintSolvePreview(value)
}

export async function previewGeometricConstraintExpressionSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  const value = await invoke<unknown>('preview_geometric_constraint_expression_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  })
  return requireGeometricConstraintSolvePreview(value)
}

export async function previewGeometricConstraintSolve(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  drivingVertex: string,
  xMm: number,
  yMm: number,
) {
  const value = await invoke<unknown>('preview_geometric_constraint_solve', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    drivingVertex,
    xMm,
    yMm,
  })
  return requireGeometricConstraintSolvePreview(value)
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

export function repairAllUnsplitIntersections(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
) {
  return invoke<ProjectSnapshot>('repair_all_unsplit_intersections', {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
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
  'annotations',
  'underlays',
  'fold_model_fingerprint',
  'boundary_length_authority_v1',
  'can_undo',
  'can_redo',
  'cutting_allowed',
  'reference_model_assets',
  'speculativeUnprovenFolds',
] as const

function normalizeProjectLayerMutationBaseSnapshot(
  value: unknown,
): ProjectSnapshot | null {
  const record = exactCoreDataRecord(
    value,
    PROJECT_LAYER_MUTATION_SNAPSHOT_KEYS,
  )
  const instructionTimeline = record && isCoreDataRecord(
    record.instruction_timeline,
  )
    ? normalizeStrictOptionalPathCertificateReferences(
        record.instruction_timeline,
      )
    : null
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
    || !instructionTimeline
    || !isCoreDataRecord(record.numeric_expressions)
    || !isCoreDataRecord(record.geometric_constraints)
    || !isCoreDataRecord(record.element_metadata)
    || !isCoreDataRecord(record.annotations)
    || !isCoreDataRecord(record.underlays)
    || typeof record.fold_model_fingerprint !== 'string'
    || !/^[0-9a-f]{64}$/u.test(record.fold_model_fingerprint)
    || typeof record.can_undo !== 'boolean'
    || typeof record.can_redo !== 'boolean'
    || typeof record.cutting_allowed !== 'boolean'
  ) return null

  const unprovenSummary = normalizeProjectSnapshotUnprovenSummary(
    record.speculativeUnprovenFolds,
  )
  if (!unprovenSummary) return null
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
  const referenceAssets = snapshotCoreDataArray(
    record.reference_model_assets,
    8,
  )
  if (!referenceAssets) return null
  const referenceModelAssets = referenceAssets.map((value) => {
    const asset = exactCoreDataRecord(value, ['asset_id', 'sha256'] as const)
    const sha256 = asset ? snapshotSha256Bytes(asset.sha256) : null
    if (!asset || !isCanonicalNonNilUuid(asset.asset_id) || !sha256) {
      return null
    }
    return Object.freeze({
      asset_id: asset.asset_id,
      sha256: Object.freeze(Array.from(sha256)),
    })
  })
  if (referenceModelAssets.some((asset) => asset === null)) return null
  const projectLayers = normalizeProjectLayerDocument(
    record.project_layers,
    creasePattern.edges as readonly Readonly<{ id: string }>[],
  )
  const boundaryLengthAuthority = normalizeBoundaryLengthAuthorityV1(
    record.boundary_length_authority_v1,
    {
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      revision: record.revision,
      paper: record.paper,
      crease_pattern: creasePattern,
    },
  )
  if (!projectLayers || !boundaryLengthAuthority) return null

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
    instruction_timeline: instructionTimeline,
    numeric_expressions:
      record.numeric_expressions as ProjectSnapshot['numeric_expressions'],
    geometric_constraints:
      record.geometric_constraints as ProjectSnapshot['geometric_constraints'],
    project_layers: projectLayers,
    element_metadata:
      record.element_metadata as ProjectSnapshot['element_metadata'],
    annotations: record.annotations as ProjectSnapshot['annotations'],
    underlays: record.underlays as ProjectSnapshot['underlays'],
    fold_model_fingerprint: record.fold_model_fingerprint,
    boundary_length_authority_v1: boundaryLengthAuthority,
    reference_model_assets: Object.freeze(
      referenceModelAssets as NonNullable<
        (typeof referenceModelAssets)[number]
      >[],
    ) as unknown as NonNullable<ProjectSnapshot['reference_model_assets']>,
    can_undo: record.can_undo,
    can_redo: record.can_redo,
    cutting_allowed: record.cutting_allowed,
    speculativeUnprovenFolds: unprovenSummary,
  })
}

function normalizeStrictOptionalPathCertificateReferences(
  value: Readonly<Record<string, unknown>>,
): ProjectSnapshot['instruction_timeline'] | null {
  const timeline = snapshotCoreDataRecord(value)
  if (!timeline) return null
  if (timeline.steps === undefined) return null
  const stepInputs = snapshotCoreDataArray(timeline.steps, 512)
  if (!stepInputs) return null
  const steps = stepInputs.map((stepValue) => {
    const step = snapshotCoreDataRecord(stepValue)
    const visual = step && snapshotCoreDataRecord(step.visual)
    if (!step || !visual) return null
    const referenceValue = visual.path_certificate_reference_v1
    const metadataValue = visual.named_technique_compiler_v1
    const cycleProofValue = visual.cycle_layer_order_proof_v1
    let normalizedCycleProof:
      InstructionVisual['cycle_layer_order_proof_v1'] | undefined
    if (cycleProofValue === undefined || cycleProofValue === null) {
      normalizedCycleProof = cycleProofValue
    } else {
      const proof = exactCoreDataRecord(cycleProofValue, [
        'version', 'model_id', 'target_order_sha256',
        'transition_count', 'pairs',
      ] as const)
      const targetOrderSha256 = proof
        ? snapshotSha256Bytes(proof.target_order_sha256)
        : null
      const pairInputs = proof
        ? snapshotCoreDataArray(proof.pairs, 50_000)
        : null
      const pairs = pairInputs?.map((pairValue, index) => {
        const pair = exactCoreDataRecord(
          pairValue,
          ['lower_face', 'upper_face'] as const,
        )
        const previous = index === 0
          ? null
          : exactCoreDataRecord(
              pairInputs[index - 1],
              ['lower_face', 'upper_face'] as const,
            )
        if (
          !pair
          || !isCanonicalNonNilUuid(pair.lower_face)
          || !isCanonicalNonNilUuid(pair.upper_face)
          || pair.lower_face === pair.upper_face
          || (
            previous
            && `${String(previous.lower_face)}:${String(previous.upper_face)}`
              >= `${pair.lower_face}:${pair.upper_face}`
          )
        ) return null
        return Object.freeze({
          lower_face: pair.lower_face,
          upper_face: pair.upper_face,
        })
      }) ?? []
      if (
        !proof
        || proof.version !== 1
        || proof.model_id
          !== 'native_continuous_layer_transport_certificate_v1'
        || !targetOrderSha256
        || !Number.isSafeInteger(proof.transition_count)
        || Number(proof.transition_count) < 1
        || !pairInputs
        || pairs.some((pair) => pair === null)
      ) return null
      normalizedCycleProof = Object.freeze({
        version: 1 as const,
        model_id:
          'native_continuous_layer_transport_certificate_v1' as const,
        target_order_sha256: targetOrderSha256,
        transition_count: Number(proof.transition_count),
        pairs: Object.freeze(pairs.map((pair) => pair!)),
      })
    }
    let normalizedMetadata: InstructionVisual['named_technique_compiler_v1']
      | undefined
    if (metadataValue !== undefined && metadataValue !== null) {
      const metadata = exactCoreDataRecord(metadataValue, [
        'version', 'model_id', 'technique_kind', 'segment_index', 'segment_count',
        'compiler_output_sha256',
      ] as const)
      const compilerOutputSha256 = metadata
        ? snapshotSha256Bytes(metadata.compiler_output_sha256)
        : null
      if (!metadata || metadata.version !== 1
        || metadata.model_id !== 'certified_named_technique_compiler_metadata_v1'
        || !['mountain', 'valley', 'squash', 'crimp', 'inside_reverse', 'outside_reverse',
          'sink', 'accordion', 'layer_selective'].includes(String(metadata.technique_kind))
        || !Number.isSafeInteger(metadata.segment_index) || Number(metadata.segment_index) < 0
        || !Number.isSafeInteger(metadata.segment_count) || Number(metadata.segment_count) < 1
        || Number(metadata.segment_index) >= Number(metadata.segment_count)
        || !compilerOutputSha256
        || !compilerOutputSha256.some((byte) => byte !== 0)) return null
      normalizedMetadata = Object.freeze({
        version: 1 as const,
        model_id: 'certified_named_technique_compiler_metadata_v1' as const,
        technique_kind: metadata.technique_kind as
          BasicFoldTimelinePreviewRequestV1['techniqueKind'],
        segment_index: Number(metadata.segment_index),
        segment_count: Number(metadata.segment_count),
        compiler_output_sha256: compilerOutputSha256,
      })
    } else {
      normalizedMetadata = metadataValue as null | undefined
    }
    let normalizedReference: PathCertificateReferenceV1 | null | undefined
    if (referenceValue === undefined || referenceValue === null) {
      normalizedReference = referenceValue
    } else {
      const reference = exactCoreDataRecord(referenceValue, [
        'version',
        'model_id',
        'binding_sha256',
        'source_pose_sha256',
        'target_pose_sha256',
        'source_model_binding_sha256',
        'transition_count',
      ] as const)
      const bindingSha256 = reference
        ? snapshotSha256Bytes(reference.binding_sha256)
        : null
      const sourcePoseSha256 = reference
        ? snapshotSha256Bytes(reference.source_pose_sha256)
        : null
      const targetPoseSha256 = reference
        ? snapshotSha256Bytes(reference.target_pose_sha256)
        : null
      const sourceModelBindingSha256 = reference
        ? snapshotSha256Bytes(reference.source_model_binding_sha256)
        : null
      if (
        !reference
        || reference.version !== 1
        || reference.model_id
          !== 'bounded_certified_pose_graph_path_reference_v1'
        || !bindingSha256
        || !sourcePoseSha256
        || !targetPoseSha256
        || !sourceModelBindingSha256
        || !bindingSha256.some((byte) => byte !== 0)
        || !sourceModelBindingSha256.some((byte) => byte !== 0)
        || !sourcePoseSha256.some(
          (byte, index) => byte !== targetPoseSha256[index],
        )
        || !Number.isSafeInteger(reference.transition_count)
        || Number(reference.transition_count) < 1
        || Number(reference.transition_count) > 64
      ) return null
      normalizedReference = Object.freeze({
        version: 1 as const,
        model_id: 'bounded_certified_pose_graph_path_reference_v1' as const,
        binding_sha256: bindingSha256,
        source_pose_sha256: sourcePoseSha256,
        target_pose_sha256: targetPoseSha256,
        source_model_binding_sha256: sourceModelBindingSha256,
        transition_count: Number(reference.transition_count),
      })
    }
    return Object.freeze({
      ...step,
      visual: Object.freeze({
        ...visual,
        ...(Object.hasOwn(visual, 'cycle_layer_order_proof_v1')
          ? { cycle_layer_order_proof_v1: normalizedCycleProof }
          : {}),
        ...(Object.hasOwn(visual, 'named_technique_compiler_v1')
          ? { named_technique_compiler_v1: normalizedMetadata }
          : {}),
        ...(Object.hasOwn(visual, 'path_certificate_reference_v1')
          ? { path_certificate_reference_v1: normalizedReference }
          : {}),
      }),
    })
  })
  if (steps.some((step) => step === null)) return null
  return Object.freeze({
    ...timeline,
    steps: Object.freeze(steps),
  }) as unknown as ProjectSnapshot['instruction_timeline']
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
  const baseUnprovenSummary = base
    ? normalizeProjectSnapshotUnprovenSummary(base.speculativeUnprovenFolds)
    : null
  const responseUnprovenSummary = record
    ? normalizeProjectSnapshotUnprovenSummary(record.speculativeUnprovenFolds)
    : null
  if (
    !base
    || !record
    || !baseUnprovenSummary
    || !responseUnprovenSummary
    || !sameProjectSnapshotUnprovenSummary(
      responseUnprovenSummary,
      baseUnprovenSummary,
    )
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
  const boundaryLengthAuthority = normalizeBoundaryLengthAuthorityV1(
    record.boundary_length_authority_v1,
    {
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      revision: record.revision,
      paper: base.paper,
      crease_pattern: base.crease_pattern,
    },
  )
  const baseBoundaryLengthAuthority = (
    base.boundary_length_authority_v1 as BoundaryLengthAuthorityV1
  )
  if (
    !projectLayers
    || !boundaryLengthAuthority
    || !sameBoundaryLengthAuthorityEntries(
      boundaryLengthAuthority,
      baseBoundaryLengthAuthority,
    )
  ) return null

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
    annotations: base.annotations,
    underlays: base.underlays,
    fold_model_fingerprint: base.fold_model_fingerprint,
    boundary_length_authority_v1: boundaryLengthAuthority,
    reference_model_assets: base.reference_model_assets,
    can_undo: record.can_undo,
    can_redo: record.can_redo,
    cutting_allowed: base.cutting_allowed,
    speculativeUnprovenFolds: baseUnprovenSummary,
  })
}

function sameBoundaryLengthAuthorityEntries(
  left: BoundaryLengthAuthorityV1,
  right: BoundaryLengthAuthorityV1,
): boolean {
  if (
    left.status !== right.status
    || left.entries.length !== right.entries.length
  ) return false
  return left.entries.every((entry, index) => {
    const other = right.entries[index]
    return Boolean(
      other
      && entry.boundary_index === other.boundary_index
      && entry.edge_id === other.edge_id
      && entry.start_vertex_id === other.start_vertex_id
      && entry.end_vertex_id === other.end_vertex_id
      && Object.is(entry.length_mm, other.length_mm)
      && entry.length_bits_be.length === other.length_bits_be.length
      && entry.length_bits_be.every(
        (byte, byteIndex) => byte === other.length_bits_be[byteIndex],
      )
    )
  })
}

type ProjectSnapshotUnprovenSummary = Readonly<{
  applied: UnprovenHistoryStatusCountsView
  unappliedRedo: UnprovenHistoryStatusCountsView
}>

function normalizeProjectSnapshotUnprovenSummary(
  value: unknown,
): ProjectSnapshotUnprovenSummary | null {
  const summary = unprovenHistorySummaryFromSnapshotV1({
    speculativeUnprovenFolds: value,
  })
  if (summary.kind !== 'known') return null
  return Object.freeze({
    applied: summary.applied,
    unappliedRedo: summary.unappliedRedo,
  })
}

function sameProjectSnapshotUnprovenSummary(
  left: ProjectSnapshotUnprovenSummary,
  right: ProjectSnapshotUnprovenSummary,
): boolean {
  const keys = [
    'awaitingProof',
    'proofBlocked',
    'unknownEvidenceInsufficient',
    'unknownResourceLimit',
    'unknownCancelled',
    'unknownDeadlineReached',
  ] as const
  return keys.every((key) =>
    left.applied[key] === right.applied[key]
    && left.unappliedRedo[key] === right.unappliedRedo[key])
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
  return !matchesProjectOccGuard({
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision: previousRevision + 1,
  }, record as Readonly<{
    project_instance_id: string
    project_id: string
    revision: number
  }>)
}

function isProjectLayerMutationBaseSnapshot(
  value: unknown,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
): value is ProjectSnapshot {
  const snapshot = normalizeProjectLayerMutationBaseSnapshot(value)
  return snapshot !== null
    && matchesProjectOccGuard({
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    }, snapshot)
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

function coreDataRecordWithOptionalKeys<
  const Required extends readonly string[],
  const Optional extends readonly string[],
>(
  value: unknown,
  requiredKeys: Required,
  optionalKeys: Optional,
): Readonly<Record<Required[number], unknown>
  & Partial<Record<Optional[number], unknown>>> | null {
  const record = snapshotCoreDataRecord(value)
  if (!record
    || requiredKeys.some((key) => !Object.hasOwn(record, key))
    || optionalKeys.some((key) =>
      Object.hasOwn(record, key) && record[key] === undefined)
    || Object.keys(record).some((key) =>
      !requiredKeys.includes(key as Required[number])
      && !optionalKeys.includes(key as Optional[number]))) return null
  return record as Readonly<Record<Required[number], unknown>
    & Partial<Record<Optional[number], unknown>>>
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
