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
  isCycleScheduleRequestV1,
  normalizeLiveHingeRegistryV1,
  normalizeStackedFoldReadResponse,
  type LiveHingeRegistryRequestV1,
  type LiveHingeRegistryResponseV1,
  type CycleScheduleRequestV1,
  type StackedFoldReadRequest,
  type StackedFoldReadResponse,
} from './stackedFoldRead.ts'

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
  continuousLayerTransportModelId: 'general_multi_face_positive_thickness_cell_transport_v1' | null
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
  reference_model_assets?: Array<{ asset_id: string; sha256: number[] }>
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
    fold_path_certificate_sha256?: ReadonlyArray<number>; confidence_score: number
    confidence_reasons: ReadonlyArray<string>; explicit_override: boolean; source_asset_fingerprint: string
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
    }>
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
  return Array.isArray(value) && value.length === length
    && value.every((item) => Number.isInteger(item) && Math.abs(item) <= absoluteMaximum)
}

function isCanonicalGenericBodyOutline(
  value: unknown, mode: 'symmetric' | 'symmetric_ccw' | 'general', minimum = 4, maximum = 16,
  coordinateMaximum = 100_000,
): value is Array<[number, number]> {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum
    || value.some((point) => !isBoundedIntegerTuple(point, 2, coordinateMaximum))) return false
  const points = value as Array<[number, number]>
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

function normalizeBeginnerGenerationConstraints(
  value: unknown,
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
  const record: Record<string, unknown> = {
    ...snapshot,
    protrusions: Object.hasOwn(snapshot, 'protrusions') ? snapshot.protrusions : [],
    bulge_targets: Object.hasOwn(snapshot, 'bulge_targets') ? snapshot.bulge_targets : [],
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
    || record.target_parts.length > 8
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
    if (!item || !Number.isInteger(item.id) || Number(item.id) < 0
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
      || (item.local_outline_tenths_mm !== undefined
        && !isCanonicalGenericBodyOutline(item.local_outline_tenths_mm,
          item.symmetry === 'bilateral' ? 'symmetric_ccw' : 'general', 3, 8, 10_000))
      || !isBoundedIntegerTuple(item.position_tenths_mm, 3, 100_000)
      || !isBoundedIntegerTuple(item.direction_milli, 3, 1_000)
      || item.direction_milli.every((axis) => axis === 0)
      || !['none', 'bilateral', 'radial'].includes(String(item.symmetry))
      || !Number.isInteger(item.curvature_degrees) || Math.abs(Number(item.curvature_degrees)) > 360
      || !['fixed', 'hinge', 'ball'].includes(String(item.joint))
      || !isBoundedIntegerTuple(item.motion_degrees, 2, 360)
      || item.motion_degrees[0] > item.motion_degrees[1]
      || !['front', 'back', 'either'].includes(String(item.side))
      || !Number.isInteger(item.priority) || Number(item.priority) < 1 || Number(item.priority) > 100
    ) return null
    protrusionIds.add(Number(item.id))
    return { ...item } as NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]
  })
  if (protrusions.some((target) => target === null)) return null
  const completeAnimal = record.target_category === 'animal'
    && targetParts.some((part) => part?.kind === 'horn' && part.count === 1)
    && targetParts.some((part) => part?.kind === 'tail' && part.count === 1)
    && targetParts.some((part) => part?.kind === 'ear' && part.count === 2)
    && targetParts.some((part) => part?.kind === 'leg' && part.count === 4)
  const animalWingParts = targetParts.filter((part) => part?.kind === 'wing')
  const completeAnimalHasWings = animalWingParts.length === 1 && animalWingParts[0]?.count === 2
  if (completeAnimal && (animalWingParts.length > 1
    || (animalWingParts.length === 1 && !completeAnimalHasWings)
    || protrusions.length !== (completeAnimalHasWings ? 5 : 4)
    || protrusions[0]?.count !== 1 || protrusions[0]?.symmetry !== 'none'
    || protrusions[0]?.direction_milli[0] !== 0 || protrusions[0]?.direction_milli[1] === 0
    || protrusions[1]?.count !== 1 || protrusions[1]?.symmetry !== 'none'
    || protrusions[1]?.direction_milli[0] === 0 || protrusions[1]?.direction_milli[1] !== 0
    || protrusions[2]?.count !== 2 || protrusions[2]?.symmetry !== 'bilateral'
    || protrusions[3]?.count !== 4 || protrusions[3]?.symmetry !== 'bilateral'
    || (completeAnimalHasWings
      && (protrusions[4]?.count !== 2 || protrusions[4]?.symmetry !== 'bilateral')))) return null
  const bulgeIds = new Set<number>()
  const bulgeTargets = record.bulge_targets.map((value) => {
    const item = exactCoreDataRecord(value, [
      'id', 'face_ids', 'range_min_tenths_mm', 'range_max_tenths_mm',
      'direction_milli', 'amount_tenths_mm', 'source_fold_model_fingerprint',
      'reference_surface_binding',
    ] as const)
    if (!item || !Number.isInteger(item.id) || Number(item.id) < 0 || bulgeIds.has(Number(item.id))
      || !Array.isArray(item.face_ids) || item.face_ids.length < 1 || item.face_ids.length > 32
      || item.face_ids.some((id) => !isCanonicalNonNilUuid(id))
      || new Set(item.face_ids).size !== item.face_ids.length
      || !isBoundedIntegerTuple(item.range_min_tenths_mm, 3, 100_000)
      || !isBoundedIntegerTuple(item.range_max_tenths_mm, 3, 100_000)
      || !isBoundedIntegerTuple(item.direction_milli, 3, 1_000)
      || !Number.isInteger(item.amount_tenths_mm) || Number(item.amount_tenths_mm) < 1
      || Number(item.amount_tenths_mm) > 1_000_000
      || typeof item.source_fold_model_fingerprint !== 'string'
      || !/^[0-9a-f]{64}$/u.test(item.source_fold_model_fingerprint)) return null
    const surface = item.reference_surface_binding === undefined ? null
      : exactCoreDataRecord(item.reference_surface_binding, [
          'asset_id', 'range_id', 'protrusion_id', 'triangle_indices', 'range_digest_sha256',
        ] as const)
    if (item.reference_surface_binding !== undefined && (!surface
      || !isCanonicalNonNilUuid(surface.asset_id)
      || !Number.isInteger(surface.range_id) || Number(surface.range_id) < 1
      || !Number.isInteger(surface.protrusion_id) || Number(surface.protrusion_id) < 1
      || !Array.isArray(surface.triangle_indices) || surface.triangle_indices.length < 1
      || surface.triangle_indices.length > 40_000
      || surface.triangle_indices.some((triangle) => !Number.isInteger(triangle) || triangle < 0)
      || new Set(surface.triangle_indices).size !== surface.triangle_indices.length
      || !isBoundedIntegerTuple(surface.range_digest_sha256, 32, 255))) return null
    const minimum = item.range_min_tenths_mm
    const maximum = item.range_max_tenths_mm
    const direction = item.direction_milli
    if (minimum.some((value, index) => value > maximum[index])
      || minimum.every((value, index) => value === maximum[index])
      || direction.every((axis) => axis === 0)) return null
    bulgeIds.add(Number(item.id))
    return { ...item, ...(surface === null ? {} : { reference_surface_binding: { ...surface } })
    } as NonNullable<BeginnerGenerationConstraintsV1['bulge_targets']>[number]
  })
  if (bulgeTargets.some((target) => target === null)) return null
  let targetAsset: BeginnerGenerationConstraintsV1['target_asset'] = null
  if (record.target_asset !== null) {
    const candidate = isCoreDataRecord(record.target_asset) ? record.target_asset : null
    if (candidate?.kind === 'reference_image') {
      const asset = exactCoreDataRecord(candidate, ['kind', 'underlay_id', 'asset_id'] as const)
      if (!asset || !isCanonicalNonNilUuid(asset.underlay_id)
        || !isCanonicalNonNilUuid(asset.asset_id)) return null
      targetAsset = {
        kind: 'reference_image',
        underlay_id: asset.underlay_id,
        asset_id: asset.asset_id,
      }
    } else {
      const asset = exactCoreDataRecord(candidate, ['kind', 'asset_id'] as const)
      if (!asset || asset.kind !== 'reference_model'
        || !isCanonicalNonNilUuid(asset.asset_id)) return null
      targetAsset = {
        kind: 'reference_model',
        asset_id: asset.asset_id,
      }
    }
  }
  let componentBridgeOverride: BeginnerGenerationConstraintsV1['component_bridge_override']
  if (record.component_bridge_override !== undefined) {
    const document = exactCoreDataRecord(record.component_bridge_override, [
      'schema_version', 'source_asset_sha256', 'component_count', 'reviewed', 'bridges',
    ] as const)
    if (!document || document.schema_version !== 1 || !isBoundedIntegerTuple(document.source_asset_sha256, 32, 255)
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
    componentBridgeOverride = { schema_version: 1, source_asset_sha256: document.source_asset_sha256.slice(), component_count: Number(document.component_count), reviewed: document.reviewed, bridges: bridges as NonNullable<typeof componentBridgeOverride>['bridges'] }
  }
  let silhouetteThresholds: BeginnerGenerationConstraintsV1['silhouette_thresholds']
  if (record.silhouette_thresholds !== undefined) {
    const thresholds = snapshotCoreDataRecord(record.silhouette_thresholds)
    if (!thresholds || thresholds.schema_version !== 1
      || Object.keys(thresholds).some((key) => !['schema_version', 'alpha', 'luma', 'polarity'].includes(key))
      || !Number.isInteger(thresholds.alpha) || Number(thresholds.alpha) < 0 || Number(thresholds.alpha) > 255
      || !Number.isInteger(thresholds.luma) || Number(thresholds.luma) < 0 || Number(thresholds.luma) > 255
      || !['dark_on_light', 'light_on_dark', 'alpha_only'].includes(String(thresholds.polarity ?? 'dark_on_light'))) return null
    silhouetteThresholds = { schema_version: 1, alpha: Number(thresholds.alpha), luma: Number(thresholds.luma), polarity: (thresholds.polarity ?? 'dark_on_light') as 'dark_on_light' | 'light_on_dark' | 'alpha_only' }
  }
  let silhouetteCropRoi: BeginnerGenerationConstraintsV1['silhouette_crop_roi']
  if (record.silhouette_crop_roi !== undefined) {
    const roi = exactCoreDataRecord(record.silhouette_crop_roi, ['schema_version', 'x_millionths', 'y_millionths', 'width_millionths', 'height_millionths'] as const)
    const values = roi && [roi.x_millionths, roi.y_millionths, roi.width_millionths, roi.height_millionths]
    if (!roi || roi.schema_version !== 1 || !values || values.some((value) => !Number.isInteger(value) || Number(value) < 0 || Number(value) > 1_000_000)
      || Number(roi.width_millionths) < 1 || Number(roi.height_millionths) < 1
      || Number(roi.x_millionths) + Number(roi.width_millionths) > 1_000_000
      || Number(roi.y_millionths) + Number(roi.height_millionths) > 1_000_000) return null
    silhouetteCropRoi = { schema_version: 1, x_millionths: Number(roi.x_millionths), y_millionths: Number(roi.y_millionths), width_millionths: Number(roi.width_millionths), height_millionths: Number(roi.height_millionths) }
  }
  const silhouetteOrientation = record.silhouette_orientation_degrees
  if (silhouetteOrientation !== undefined && ![0, 90, 180, 270].includes(Number(silhouetteOrientation))) return null
  let silhouetteMirror: BeginnerGenerationConstraintsV1['silhouette_mirror']
  if (record.silhouette_mirror !== undefined) {
    const mirror = exactCoreDataRecord(record.silhouette_mirror, ['schema_version', 'mirror_x', 'mirror_y'] as const)
    if (!mirror || mirror.schema_version !== 1 || typeof mirror.mirror_x !== 'boolean'
      || typeof mirror.mirror_y !== 'boolean') return null
    silhouetteMirror = { schema_version: 1, mirror_x: mirror.mirror_x, mirror_y: mirror.mirror_y }
  }
  return Object.freeze({
    schema_version: 1,
    maximum_steps: Number(record.maximum_steps),
    detail_level: record.detail_level,
    ...(record.generic_body_size_tenths_mm === undefined ? {} : {
      generic_body_size_tenths_mm: record.generic_body_size_tenths_mm as [number, number],
    }),
    ...(record.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: (record.generic_body_outline_tenths_mm as Array<[number, number]>)
        .map((point) => [...point] as [number, number]),
    }),
    generic_body_outline_mode: record.generic_body_outline_mode === 'general' ? 'general' : 'symmetric',
    target_category: record.target_category,
    ...(record.custom_object_display_name === undefined ? {} : {
      custom_object_display_name: record.custom_object_display_name,
    }),
    target_parts: targetParts,
    skeleton_segments: skeletonSegments,
    ...(componentBridgeOverride ? { component_bridge_override: componentBridgeOverride } : {}),
    ...(silhouetteThresholds ? { silhouette_thresholds: silhouetteThresholds } : {}),
    ...(silhouetteCropRoi ? { silhouette_crop_roi: silhouetteCropRoi } : {}),
    ...(silhouetteOrientation === undefined ? {} : { silhouette_orientation_degrees: Number(silhouetteOrientation) as 0 | 90 | 180 | 270 }),
    ...(silhouetteMirror ? { silhouette_mirror: silhouetteMirror } : {}),
    ...(hadProtrusions ? { protrusions } : {}),
    ...(hadBulgeTargets ? { bulge_targets: bulgeTargets } : {}),
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
  if (!record || requiredKeys.some((key) => !Object.hasOwn(record, key))
    || Object.keys(record).some((key) => ![...requiredKeys, ...optionalKeys].includes(key as never))) return null
  if (!record || record.schema_version !== 1 || record.format !== expectedFormat
    || record.source_underlay_id !== expectedUnderlayId
    || record.source_asset_id !== expectedAssetId
    || !Array.isArray(record.source_sha256) || record.source_sha256.length !== 32
    || record.source_sha256.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
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
    source_sha256: Object.freeze(record.source_sha256.slice()),
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
    revision: number; image_sha256: number[]; reference_sha256: number[]; source_count: 2
    image_component_count: number; reference_component_count: number
    image_branch_count: number; reference_branch_count: number
    normalized_extent_error: number; agreement_score: number; apply_allowed: boolean
    reason: 'image_glb_agreement_v1' | 'image_glb_disagreement_v1'
  }
  reference_consensus_analysis: null | {
    schema_version: 1; revision: number; source_count: number; excluded_asset_id: string | null
    pair_count: number; disagreement_count: number; agreement_score: number; apply_allowed: boolean
    reason: 'reference_consensus_agreement_v1' | 'reference_consensus_multiple_disagreements_v1'
    pairs: Array<{ left_asset_id: string; right_asset_id: string; component_error: number
      normalized_extent_error: number; branch_error: number; agreement_score: number
      disagrees: boolean; pair_digest_sha256: number[] }>
  }
}

export type BeginnerGeneratedPlanAssessmentV1 = {
  kind: BeginnerGeneratedPlanV1['kind']
  expected_candidate_edge_id: string
  proof_scope: 'necessary' | 'sufficient' | 'indeterminate'
  apply_allowed: boolean
  shape_approximation_score: number | null
  shape_difference_reason: 'crease_preview_has_no_surface_mesh' | 'certified_flat_surface_v1' | 'component_aware_quantized_shape_v1' | null
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
    | 'global_flat_foldability_proven'
    | 'global_flat_foldability_impossible'
    | 'global_resource_limit'
    | 'global_timeout'
    | 'global_indeterminate'
    | 'multi_reference_disagreement'
}

export type BeginnerGeneratedPlanV1 = {
  schema_version: 1
  kind:
    | 'symmetric_four_leg_base'
    | 'symmetric_wing_base'
    | 'symmetric_bird_base'
    | 'asymmetric_bird_landmark_base'
    | 'asymmetric_four_leg_landmark_base'
    | 'asymmetric_insect_landmark_base'
    | 'asymmetric_fish_landmark_base'
    | 'symmetric_fish_base'
    | 'symmetric_ear_base'
    | 'symmetric_horn_base'
    | 'symmetric_antenna_base'
    | 'symmetric_insect_leg_pair_base'
    | 'symmetric_six_leg_base'
    | 'center_axis_tail_base'
    | 'center_axis_horn_base'
    | 'center_axis_antenna_base'
    | 'composite_tail_ear_base'
    | 'composite_horn_ear_base'
    | 'composite_horn_tail_base'
    | 'composite_horn_tail_ear_base'
    | 'composite_wing_antenna_base'
    | 'composite_complete_insect_base'
    | 'composite_complete_animal_base'
    | 'composite_complete_winged_animal_base'
    | 'composite_generic_target_base'
    | 'vertical_book_fold'
    | 'horizontal_book_fold'
    | 'diagonal_fold'
  crease_pattern: {
    vertices: Array<{ id: string; position: { x: number; y: number } }>
    edges: Array<{ id: string; start: string; end: string; kind: 'mountain' | 'valley' }>
  }
  instruction_codes: string[]
  target_parts: BeginnerGenerationConstraintsV1['target_parts']
  skeleton_segments: BeginnerGenerationConstraintsV1['skeleton_segments']
  target_asset: BeginnerGenerationConstraintsV1['target_asset']
  semantic_landmark_provenance?: {
    schema_version: 1
    ordered_bindings: Array<{ ordinal: number; role: string; physical_ray: number }>
    physical_ray_group_sha256: number[][]
  }
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
    'plan_assessments',
    'candidates',
    'multi_reference_fusion',
    'reference_consensus_analysis',
  ] as const)
  const fusion = response && response.multi_reference_fusion === null ? null
    : exactCoreDataRecord(response?.multi_reference_fusion, [
      'revision', 'image_sha256', 'reference_sha256', 'source_count', 'image_component_count',
      'reference_component_count', 'image_branch_count', 'reference_branch_count',
      'normalized_extent_error', 'agreement_score', 'apply_allowed', 'reason',
    ] as const)
  const consensus = response && response.reference_consensus_analysis === null ? null
    : exactCoreDataRecord(response?.reference_consensus_analysis, [
      'schema_version', 'revision', 'source_count', 'excluded_asset_id', 'pair_count',
      'disagreement_count', 'agreement_score', 'apply_allowed', 'reason', 'pairs',
    ] as const)
  const consensusPairs = consensus && Array.isArray(consensus.pairs) ? consensus.pairs.map((raw) =>
    exactCoreDataRecord(raw, ['left_asset_id', 'right_asset_id', 'component_error',
      'normalized_extent_error', 'branch_error', 'agreement_score', 'disagrees', 'pair_digest_sha256'] as const)) : []
  if (
    !response
    || response.schema_version !== 1
    || response.project_instance_id !== expectedProjectInstanceId
    || response.project_id !== expectedProjectId
    || response.revision !== expectedRevision
    || response.requested_candidate_count !== requestedCandidateCount
    || response.bulge_treatment !== 'target_shape_approximation'
    || response.elasticity_model !== 'not_computed'
    || !['ready', 'resource_limit', 'unsupported_paper', 'unsupported_techniques', 'missing_target_category', 'missing_required_parts', 'missing_target_asset', 'unsupported_animal_template', 'unsupported_insect_template']
      .includes(String(response.generation_status))
    || !Array.isArray(response.generated_plans)
    || response.generated_plans.length > 3
    || !Array.isArray(response.plan_assessments)
    || response.plan_assessments.length !== response.generated_plans.length
    || !Array.isArray(response.candidates)
    || response.candidates.length < 1
    || response.candidates.length > 3
    || response.candidates.length !== requestedCandidateCount
    || (fusion !== null && (!fusion || fusion.revision !== expectedRevision || fusion.source_count !== 2
      || !isBoundedIntegerTuple(fusion.image_sha256, 32, 255) || !isBoundedIntegerTuple(fusion.reference_sha256, 32, 255)
      || [fusion.image_component_count, fusion.reference_component_count].some((count) => !Number.isInteger(count) || Number(count) < 1 || Number(count) > 8)
      || [fusion.image_branch_count, fusion.reference_branch_count].some((count) => !Number.isInteger(count) || Number(count) < 1 || Number(count) > 16)
      || !Number.isInteger(fusion.normalized_extent_error) || Number(fusion.normalized_extent_error) < 0 || Number(fusion.normalized_extent_error) > 100
      || !Number.isInteger(fusion.agreement_score) || Number(fusion.agreement_score) < 0 || Number(fusion.agreement_score) > 100
      || typeof fusion.apply_allowed !== 'boolean'
      || !['image_glb_agreement_v1', 'image_glb_disagreement_v1'].includes(String(fusion.reason))
      || (fusion.reason === 'image_glb_agreement_v1') !== fusion.apply_allowed))
    || (consensus !== null && (!consensus || consensus.schema_version !== 1 || consensus.revision !== expectedRevision
      || !Number.isInteger(consensus.source_count) || Number(consensus.source_count) < 2 || Number(consensus.source_count) > 4
      || (consensus.excluded_asset_id !== null && !isCanonicalNonNilUuid(consensus.excluded_asset_id))
      || !Number.isInteger(consensus.pair_count) || Number(consensus.pair_count) < 1 || Number(consensus.pair_count) > 6
      || consensusPairs.length !== consensus.pair_count || !Number.isInteger(consensus.disagreement_count)
      || Number(consensus.disagreement_count) < 0 || Number(consensus.disagreement_count) > Number(consensus.pair_count)
      || !Number.isInteger(consensus.agreement_score) || Number(consensus.agreement_score) < 0 || Number(consensus.agreement_score) > 100
      || typeof consensus.apply_allowed !== 'boolean'
      || !['reference_consensus_agreement_v1', 'reference_consensus_multiple_disagreements_v1'].includes(String(consensus.reason))
      || (Number(consensus.disagreement_count) < 2) !== consensus.apply_allowed
      || consensusPairs.some((pair) => !pair || !isCanonicalNonNilUuid(pair.left_asset_id) || !isCanonicalNonNilUuid(pair.right_asset_id)
        || pair.left_asset_id === pair.right_asset_id || !isBoundedIntegerTuple(pair.pair_digest_sha256, 32, 255)
        || [pair.component_error, pair.normalized_extent_error, pair.branch_error, pair.agreement_score]
          .some((metric) => !Number.isInteger(metric) || Number(metric) < 0 || Number(metric) > 100)
        || typeof pair.disagrees !== 'boolean')))
  ) return null
  const candidates = response.candidates.map((candidate, index) => {
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
  const generatedPlans = response.generated_plans.map((plan) => {
    const record = exactCoreDataRecord(plan, [
      'schema_version', 'kind', 'crease_pattern', 'instruction_codes', 'target_parts',
      'skeleton_segments',
      'target_asset',
      'semantic_landmark_provenance',
    ] as const)
    const pattern = record && exactCoreDataRecord(record.crease_pattern, ['vertices', 'edges'] as const)
    if (
      !record
      || record.schema_version !== 1
      || !['symmetric_four_leg_base', 'symmetric_wing_base', 'symmetric_bird_base', 'asymmetric_bird_landmark_base', 'asymmetric_four_leg_landmark_base', 'asymmetric_insect_landmark_base', 'asymmetric_fish_landmark_base', 'symmetric_fish_base', 'symmetric_ear_base', 'symmetric_horn_base', 'symmetric_antenna_base', 'symmetric_insect_leg_pair_base', 'symmetric_six_leg_base', 'center_axis_tail_base', 'center_axis_horn_base', 'center_axis_antenna_base', 'composite_tail_ear_base', 'composite_horn_ear_base', 'composite_horn_tail_base', 'composite_horn_tail_ear_base', 'composite_wing_antenna_base', 'composite_complete_insect_base', 'composite_complete_animal_base', 'composite_complete_winged_animal_base', 'composite_generic_target_base', 'vertical_book_fold', 'horizontal_book_fold', 'diagonal_fold'].includes(String(record.kind))
      || !pattern
      || !Array.isArray(pattern.vertices)
      || pattern.vertices.length < 2
      || pattern.vertices.length > (record.kind === 'composite_generic_target_base' ? 33 : record.kind === 'composite_complete_insect_base' ? 21 : record.kind === 'composite_complete_winged_animal_base' ? 15 : record.kind === 'composite_complete_animal_base' ? 11 : record.kind === 'symmetric_six_leg_base' ? 13 : record.kind === 'composite_wing_antenna_base' ? 9 : record.kind === 'composite_horn_tail_ear_base' ? 7 : ['composite_tail_ear_base', 'composite_horn_ear_base'].includes(String(record.kind)) ? 6 : 5)
      || !Array.isArray(pattern.edges)
      || pattern.edges.length < 1
      || pattern.edges.length > (record.kind === 'composite_generic_target_base' ? 32 : record.kind === 'composite_complete_insect_base' ? 20 : record.kind === 'composite_complete_winged_animal_base' ? 14 : record.kind === 'composite_complete_animal_base' ? 10 : record.kind === 'symmetric_six_leg_base' ? 12 : record.kind === 'composite_wing_antenna_base' ? 8 : record.kind === 'composite_horn_tail_ear_base' ? 6 : ['composite_tail_ear_base', 'composite_horn_ear_base'].includes(String(record.kind)) ? 5 : 4)
      || !Array.isArray(record.instruction_codes)
      || record.instruction_codes.length !== 1
      || !record.instruction_codes.every((code) =>
        ['symmetric_four_leg_base', 'symmetric_wing_base', 'symmetric_bird_base', 'asymmetric_bird_landmark_base', 'asymmetric_four_leg_landmark_base', 'asymmetric_insect_landmark_base', 'asymmetric_fish_landmark_base', 'symmetric_fish_base', 'symmetric_ear_base', 'symmetric_horn_base', 'symmetric_antenna_base', 'symmetric_insect_leg_pair_base', 'symmetric_six_leg_base', 'center_axis_tail_base', 'center_axis_horn_base', 'center_axis_antenna_base', 'composite_tail_ear_base', 'composite_horn_ear_base', 'composite_horn_tail_base', 'composite_horn_tail_ear_base', 'composite_wing_antenna_base', 'composite_complete_insect_base', 'composite_complete_animal_base', 'composite_complete_winged_animal_base', 'composite_generic_target_base', 'book_fold_vertical', 'book_fold_horizontal', 'diagonal_fold'].includes(String(code)))
    ) return null
    const normalizedPlanInputs = normalizeBeginnerGenerationConstraints({
      schema_version: 1,
      maximum_steps: 1,
      detail_level: 'simple',
      target_category: record.kind === 'composite_generic_target_base'
        ? 'custom_object'
        : record.kind === 'asymmetric_insect_landmark_base' ? 'insect' : 'animal',
      target_parts: record.target_parts,
      skeleton_segments: record.skeleton_segments,
      target_asset: record.target_asset,
      allowed_techniques: ['valley_fold'],
    })
    if (!normalizedPlanInputs) return null
    const semantic = record.semantic_landmark_provenance === undefined ? null
      : exactCoreDataRecord(record.semantic_landmark_provenance, [
        'schema_version', 'ordered_bindings', 'physical_ray_group_sha256',
      ] as const)
    const semanticRoles = record.kind === 'asymmetric_insect_landmark_base'
      ? [
        'head', 'tail', 'wing_left', 'wing_right', 'leg_front_left', 'leg_front_right',
        'leg_middle_left', 'leg_middle_right', 'leg_rear_left', 'leg_rear_right',
      ]
      : record.kind === 'asymmetric_fish_landmark_base'
        ? ['head', 'tail', 'fin_left', 'fin_right']
        : null
    const semanticBindings = semantic && Array.isArray(semantic.ordered_bindings)
      ? semantic.ordered_bindings.map((value, index) => {
        const binding = exactCoreDataRecord(value, ['ordinal', 'role', 'physical_ray'] as const)
        return binding && binding.ordinal === index && binding.role === semanticRoles?.[index]
          && Number.isInteger(binding.physical_ray) && Number(binding.physical_ray) >= 0
          && Number(binding.physical_ray) < 4
          ? { ordinal: index, role: String(binding.role), physical_ray: Number(binding.physical_ray) }
          : null
      }) : null
    const rayDigests = semantic && Array.isArray(semantic.physical_ray_group_sha256)
      ? semantic.physical_ray_group_sha256 : null
    if ((semanticRoles !== null) !== (semantic !== null)
      || (semantic && (semantic.schema_version !== 1 || !semanticBindings
        || semanticBindings.length !== semanticRoles?.length
        || semanticBindings.some((binding) => binding === null)
        || rayDigests?.length !== 4
        || rayDigests.some((digest) => !Array.isArray(digest) || digest.length !== 32
          || digest.some((byte) => !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255))))) return null
    const vertices = pattern.vertices.map((vertex) => {
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
    const edges = pattern.edges.map((value) => {
      const edge = exactCoreDataRecord(value, ['id', 'start', 'end', 'kind'] as const)
      if (!edge
        || !isCanonicalNonNilUuid(edge.id) || !isCanonicalNonNilUuid(edge.start)
        || !isCanonicalNonNilUuid(edge.end) || edge.start === edge.end
        || !vertexIds.has(edge.start) || !vertexIds.has(edge.end)
        || !['mountain', 'valley'].includes(String(edge.kind))) return null
      return {
        id: edge.id,
        start: edge.start,
        end: edge.end,
        kind: edge.kind,
      } as BeginnerGeneratedPlanV1['crease_pattern']['edges'][number]
    })
    if (edges.some((edge) => edge === null)) return null
    const admittedEdges = edges as BeginnerGeneratedPlanV1['crease_pattern']['edges']
    if (new Set(admittedEdges.map((edge) => edge.id)).size !== admittedEdges.length) return null
    return {
      schema_version: 1,
      kind: record.kind,
      crease_pattern: { vertices: admittedVertices, edges: admittedEdges },
      instruction_codes: record.instruction_codes.slice(),
      target_parts: normalizedPlanInputs.target_parts,
      skeleton_segments: normalizedPlanInputs.skeleton_segments,
      target_asset: normalizedPlanInputs.target_asset,
      ...(semantic && semanticBindings && rayDigests ? { semantic_landmark_provenance: {
        schema_version: 1 as const,
        ordered_bindings: semanticBindings as Array<{ ordinal: number; role: string; physical_ray: number }>,
        physical_ray_group_sha256: rayDigests as number[][],
      } } : {}),
    } as BeginnerGeneratedPlanV1
  })
  if (generatedPlans.some((plan) => plan === null)
    || (response.generation_status === 'ready') !== (generatedPlans.length > 0)) return null
  const admittedPlans = generatedPlans as BeginnerGeneratedPlanV1[]
  const planAssessments = response.plan_assessments.map((assessment, index) => {
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
      || ![null, 'crease_preview_has_no_surface_mesh', 'certified_flat_surface_v1', 'component_aware_quantized_shape_v1'].includes(
        record.shape_difference_reason as null | string,
      )
      || (componentComparison !== null && (!componentComparison
        || !Number.isInteger(componentComparison.component_count)
        || Number(componentComparison.component_count) < 2 || Number(componentComparison.component_count) > 8
        || !Number.isInteger(componentComparison.matched_branch_count)
        || Number(componentComparison.matched_branch_count) < 0
        || Number(componentComparison.matched_branch_count) > Number(componentComparison.component_count)
        || !Number.isInteger(componentComparison.work_units) || Number(componentComparison.work_units) > 64
        || !componentScores || componentScores.some((score) => !Number.isInteger(score) || Number(score) < 0 || Number(score) > 100)
        || componentComparison.extent_weight !== 45 || componentComparison.branch_weight !== 35
        || componentComparison.bridge_weight !== 20))
      || ((record.shape_difference_reason === 'component_aware_quantized_shape_v1') !== (componentComparison !== null))
      || ((record.shape_approximation_score === null)
        !== (record.shape_difference_reason === null))
      || ![
        'geometry_invalid', 'folded_pose_simulation_failed', 'fold_path_certificate_unavailable',
        'manufacturability_missing_vertex',
        'manufacturability_minimum_crease_spacing', 'manufacturability_minimum_face_area',
        'manufacturability_paper_boundary_margin', 'necessary_conditions_satisfied',
        'necessary_conditions_violated', 'local_analysis_blocked',
        'local_theorem_not_applicable', 'local_analysis_indeterminate',
        'global_flat_foldability_proven', 'global_flat_foldability_impossible',
        'global_resource_limit', 'global_timeout', 'global_indeterminate',
        'multi_reference_disagreement',
      ].includes(String(record.reason))
      || (record.apply_allowed === false
        && !['geometry_invalid', 'folded_pose_simulation_failed', 'fold_path_certificate_unavailable',
          'manufacturability_missing_vertex',
          'manufacturability_minimum_crease_spacing', 'manufacturability_minimum_face_area',
          'manufacturability_paper_boundary_margin', 'necessary_conditions_violated', 'local_analysis_blocked',
          'global_flat_foldability_impossible']
          .concat('multi_reference_disagreement')
          .includes(String(record.reason)))
      || (record.proof_scope === 'indeterminate' && record.apply_allowed !== true
        && record.reason !== 'multi_reference_disagreement')
      || (record.reason === 'global_flat_foldability_proven'
        && (record.proof_scope !== 'sufficient' || record.apply_allowed !== true))
      || (record.reason === 'global_flat_foldability_impossible'
        && (record.proof_scope !== 'necessary' || record.apply_allowed !== false))
      || (['global_resource_limit', 'global_timeout', 'global_indeterminate']
        .includes(String(record.reason))
        && record.proof_scope !== 'indeterminate')
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
    generated_plans: admittedPlans,
    plan_assessments: planAssessments as BeginnerGeneratedPlanAssessmentV1[],
    candidates: admitted.slice(),
    multi_reference_fusion: fusion as BeginnerCandidateResponseV1['multi_reference_fusion'],
    reference_consensus_analysis: consensus === null ? null : Object.freeze({
      ...consensus, pairs: Object.freeze(consensusPairs.map((pair) => Object.freeze({ ...pair }))),
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
  const record = snapshotCoreDataRecord(value)
  if (!record || requiredKeys.some((key) => !Object.hasOwn(record, key))
    || Object.keys(record).some((key) => ![...requiredKeys, 'generation_provenance',
      'reference_surface_landmarks_tenths_mm', 'outline_edit_authority',
      'archived_reference_model_asset_ids', 'reference_consensus_v1'].includes(key as never))) return null
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
  const provenance = record.generation_provenance === undefined ? null : exactCoreDataRecord(
    record.generation_provenance, ['schema_version', 'topology_authority_sha256', 'fold_path_certificate_sha256', 'confidence_score',
      'confidence_reasons', 'explicit_override', 'source_asset_fingerprint', 'generic_tree', 'reference_consensus'] as const)
  const provenanceConsensus = provenance?.reference_consensus === undefined ? null : exactCoreDataRecord(
    provenance.reference_consensus, ['schema_version', 'source_revision', 'bindings', 'excluded_asset_id', 'pair_digests_sha256'] as const)
  const genericTree = provenance?.generic_tree === undefined ? null : exactCoreDataRecord(
    provenance.generic_tree, ['schema_version', 'target_category', 'source', 'asset_content_sha256', 'tree_topology_sha256',
      'normalized_length_ratios', 'orientation', 'generator_version', 'authorizes_apply', 'instruction_proposal'] as const)
  const treeProposal = genericTree?.instruction_proposal === undefined ? null : exactCoreDataRecord(
    genericTree.instruction_proposal, ['schema_version', 'topology_sha256', 'generator_version', 'authorizes_apply',
      'physical_motion_proof', 'steps'] as const)
  if (record.generation_provenance !== undefined && (!provenance || provenance.schema_version !== 1
    || !Array.isArray(provenance.topology_authority_sha256) || provenance.topology_authority_sha256.length !== 32
    || provenance.topology_authority_sha256.some((byte) => !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255)
    || (provenance.fold_path_certificate_sha256 !== undefined
      && (!Array.isArray(provenance.fold_path_certificate_sha256) || provenance.fold_path_certificate_sha256.length !== 32
        || provenance.fold_path_certificate_sha256.some((byte) => !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255)))
    || !Number.isInteger(provenance.confidence_score) || Number(provenance.confidence_score) < 0 || Number(provenance.confidence_score) > 100
    || !Array.isArray(provenance.confidence_reasons) || provenance.confidence_reasons.length > 8
    || provenance.confidence_reasons.some((reason) => typeof reason !== 'string' || reason.length < 1 || reason.length > 64)
    || typeof provenance.explicit_override !== 'boolean' || typeof provenance.source_asset_fingerprint !== 'string'
    || provenance.source_asset_fingerprint.length < 1 || provenance.source_asset_fingerprint.length > 128
    || (provenance.reference_consensus !== undefined && (!provenanceConsensus || provenanceConsensus.schema_version !== 1
      || !Number.isSafeInteger(provenanceConsensus.source_revision) || Number(provenanceConsensus.source_revision) < 0
      || !Array.isArray(provenanceConsensus.bindings) || provenanceConsensus.bindings.length < 2 || provenanceConsensus.bindings.length > 4
      || provenanceConsensus.bindings.some((raw) => { const binding = exactCoreDataRecord(raw, ['kind', 'asset_id', 'sha256', 'quality'] as const); return !binding
        || !['image', 'reference_model'].includes(String(binding.kind)) || !isCanonicalNonNilUuid(binding.asset_id)
        || !isBoundedIntegerTuple(binding.sha256, 32, 255) || !Number.isInteger(binding.quality)
        || Number(binding.quality) < 0 || Number(binding.quality) > 100 })
      || (provenanceConsensus.excluded_asset_id !== undefined && (!isCanonicalNonNilUuid(provenanceConsensus.excluded_asset_id)
        || !(provenanceConsensus.bindings as Array<Record<string, unknown>>).some((binding) => binding.asset_id === provenanceConsensus.excluded_asset_id)))
      || !Array.isArray(provenanceConsensus.pair_digests_sha256) || provenanceConsensus.pair_digests_sha256.length < 1
      || provenanceConsensus.pair_digests_sha256.length > 6
      || provenanceConsensus.pair_digests_sha256.some((digest) => !isBoundedIntegerTuple(digest, 32, 255))))
    || (provenance.generic_tree !== undefined && (!genericTree || genericTree.schema_version !== 1
      || !['image_silhouette', 'glb_geometry', 'manual_skeleton'].includes(String(genericTree.source))
      || (genericTree.target_category !== undefined && genericTree.target_category !== 'custom_object')
      || (genericTree.asset_content_sha256 !== undefined && !isBoundedIntegerTuple(genericTree.asset_content_sha256, 32, 255))
      || !isBoundedIntegerTuple(genericTree.tree_topology_sha256, 32, 255)
      || !Array.isArray(genericTree.normalized_length_ratios) || genericTree.normalized_length_ratios.length < 1
      || genericTree.normalized_length_ratios.length > 16 || genericTree.normalized_length_ratios.some(
        (ratio) => !Number.isSafeInteger(ratio) || Number(ratio) < 1_000_000)
      || !['horizontal', 'vertical'].includes(String(genericTree.orientation))
      || genericTree.generator_version !== 1 || genericTree.authorizes_apply !== false
      || (genericTree.instruction_proposal !== undefined && (!treeProposal || treeProposal.schema_version !== 1
        || !isBoundedIntegerTuple(treeProposal.topology_sha256, 32, 255)
        || JSON.stringify(treeProposal.topology_sha256) !== JSON.stringify(genericTree.tree_topology_sha256)
        || treeProposal.generator_version !== 1 || treeProposal.authorizes_apply !== false
        || treeProposal.physical_motion_proof !== false || !Array.isArray(treeProposal.steps)
        || treeProposal.steps.length < 1 || treeProposal.steps.length > 16
        || treeProposal.steps.some((rawStep, index, all) => {
          const step = exactCoreDataRecord(rawStep, ['canonical_crease_id', 'tree_depth', 'assignment', 'target_branch', 'fixed_side', 'caution'] as const)
          const previous = index === 0 ? null : exactCoreDataRecord(all[index - 1], ['canonical_crease_id', 'tree_depth', 'assignment', 'target_branch', 'fixed_side', 'caution'] as const)
          return !step || typeof step.canonical_crease_id !== 'string' || step.canonical_crease_id.length < 1 || step.canonical_crease_id.length > 64
            || !Number.isInteger(step.tree_depth) || Number(step.tree_depth) < 0 || Number(step.tree_depth) > 16
            || !['mountain', 'valley'].includes(String(step.assignment)) || typeof step.target_branch !== 'string'
            || step.target_branch.length < 1 || step.target_branch.length > 96 || !['root', 'leaf'].includes(String(step.fixed_side))
            || typeof step.caution !== 'string' || step.caution.length < 1 || step.caution.length > 256
            || (previous !== null && (Number(previous.tree_depth) > Number(step.tree_depth)
              || (previous.tree_depth === step.tree_depth && String(previous.canonical_crease_id) >= step.canonical_crease_id)))
        }))))))) return null
  const landmarks = record.reference_surface_landmarks_tenths_mm
  if (landmarks !== undefined && (!Array.isArray(landmarks) || landmarks.length < 1 || landmarks.length > 256
    || landmarks.some((point) => !isBoundedIntegerTuple(point, 3, 2_147_483_648)))) return null
  const outlineAuthority = record.outline_edit_authority === undefined ? null
    : exactCoreDataRecord(record.outline_edit_authority, [
        'schema_version', 'source_asset_id', 'source_sha256', 'edits',
      ] as const)
  if (record.outline_edit_authority !== undefined && (!outlineAuthority
    || outlineAuthority.schema_version !== 1 || !isCanonicalNonNilUuid(outlineAuthority.source_asset_id)
    || !isBoundedIntegerTuple(outlineAuthority.source_sha256, 32, 255)
    || !Array.isArray(outlineAuthority.edits) || outlineAuthority.edits.length < 1
    || outlineAuthority.edits.length > 8)) return null
  const outlineEdits = outlineAuthority === null ? [] : (outlineAuthority.edits as unknown[]).map((edit) => {
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
    if (!data || (kind === 'split_vertical' && (!validCandidateId(data.source_candidate_id)
      || !Number.isSafeInteger(data.split_x) || Number(data.split_x) < 0
      || Number(data.split_x) > 4_294_967_295 || !Array.isArray(data.fragment_kinds)
      || data.fragment_kinds.length !== 2 || !data.fragment_kinds.every(validPartKind)
      || data.fragment_kinds[0] === data.fragment_kinds[1]))
      || (kind === 'merge' && (!Array.isArray(data.source_candidate_ids)
        || data.source_candidate_ids.length !== 2
        || !validCandidateId(data.source_candidate_ids[0])
        || !validCandidateId(data.source_candidate_ids[1])
        || Number(data.source_candidate_ids[0]) >= Number(data.source_candidate_ids[1])
        || !validPartKind(data.merged_kind)))) return null
    return Object.freeze({ ...record })
  })
  if (outlineEdits.some((edit) => edit === null)) return null
  const archivedAssets = record.archived_reference_model_asset_ids === undefined ? []
    : record.archived_reference_model_asset_ids
  if (!Array.isArray(archivedAssets) || archivedAssets.length > 8
    || archivedAssets.some((id) => !isCanonicalNonNilUuid(id))
    || new Set(archivedAssets).size !== archivedAssets.length) return null
  const consensus = record.reference_consensus_v1 === undefined ? null
    : exactCoreDataRecord(record.reference_consensus_v1, ['schema_version', 'bindings', 'excluded_asset_id'] as const)
  const consensusBindings = consensus?.bindings
  if (record.reference_consensus_v1 !== undefined && (!consensus || consensus.schema_version !== 1
    || !Array.isArray(consensusBindings) || consensusBindings.length < 2 || consensusBindings.length > 4)) return null
  const normalizedConsensusBindings = consensus === null ? [] : (consensusBindings as unknown[]).map((raw) => {
    const binding = exactCoreDataRecord(raw, ['kind', 'asset_id', 'sha256', 'quality'] as const)
    if (!binding || !['image', 'reference_model'].includes(String(binding.kind))
      || !isCanonicalNonNilUuid(binding.asset_id) || !isBoundedIntegerTuple(binding.sha256, 32, 255)
      || !Number.isInteger(binding.quality) || Number(binding.quality) < 0 || Number(binding.quality) > 100) return null
    return Object.freeze({ kind: binding.kind as 'image' | 'reference_model', asset_id: String(binding.asset_id),
      sha256: Object.freeze((binding.sha256 as number[]).slice()), quality: Number(binding.quality) })
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
    ...(landmarks === undefined ? {} : { reference_surface_landmarks_tenths_mm: Object.freeze(
      landmarks.map((point) => Object.freeze((point as number[]).slice()) as readonly [number, number, number]),
    ) }),
    ...(outlineAuthority === null ? {} : { outline_edit_authority: Object.freeze({
      schema_version: 1 as const,
      source_asset_id: String(outlineAuthority.source_asset_id),
      source_sha256: Object.freeze((outlineAuthority.source_sha256 as number[]).slice()),
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
      topology_authority_sha256: Object.freeze(
        (provenance.topology_authority_sha256 as unknown[]).map(Number),
      ) as ReadonlyArray<number>,
      ...(provenance.fold_path_certificate_sha256 === undefined ? {} : {
        fold_path_certificate_sha256: Object.freeze(
          (provenance.fold_path_certificate_sha256 as unknown[]).map(Number),
        ) as ReadonlyArray<number>,
      }),
      confidence_score: Number(provenance.confidence_score),
      confidence_reasons: Object.freeze((provenance.confidence_reasons as string[]).slice()),
      explicit_override: provenance.explicit_override as boolean,
      source_asset_fingerprint: provenance.source_asset_fingerprint as string,
      ...(provenanceConsensus === null ? {} : { reference_consensus: Object.freeze({
        schema_version: 1 as const, source_revision: Number(provenanceConsensus.source_revision),
        bindings: Object.freeze((provenanceConsensus.bindings as Array<Record<string, unknown>>).map((binding) => Object.freeze({ ...binding }))) as NonNullable<BeginnerDesignProfileV1['reference_consensus_v1']>['bindings'],
        ...(provenanceConsensus.excluded_asset_id === undefined ? {} : { excluded_asset_id: String(provenanceConsensus.excluded_asset_id) }),
        pair_digests_sha256: Object.freeze((provenanceConsensus.pair_digests_sha256 as number[][]).map((digest) => Object.freeze(digest.slice()))),
      }) }),
      ...(genericTree === null ? {} : { generic_tree: Object.freeze({
        schema_version: 1 as const,
        ...(genericTree.target_category === undefined ? {} : { target_category: 'custom_object' as const }),
        source: genericTree.source as 'image_silhouette' | 'glb_geometry' | 'manual_skeleton',
        ...(genericTree.asset_content_sha256 === undefined ? {} : {
          asset_content_sha256: Object.freeze((genericTree.asset_content_sha256 as number[]).slice()),
        }),
        tree_topology_sha256: Object.freeze((genericTree.tree_topology_sha256 as number[]).slice()),
        normalized_length_ratios: Object.freeze((genericTree.normalized_length_ratios as number[]).slice()),
        orientation: genericTree.orientation as 'horizontal' | 'vertical', generator_version: 1 as const,
        authorizes_apply: false as const,
        ...(treeProposal === null ? {} : { instruction_proposal: Object.freeze({
          schema_version: 1 as const, topology_sha256: Object.freeze((treeProposal.topology_sha256 as number[]).slice()),
          generator_version: 1 as const, authorizes_apply: false as const, physical_motion_proof: false as const,
          steps: Object.freeze((treeProposal.steps as Array<Record<string, unknown>>).map((step) => Object.freeze({
            canonical_crease_id: step.canonical_crease_id as string, tree_depth: step.tree_depth as number,
            assignment: step.assignment as 'mountain' | 'valley', target_branch: step.target_branch as string,
            fixed_side: step.fixed_side as 'root' | 'leaf', caution: step.caution as string,
          }))),
        }) }),
      }) }),
    }) }),
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
    && JSON.stringify(profile.generation_provenance) === JSON.stringify(expected.generation_provenance)
    && JSON.stringify(profile.reference_surface_landmarks_tenths_mm)
      === JSON.stringify(expected.reference_surface_landmarks_tenths_mm)
    && JSON.stringify(profile.outline_edit_authority)
      === JSON.stringify(expected.outline_edit_authority)
    && JSON.stringify(profile.archived_reference_model_asset_ids ?? [])
      === JSON.stringify(expected.archived_reference_model_asset_ids ?? [])
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
    || record.project_instance_id !== expectedProjectInstanceId
    || record.project_id !== expectedProjectId
    || record.revision !== expectedRevision
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
  if (!response || response.project_instance_id !== expectedProjectInstanceId
    || response.project_id !== expectedProjectId || response.revision !== expectedRevision
    || !isBoundedIntegerTuple(response.source_asset_sha256, 32, 255)
    || !suggestion || !isCanonicalNonNilUuid(suggestion.asset_id)
    || suggestion.method !== 'bounded_bbox_area_normal_v1'
    || ![null, 'wing', 'fin', 'ear', 'horn', 'antenna', 'leg', 'tail'].includes(suggestion.suggested_part_kind as null | string)
    || !isBoundedIntegerTuple(suggestion.bbox_min_tenths_mm, 3, 2_147_483_648)
    || !isBoundedIntegerTuple(suggestion.bbox_max_tenths_mm, 3, 2_147_483_647)
    || !isBoundedIntegerTuple(suggestion.dominant_normal_milli, 3, 1000)
    || !Number.isSafeInteger(suggestion.surface_area_milli)
    || Number(suggestion.surface_area_milli) < 0
    || !Array.isArray(suggestion.surface_landmarks_tenths_mm)
    || suggestion.surface_landmarks_tenths_mm.length < 1
    || suggestion.surface_landmarks_tenths_mm.length > 256
    || suggestion.surface_landmarks_tenths_mm.some((point) => !isBoundedIntegerTuple(point, 3, 2_147_483_648))) {
    throw new Error('invalid reference model suggestion')
  }
  if (!Array.isArray(suggestion.surface_ranges) || suggestion.surface_ranges.length < 1
    || suggestion.surface_ranges.length > 8) throw new Error('invalid reference model suggestion')
  const surfaceRanges = suggestion.surface_ranges.map((value, index) => {
    const range = exactCoreDataRecord(value, [
      'id', 'triangle_indices', 'range_min_tenths_mm', 'range_max_tenths_mm', 'digest_sha256',
    ] as const)
    if (!range || range.id !== index + 1 || !Array.isArray(range.triangle_indices)
      || range.triangle_indices.length < 1 || range.triangle_indices.length > 40_000
      || range.triangle_indices.some((triangle) => !Number.isInteger(triangle) || triangle < 0)
      || !isBoundedIntegerTuple(range.range_min_tenths_mm, 3, 2_147_483_648)
      || !isBoundedIntegerTuple(range.range_max_tenths_mm, 3, 2_147_483_647)
      || !isBoundedIntegerTuple(range.digest_sha256, 32, 255)
      || range.digest_sha256.some((byte) => byte < 0)) throw new Error('invalid reference model suggestion')
    return Object.freeze({ ...range,
      triangle_indices: Object.freeze(range.triangle_indices.slice()),
      digest_sha256: Object.freeze(range.digest_sha256.slice()),
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
  const stickBars = Array.isArray(suggestion.stick_bars) ? suggestion.stick_bars.map((value, index) => {
    const bar = exactCoreDataRecord(value, ['id', 'start_tenths_mm', 'end_tenths_mm', 'thickness_tenths_mm'] as const)
    if (!bar || bar.id !== index || !isBoundedIntegerTuple(bar.start_tenths_mm, 3, 2_147_483_648)
      || !isBoundedIntegerTuple(bar.end_tenths_mm, 3, 2_147_483_648)
      || !Number.isInteger(bar.thickness_tenths_mm) || Number(bar.thickness_tenths_mm) < 1
      || Number(bar.thickness_tenths_mm) > 65_535) return null
    return Object.freeze({ ...bar })
  }) : []
  const protrusions = constraints?.protrusions ?? []
  const bilateralProtrusions = protrusions.filter((target) => target.symmetry === 'bilateral')
  // Native may generalize an explicitly authored generic target to at most
  // eight bounded features. Geometry supplies measurements only; semantic
  // kinds remain the user's current target_parts and apply still requires
  // exact live-suggestion revalidation plus confirmation.
  if (!constraints || !generalConstraints || generalProtrusions.length < 1
    || generalProtrusions.length > 32 || stickBars.length !== 3 || stickBars.some((bar) => !bar)
    || !isBoundedIntegerTuple(suggestion.principal_axis_extents_tenths_mm, 3, 2_147_483_647)
    || suggestion.principal_axis_extents_tenths_mm.some((extent) => extent < 1)
    || !Number.isInteger(suggestion.quality_score) || Number(suggestion.quality_score) < 0 || Number(suggestion.quality_score) > 100
    || !Array.isArray(suggestion.quality_reasons) || suggestion.quality_reasons.length < 1 || suggestion.quality_reasons.length > 8
    || suggestion.quality_reasons.some((reason) => !['strict_glb_vertex_index_bounds', 'deterministic_aabb_principal_axes'].includes(String(reason)))
    || !Number.isInteger(suggestion.component_count) || Number(suggestion.component_count) < 1 || Number(suggestion.component_count) > 8
    || typeof suggestion.inferred_component_bridges !== 'boolean'
    || (suggestion.inferred_component_bridges !== (Number(suggestion.component_count) > 1))
    || !Array.isArray(suggestion.insufficiency_reasons) || suggestion.insufficiency_reasons.length > 8
    || suggestion.insufficiency_reasons.some((reason) => !['insufficient_distinct_vertices', 'protrusion_candidate_limit_reached', 'component_bridges_are_estimated'].includes(String(reason)))
    || protrusions.length < 1 || protrusions.length > 8
    || !Array.isArray(suggestion.pair_bindings)
    || suggestion.pair_bindings.length !== bilateralProtrusions.length
    || suggestion.pair_bindings.some((binding, index) => {
      const record = exactCoreDataRecord(binding, ['pair_index', 'protrusion_id', 'center_y_tenths_mm'] as const)
      return !record || record.pair_index !== index
        || record.protrusion_id !== bilateralProtrusions[index]?.id
        || record.center_y_tenths_mm !== bilateralProtrusions[index]?.position_tenths_mm[1]
    })) {
    throw new Error('invalid reference model suggestion')
  }
  return Object.freeze({ ...suggestion, source_asset_sha256: Object.freeze(response.source_asset_sha256.slice()),
    surface_ranges: Object.freeze(surfaceRanges),
    surface_landmarks_tenths_mm: Object.freeze(suggestion.surface_landmarks_tenths_mm.map(
      (point) => Object.freeze((point as number[]).slice()) as unknown as readonly [number, number, number],
    )),
    ...(constraints.generic_body_outline_tenths_mm === undefined ? {} : {
      generic_body_outline_tenths_mm: constraints.generic_body_outline_tenths_mm,
      generic_body_outline_mode: constraints.generic_body_outline_mode,
    }),
    protrusions: Object.freeze(protrusions.slice()),
    general_protrusion_candidates: Object.freeze(generalProtrusions.slice()),
    stick_bars: Object.freeze(stickBars as NonNullable<(typeof stickBars)[number]>[]),
    component_count: Number(suggestion.component_count),
    inferred_component_bridges: suggestion.inferred_component_bridges as boolean,
    pair_bindings: Object.freeze(suggestion.pair_bindings.slice()) }) as BeginnerReferenceModelSuggestionV1
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
  if (surfaceAssignments.length < 2 || surfaceAssignments.length > 8
    || new Set(surfaceAssignments.map((item) => item.range_id)).size !== surfaceAssignments.length
    || new Set(surfaceAssignments.map((item) => item.protrusion_id)).size !== surfaceAssignments.length
    || surfaceAssignments.some((item) => !Number.isInteger(item.range_id)
      || item.range_id < 1 || item.range_id > 8 || !Number.isInteger(item.protrusion_id)
      || item.protrusion_id < 1 || item.protrusion_id > 65_535)) {
    return Promise.reject(new Error('invalid reference model surface selection'))
  }
  if (surfaceEdits.length !== surfaceAssignments.length
    || new Set(surfaceEdits.map((item) => item.range_id)).size !== surfaceEdits.length
    || surfaceEdits.some((item) => item.base_digest_sha256.length !== 32
      || item.base_digest_sha256.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
      || item.triangle_indices.length < 1 || item.triangle_indices.length > 40_000
      || new Set(item.triangle_indices).size !== item.triangle_indices.length
      || item.triangle_indices.some((triangle) => !Number.isInteger(triangle) || triangle < 0)
      || !isBoundedIntegerTuple(item.bulge_direction_milli, 3, 1_000)
      || item.bulge_direction_milli.every((axis) => axis === 0)
      || !Number.isInteger(item.bulge_amount_tenths_mm)
      || item.bulge_amount_tenths_mm < 1 || item.bulge_amount_tenths_mm > 1_000_000)) {
    return Promise.reject(new Error('invalid reference model surface edit'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_reference_model_features', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    expectedSuggestion, surfaceAssignments: surfaceAssignments.map((item) => ({ ...item })),
    surfaceEdits: surfaceEdits.map((item) => ({ ...item,
      base_digest_sha256: [...item.base_digest_sha256], triangle_indices: [...item.triangle_indices],
      bulge_direction_milli: [...item.bulge_direction_milli],
    })), confirmed: true,
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
    protrusion_id: number, generated_feature_id: number, endpoint_count: 1 | 2 | 4,
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
): Promise<BeginnerGridEvaluationResponse> {
  const value = await invoke<unknown>('evaluate_beginner_parameter_grid', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision, requestGenerationId,
  })
  const response = exactCoreDataRecord(value, [
    'request_generation_id', 'project_instance_id', 'project_id', 'revision', 'grid_hash',
    'evaluated_grid_points', 'global_checked_candidates', 'refinement_iterations', 'candidates',
  ] as const)
  if (!response || response.request_generation_id !== requestGenerationId
    || response.project_instance_id !== expectedProjectInstanceId
    || response.project_id !== expectedProjectId || response.revision !== expectedRevision
    || response.evaluated_grid_points !== 27 || response.global_checked_candidates !== 3
    || !Number.isInteger(response.refinement_iterations) || Number(response.refinement_iterations) < 0
    || Number(response.refinement_iterations) > 24
    || !Array.isArray(response.grid_hash)
    || response.grid_hash.length !== 32
    || response.grid_hash.some((byte) => !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255)
    || !Array.isArray(response.candidates) || response.candidates.length < 1 || response.candidates.length > 3) {
    throw new Error('invalid beginner parameter grid response')
  }
  const rawCandidates = response.candidates.map((value) => exactCoreDataRecord(
    value, ['point', 'primary_score', 'plan', 'assessment', 'local_proof_scope',
      'global_proof_scope', 'complexity_score', 'scale_deviation_penalty',
      'paper_efficiency_score',
      'spacing_deviation_penalty', 'detail_mismatch_penalty', 'outcome_reason', 'contour_witness',
      'refinement_iterations', 'strict_improvements', 'refinement_starts'] as const,
  ))
  if (rawCandidates.some((candidate) => candidate === null)) {
    throw new Error('invalid beginner parameter grid response')
  }
  const admitted = rawCandidates as NonNullable<(typeof rawCandidates)[number]>[]
  const normalizedPlans = normalizeBeginnerCandidateResponse({
    schema_version: 1,
    project_instance_id: expectedProjectInstanceId,
    project_id: expectedProjectId,
    revision: expectedRevision,
    requested_candidate_count: 3,
    bulge_treatment: 'target_shape_approximation',
    elasticity_model: 'not_computed',
    generation_status: 'ready',
    generated_plans: admitted.map((candidate) => candidate.plan),
    plan_assessments: admitted.map((candidate) => candidate.assessment),
    candidates: [0, 1, 2].map((index) => ({
      schema_version: 1, kind: ['recommended', 'shape_focused', 'foldability_focused'][index],
      rank: index + 1, total_score: 100 - index, shape_score: 100 - index,
      target_approximation_score: 100 - index, foldability_score: 100 - index,
      step_count_score: 100 - index, paper_efficiency_score: 100 - index,
    })),
  }, expectedProjectInstanceId, expectedProjectId, expectedRevision, 3)
  if (!normalizedPlans) throw new Error('invalid beginner parameter grid response')
  const candidates = admitted.map((candidate, index) => {
    const point = exactCoreDataRecord(candidate.point, [
      'id', 'scale_percent', 'spacing_percent', 'detail_level',
    ] as const)
    const witness = exactCoreDataRecord(candidate.contour_witness, [
      'body_contour_points', 'local_bindings', 'generic_feature_bindings', 'skeleton_branch_bindings',
      'skeleton_tree_authority_sha256', 'witnessed_vertices', 'witnessed_creases', 'topology_authority_hash',
      'max_contour_error_millionths',
    ] as const)
    const bindings = witness && Array.isArray(witness.local_bindings)
      ? witness.local_bindings.map((binding) => exactCoreDataRecord(binding, [
          'protrusion_id', 'contour_points', 'generated_face_id', 'vertex_start', 'crease_start',
        ] as const))
      : []
    const featureBindings = witness && Array.isArray(witness.generic_feature_bindings)
      ? witness.generic_feature_bindings.map((binding) => exactCoreDataRecord(binding, [
          'protrusion_id', 'generated_feature_id', 'endpoint_count', 'crease_start',
          'crease_authority_sha256', 'skeleton_segment_id', 'skeleton_endpoint',
          'mount_distance_squared_tenths_mm',
        ] as const))
      : []
    const witnessPointCount = witness && bindings.every((binding) => binding !== null)
      ? Number(witness.body_contour_points) + bindings.reduce((sum, binding) => sum + Number(binding?.contour_points), 0)
      : -1
    const branchBindings = witness && Array.isArray(witness.skeleton_branch_bindings)
      ? witness.skeleton_branch_bindings.map((branch) => exactCoreDataRecord(branch, [
          'segment_id', 'parent_segment_id', 'parent_endpoint', 'child_endpoint',
          'generated_feature_ids',
        ] as const)) : []
    if (!point || !witness || !Number.isInteger(point.id) || Number(point.id) < 0 || Number(point.id) > 26
      || !Number.isInteger(point.scale_percent) || Number(point.scale_percent) < 10 || Number(point.scale_percent) > 45
      || !Number.isInteger(point.spacing_percent) || Number(point.spacing_percent) < 20 || Number(point.spacing_percent) > 80
      || !['simple', 'standard', 'detailed'].includes(String(point.detail_level))
      || !Number.isInteger(candidate.primary_score) || Number(candidate.primary_score) < 0
      || Number(candidate.primary_score) > 1000
      || candidate.local_proof_scope !== 'necessary'
      || candidate.global_proof_scope !== normalizedPlans.plan_assessments[index].proof_scope
      || candidate.outcome_reason !== normalizedPlans.plan_assessments[index].reason
      || !Number.isInteger(witness.body_contour_points) || Number(witness.body_contour_points) < 0 || Number(witness.body_contour_points) > 16
      || bindings.length !== (witness.local_bindings as unknown[]).length || bindings.length > 8
      || featureBindings.length !== (witness.generic_feature_bindings as unknown[]).length
      || featureBindings.length > 8
      || branchBindings.length !== (witness.skeleton_branch_bindings as unknown[]).length
      || branchBindings.length > 32
      || (normalizedPlans.generated_plans[index].kind === 'composite_generic_target_base'
        ? featureBindings.length < 2
        : featureBindings.length !== 0)
      || bindings.some((binding, bindingIndex) => !binding || !Number.isInteger(binding.protrusion_id)
        || Number(binding.protrusion_id) < 1 || Number(binding.protrusion_id) > 65535
        || !Number.isInteger(binding.contour_points) || Number(binding.contour_points) < 3 || Number(binding.contour_points) > 8
        || binding.generated_face_id !== bindingIndex + 1
        || !Number.isInteger(binding.vertex_start) || Number(binding.vertex_start) < 0
        || !Number.isInteger(binding.crease_start) || Number(binding.crease_start) < 0
        || (bindingIndex > 0 && (Number(binding.vertex_start) !== Number(bindings[bindingIndex - 1]?.vertex_start)
          + Number(bindings[bindingIndex - 1]?.contour_points)
          || Number(binding.crease_start) !== Number(bindings[bindingIndex - 1]?.crease_start)
            + Number(bindings[bindingIndex - 1]?.contour_points)))
        || (bindingIndex > 0 && Number(bindings[bindingIndex - 1]?.protrusion_id) >= Number(binding.protrusion_id)))
      || featureBindings.some((binding, bindingIndex) => !binding
        || !Number.isInteger(binding.protrusion_id) || Number(binding.protrusion_id) < 1
        || Number(binding.protrusion_id) > 65535
        || binding.generated_feature_id !== binding.protrusion_id
        || ![1, 2, 4].includes(Number(binding.endpoint_count))
        || !Number.isInteger(binding.crease_start) || Number(binding.crease_start) < 0
        || !isBoundedIntegerTuple(binding.crease_authority_sha256, 32, 255)
        || binding.crease_authority_sha256.some((byte) => byte < 0)
        || !Number.isInteger(binding.skeleton_segment_id) || Number(binding.skeleton_segment_id) < 1
        || Number(binding.skeleton_segment_id) > 65535
        || !['start', 'end'].includes(String(binding.skeleton_endpoint))
        || !Number.isSafeInteger(binding.mount_distance_squared_tenths_mm)
        || Number(binding.mount_distance_squared_tenths_mm) < 0
        || Number(binding.crease_start) + Number(binding.endpoint_count)
          > normalizedPlans.generated_plans[index].crease_pattern.edges.length
        || (bindingIndex > 0 && Number(featureBindings[bindingIndex - 1]?.protrusion_id)
          >= Number(binding.protrusion_id)))
      || featureBindings.some((binding, index) => featureBindings.some((other, otherIndex) =>
        index !== otherIndex && binding && other
          && Number(binding.crease_start) < Number(other.crease_start) + Number(other.endpoint_count)
          && Number(other.crease_start) < Number(binding.crease_start) + Number(binding.endpoint_count)))
      || branchBindings.some((branch, index) => !branch
        || !Number.isInteger(branch.segment_id) || Number(branch.segment_id) < 1
        || (index === 0 ? branch.parent_segment_id !== null
          : !Number.isInteger(branch.parent_segment_id) || Number(branch.parent_segment_id) < 1)
        || (index === 0 ? branch.parent_endpoint !== null || branch.child_endpoint !== null
          : !['start', 'end'].includes(String(branch.parent_endpoint))
            || !['start', 'end'].includes(String(branch.child_endpoint))
            || !branchBindings.slice(0, index).some(
              (parent) => parent?.segment_id === branch.parent_segment_id))
        || branchBindings.slice(0, index).some(
          (previous) => previous?.segment_id === branch.segment_id)
        || !Array.isArray(branch.generated_feature_ids)
        || new Set(branch.generated_feature_ids).size !== branch.generated_feature_ids.length
        || branch.generated_feature_ids.some((id) => !featureBindings.some(
          (binding) => binding?.generated_feature_id === id)))
      || !isBoundedIntegerTuple(witness.skeleton_tree_authority_sha256, 32, 255)
      || !Number.isInteger(witness.witnessed_vertices) || Number(witness.witnessed_vertices) !== witnessPointCount
      || !Number.isInteger(witness.witnessed_creases) || Number(witness.witnessed_creases) !== witnessPointCount
      || !Array.isArray(witness.topology_authority_hash) || witness.topology_authority_hash.length !== 32
      || witness.topology_authority_hash.some((byte) => !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255)
      || !Number.isInteger(witness.max_contour_error_millionths)
      || Number(witness.max_contour_error_millionths) < 0 || Number(witness.max_contour_error_millionths) > 1
      || normalizedPlans.generated_plans[index].crease_pattern.vertices.length < witnessPointCount
      || normalizedPlans.generated_plans[index].crease_pattern.edges.length < witnessPointCount
      || bindings.some((binding) => Number(binding?.vertex_start) + Number(binding?.contour_points)
        > normalizedPlans.generated_plans[index].crease_pattern.vertices.length
        || Number(binding?.crease_start) + Number(binding?.contour_points)
          > normalizedPlans.generated_plans[index].crease_pattern.edges.length)
      || !Number.isInteger(candidate.complexity_score) || Number(candidate.complexity_score) < 0 || Number(candidate.complexity_score) > 100
      || !Number.isInteger(candidate.paper_efficiency_score)
      || Number(candidate.paper_efficiency_score) < 0 || Number(candidate.paper_efficiency_score) > 100
      || !Number.isInteger(candidate.refinement_iterations) || Number(candidate.refinement_iterations) < 0 || Number(candidate.refinement_iterations) > 8
      || !Number.isInteger(candidate.strict_improvements) || Number(candidate.strict_improvements) < 0
      || Number(candidate.strict_improvements) > Number(candidate.refinement_iterations) + 1
      || !Number.isInteger(candidate.refinement_starts) || Number(candidate.refinement_starts) < 1
      || Number(candidate.refinement_starts) > 5
      || ![candidate.scale_deviation_penalty, candidate.spacing_deviation_penalty, candidate.detail_mismatch_penalty]
        .every((penalty) => Number.isInteger(penalty) && Number(penalty) >= 0 && Number(penalty) <= 1000)
      || Number(candidate.primary_score) !== 1000 - Number(candidate.scale_deviation_penalty)
        - Number(candidate.spacing_deviation_penalty) - Number(candidate.detail_mismatch_penalty)
      || (index > 0 && (Number(admitted[index - 1].primary_score) < Number(candidate.primary_score)
        || (Number(admitted[index - 1].primary_score) === Number(candidate.primary_score)
          && Number((exactCoreDataRecord(admitted[index - 1].point, ['id', 'scale_percent', 'spacing_percent', 'detail_level'] as const))?.id) >= Number(point.id))))) {
      throw new Error('invalid beginner parameter grid response')
    }
    return Object.freeze({ point: Object.freeze(point) as BeginnerParameterGridPointV1,
      primary_score: Number(candidate.primary_score), plan: normalizedPlans.generated_plans[index],
      assessment: normalizedPlans.plan_assessments[index], local_proof_scope: 'necessary' as const,
      global_proof_scope: candidate.global_proof_scope as BeginnerGeneratedPlanAssessmentV1['proof_scope'],
      complexity_score: Number(candidate.complexity_score),
      paper_efficiency_score: Number(candidate.paper_efficiency_score),
      scale_deviation_penalty: Number(candidate.scale_deviation_penalty),
      spacing_deviation_penalty: Number(candidate.spacing_deviation_penalty),
      detail_mismatch_penalty: Number(candidate.detail_mismatch_penalty),
      outcome_reason: candidate.outcome_reason as BeginnerGeneratedPlanAssessmentV1['reason'],
      refinement_iterations: Number(candidate.refinement_iterations),
      strict_improvements: Number(candidate.strict_improvements),
      refinement_starts: Number(candidate.refinement_starts),
      contour_witness: Object.freeze({
        body_contour_points: Number(witness.body_contour_points),
        local_bindings: Object.freeze(bindings.map((binding) => Object.freeze({
          protrusion_id: Number(binding?.protrusion_id), contour_points: Number(binding?.contour_points),
          generated_face_id: Number(binding?.generated_face_id),
          vertex_start: Number(binding?.vertex_start), crease_start: Number(binding?.crease_start),
        }))),
        generic_feature_bindings: Object.freeze(featureBindings.map((binding) => Object.freeze({
          protrusion_id: Number(binding?.protrusion_id),
          generated_feature_id: Number(binding?.generated_feature_id),
          crease_authority_sha256: Object.freeze(
            (binding?.crease_authority_sha256 as number[]).slice()),
          endpoint_count: Number(binding?.endpoint_count) as 1 | 2 | 4,
          crease_start: Number(binding?.crease_start),
          skeleton_segment_id: Number(binding?.skeleton_segment_id),
          skeleton_endpoint: String(binding?.skeleton_endpoint) as 'start' | 'end',
          mount_distance_squared_tenths_mm: Number(binding?.mount_distance_squared_tenths_mm),
        }))),
        skeleton_branch_bindings: Object.freeze(branchBindings.map((branch) => Object.freeze({
          segment_id: Number(branch?.segment_id),
          parent_segment_id: branch?.parent_segment_id === null ? null : Number(branch?.parent_segment_id),
          parent_endpoint: branch?.parent_endpoint as 'start' | 'end' | null,
          child_endpoint: branch?.child_endpoint as 'start' | 'end' | null,
          generated_feature_ids: Object.freeze((branch?.generated_feature_ids as number[]).slice()),
        }))),
        skeleton_tree_authority_sha256: Object.freeze(
          witness.skeleton_tree_authority_sha256.slice()) as ReadonlyArray<number>,
        witnessed_vertices: Number(witness.witnessed_vertices),
        witnessed_creases: Number(witness.witnessed_creases),
        topology_authority_hash: Object.freeze(witness.topology_authority_hash.slice()) as ReadonlyArray<number>,
        max_contour_error_millionths: Number(witness.max_contour_error_millionths),
      }) })
  })
  if (new Set(candidates.map((candidate) => candidate.point.id)).size !== candidates.length) {
    throw new Error('invalid beginner parameter grid response')
  }
  return Object.freeze({ request_generation_id: requestGenerationId,
    project_instance_id: expectedProjectInstanceId, project_id: expectedProjectId,
    revision: expectedRevision, grid_hash: Object.freeze(response.grid_hash.slice()) as ReadonlyArray<number>,
    evaluated_grid_points: 27, global_checked_candidates: 3,
    refinement_iterations: Number(response.refinement_iterations), candidates: Object.freeze(candidates) })
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
    || !grid.candidates.includes(candidate)
    || candidate.assessment.proof_scope !== 'sufficient'
    || candidate.assessment.reason !== 'global_flat_foldability_proven'
    || !candidate.assessment.apply_allowed) {
    return Promise.reject(new Error('grid candidate lacks a live sufficient proof'))
  }
  return invoke<ProjectSnapshot>('apply_beginner_parameter_grid_candidate', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    requestGenerationId: grid.request_generation_id,
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
  estimate: Readonly<{ protrusion_count: 1 | 2 | 3 | 4 | 6 | 10; scale_percent: number; spacing_percent: number }>
  candidates: ReadonlyArray<Readonly<{ id: number; scale_percent: number; spacing_percent: number
    approximation_score: number; complexity_score: number; required_protrusion_count: 1 | 2 | 3 | 4 | 6 | 10 }>>
}>

export async function getBeginnerSymmetricParameterEstimate(
  projectId: string, revision: number, projectInstanceId: string,
): Promise<BeginnerSymmetricParameterEstimateResponse> {
  const value = await invoke<unknown>('get_beginner_symmetric_parameter_estimate', {
    expectedProjectInstanceId: projectInstanceId, expectedProjectId: projectId, expectedRevision: revision,
  })
  const record = exactCoreDataRecord(value, ['project_instance_id', 'project_id', 'revision', 'estimate', 'candidates'] as const)
  const estimate = exactCoreDataRecord(record?.estimate, ['protrusion_count', 'scale_percent', 'spacing_percent'] as const)
  if (!record || record.project_instance_id !== projectInstanceId || record.project_id !== projectId
    || record.revision !== revision || !estimate || ![1, 2, 3, 4, 6, 10].includes(Number(estimate.protrusion_count))
    || !Number.isInteger(estimate.scale_percent) || Number(estimate.scale_percent) < 10 || Number(estimate.scale_percent) > 45
    || !Number.isInteger(estimate.spacing_percent) || Number(estimate.spacing_percent) < 20 || Number(estimate.spacing_percent) > 80
    || !Array.isArray(record.candidates) || record.candidates.length !== 3) {
    throw new Error('invalid symmetric parameter estimate')
  }
  const candidates = record.candidates.map((value, index) => {
    const item = exactCoreDataRecord(value, ['id', 'scale_percent', 'spacing_percent', 'approximation_score', 'complexity_score', 'required_protrusion_count'] as const)
    if (!item || item.id !== index || ![1, 2, 3, 4, 6, 10].includes(Number(item.required_protrusion_count))
      || !Number.isInteger(item.scale_percent) || Number(item.scale_percent) < 10 || Number(item.scale_percent) > 45
      || !Number.isInteger(item.spacing_percent) || Number(item.spacing_percent) < 20 || Number(item.spacing_percent) > 80
      || !Number.isInteger(item.approximation_score) || Number(item.approximation_score) < 0 || Number(item.approximation_score) > 100
      || !Number.isInteger(item.complexity_score) || Number(item.complexity_score) < 0 || Number(item.complexity_score) > 100) throw new Error('invalid symmetric parameter candidates')
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
  if (!record || record.project_instance_id !== expectedProjectInstanceId
    || record.project_id !== expectedProjectId || record.revision !== expectedRevision
    || record.underlay_id !== underlayId || record.asset_id !== assetId
    || !isBoundedIntegerTuple(record.source_sha256, 32, 255)
    || record.source_sha256.some((byte) => byte < 0)
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
    source_sha256: Object.freeze(record.source_sha256.slice()),
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
  if (!record || record.project_instance_id !== proposal.project_instance_id
    || record.project_id !== proposal.project_id || record.revision !== proposal.revision
    || record.underlay_id !== proposal.underlay_id || record.asset_id !== proposal.asset_id
    || record.selected_outline_id !== candidate.id || !Array.isArray(record.suggestions)
    || record.suggestions.length < 2 || record.suggestions.length > 8) throw new BeginnerRecognitionError('native_failure')
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
  if (assignments.some((assignment) => assignment.source_candidate_ids
    && (assignment.source_candidate_ids.length < 1 || assignment.source_candidate_ids.length > 2
      || new Set(assignment.source_candidate_ids).size !== assignment.source_candidate_ids.length
      || assignment.source_candidate_ids.some((id) => !Number.isInteger(id) || id < 0 || id > 15)))
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
    || !Number.isSafeInteger(request.maxPairTests) || request.maxPairTests < 0) {
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
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  targetAngles: readonly Readonly<{ edge: string; angleDegrees: number }>[]
  maxStates: number
  maxTransitions: number
  levelCount: 3 | 5 | 9
  cycleScheduleV1?: CycleScheduleRequestV1
}>): Promise<DyadicPoseGraphReadResponseV1> {
  if (!isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !Number.isSafeInteger(request.maxStates) || request.maxStates < 1 || request.maxStates > 2187
    || !Number.isSafeInteger(request.maxTransitions) || request.maxTransitions < 1 || request.maxTransitions > 20412
    || ![3, 5, 9].includes(request.levelCount)
    || !Array.isArray(request.targetAngles) || request.targetAngles.length === 0 || request.targetAngles.length > 64
    || request.targetAngles.some((entry) => !isCanonicalNonNilUuid(entry.edge)
      || !Number.isFinite(entry.angleDegrees) || entry.angleDegrees < 0 || entry.angleDegrees > 180)) {
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
  if (!isCanonicalNonNilUuid(request.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(request.expectedProjectId)
    || !Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 0
    || !Number.isSafeInteger(request.maxStates) || request.maxStates < 1 || request.maxStates > 2187
    || !Number.isSafeInteger(request.maxTransitions) || request.maxTransitions < 1 || request.maxTransitions > 20412
    || ![3, 5, 9].includes(request.levelCount)
    || !Array.isArray(request.targetAngles) || request.targetAngles.length === 0 || request.targetAngles.length > 64
    || request.targetAngles.some((entry) => !isCanonicalNonNilUuid(entry.edge) || !Number.isFinite(entry.angleDegrees) || entry.angleDegrees < 0 || entry.angleDegrees > 180)
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
  if (!isCanonicalNonNilUuid(previewToken)) return Promise.reject(new Error('invalid dyadic preview token'))
  return invoke<void>('cancel_dyadic_pose_path_preview_v1', { request: { previewToken } })
}

export function proposeCurrentCyclePoseV1(
  request: CurrentCyclePosePreviewRequestV1,
): Promise<CurrentCyclePosePreviewResponseV1> {
  const keys = Object.keys(request).sort().join(',')
  if (
    keys !== 'cycleScheduleV1,expectedProjectId,expectedProjectInstanceId,expectedRevision' &&
    keys !== 'cycleScheduleV1,expectedProjectId,expectedProjectInstanceId,expectedRevision,progressRequestId'
  ) return Promise.reject(new Error('invalid current-cycle preview request'))
  if (
    !isCanonicalNonNilUuid(request.expectedProjectInstanceId) ||
    !isCanonicalNonNilUuid(request.expectedProjectId) ||
    !Number.isSafeInteger(request.expectedRevision) ||
    request.expectedRevision < 0 ||
    !(isCycleScheduleRequestV1(request.cycleScheduleV1)
      || (request.cycleScheduleV1.version === 2
        && request.cycleScheduleV1.entries.length === 0
        && (!('endpointDenominator' in request.cycleScheduleV1)
          || [1, 2, 4, 8, 16].includes(Number(request.cycleScheduleV1.endpointDenominator))))) ||
    (request.progressRequestId !== undefined &&
      (!/^[\x20-\x7e]+$/.test(request.progressRequestId) || request.progressRequestId.length > 128))
  ) return Promise.reject(new Error('invalid current-cycle preview request'))
  return invoke<unknown>('propose_current_cycle_pose_v1', { request }).then((payload) =>
    normalizeCurrentCyclePosePreviewResponseV1(payload, request.expectedRevision))
}

export function normalizeCurrentCyclePosePreviewResponseV1(
  payload: unknown,
  expectedRevision: number,
): CurrentCyclePosePreviewResponseV1 {
    if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) {
      throw new Error('invalid current-cycle preview response')
    }
    const value = payload as Record<string, unknown>
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
        value.continuousLayerTransportModelId !== 'general_multi_face_positive_thickness_cell_transport_v1') ||
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
      !isLayerOrderPairsV1(sourceLayerOrder) ||
      !isLayerOrderPairsV1(targetLayerOrder) ||
      JSON.stringify(sourceLayerOrder) !== JSON.stringify(targetLayerOrder) ||
      (value.continuousLayerTransportModelId === null
        ? continuousLayerTransitionCount !== 0 ||
          value.continuousLayerPairOrderCount !== 0 ||
          value.continuousLayerTargetOrderSha256 !== null
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
    if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) return
    const value = payload as Record<string, unknown>
    if (
      Object.keys(value).sort().join(',') !==
        'authorizesProjectMutation,completedWork,requestId,status,totalWork,version' ||
      value.version !== 1 ||
      typeof value.requestId !== 'string' ||
      value.requestId.length === 0 || value.requestId.length > 128 ||
      !['running', 'certified', 'cancelled', 'failed'].includes(String(value.status)) ||
      !Number.isSafeInteger(value.completedWork) || Number(value.completedWork) < 0 ||
      Number(value.completedWork) > 2 || value.totalWork !== 2 ||
      value.authorizesProjectMutation !== false
    ) return
    onProgress(value as CurrentCyclePoseProgressV1)
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
    if (
      typeof payload !== 'object' ||
      payload === null ||
      Array.isArray(payload)
    ) return
    const value = payload as Record<string, unknown>
    if (
      Object.keys(value).length !== 7 ||
      value.version !== 1 ||
      typeof value.requestId !== 'string' ||
      value.requestId.length === 0 ||
      value.requestId.length > 128 ||
      !Number.isSafeInteger(value.exploredStateCount) ||
      Number(value.exploredStateCount) < 0 ||
      Number(value.exploredStateCount) > 32 ||
      !Number.isSafeInteger(value.evaluatedTransitionCount) ||
      Number(value.evaluatedTransitionCount) < 0 ||
      Number(value.evaluatedTransitionCount) > 64 ||
      value.stateLimit !== 32 ||
      value.transitionLimit !== 64 ||
      value.authorizesProjectMutation !== false
    ) return
    onProgress(value as StackedFoldReadProgressV1)
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

export function appendGenericTreeInstructionProposal(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  expectedTopologySha256: ReadonlyArray<number>,
) {
  if (!isCanonicalNonNilUuid(expectedProjectInstanceId) || !isCanonicalNonNilUuid(expectedProjectId)
    || !isProjectRevision(expectedRevision) || !isBoundedIntegerTuple(expectedTopologySha256, 32, 255)) {
    return Promise.reject(new Error('invalid_generic_tree_instruction_request'))
  }
  return invoke<ProjectSnapshot>('append_generic_tree_instruction_proposal', {
    expectedProjectInstanceId, expectedProjectId, expectedRevision,
    expectedTopologySha256: Array.from(expectedTopologySha256), confirmed: true,
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
  const source = snapshotCoreDataRecord(value)
  if (!source) return null
  const { reference_model_assets: referenceAssetsValue, ...baseValue } = source
  const record = exactCoreDataRecord(
    baseValue,
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
    || !hasStrictOptionalPathCertificateReferences(record.instruction_timeline)
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
  const referenceAssets = referenceAssetsValue === undefined ? [] : referenceAssetsValue
  if (!Array.isArray(referenceAssets) || referenceAssets.length > 8) return null
  const referenceModelAssets = (referenceAssets as unknown[]).map((value) =>
    exactCoreDataRecord(value, ['asset_id', 'sha256'] as const))
  if (referenceModelAssets.some((asset) => !asset || !isCanonicalNonNilUuid(asset.asset_id)
    || !isBoundedIntegerTuple(asset.sha256, 32, 255))) return null
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
    reference_model_assets: referenceModelAssets.map((asset) => ({
      asset_id: String(asset?.asset_id), sha256: [...(asset?.sha256 as number[])],
    })),
    can_undo: record.can_undo,
    can_redo: record.can_redo,
    cutting_allowed: record.cutting_allowed,
  })
}

function hasStrictOptionalPathCertificateReferences(value: Readonly<Record<string, unknown>>) {
  if (value.steps === undefined) return true
  if (!Array.isArray(value.steps)) return false
  return value.steps.every((stepValue) => {
    const step = snapshotCoreDataRecord(stepValue)
    const visual = step && snapshotCoreDataRecord(step.visual)
    if (!visual) return false
    const referenceValue = visual.path_certificate_reference_v1
    const metadataValue = visual.named_technique_compiler_v1
    if (metadataValue !== undefined && metadataValue !== null) {
      const metadata = exactCoreDataRecord(metadataValue, [
        'version', 'model_id', 'technique_kind', 'segment_index', 'segment_count',
        'compiler_output_sha256',
      ] as const)
      if (!metadata || metadata.version !== 1
        || metadata.model_id !== 'certified_named_technique_compiler_metadata_v1'
        || !['mountain', 'valley', 'squash', 'crimp', 'inside_reverse', 'outside_reverse',
          'sink', 'accordion', 'layer_selective'].includes(String(metadata.technique_kind))
        || !Number.isSafeInteger(metadata.segment_index) || Number(metadata.segment_index) < 0
        || !Number.isSafeInteger(metadata.segment_count) || Number(metadata.segment_count) < 1
        || Number(metadata.segment_index) >= Number(metadata.segment_count)
        || !isBoundedIntegerTuple(metadata.compiler_output_sha256, 32, 255)
        || !(metadata.compiler_output_sha256 as number[]).some((byte) => byte !== 0)) return false
    }
    if (referenceValue === undefined || referenceValue === null) return true
    const reference = exactCoreDataRecord(referenceValue, [
      'version',
      'model_id',
      'binding_sha256',
      'source_pose_sha256',
      'target_pose_sha256',
      'source_model_binding_sha256',
      'transition_count',
    ] as const)
    return Boolean(
      reference
      && reference.version === 1
      && reference.model_id === 'bounded_certified_pose_graph_path_reference_v1'
      && isBoundedIntegerTuple(reference.binding_sha256, 32, 255)
      && isBoundedIntegerTuple(reference.source_pose_sha256, 32, 255)
      && isBoundedIntegerTuple(reference.target_pose_sha256, 32, 255)
      && isBoundedIntegerTuple(reference.source_model_binding_sha256, 32, 255)
      && (reference.binding_sha256 as number[]).some((byte) => byte !== 0)
      && (reference.source_model_binding_sha256 as number[]).some((byte) => byte !== 0)
      && JSON.stringify(reference.source_pose_sha256)
        !== JSON.stringify(reference.target_pose_sha256)
      && Number.isSafeInteger(reference.transition_count)
      && Number(reference.transition_count) >= 1
      && Number(reference.transition_count) <= 64
    )
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
