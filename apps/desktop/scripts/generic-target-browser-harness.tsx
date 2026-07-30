import { createRoot } from 'react-dom/client'
import { useEffect, useRef, useState } from 'react'
import { GenericTargetBindingList } from '../src/components/GenericTargetBindingList.tsx'
import { ProtrusionDimensionEditor } from '../src/components/ProtrusionDimensionEditor.tsx'
import { GenericBodyOutlineEditor } from '../src/components/GenericBodyOutlineEditor.tsx'
import { BeginnerShapeCanvasPreview } from '../src/components/BeginnerShapeCanvasPreview.tsx'
import { RecognitionContourCopyAction } from '../src/components/RecognitionContourCopyAction.tsx'
import { finishBeginnerGridCancellation, runBeginnerGridApplyWorkflow } from '../src/lib/beginnerGridWorkflow.ts'
import {
  applyBeginnerParameterGridCandidate,
  evaluateBeginnerParameterGrid,
  getProjectSnapshot,
  redo as redoProject,
  undo as undoProject,
  type BeginnerDesignProfileV1,
  type BeginnerGenerationConstraintsV1,
  type BeginnerGridEvaluationResponse,
  type ProjectSnapshot,
} from '../src/lib/coreClient.ts'
import '../src/App.css'

const initialBindings: NonNullable<BeginnerGenerationConstraintsV1['protrusions']> = [
  { id: 1, count: 1, symmetry: 'none', length_tenths_mm: 270, thickness_tenths_mm: 50,
    position_tenths_mm: [0, 0, 0], direction_milli: [0, 1000, 0], curvature_degrees: 0,
    joint: 'fixed', motion_degrees: [0, 0], side: 'either', priority: 50 },
  { id: 2, count: 2, symmetry: 'bilateral', length_tenths_mm: 220, thickness_tenths_mm: 40,
    position_tenths_mm: [0, 0, 0], direction_milli: [1000, 0, 0], curvature_degrees: 0,
    joint: 'fixed', motion_degrees: [0, 0], side: 'either', priority: 60 },
]

const GENERAL14_INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const GENERAL14_PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const GENERAL14_GENERATION_ID = '33333333-3333-4333-8333-333333333333'
const GENERAL14_AUTHORITY_TOKEN = '44444444-4444-4444-8444-444444444444'
const GENERAL14_GRID_HASH = [
  224, 59, 9, 238, 119, 51, 70, 177,
  12, 139, 19, 69, 142, 139, 157, 2,
  55, 85, 134, 120, 49, 93, 4, 65,
  125, 141, 52, 157, 74, 39, 236, 192,
] as const
const general14Uuid = (namespace: number, index: number) =>
  `${namespace.toString(16).padStart(8, '0')}`
  + `-0000-4000-8000-${index.toString(16).padStart(12, '0')}`

const GENERAL14_PROFILE: BeginnerDesignProfileV1 = {
  schema_version: 1,
  preset: 'balanced',
  shape_fidelity_weight: 35,
  foldability_weight: 35,
  step_count_weight: 15,
  paper_efficiency_weight: 15,
  generation_constraints: {
    schema_version: 1,
    maximum_steps: 60,
    detail_level: 'standard',
    target_category: 'animal',
    target_parts: [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'fin', count: 8 },
      { kind: 'tail', count: 6 },
    ],
    skeleton_segments: [
      {
        id: 10,
        start: { x_tenths_mm: 0, y_tenths_mm: 0 },
        end: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
        thickness_tenths_mm: 10,
      },
      {
        id: 20,
        start: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
        end: { x_tenths_mm: 1_000, y_tenths_mm: 500 },
        thickness_tenths_mm: 10,
      },
    ],
    protrusions: Array.from({ length: 14 }, (_, index) => ({
      id: index + 1,
      count: 1,
      symmetry: 'none' as const,
      length_tenths_mm: 200 + index,
      thickness_tenths_mm: 20,
      position_tenths_mm: [index * 10, 0, 0] as [number, number, number],
      direction_milli: [0, 1_000, 0] as [number, number, number],
      curvature_degrees: 0,
      joint: 'fixed' as const,
      motion_degrees: [0, 0] as [number, number],
      side: 'either' as const,
      priority: 50,
    })),
    bulge_targets: [],
    target_asset: null,
    allowed_techniques: ['valley_fold'],
  },
}

function makeGeneral14GridResponse() {
  const center = {
    id: general14Uuid(1, 1),
    position: { x: 0, y: 0 },
  }
  const featureVertices = Array.from({ length: 14 }, (_, index) => ({
    id: general14Uuid(2, index + 1),
    position: { x: index + 1, y: 1 },
  }))
  const supportVertices = Array.from({ length: 4 }, (_, index) => ({
    id: general14Uuid(3, index + 1),
    position: { x: index, y: 2 },
  }))
  const treeVertices = Array.from({ length: 3 }, (_, index) => ({
    id: general14Uuid(4, index + 1),
    position: { x: 10 + Math.min(index, 1), y: 10 + Math.max(0, index - 1) },
  }))
  const supportEdges = supportVertices.map((vertex, index) => ({
    id: general14Uuid(5, index + 1),
    start: center.id,
    end: vertex.id,
    kind: 'valley' as const,
  }))
  const featureEdges = featureVertices.map((vertex, index) => ({
    id: general14Uuid(6, index + 1),
    start: center.id,
    end: vertex.id,
    kind: 'valley' as const,
  }))
  const treeEdges = [
    {
      id: general14Uuid(7, 1),
      start: treeVertices[0]!.id,
      end: treeVertices[1]!.id,
      kind: 'auxiliary' as const,
    },
    {
      id: general14Uuid(7, 2),
      start: treeVertices[1]!.id,
      end: treeVertices[2]!.id,
      kind: 'auxiliary' as const,
    },
  ]
  const plan = {
    schema_version: 1,
    kind: 'composite_generic_target_base',
    crease_pattern: {
      vertices: [center, ...featureVertices, ...supportVertices, ...treeVertices],
      edges: [...supportEdges, ...featureEdges, ...treeEdges],
    },
    instruction_codes: [
      'bounded_tree_river_axial_v1:4000000,1000000',
      'bounded_radial_corner_support_v1:added=4:covered=4',
      'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
      'bounded_tree_paper_orientation_v1:horizontal',
    ],
    target_parts: GENERAL14_PROFILE.generation_constraints.target_parts
      .map((part) => ({ ...part })),
    skeleton_segments: GENERAL14_PROFILE.generation_constraints
      .skeleton_segments.map((segment) => ({
        ...segment,
        start: { ...segment.start },
        end: { ...segment.end },
      })),
    target_asset: null,
  }
  const assessment = {
    kind: plan.kind,
    expected_candidate_edge_id: supportEdges[0]!.id,
    proof_scope: 'sufficient',
    apply_allowed: true,
    reason: 'native_fold_path_certified',
    shape_approximation_score: null,
    shape_difference_reason: null,
    component_shape_comparison: null,
  }
  return {
    request_generation_id: GENERAL14_GENERATION_ID,
    authority_token: GENERAL14_AUTHORITY_TOKEN,
    project_instance_id: GENERAL14_INSTANCE_ID,
    project_id: GENERAL14_PROJECT_ID,
    revision: 7,
    grid_hash: [...GENERAL14_GRID_HASH],
    evaluated_grid_points: 27,
    global_checked_candidates: 3,
    refinement_iterations: 0,
    candidates: [{
      point: {
        id: 13,
        scale_percent: 27,
        spacing_percent: 50,
        detail_level: 'standard',
      },
      primary_score: 980,
      plan,
      assessment,
      local_proof_scope: 'necessary',
      global_proof_scope: 'sufficient',
      complexity_score: 100,
      paper_efficiency_score: 50,
      scale_deviation_penalty: 20,
      spacing_deviation_penalty: 0,
      detail_mismatch_penalty: 0,
      outcome_reason: assessment.reason,
      contour_witness: {
        body_contour_points: 0,
        local_bindings: [],
        generic_feature_bindings: Array.from({ length: 14 }, (_, index) => ({
          protrusion_id: index + 1,
          generated_feature_id: index + 1,
          endpoint_count: 1,
          crease_start: 4 + index,
          crease_authority_sha256: Array(32).fill(10 + index),
          skeleton_segment_id: index === 0 ? 10 : 20,
          skeleton_endpoint: index === 0 ? 'start' : 'end',
          mount_distance_squared_tenths_mm: 0,
        })),
        skeleton_branch_bindings: [
          {
            segment_id: 10,
            parent_segment_id: null,
            parent_endpoint: null,
            child_endpoint: null,
            generated_feature_ids: [1],
          },
          {
            segment_id: 20,
            parent_segment_id: 10,
            parent_endpoint: 'end',
            child_endpoint: 'start',
            generated_feature_ids: Array.from(
              { length: 13 },
              (_, index) => index + 2,
            ),
          },
        ],
        skeleton_tree_authority_sha256: Array(32).fill(2),
        witnessed_vertices: 0,
        witnessed_creases: 0,
        topology_authority_hash: Array(32).fill(3),
        max_contour_error_millionths: 0,
      },
      refinement_iterations: 0,
      strict_improvements: 0,
      refinement_starts: 1,
    }],
  }
}

const GENERAL14_GRID_RESPONSE = makeGeneral14GridResponse()
const GENERAL14_FEATURE_BINDINGS =
  GENERAL14_GRID_RESPONSE.candidates[0]!.contour_witness
    .generic_feature_bindings
type General14Snapshot = ProjectSnapshot & Readonly<{
  general_semantic_feature_bindings_v1: typeof GENERAL14_FEATURE_BINDINGS
}>
const general14Snapshot = (
  revision: number,
  bindings: typeof GENERAL14_FEATURE_BINDINGS,
) => ({
  project_instance_id: GENERAL14_INSTANCE_ID,
  project_id: GENERAL14_PROJECT_ID,
  revision,
  beginner_design_profile: structuredClone(GENERAL14_PROFILE),
  general_semantic_feature_bindings_v1: structuredClone(bindings),
  can_undo: bindings.length === 14,
  can_redo: bindings.length === 0,
}) as unknown as General14Snapshot
const exactKeys = (value: unknown, expected: readonly string[]) =>
  typeof value === 'object'
  && value !== null
  && !Array.isArray(value)
  && Object.keys(value).sort().join('\u0000')
    === [...expected].sort().join('\u0000')
const sameJson = (left: unknown, right: unknown) =>
  JSON.stringify(left) === JSON.stringify(right)
const general14Evidence = {
  evaluations: 0,
  applies: 0,
  undos: 0,
  redos: 0,
  reopens: 0,
  strictEvaluateDto: false,
  strictApplyDto: false,
  maximumDeclaredSemanticCount: 14,
}
let general14LiveSnapshot = general14Snapshot(7, [])
let general14RedoBindings: typeof GENERAL14_FEATURE_BINDINGS | null = null

function assertGeneral14Fixture() {
  const semanticEndpoints =
    GENERAL14_PROFILE.generation_constraints.target_parts.reduce(
      (sum, part) =>
        part.kind === 'head' || part.kind === 'torso'
          ? sum
          : sum + part.count,
      0,
    )
  const physicalBindings =
    GENERAL14_PROFILE.generation_constraints.protrusions ?? []
  const witnessedEndpoints = GENERAL14_FEATURE_BINDINGS.reduce(
    (sum, binding) => sum + binding.endpoint_count,
    0,
  )
  if (
    semanticEndpoints !== 14
    || physicalBindings.length !== 14
    || physicalBindings.some((binding) => binding.count !== 1)
    || GENERAL14_FEATURE_BINDINGS.length !== 14
    || witnessedEndpoints !== 14
    || GENERAL14_FEATURE_BINDINGS.some((binding, index) =>
      binding.protrusion_id !== index + 1
      || binding.generated_feature_id !== index + 1
      || binding.endpoint_count !== 1)
  ) throw new Error('general semantic count 14 fixture crossed its strict bound')
}

Object.assign(window, {
  __ORIGAMI2_GENERIC_TARGET_GENERAL14_EVIDENCE__: general14Evidence,
  __TAURI_INTERNALS__: {
    invoke: async (command: string, args?: Record<string, unknown>) => {
      if (command === 'evaluate_beginner_parameter_grid') {
        if (!exactKeys(args, [
          'expectedProjectInstanceId',
          'expectedProjectId',
          'expectedRevision',
          'requestGenerationId',
        ]) || args?.expectedProjectInstanceId !== GENERAL14_INSTANCE_ID
          || args?.expectedProjectId !== GENERAL14_PROJECT_ID
          || args?.expectedRevision !== 7
          || args?.requestGenerationId !== GENERAL14_GENERATION_ID) {
          throw new Error('invalid general semantic count 14 preview request')
        }
        assertGeneral14Fixture()
        general14Evidence.evaluations += 1
        general14Evidence.strictEvaluateDto = true
        return structuredClone(GENERAL14_GRID_RESPONSE)
      }
      if (command === 'apply_beginner_parameter_grid_candidate') {
        const candidate = GENERAL14_GRID_RESPONSE.candidates[0]!
        if (!exactKeys(args, [
          'expectedProjectInstanceId',
          'expectedProjectId',
          'expectedRevision',
          'requestGenerationId',
          'authorityToken',
          'expectedProfile',
          'expectedGridHash',
          'selectedPoint',
          'expectedCandidateEdgeId',
          'expectedTopologyAuthorityHash',
          'confirmed',
        ]) || args?.expectedProjectInstanceId !== GENERAL14_INSTANCE_ID
          || args?.expectedProjectId !== GENERAL14_PROJECT_ID
          || args?.expectedRevision !== 7
          || args?.requestGenerationId !== GENERAL14_GENERATION_ID
          || args?.authorityToken !== GENERAL14_AUTHORITY_TOKEN
          || !sameJson(args?.expectedProfile, GENERAL14_PROFILE)
          || !sameJson(args?.expectedGridHash, GENERAL14_GRID_HASH)
          || !sameJson(args?.selectedPoint, candidate.point)
          || args?.expectedCandidateEdgeId
            !== candidate.assessment.expected_candidate_edge_id
          || !sameJson(
            args?.expectedTopologyAuthorityHash,
            candidate.contour_witness.topology_authority_hash,
          )
          || args?.confirmed !== true) {
          throw new Error('invalid general semantic count 14 apply request')
        }
        assertGeneral14Fixture()
        general14Evidence.applies += 1
        general14Evidence.strictApplyDto = true
        general14RedoBindings = null
        general14LiveSnapshot = general14Snapshot(
          8,
          GENERAL14_FEATURE_BINDINGS,
        )
        return structuredClone(general14LiveSnapshot)
      }
      if (command === 'undo' || command === 'redo') {
        if (!exactKeys(args, [
          'expectedProjectInstanceId',
          'expectedProjectId',
          'expectedRevision',
        ]) || args?.expectedProjectInstanceId !== GENERAL14_INSTANCE_ID
          || args?.expectedProjectId !== GENERAL14_PROJECT_ID
          || args?.expectedRevision !== general14LiveSnapshot.revision) {
          throw new Error(`invalid general semantic count 14 ${command}`)
        }
        if (command === 'undo') {
          if (GENERAL14_FEATURE_BINDINGS.length
            !== general14LiveSnapshot.general_semantic_feature_bindings_v1.length) {
            throw new Error('general semantic count 14 undo lost authority')
          }
          general14Evidence.undos += 1
          general14RedoBindings = structuredClone(GENERAL14_FEATURE_BINDINGS)
          general14LiveSnapshot = general14Snapshot(
            general14LiveSnapshot.revision + 1,
            [],
          )
        } else {
          if (general14RedoBindings?.length !== 14) {
            throw new Error('general semantic count 14 redo lacks authority')
          }
          general14Evidence.redos += 1
          general14LiveSnapshot = general14Snapshot(
            general14LiveSnapshot.revision + 1,
            general14RedoBindings,
          )
          general14RedoBindings = null
        }
        return structuredClone(general14LiveSnapshot)
      }
      if (command === 'project_snapshot') {
        general14Evidence.reopens += 1
        return structuredClone(general14LiveSnapshot)
      }
      throw new Error(`unexpected generic target harness command ${command}`)
    },
  },
})

const snapshotGeneral14Bindings = (snapshot: ProjectSnapshot | null) =>
  snapshot === null
    ? []
    : (snapshot as General14Snapshot)
        .general_semantic_feature_bindings_v1

function Harness() {
  const [metricPreset, setMetricPreset] = useState<'balanced' | 'shape' | 'foldability'>('balanced')
  const [recognized, setRecognized] = useState(false), [preview, setPreview] = useState(false)
  const [status, setStatus] = useState('Waiting for image or GLB'), [applied, setApplied] = useState(false)
  const [bindings, setBindings] = useState([...initialBindings])
  const [kinds, setKinds] = useState<Array<'leg' | 'horn' | 'ear' | 'wing' | 'fin' | 'antenna' | 'tail'>>(['tail', 'fin'])
  const [excludedImageCandidate, setExcludedImageCandidate] = useState<{
    binding: (typeof initialBindings)[number]
    kind: (typeof kinds)[number]
    outlineEvidence: string
  } | null>(null)
  const [outline, setOutline] = useState<Array<[number, number]>>([])
  const [outlineMode, setOutlineMode] = useState<'symmetric' | 'general'>('symmetric')
  const [selectedCandidate, setSelectedCandidate] = useState(1)
  const [candidateShortage, setCandidateShortage] = useState(false)
  const [glbWitness, setGlbWitness] = useState<{ bounds: string, bulges: number, discrepancy: number } | null>(null)
  const [selectedSurfaceRanges, setSelectedSurfaceRanges] = useState<number[]>([])
  const [surfaceRangesConfirmed, setSurfaceRangesConfirmed] = useState(false)
  const [skeletonTreeConfirmed, setSkeletonTreeConfirmed] = useState(true)
  const [recognizedSkeletonEndMm, setRecognizedSkeletonEndMm] = useState(10)
  const [activeReferenceAsset, setActiveReferenceAsset] = useState(1)
  const [archivedReferences, setArchivedReferences] = useState<number[]>([])
  const [mergedAuthorities, setMergedAuthorities] = useState(false)
  const [authorityValid, setAuthorityValid] = useState(true)
  const [imageDecode, setImageDecode] = useState<string | null>(null)
  const [imageMeaningsConfirmed, setImageMeaningsConfirmed] = useState(true)
  const [outlineEditConfirmed, setOutlineEditConfirmed] = useState(true)
  const [outlineEdit, setOutlineEdit] = useState<'split' | 'merge' | null>(null)
  const [outlineSplitX, setOutlineSplitX] = useState(50)
  const [confirmedImageFeatureCount, setConfirmedImageFeatureCount] = useState<number | null>(null)
  const [segmentation, setSegmentation] = useState<string | null>(null)
  const [acceptedSegments, setAcceptedSegments] = useState<number[]>([1, 2])
  const [confidence, setConfidence] = useState<{ score: number, reason: string, low: boolean } | null>(null)
  const [confidenceOverride, setConfidenceOverride] = useState(false)
  const [asymmetricBird, setAsymmetricBird] = useState(false)
  const [asymmetricFourLeg, setAsymmetricFourLeg] = useState(false)
  const [asymmetricInsect, setAsymmetricInsect] = useState(false)
  const [asymmetricFish, setAsymmetricFish] = useState(false)
  const [exportStatus, setExportStatus] = useState<string | null>(null)
  const [general14Grid, setGeneral14Grid] =
    useState<BeginnerGridEvaluationResponse | null>(null)
  const [general14HistorySnapshot, setGeneral14HistorySnapshot] =
    useState<ProjectSnapshot | null>(null)
  const witnessCanvas = useRef<HTMLCanvasElement>(null)
  const contourScore = Math.min(100, 80 + Math.max(0, outline.length - 4)
    + bindings.reduce((sum, target) => sum + Math.max(0, (target.local_outline_tenths_mm?.length ?? 3) - 3), 0))
  const contourPointCount = outline.length + bindings.reduce(
    (sum, target) => sum + (target.local_outline_tenths_mm?.length ?? 0), 0)
  const synthesizedCandidateCount = Math.min(8, Math.max(3,
    bindings.length + Math.floor(contourPointCount / 4)))
  const evaluate = useRef<HTMLButtonElement>(null)
  const depthError = glbWitness ? Math.abs(65 - (selectedCandidate === 1 ? 62 : 58)) : 0
  const threeDimensionalScore = Math.max(0, 100 - depthError * 4 - (glbWitness?.bulges ?? 0) * 2)
  useEffect(() => {
    if (!preview && !applied) return
    const canvas = witnessCanvas.current, context = canvas?.getContext('2d')
    if (!canvas || !context) return
    context.clearRect(0, 0, canvas.width, canvas.height)
    const points = outline.length >= 4 ? outline : [[-50, -50], [50, -50], [50, 50], [-50, 50]]
    const scale = selectedCandidate === 1 ? 1 : 0.82
    context.strokeStyle = '#2563eb'; context.lineWidth = 3; context.beginPath()
    points.forEach(([x, y], index) => { const px = 160 + x * scale, py = 100 + y * scale
      if (index === 0) context.moveTo(px, py); else context.lineTo(px, py) })
    context.closePath(); context.stroke()
    context.strokeStyle = '#dc2626'; context.lineWidth = 1
    points.forEach(([x, y], index) => { const angle = index * Math.PI * 2 / points.length
      context.beginPath(); context.moveTo(160 + x * scale, 100 + y * scale)
      context.lineTo(160 + Math.cos(angle) * 78, 100 + Math.sin(angle) * 78); context.stroke() })
  }, [preview, applied, outline, selectedCandidate])
  const focus = () => requestAnimationFrame(() => evaluate.current?.focus())
  const canonicalize = (targets: typeof bindings) => targets.map(
    (target, index) => ({ ...target, id: index + 1 }),
  )
  const move = (index: number, offset: -1 | 1) => setBindings((current) => {
    const destination = index + offset
    if (destination < 0 || destination >= current.length) return current
    const moved = [...current]
    ;[moved[index], moved[destination]] = [moved[destination]!, moved[index]!]
    setKinds((currentKinds) => {
      const movedKinds = [...currentKinds]
      ;[movedKinds[index], movedKinds[destination]] = [movedKinds[destination]!, movedKinds[index]!]
      return movedKinds
    })
    return moved
  })
  const recognize = (source: string) => {
    setAuthorityValid(true)
    setBindings(initialBindings.map((target) => ({ ...target })))
    setKinds(['tail', 'fin'])
    setExcludedImageCandidate(null)
    setRecognized(true); setPreview(false); setCandidateShortage(false); setStatus(`${source} recognized two bounded bindings`)
    setMergedAuthorities(false)
    if (source === 'Image' || source === 'JPEG EXIF') {
      setImageMeaningsConfirmed(false)
      setOutlineEditConfirmed(true); setOutlineEdit(null)
      setConfirmedImageFeatureCount(null)
      setOutlineMode('general'); setOutline([[-50, -40], [50, -40], [45, 40], [-40, 40]])
      setBindings(initialBindings.map((target) => ({ ...target,
        local_outline_tenths_mm: [[-18, -8], [18, -8], [0, 28]] })))
      setImageDecode(source === 'JPEG EXIF' ? 'JPEG RGB · EXIF orientation 6 normalized' : 'PNG RGBA · alpha/luminance mask')
      setSegmentation('2 protrusions · binding 1 asymmetric · binding 2 bilateral')
      setAcceptedSegments([1, 2])
      setConfidence({ score: 88, reason: 'dominant_component, bounded_simplification_error', low: false })
      setConfidenceOverride(false)
    } else { setImageDecode(null); setSegmentation(null); setConfidence(null); setImageMeaningsConfirmed(true) }
    if (source === 'GLB') {
      setSelectedSurfaceRanges([]); setSurfaceRangesConfirmed(false)
      setOutlineMode('general'); setOutline([[-60, -40], [60, -40], [55, 40], [-50, 40]])
      setBindings(initialBindings.map((target) => ({ ...target,
        local_outline_tenths_mm: [[-20, -10], [20, -10], [0, 30]] })))
      setGlbWitness({ bounds: '120×80×65 mm', bulges: 2, discrepancy: 7 })
    } else setGlbWitness(null)
  }
  const previewGeneral14 = async () => {
    const grid = await evaluateBeginnerParameterGrid(
      GENERAL14_PROJECT_ID,
      7,
      GENERAL14_INSTANCE_ID,
      GENERAL14_GENERATION_ID,
      GENERAL14_PROFILE,
    )
    const featureBindings =
      grid.candidates[0]?.contour_witness.generic_feature_bindings ?? []
    const endpointCount = featureBindings.reduce(
      (sum, binding) => sum + binding.endpoint_count,
      0,
    )
    if (featureBindings.length !== 14 || endpointCount !== 14) {
      throw new Error('strict general semantic count 14 DTO was not retained')
    }
    setGeneral14Grid(grid)
    setGeneral14HistorySnapshot(null)
    setStatus('General semantic count 14 preview admitted by strict native DTO')
  }
  const applyGeneral14 = () => {
    const grid = general14Grid
    const candidate = grid?.candidates[0]
    if (!grid || !candidate) return
    void runBeginnerGridApplyWorkflow({
      confirm: () => true,
      apply: async () => {
        const next = await applyBeginnerParameterGridCandidate(
          GENERAL14_PROJECT_ID,
          7,
          GENERAL14_INSTANCE_ID,
          grid,
          GENERAL14_PROFILE,
          candidate,
        )
        setGeneral14HistorySnapshot(next)
        return true
      },
      clearPreview: () => setGeneral14Grid(null),
      restoreFocus: () => undefined,
    }).then((appliedGeneral14) => {
      if (appliedGeneral14) {
        setStatus('General semantic count 14 applied with 14 persisted bindings')
      }
    })
  }
  const mutateGeneral14History = async (action: 'undo' | 'redo') => {
    const current = general14HistorySnapshot
    if (!current) return
    const next = await (action === 'undo' ? undoProject : redoProject)(
      current.project_id,
      current.revision,
      current.project_instance_id,
    )
    const bindingCount = snapshotGeneral14Bindings(next).length
    if (
      (action === 'undo' && bindingCount !== 0)
      || (action === 'redo' && bindingCount !== 14)
    ) throw new Error(`general semantic count 14 ${action} drifted`)
    setGeneral14HistorySnapshot(next)
    setStatus(action === 'undo'
      ? 'General semantic count 14 undone'
      : 'General semantic count 14 redone with 14 persisted bindings')
  }
  const reopenGeneral14 = async () => {
    const next = await getProjectSnapshot()
    if (snapshotGeneral14Bindings(next).length !== 14) {
      throw new Error('general semantic count 14 reopen lost bindings')
    }
    setGeneral14HistorySnapshot(next)
    setStatus(
      'General semantic count 14 reopened from project snapshot with 14 persisted bindings',
    )
  }
  const general14PreviewBindings =
    general14Grid?.candidates[0]?.contour_witness.generic_feature_bindings ?? []
  const general14HistoryBindings =
    snapshotGeneral14Bindings(general14HistorySnapshot)
  return <main><h1>Bounded generic target</h1>
    <button onClick={() => void previewGeneral14()}>
      Preview general semantic count 14
    </button>
    {general14Grid && <section aria-label="General semantic count 14 preview">
      <p>Strict production DTO retained 14 semantic endpoints and 14 feature bindings.</p>
      <ol aria-label="General semantic count 14 preview bindings">
        {general14PreviewBindings.map((binding) =>
          <li key={binding.generated_feature_id}>
            Binding {binding.generated_feature_id}: protrusion {binding.protrusion_id},
            endpoints {binding.endpoint_count}
          </li>)}
      </ol>
      <button onClick={applyGeneral14}>Apply general semantic count 14</button>
    </section>}
    {general14HistorySnapshot && <section aria-label="General semantic count 14 history">
      <p>Persisted general feature bindings: {general14HistoryBindings.length}/14.</p>
      <ol aria-label="Persisted general semantic count 14 bindings">
        {general14HistoryBindings.map((binding) =>
          <li key={binding.generated_feature_id}>
            Binding {binding.generated_feature_id}: endpoint count {binding.endpoint_count}
          </li>)}
      </ol>
      <button
        disabled={general14HistoryBindings.length !== 14}
        onClick={() => void mutateGeneral14History('undo')}
      >
        Undo general semantic count 14
      </button>
      <button
        disabled={general14HistoryBindings.length !== 0}
        onClick={() => void mutateGeneral14History('redo')}
      >
        Redo general semantic count 14
      </button>
      <button
        disabled={general14HistoryBindings.length !== 14}
        onClick={() => void reopenGeneral14()}
      >
        Reopen general semantic count 14 snapshot
      </button>
      <button onClick={() => {
        setGeneral14Grid(null)
        setGeneral14HistorySnapshot(null)
        setStatus('General semantic count 14 lifecycle reset')
      }}>
        Reset general semantic count 14 lifecycle
      </button>
    </section>}
    <button onClick={() => setMetricPreset('balanced')}>Use balanced metric</button>
    <button onClick={() => setMetricPreset('shape')}>Use shape-priority metric</button>
    <button onClick={() => setMetricPreset('foldability')}>Use foldability-priority metric</button>
    <button onClick={() => recognize('Empty generic target')}>Create empty generic target</button>
    <button onClick={() => recognize('Image')}>Recognize mixed target image</button>
    <button onClick={() => {
      recognize('Image')
      setBindings([...initialBindings, { ...initialBindings[0]!, id: 3 }])
      setKinds(['tail', 'fin', 'ear'])
      setStatus('Image outline proposal contains 2 parts + 1 possible noise candidate')
    }}>Recognize image with noise candidate</button>
    <button onClick={() => {
      recognize('Image')
      setBindings([1, 2, 3].map((id) => ({ ...initialBindings[0]!, id, count: 1, symmetry: 'none' as const })))
      setKinds(['tail', 'tail', 'tail'])
      setStatus('Three exact image candidate IDs assigned explicitly to the same tail meaning')
    }}>Assign three candidates the same explicit meaning</button>
    <button onClick={() => recognize('JPEG EXIF')}>Recognize EXIF JPEG silhouette</button>
    <button onClick={() => { recognize('JPEG EXIF'); setConfidence({ score: 42, reason: 'low_component_ratio, bounded_curvature', low: true }) }}>Recognize low confidence JPEG</button>
    <button onClick={() => setStatus('Rejected confidence: tampered score or reason')}>Try tampered confidence</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected image: decoded pixel resource limit') }}>Try oversized decoded image</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected image: stale decoded asset') }}>Try stale decoded image</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected segmentation: overlapping or too-thin protrusion') }}>Try invalid protrusion segmentation</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected segmentation: noise exceeds bounded curvature budget') }}>Try noisy silhouette segmentation</button>
    <button onClick={() => recognize('GLB')}>Recognize mixed target GLB</button>
    <button onClick={() => { setAsymmetricBird(true); setStatus('Asymmetric bird landmarks bound: head · tail · left wing · right wing') }}>Recognize asymmetric bird landmarks</button>
    <button onClick={() => {
      setAsymmetricFourLeg(true)
      setStatus('Asymmetric four-leg landmarks bound individually')
    }}>Recognize asymmetric four-leg landmarks</button>
    {asymmetricFourLeg && <ul aria-label="Asymmetric four-leg landmark bindings">
      <li>front-left leg · landmark leg-front-left</li>
      <li>front-right leg · landmark leg-front-right</li>
      <li>rear-left leg · landmark leg-rear-left</li>
      <li>rear-right leg · landmark leg-rear-right</li>
    </ul>}
    <button onClick={() => {
      setAsymmetricInsect(true)
      setStatus('Asymmetric insect semantic landmarks bound to certified four-ray groups')
    }}>Recognize asymmetric insect landmarks</button>
    {asymmetricInsect && <section aria-label="Asymmetric insect semantic provenance">
      <ol aria-label="Ordered insect landmark bindings">
        {['head', 'tail', 'wing_left', 'wing_right', 'leg_front_left', 'leg_front_right',
          'leg_middle_left', 'leg_middle_right', 'leg_rear_left', 'leg_rear_right']
          .map((role, ordinal) => <li key={role}>{ordinal}: {role} · physical ray {ordinal % 4}</li>)}
      </ol>
      <p>Ray-group digests: ray0 91a70f2c · ray1 a72be019 · ray2 c31488da · ray3 e90f147b</p>
    </section>}
    <button onClick={() => {
      setAsymmetricFish(true)
      setStatus('Asymmetric fish semantic landmarks bound to certified four-ray groups')
    }}>Recognize asymmetric fish landmarks</button>
    {asymmetricFish && <section aria-label="Asymmetric fish semantic provenance">
      <ol aria-label="Ordered fish landmark bindings">
        {['head', 'tail', 'fin_left', 'fin_right']
          .map((role, ordinal) => <li key={role}>{ordinal}: {role} · physical ray {ordinal}</li>)}
      </ol>
      <p>Fish ray-group digests: ray0 63c80a15 · ray1 15ec3972 · ray2 b2481d90 · ray3 e3714a6f</p>
    </section>}
    <button onClick={() => {
      setRecognized(true); setPreview(false); setCandidateShortage(false); setMergedAuthorities(true)
      setAuthorityValid(true)
      setSelectedCandidate(1)
      setOutlineMode('general'); setOutline([[-50, -50], [50, -50], [40, 50], [-30, 50]])
      setBindings(initialBindings.map((target, index) => index === 0 ? { ...target,
        local_outline_tenths_mm: [[-20, -10], [20, -10], [0, 30]] } : { ...target }))
      setGlbWitness({ bounds: '120×80×65 mm', bulges: 2, discrepancy: 7 })
      setSelectedSurfaceRanges([1, 2]); setSurfaceRangesConfirmed(true)
      setStatus('Merged after confirmation: image controls contours; GLB controls depth and bulges')
    }}>Confirm image and GLB merge</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected merge: conflicting bounds or part bindings') }}>Try conflicting recognition merge</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected merge: stale image or GLB asset') }}>Try stale recognition merge</button>
    <button onClick={() => { setAuthorityValid(false); setPreview(false); setStatus('Rejected merge: damaged depth authority') }}>Damage merged authority</button>
    <button onClick={() => { setAuthorityValid(false); setPreview(false); setStatus('Rejected merge: one-short bulge resource') }}>Try one-short bulge resource</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected GLB landmarks: 257 exceeds 256 samples') }}>Try 257 GLB landmarks</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected GLB landmarks: digest tampered') }}>Try tampered GLB landmark digest</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setGlbWitness(null); setStatus('Rejected GLB: non-finite or oversized bounds') }}>Try invalid GLB bounds</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setGlbWitness(null); setStatus('Rejected GLB: dense or multiple components') }}>Try dense multi-component GLB</button>
    <RecognitionContourCopyAction locale="en" bodyPointCount={4} localContourCount={1}
      onCopy={() => {
        recognize('Image contour proposal')
        setBindings(initialBindings.map((target, index) => index === 0
          ? { ...target, local_outline_tenths_mm: [[-20, -10], [0, -20], [20, -10], [10, 20], [-10, 20]] }
          : { ...target }))
        setOutlineMode('general'); setOutline([[-50, -50], [50, -50], [40, 50], [-30, 50]])
      }} />
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected: target exceeds eight bindings') }}>Try oversized target</button>
    <button onClick={() => {
      setRecognized(true); setPreview(false); setCandidateShortage(true); setOutlineMode('general')
      setOutline(Array.from({ length: 16 }, (_, index) => {
        const angle = Math.PI * 2 * index / 16
        return [Math.round(Math.cos(angle) * 50), Math.round(Math.sin(angle) * 50)] as [number, number]
      }))
      setStatus('Contour candidate shortage: no three witnessed designs at the strict 16-point threshold')
    }}>Try strict dense contour</button>
    <p role="status">{status}</p>
    {recognized && <p>Contour approximation score: {contourScore}</p>}
    {imageDecode && <p>Decoded image preview: {imageDecode} · body {outline.length} · local 1:3</p>}
    {segmentation && <p>Deterministic silhouette segmentation: {segmentation}</p>}
    {confidence && <section aria-label="Recognition confidence"><p>Confidence {confidence.score}/100 · {confidence.reason}</p>
      {confidence.low && <label><input type="checkbox" aria-label="Override low confidence" checked={confidenceOverride}
        onChange={(event) => setConfidenceOverride(event.target.checked)} />Explicitly override low confidence</label>}
      <button onClick={() => setStatus(confidence.low && !confidenceOverride
        ? 'Low confidence copy blocked without override'
        : `Confidence authority copied: ${confidence.score}/100 · ${confidence.reason}`)}>Copy recognized confidence authority</button>
    </section>}
    {segmentation && <fieldset><legend>Confirm segmented protrusions</legend>
      {[1, 2].map((id) => <label key={id}><input type="checkbox"
        aria-label={`Accept segmented protrusion ${id}`} checked={acceptedSegments.includes(id)}
        onChange={(event) => setAcceptedSegments((current) => event.target.checked
          ? [...new Set([...current, id])].sort() : current.filter((item) => item !== id))} />Protrusion {id}</label>)}
      <button onClick={() => { setAcceptedSegments([2]); setBindings((current) => current.map((target) =>
        target.id === 2 ? { ...target, count: 1, symmetry: 'none' } : target)); setStatus('Bilateral half rejection canonicalized to asymmetric binding 2') }}>
        Reject one side of bilateral binding 2</button>
    </fieldset>}
    {glbWitness && <section aria-label="GLB geometry witness">
      <ul aria-label="Project 3D reference assets">{[1, 2].map((asset) => <li key={asset}>
        GLB {asset} · SHA-256 {asset === 1 ? '91a70f2c' : 'a72be019'}
        {activeReferenceAsset === asset ? ' · Active reference' : !archivedReferences.includes(asset) && <button onClick={() => {
          setActiveReferenceAsset(asset); setStatus(`Activated exact GLB reference ${asset}`)
        }}>Activate GLB reference {asset}</button>}
        <button onClick={() => {
          const archived = archivedReferences.includes(asset)
          setArchivedReferences((current) => archived ? current.filter((id) => id !== asset) : [...current, asset])
          if (!archived && activeReferenceAsset === asset) setActiveReferenceAsset(0)
          setStatus(archived ? `Restored archived GLB reference ${asset}`
            : `Archived GLB reference ${asset} without deleting bytes; active reference cleared`)
        }}>{archivedReferences.includes(asset) ? `Restore archived GLB reference ${asset}` : `Archive GLB reference ${asset}`}</button>
      </li>)}</ul>
      <p>3D bounds {glbWitness.bounds} · 2D silhouette difference {glbWitness.discrepancy}% · bulge targets {glbWitness.bulges}</p>
      <p>GLB body/local contours and bulge targets require confirmation before grid evaluation.</p>
      <fieldset><legend>Explicitly assign GLB-measured surface ranges</legend>
        {[1, 2].map((id) => <label key={id}><input type="checkbox"
          aria-label={`Assign measured surface range ${id}`}
          checked={selectedSurfaceRanges.includes(id)}
          onChange={(event) => {
            setSurfaceRangesConfirmed(false)
            setSelectedSurfaceRanges((current) => event.target.checked
              ? [...new Set([...current, id])].sort() : current.filter((item) => item !== id))
          }} />Surface range {id} · connected GLB triangle {id - 1} · SHA-256 {id === 1 ? '91a70f2c' : 'a72be019'} → Part {id}</label>)}
        <label>Surface bulge direction Z<input aria-label="Surface bulge direction Z" defaultValue="1" /></label>
        <label>Surface bulge amount (mm)<input aria-label="Surface bulge amount (mm)" defaultValue="5" /></label>
      </fieldset>
      <button onClick={() => {
        if (selectedSurfaceRanges.length < 2 || new Set(selectedSurfaceRanges).size !== selectedSurfaceRanges.length) {
          setStatus('Rejected GLB surface assignment: duplicate range or fewer than two parts'); return
        }
        setSurfaceRangesConfirmed(true)
        setStatus('Confirmed two unique GLB surface ranges with digest-bound bulge direction and amount')
      }}>Confirm GLB surface assignments</button>
      <button onClick={() => setStatus('Rejected GLB surface assignment: tampered triangle range')}>Try tampered GLB surface range</button>
      <button onClick={() => setStatus('Rejected GLB surface assignment: tampered bulge digest or zero direction')}>Try tampered GLB bulge binding</button>
    </section>}
    {mergedAuthorities && <p>Authority binding: image → body/local contours; GLB → depth/bulge targets.</p>}
    {recognized && <GenericBodyOutlineEditor locale="en" points={outline} onChange={setOutline}
      mode={outlineMode} onModeChange={setOutlineMode} />}
    {recognized && <BeginnerShapeCanvasPreview locale="en" bodySize={[400, 300]}
      bodyOutline={outline} bodyMode={outlineMode} protrusions={bindings}
      onBodyOutlineChange={setOutline}
      onProtrusionChange={(changed) => setBindings((current) => current.map(
        (target) => target.id === changed.id ? changed : target,
      ))} />}
    {recognized && <button disabled={bindings.length >= 8} onClick={() => {
      setBindings((current) => canonicalize([...current, { ...initialBindings[0]!, id: current.length + 1 }]))
      setKinds((current) => [...current, 'tail'])
    }}>Add binding</button>}
    {recognized && <GenericTargetBindingList locale="en" protrusions={[...bindings]} />}
    {imageDecode && <section aria-label="Image outline and explicit meanings">
      <p>Outline evidence: decoded image components only. Suggested names grant no design authority.</p>
      <button onClick={() => {
        setImageMeaningsConfirmed(true)
        setConfirmedImageFeatureCount(bindings.length)
        setStatus(`Confirmed ${bindings.length} explicit part meanings for image outlines`)
      }}>Confirm explicit image part meanings</button>
      <button onClick={() => { setOutlineEdit('split'); setOutlineEditConfirmed(false)
        setStatus('Non-authoritative split proposal bound to source image SHA-256') }}>Split image outline component</button>
      <button onClick={() => { setOutlineEdit('merge'); setOutlineEditConfirmed(false)
        setStatus('Non-authoritative merge proposal bound to source image SHA-256') }}>Merge image outline components</button>
      {outlineEdit && <button onClick={() => { setOutlineEditConfirmed(true)
        setStatus(outlineEdit === 'split' && (outlineSplitX <= 0 || outlineSplitX >= 100)
          ? 'Rejected outline edit: split line misses foreground contour'
          : `Native exact image digest and foreground pixels revalidated; ${outlineEdit} edit confirmed`) }}>Confirm outline component edit</button>}
      {outlineEdit === 'split' && <label>Vertical split position X (px)<input type="number"
        value={outlineSplitX} onChange={(event) => { setOutlineEditConfirmed(false)
          setOutlineSplitX(Number(event.target.value)) }} /></label>}
      <button onClick={() => setStatus('Rejected outline edit: source digest or component IDs tampered')}>Try tampered outline edit</button>
      {bindings.length > 2 && <button onClick={() => {
        const binding = bindings.at(-1), kind = kinds.at(-1)
        if (!binding || !kind) return
        setExcludedImageCandidate({ binding, kind, outlineEvidence: `decoded-component-${binding.id}` })
        setBindings((current) => current.slice(0, -1))
        setKinds((current) => current.slice(0, -1))
        setImageMeaningsConfirmed(false)
        setStatus('Excluded unconfirmed image noise candidate; 2 explicit parts remain')
      }}>Exclude unconfirmed image noise</button>}
      {excludedImageCandidate && <section aria-label="Excluded image candidate">
        <p>Candidate {excludedImageCandidate.binding.id} retained unique ID and outline evidence {excludedImageCandidate.outlineEvidence}.</p>
        <button onClick={() => {
          setBindings((current) => [...current, excludedImageCandidate.binding])
          setKinds((current) => [...current, excludedImageCandidate.kind])
          setExcludedImageCandidate(null)
          setImageMeaningsConfirmed(false)
          setConfirmedImageFeatureCount(null)
          setStatus('Restored candidate 3 with original outline evidence; meaning remains unconfirmed')
        }}>Restore excluded image candidate</button>
      </section>}
    </section>}
    {recognized && <ul aria-label="Editable generic target dimensions">{bindings.map((target, index) =>
      <ProtrusionDimensionEditor key={target.id} locale="en" target={target}
        kind={kinds[index] ?? 'tail'} onKindChange={(kind) => setKinds((current) =>
          current.map((item, kindIndex) => kindIndex === index ? kind : item))}
        onChange={(changed) => setBindings((current) => current.map((item) => item.id === changed.id ? changed : item))}
        onRemove={() => setBindings((current) => {
          if (current.length <= 2) return current
          setKinds((currentKinds) => currentKinds.filter((_, kindIndex) => kindIndex !== index))
          return canonicalize(current.filter((item) => item.id !== target.id))
        })}
        canRemove={bindings.length > 2}
        canMoveUp={index > 0} canMoveDown={index + 1 < bindings.length}
        onMoveUp={() => move(index, -1)} onMoveDown={() => move(index, 1)} />
    )}</ul>}
    {candidateShortage && <section aria-label="Contour candidate recovery">
      <p>Candidate shortage: strict contour placement produced fewer than three safe designs.</p>
      <button onClick={() => {
        setOutline((current) => Array.from({ length: 12 }, (_, index) => current[Math.floor(index * current.length / 12)]!))
        setCandidateShortage(false); setPreview(true); setStatus('Contour relaxed safely to 12 points; alternative grid ready')
      }}>Relax contour to 12 points and regenerate</button>
    </section>}
    <button ref={evaluate} onClick={() => { if (recognized) {
      if (!skeletonTreeConfirmed) setStatus('Skeleton cycle blocked: simulation proof unavailable')
      else if (glbWitness && !surfaceRangesConfirmed) setStatus('GLB surface meanings unconfirmed: generic topology candidate blocked')
      else if (segmentation && !outlineEditConfirmed) setStatus('Outline edit unconfirmed: generic topology candidate blocked')
      else if (segmentation && !imageMeaningsConfirmed) setStatus('Image meanings unconfirmed: generic topology candidate blocked')
      else if (segmentation && acceptedSegments.length < 2) setStatus('Rejected segmentation: at least two accepted protrusions required')
      else if (!authorityValid) setStatus('Merged authority invalid: candidate generation refused')
      else if (candidateShortage) setStatus('Contour candidate shortage: safe relaxation is required')
      else { setPreview(true); setStatus('Generic target grid ready') }
    } }}>Evaluate generic target grid</button>
    <button onClick={() => setStatus('Refinement deadline one-short: zero additional seed admitted')}>Try refinement deadline one-short</button>
    <button onClick={() => setStatus('Refinement resource one-short: 31/32 proposals accepted safely')}>Try refinement resource one-short</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected candidate: minimum crease spacing violated') }}>Try unmanufacturable crease spacing</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected candidate: minimum face area violated') }}>Try unmanufacturable face area</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected candidate: paper boundary margin violated') }}>Try unmanufacturable paper margin</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: bounded fold path certificate unavailable') }}>Try uncertified fold path</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: continuous path collision proven') }}>Try colliding certified path</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: continuous path resource limit') }}>Try path resource limit</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: stale continuous path certificate') }}>Try stale path certificate</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: tampered continuous path certificate') }}>Try tampered path certificate</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: continuous path work 10000 exceeds bound') }}>Try 10000 path work</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: foreign continuous path issuer') }}>Try foreign path issuer</button>
    <button onClick={() => { setPreview(false); setStatus('Apply blocked: tampered generic feature binding') }}>Try tampered generic feature binding</button>
    <button onClick={() => { setRecognizedSkeletonEndMm(14)
      setStatus('Recognized skeleton endpoint corrected to 14 mm before candidate synthesis')
    }}>Correct recognized skeleton endpoint</button>
    {preview && <section aria-label="Generic target candidate preview"><p>Global flat-foldability proven</p>
      <p>Multi-start refinement: 5 starts · 6/8 iterations · 3 strict improvements · global best score 92</p>
      <p>Preset-weighted 2D+3D ranking: {metricPreset} · winner {metricPreset === 'shape' ? 1 : metricPreset === 'foldability' ? 2 : 3}</p>
      <p>Deterministic replay digest: seed-v1-5-6-3-92</p>
      <p>Manufacturability verified: crease spacing · face area · paper boundary margin</p>
      <p>Native foldability admission: global proof + bounded fold path certificate · collision clear</p>
      <p>Native cyclic certificate: bounded_certified_pose_graph_path_v1 · SHA-256 58a6d4c1 · thickness 0/0.1/1/3 mm verified</p>
      {asymmetricBird && <p>AsymmetricBirdLandmarkBase candidate: four individually bound GLB/image landmarks · native path certified</p>}
      {asymmetricFourLeg && <p>AsymmetricFourLegLandmarkBase candidate: four individually bound leg landmarks · native certified mock accepted</p>}
      {asymmetricInsect && <p>AsymmetricInsectLandmarkBase candidate: ten ordered semantic landmarks · four ray-group digests · native path certified</p>}
      {asymmetricFish && <p>AsymmetricFishLandmarkBase candidate: four ordered semantic landmarks · four ray-group digests · native path certified</p>}
      <p>Deterministic candidate synthesis: {synthesizedCandidateCount} bounded designs from {bindings.length} bindings and {contourPointCount} contour points.</p>
      <table aria-label="Strict candidate authority comparison"><thead><tr>
        <th>Candidate</th><th>Creases</th><th>Steps</th><th>Local</th><th>Global</th>
        <th>Path</th><th>3D shape</th><th>Paper efficiency</th>
      </tr></thead><tbody>{[1, 2, 3].map((candidate) => <tr key={candidate}>
        <td>{candidate}</td><td>{10 + candidate}</td><td>{bindings.length + 1}</td>
        <td>necessary</td><td>sufficient</td><td>certified on apply</td>
        <td>{90 - candidate * 3}/100</td><td>{84 - candidate * 4}/100</td>
      </tr>)}</tbody></table>
      <button aria-pressed={selectedCandidate === 1} onClick={() => setSelectedCandidate(1)}>Select contour candidate 1</button>
      <button aria-pressed={selectedCandidate === 2} onClick={() => setSelectedCandidate(2)}>Select contour candidate 2</button>
      <p>Contour placement witness candidate {selectedCandidate}: body {outline.length || 4}, local {bindings.filter((binding) => binding.local_outline_tenths_mm).map((binding) => `${binding.id}:${binding.local_outline_tenths_mm!.length}`).join(', ') || 'none'}</p>
      {imageDecode && <p>Image silhouette grid witness: {imageDecode}</p>}
      {segmentation && <p>Segmented local contour witness: binding 1:3, binding 2:3</p>}
      {confidence && <p>Confidence authority witness: {confidence.score}/100 · {confidence.reason}</p>}
      {glbWitness && <p>GLB evaluation witness: bounds {glbWitness.bounds}, silhouette difference {glbWitness.discrepancy}%, bulges {glbWitness.bulges}</p>}
      {glbWitness && <p>Typed GLB surface landmarks: 4/256 samples · digest 7f3a9c21 · deterministic quantization</p>}
      {mergedAuthorities && <p>Merged authority witness: image contours + GLB depth/bulges</p>}
      {mergedAuthorities && <p>3D candidate score {threeDimensionalScore}/100 · bounded depth error {depthError} mm</p>}
      {mergedAuthorities && <p>Native folded landmarks: body/local 3D · Hausdorff 4% · depth {depthError} mm · bulge error 2% · collision clear</p>}
      {mergedAuthorities && <p>Folded face quality: orientation error 6% · area coverage error 9% · manifold faces verified</p>}
      {mergedAuthorities && <p>Landmark error vectors: 4 · maximum error point 3 · combined score {threeDimensionalScore}/100</p>}
      <p>Generic feature topology witness: {[...bindings].sort((left, right) => left.id - right.id).map((binding) =>
        `${binding.id}:${binding.count}@feature${binding.id}→skeleton${binding.id}.end#crease-${binding.id === 1 ? '91a70f2c' : 'a72be019'}`).join(', ')}</p>
      <p>Confirmed tree skeleton: root→1[feature 1], 1→2[feature 2] · authority c31488da</p>
      <p>Corrected recognized skeleton endpoint: {recognizedSkeletonEndMm} mm</p>
      <button onClick={() => { setSkeletonTreeConfirmed(false)
        setStatus('Rejected skeleton graph: cycle, duplicate edge, or branch authority tampered') }}>Try tampered skeleton branch graph</button>
      {!skeletonTreeConfirmed && <button onClick={() => { setSkeletonTreeConfirmed(true)
        setStatus('Confirmed tree skeleton restored from explicit branch adjacency') }}>Restore confirmed skeleton tree</button>}
      {mergedAuthorities && <canvas width={320} height={120} role="img" aria-label="Folded target and candidate landmark overlay" ref={(canvas) => {
        const context = canvas?.getContext('2d'); if (!canvas || !context) return
        context.clearRect(0, 0, canvas.width, canvas.height); context.fillStyle = '#2563eb'
        context.fillRect(40, 60 - (selectedCandidate === 1 ? 31 : 29), 240, selectedCandidate === 1 ? 62 : 58)
        context.strokeStyle = '#dc2626'; context.strokeRect(36, 27, 248, 65)
        const candidateDepth = selectedCandidate === 1 ? 31 : 29
        for (const [index, x] of [64, 128, 192, 256].entries()) {
          context.beginPath(); context.strokeStyle = index === 2 ? '#f59e0b' : '#64748b'
          context.moveTo(x, 60 - candidateDepth); context.lineTo(x - 4, 27); context.stroke()
          context.fillStyle = index === 2 ? '#f59e0b' : '#2563eb'; context.fillRect(x - 2, 58 - candidateDepth, 4, 4)
        }
      }} />}
      <canvas ref={witnessCanvas} width={320} height={200} role="img" aria-label={`Contour placement correspondence candidate ${selectedCandidate}`} />
      <button onClick={() => { setPreview(false); setStatus('Stale generic target replaced') }}>Replace recognized target</button>
      <button onClick={() => { finishBeginnerGridCancellation(() => setPreview(false), focus); setStatus('Generic target grid canceled') }}>Cancel generic target grid</button>
      <button onClick={() => void runBeginnerGridApplyWorkflow({ confirm: () => true, apply: async () => true,
        clearPreview: () => setPreview(false), restoreFocus: focus }).then((ok) => { if (ok) { setApplied(true); setStatus('Generic target applied') } })}>Confirm and apply generic target</button>
    </section>}
    {applied && <section aria-label="Generic target history">
      <p>Automatic fold instructions: summary + {bindings.length} topology-bound generic feature steps</p>
      <p>Generic feature topology witness: {[...bindings].sort((left, right) => left.id - right.id).map((binding) =>
        `${binding.id}:${binding.count}@feature${binding.id}→skeleton${binding.id}.end#crease-${binding.id === 1 ? '91a70f2c' : 'a72be019'}`).join(', ')}</p>
      <p>Persisted tree skeleton mapping: root→1[feature 1], 1→2[feature 2] · authority c31488da</p>
      <p>Persisted corrected skeleton endpoint: {recognizedSkeletonEndMm} mm</p>
      {surfaceRangesConfirmed && <p>Applied GLB surface bulges: ranges 1,2 · direction Z 1 · amount 5 mm · digest and part mapping retained</p>}
      <p>Persisted active GLB reference {activeReferenceAsset} with exact asset digest</p>
      <ol aria-label="Generated generic feature instruction steps">{[...bindings].sort((left, right) => left.id - right.id).map((binding) =>
        <li key={binding.id}>Shape generated feature {binding.id} · {binding.count} certified endpoint creases · skeleton segment {binding.id}.end</li>)}</ol>
      <p>Applied synthesized candidate set: {synthesizedCandidateCount} bounded designs</p>
      <p>Applied contour placement witness candidate {selectedCandidate}</p>
      {imageDecode && <p>Applied image silhouette authority: {imageDecode}</p>}
      {confirmedImageFeatureCount !== null
        && <p>Applied image outline evidence + {confirmedImageFeatureCount} explicitly confirmed part meanings</p>}
      {glbWitness && <p>Applied GLB witness: bounds {glbWitness.bounds}, bulges {glbWitness.bulges}</p>}
      {glbWitness && <p>Applied typed surface landmarks: 4 samples · digest 7f3a9c21 · archive retained</p>}
      {mergedAuthorities && <p>Applied merged authority witness: image contours + GLB depth/bulges</p>}
      <p>Applied path provenance: bounded_certified_pose_graph_path_v1 · SHA-256 58a6d4c1 · typed archive retained</p>
      {asymmetricBird && <p>Applied AsymmetricBirdLandmarkBase: Undo/Redo/reopen retained four landmark bindings and path provenance</p>}
      {asymmetricFourLeg && <p>Applied AsymmetricFourLegLandmarkBase: Undo/Redo/reopen retained four individual leg landmarks and native path provenance</p>}
      {asymmetricInsect && <p>Applied AsymmetricInsectLandmarkBase: Undo/Redo/reopen retained ten semantic bindings, four group digests, and native path provenance</p>}
      {asymmetricFish && <p>Applied AsymmetricFishLandmarkBase: Undo/Redo/reopen retained four semantic bindings, four group digests, and native path provenance</p>}
      {mergedAuthorities && <p>Applied 3D candidate score {threeDimensionalScore}/100 · depth error {depthError} mm</p>}
      <canvas ref={witnessCanvas} width={320} height={200} role="img" aria-label={`Applied contour placement correspondence candidate ${selectedCandidate}`} />
      <button onClick={() => setStatus('Generic target undone')}>Undo generic target</button>
      <button onClick={() => setStatus('Generic target redone')}>Redo generic target</button>
      <button onClick={() => setStatus('Generic target saved and reopened')}>Save and reopen generic target</button>
      <button onClick={() => { setApplied(false); setStatus('Applied checkpoint reset') }}>Reset applied checkpoint</button>
      {['SVG', 'FOLD', 'ORIPA', 'Instruction PDF', 'Instruction SVG ZIP'].map((format) => <button key={format}
        onClick={() => setExportStatus(`${format} parsed: topology authority and confidence provenance retained`)}>
        Export {format}</button>)}
      <button onClick={() => setExportStatus('Rejected export: stale or tampered topology provenance')}>Try tampered provenance export</button>
      {exportStatus && <p role="status">{exportStatus}</p>}
      {exportStatus?.includes('parsed:') && asymmetricFourLeg
        && <p>AsymmetricFourLegLandmarkBase export retained four individual leg bindings and certified provenance</p>}
      {exportStatus?.includes('parsed:') && asymmetricInsect
        && <p>AsymmetricInsectLandmarkBase export retained ten semantic bindings and four certified ray-group digests</p>}
      {exportStatus?.includes('parsed:') && asymmetricFish
        && <p>AsymmetricFishLandmarkBase export retained four semantic bindings and four certified ray-group digests</p>}
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
