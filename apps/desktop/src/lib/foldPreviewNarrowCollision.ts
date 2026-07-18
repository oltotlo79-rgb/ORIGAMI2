import { Matrix4, Vector3 } from 'three'
import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
  calculateFoldPreviewBroadPhaseNumericalMargin,
  findFoldPreviewPoseBroadPhaseCandidates,
  type FoldPreviewBroadPhaseCandidate,
  type FoldPreviewBroadPhaseResult,
  type FoldPreviewCollisionAdjacency,
  type FoldPreviewCollisionPoseFace,
} from './foldPreviewCollision.ts'
import {
  triangulateFoldPreviewPolygon,
  type FoldPreviewTriangleIndices,
} from './foldPreviewGeometry.ts'
import {
  prepareFoldPreviewHingeContactPolicy,
  type FoldPreviewHingeContactConstraint,
  type FoldPreviewHingeContactDecision,
  type FoldPreviewHingeContactPair,
  type FoldPreviewHingeContactPolicy,
} from './foldPreviewHingeCollision.ts'
import {
  deriveFoldPreviewTrianglePrismWitness,
  type FoldPreviewTrianglePrismWitness,
  type FoldPreviewWitnessFrame,
} from './foldPreviewNarrowCollisionWitness.ts'
import {
  provesFoldPreviewBinary64TransversalTriangleIntersection,
  provesFoldPreviewBinary64SharedVertexOnlyIntersection,
} from './foldPreviewExactTriangleIntersection.ts'
import {
  classifyFoldPreviewTopologyContact,
  FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
} from './foldPreviewTopologyContactPolicy.ts'

export const MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS = 1_000_000
/**
 * Maximum BigInt binary64 intersection-certificate requests per immutable pose
 * analysis. The public work-field name remains versioned for compatibility;
 * transversal and shared-feature-only certificates share this one cap.
 */
export const MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS = 256
/** Bounds synchronous deep-copy and triangulation during preview setup. */
export const MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES = 100_000
/** Bounds explanatory derivation work independently of collision classification. */
export const MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES = 16
export const MAX_FOLD_PREVIEW_TOPOLOGY_CONTACT_VERTEX_IDS = 16

const SAT_MARGIN_FACTOR = 4
const PAIR_WORLD_ROUNDING_MARGIN_FACTOR = 32
const PAIR_WORLD_CONTACT_ZERO_ULP_FACTOR = 2
const PARALLEL_AXIS_TOLERANCE = Number.EPSILON * 128
const RIGID_TRANSFORM_TOLERANCE = 1e-10
/**
 * A shared-vertex singleton is suppressible only while the two material
 * mid-surface orientations are affirmatively co-oriented.  This is a
 * dimensionless proof threshold, not an angular or thickness allowance.
 */
const SHARED_VERTEX_COORIENTED_NORMAL_DOT_TOLERANCE =
  RIGID_TRANSFORM_TOLERANCE
const MAX_FOLD_PREVIEW_FULL_SCAN_JOB_WORK_UNITS =
  MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
  + MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
const MAX_FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_WORK_UNITS =
  MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
  + MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES

export type FoldPreviewExactTransversalProofWork = Readonly<{
  algorithm: 'binary64_transversal_triangle_intersection_v1'
  maximumAttempts:
    typeof MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
  /** Exact certificate requests entered; this never exceeds maximumAttempts. */
  attempted: number
  /** Requests left unproved after the cap; every such pair is indeterminate. */
  skippedByLimit: number
}>

/**
 * Returns the exact SAT/hinge-policy margin for an upper bound on all absolute
 * world coordinates in a pose.
 */
export function calculateFoldPreviewNarrowPhaseNumericalMargin(
  coordinateScale: number,
): number | null {
  const broadPhaseMargin =
    calculateFoldPreviewBroadPhaseNumericalMargin(coordinateScale)
  if (broadPhaseMargin === null) return null
  const margin = broadPhaseMargin * SAT_MARGIN_FACTOR
  return Number.isFinite(margin) ? margin : null
}

export type FoldPreviewNarrowPhaseInteraction = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  hingeEdgeIds: readonly string[]
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
  topologyContact?: FoldPreviewTopologyContactSummary
  hingeDecision?: FoldPreviewHingeContactDecision
}>

export type FoldPreviewTopologyContactSummary = Readonly<{
  policyVersion: typeof FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
  decision: 'allowed_shared_vertex_contact'
  /** True only when every non-separated triangle pair in this face pair is allowed. */
  exclusive: boolean
  sharedVertexIds: readonly string[]
  omittedSharedVertexIdCount: number
  featureContactPairCount: number
  thicknessOverlapPairCount: number
  rawTouchingPairCount: number
  rawPenetratingPairCount: number
  rawIndeterminatePairCount: number
}>

const trustedAllowedSharedVertexInteractions = new WeakSet<object>()
const trustedSharedVertexGeometryCertificates = new WeakSet<object>()
const trustedSharedVertexGeometryProvenance =
  new WeakMap<object, FoldPreviewSharedVertexGeometryProvenance>()
const trustedSharedVertexRuntimeEvidence = new WeakSet<object>()
const trustedSharedVertexRuntimeProvenance =
  new WeakMap<object, FoldPreviewSharedVertexRuntimeProvenance>()
const trustedHingeRuntimeEvidence = new WeakSet<object>()
const trustedHingeRuntimeProvenance =
  new WeakMap<object, FoldPreviewHingeRuntimeProvenance>()
const weakSetHasIntrinsic = WeakSet.prototype.has
const weakSetAddIntrinsic = WeakSet.prototype.add
const weakMapGetIntrinsic = WeakMap.prototype.get
const weakMapSetIntrinsic = WeakMap.prototype.set
const reflectApplyIntrinsic = Reflect.apply

/**
 * Validates the public diagnostic before any caller suppresses a collision
 * stop. Only an exact interaction snapshot issued by this analyzer can carry
 * the allowance; clones, malformed values, and partial summaries fail closed.
 */
export function isFoldPreviewExclusiveAllowedSharedVertexContact(
  interaction: FoldPreviewNarrowPhaseInteraction,
) {
  try {
    if (
      typeof interaction !== 'object'
      || interaction === null
      || !reflectApplyIntrinsic(
        weakSetHasIntrinsic,
        trustedAllowedSharedVertexInteractions,
        [interaction],
      )
    ) return false
    return validateExclusiveAllowedSharedVertexContact(interaction)
  } catch {
    return false
  }
}

function validateExclusiveAllowedSharedVertexContact(
  interaction: FoldPreviewNarrowPhaseInteraction,
) {
  const relation = interaction.relation
  const geometryClass = interaction.geometryClass
  const value = interaction.topologyContact
  if (
    relation !== 'non_adjacent'
    || geometryClass !== 'touching'
    || !value
    || value.policyVersion
      !== FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
    || value.decision !== 'allowed_shared_vertex_contact'
    || value.exclusive !== true
  ) return false
  const sharedVertexIds = value.sharedVertexIds
  if (!Array.isArray(sharedVertexIds)) return false
  const sharedVertexIdCount = sharedVertexIds.length
  if (
    !Number.isSafeInteger(sharedVertexIdCount)
    || sharedVertexIdCount === 0
    || sharedVertexIdCount
      > MAX_FOLD_PREVIEW_TOPOLOGY_CONTACT_VERTEX_IDS
  ) return false
  const seenIds = new Set<string>()
  for (let index = 0; index < sharedVertexIdCount; index += 1) {
    const id = sharedVertexIds[index]
    if (!validId(id) || seenIds.has(id)) return false
    seenIds.add(id)
  }
  const omittedSharedVertexIdCount = value.omittedSharedVertexIdCount
  const featureContactPairCount = value.featureContactPairCount
  const thicknessOverlapPairCount = value.thicknessOverlapPairCount
  const rawTouchingPairCount = value.rawTouchingPairCount
  const rawPenetratingPairCount = value.rawPenetratingPairCount
  const rawIndeterminatePairCount = value.rawIndeterminatePairCount
  const counts = [
    omittedSharedVertexIdCount,
    featureContactPairCount,
    thicknessOverlapPairCount,
    rawTouchingPairCount,
    rawPenetratingPairCount,
    rawIndeterminatePairCount,
  ]
  if (counts.some((count) => !Number.isSafeInteger(count) || count < 0)) {
    return false
  }
  const pairCount = featureContactPairCount + thicknessOverlapPairCount
  return Number.isSafeInteger(pairCount)
    && pairCount > 0
    && rawTouchingPairCount
      + rawPenetratingPairCount
      + rawIndeterminatePairCount
      === pairCount
}

export type FoldPreviewNarrowPhaseWitnessSample = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'non_adjacent'
  firstTriangleIndex: number
  secondTriangleIndex: number
  geometryClass: 'touching' | 'penetrating'
  witness: FoldPreviewTrianglePrismWitness
}>

export type FoldPreviewNarrowPhaseWitnessCoverage = Readonly<{
  scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1'
  /** Definitive tested pairs matching their final face-interaction severity. */
  eligiblePairCount: number
  /** Eligible pairs submitted to the bounded witness derivation helper. */
  attemptedPairCount: number
  /** Attempted pairs for which conservative witness derivation returned null. */
  unavailablePairCount: number
  /** Eligible pairs not submitted because the independent limit was reached. */
  omittedByLimitCount: number
  /**
   * False when an authoritative non-adjacent scan stopped at its first
   * penetration, or when neither the positive-thickness prism classifier nor
   * the zero-thickness surface classifier scanned a non-adjacent pair.
   */
  authoritativePairScanComplete: boolean
}>

export type FoldPreviewFullScanNonAdjacentWitnessCoverage<
  ConstraintsRepresented extends boolean = boolean,
> = Readonly<{
  scope: 'all_broad_phase_non_adjacent_triangle_pairs_full_scan_v2'
  /** Non-adjacent face pairs not rejected by the conservative broad phase. */
  broadPhaseCandidateCount: number
  /** Every triangle pair belonging to those face pairs. */
  expectedTrianglePairCount: number
  /** Every triangle pair actually visited by the no-early-exit loop. */
  trianglePairTests: number
  /** Triangle pairs rejected by their exact world-space prism AABBs. */
  aabbRejectedPairCount: number
  /** Triangle pairs submitted to the authoritative prism SAT classifier. */
  satTests: number
  satSeparatedPairCount: number
  /** Topology-certified shared-vertex pairs excluded from collision witnesses. */
  allowedSharedVertexPairCount: number
  touchingPairCount: number
  penetratingPairCount: number
  indeterminatePairCount: number
  /** All definitive touching and penetrating pairs, regardless of face severity. */
  eligiblePairCount: number
  /** Eligible pairs submitted to the independently bounded witness helper. */
  attemptedPairCount: number
  /** Attempted pairs with a conservative witness result. */
  availablePairCount: number
  /** Attempted pairs for which conservative witness derivation returned null. */
  unavailablePairCount: number
  /** Eligible pairs not attempted because the independent limit was reached. */
  omittedByLimitCount: number
  /** Always true for a returned v2 result; work-limit failures return null. */
  authoritativePairScanComplete: true
  /**
   * True only when no pair is indeterminate and every definitive collision
   * pair has an available witness in the returned complete variant.
   */
  allCollisionConstraintsRepresented: ConstraintsRepresented
}>

export type FoldPreviewFullScanNonAdjacentWitnessUnavailableReason =
  | 'indeterminate_pair'
  | 'witness_limit_exceeded'
  | 'witness_derivation_failed'

type FoldPreviewFullScanNonAdjacentWitnessBase = Readonly<{
  algorithm: 'full_non_adjacent_prism_witness_scan_v2'
  sourcePose: 'analyzed_input_pose'
  requestIdentityBound: false
  collisionThickness: number
  numericalMargin: number
  exactTransversalProofWork: FoldPreviewExactTransversalProofWork
  /**
   * This is diagnostic geometry only. It is not a legal origami movement and
   * must be rebound to an immutable pose before any later verification.
   */
  autoApplicable: false
}>

export type FoldPreviewFullScanNonAdjacentWitnessSet =
  | (FoldPreviewFullScanNonAdjacentWitnessBase & Readonly<{
      kind: 'complete'
      coverage: FoldPreviewFullScanNonAdjacentWitnessCoverage<true>
      witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
    }>)
  | (FoldPreviewFullScanNonAdjacentWitnessBase & Readonly<{
      kind: 'unavailable'
      coverage: FoldPreviewFullScanNonAdjacentWitnessCoverage<false>
      reasons: readonly FoldPreviewFullScanNonAdjacentWitnessUnavailableReason[]
      /** Partial samples are intentionally withheld from correction callers. */
      witnessSamples: readonly never[]
    }>)

export const FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION =
  'full_non_adjacent_prism_witness_scan_job_v1'

export type FoldPreviewFullScanNonAdjacentWitnessJobWork = Readonly<{
  /**
   * Counts only resumable work. Transform snapshotting, broad phase, and prism
   * preparation happen synchronously in the job factory and are not included.
   */
  totalWorkUnits: number
  /** One visited triangle-prism pair consumes one work unit. */
  trianglePairTests: number
  /**
   * One selected witness attempt consumes one work unit, including an attempt
   * rejected before the witness helper because its prepared frame is absent.
   */
  witnessDerivations: number
}>

export type FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds = Readonly<{
  /** Exact pair visits required after synchronous factory preparation. */
  expectedTrianglePairCount: number
  /** At most one derivation per eligible pair, capped by the sample limit. */
  maximumWitnessDerivations: number
  /** Finite upper bound for all resumable work after factory preparation. */
  maximumTotalWorkUnits: number
}>

export type FoldPreviewFullScanNonAdjacentWitnessJobStep =
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      kind: 'pending'
      phase: 'triangle_pair_scan' | 'witness_derivation'
      work: FoldPreviewFullScanNonAdjacentWitnessJobWork
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      kind: 'complete'
      result: FoldPreviewFullScanNonAdjacentWitnessSet
      work: FoldPreviewFullScanNonAdjacentWitnessJobWork
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      kind: 'indeterminate'
      reason:
        | 'invalid_work_budget'
        | 'scan_error'
        | 'work_accounting_error'
      work: FoldPreviewFullScanNonAdjacentWitnessJobWork
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      kind: 'cancelled'
      work: FoldPreviewFullScanNonAdjacentWitnessJobWork
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>

export type FoldPreviewFullScanNonAdjacentWitnessJob = Readonly<{
  /**
   * Performs at most `workBudget` resumable units. One triangle-prism pair
   * visit (including an AABB rejection) or one selected witness derivation is
   * one unit.
   *
   * This does not bound the whole calling frame: transform snapshotting, broad
   * phase, and prism construction are still synchronous in the factory.
   */
  step(
    workBudget: number,
  ): FoldPreviewFullScanNonAdjacentWitnessJobStep
  /** Immutable finite bounds known after synchronous factory preparation. */
  workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
  cancel(): void
}>

export type FoldPreviewNarrowPhaseResult = Readonly<{
  broadPhaseCandidates: number
  broadPhaseNonAdjacentCandidates: number
  broadPhaseHingeAdjacentCandidates: number
  interactions: readonly FoldPreviewNarrowPhaseInteraction[]
  trianglePairTests: number
  satTests: number
  numericalMargin: number
  exactTransversalProofWork: FoldPreviewExactTransversalProofWork
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
  witnessCoverage: FoldPreviewNarrowPhaseWitnessCoverage
}>

export const FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION =
  'narrow_phase_sat_witness_cursor_job_v1'

export type FoldPreviewNarrowPhaseAnalysisJobWork = Readonly<{
  totalWorkUnits: number
  /** One visited triangle-prism pair, including an AABB rejection, is one. */
  trianglePairTests: number
  /** One selected witness attempt, including a conservative failure, is one. */
  witnessDerivations: number
}>

export type FoldPreviewNarrowPhaseAnalysisJobWorkBounds = Readonly<{
  /** This first-stage cursor contract never claims a wall-clock step bound. */
  entireStepTimeBounded: false
  /** Transform snapshot, broad phase, and prism construction are synchronous. */
  synchronousFactoryPreparation: true
  /** Hinge policy finalization remains synchronous after every pair scan. */
  synchronousHingePolicyFinalization: true
  /** Complete-result snapshotting and deep freezing remain synchronous. */
  synchronousResultFinalization: true
  /**
   * Exact pair count if every broad-phase candidate were exhaustively scanned.
   * The authoritative scan can visit fewer pairs because penetrations stop
   * eligible candidates early.
   */
  potentialTrianglePairCount: number
  /** Exact maximum charged pair visits after applying the global safety cap. */
  maximumTrianglePairTests: number
  /** Exact structural upper bound for selected non-adjacent witness attempts. */
  maximumWitnessDerivations: number
  /** Finite upper bound for SAT/witness cursor units after factory preparation. */
  maximumTotalWorkUnits: number
}>

export type FoldPreviewNarrowPhaseAnalysisJobStep =
  | Readonly<{
      version: typeof FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
      kind: 'pending'
      phase: 'triangle_pair_scan' | 'witness_derivation'
      work: FoldPreviewNarrowPhaseAnalysisJobWork
      workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>
  | Readonly<{
      version: typeof FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
      kind: 'complete'
      result: FoldPreviewNarrowPhaseResult
      work: FoldPreviewNarrowPhaseAnalysisJobWork
      workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>
  | Readonly<{
      version: typeof FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
      kind: 'indeterminate'
      reason:
        | 'invalid_work_budget'
        | 'work_limit_exceeded'
        | 'scan_error'
        | 'work_accounting_error'
      work: FoldPreviewNarrowPhaseAnalysisJobWork
      workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>
  | Readonly<{
      version: typeof FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
      kind: 'cancelled'
      work: FoldPreviewNarrowPhaseAnalysisJobWork
      workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
      exactTransversalProofWork: FoldPreviewExactTransversalProofWork
    }>

export type FoldPreviewNarrowPhaseAnalysisJob = Readonly<{
  /**
   * Advances at most `workBudget` resumable cursor units.
   *
   * This is not a whole-frame wall-clock bound. Transform snapshotting, broad
   * phase and prism construction happen synchronously in the factory. Both
   * zero- and positive-thickness triangle pairs use the resumable cursor. A
   * pair unit which completes a
   * hinge-adjacent candidate can also synchronously run the hinge-contact
   * policy outside the metered cursor work, and a witness unit runs its
   * derivation helper to completion. Complete-result snapshotting and deep
   * freezing are also synchronous. A true frame-time bound therefore requires
   * later resumable hinge-policy and result-finalization stages.
   */
  step(workBudget: number): FoldPreviewNarrowPhaseAnalysisJobStep
  /** Exact immutable bounds known after synchronous factory preparation. */
  workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
  cancel(): void
}>

export type FoldPreviewNarrowPhaseAnalyzer = Readonly<{
  /**
   * Synchronously drains createAnalysisJob(). This preserves the legacy
   * result and traversal order but does not provide a frame-time bound.
   */
  analyze(
    faceTransforms: ReadonlyMap<string, Matrix4>,
    thickness: number,
  ): FoldPreviewNarrowPhaseResult | null
  /**
   * Creates a resumable authoritative SAT/witness cursor. Transform
   * snapshotting, broad phase, and prism preparation remain synchronous
   * factory work; zero-thickness surface classification is cursor work.
   */
  createAnalysisJob(
    faceTransforms: ReadonlyMap<string, Matrix4>,
    thickness: number,
  ): FoldPreviewNarrowPhaseAnalysisJob | null
  /**
   * Creates a resumable no-early-exit scan of every non-adjacent triangle pair
   * admitted by the broad phase. Transform snapshotting, broad phase, and prism
   * construction are synchronous factory work in this first-stage API; only
   * triangle-pair classification and witness derivation are resumable.
   */
  createFullScanNonAdjacentWitnessSetJob(
    faceTransforms: ReadonlyMap<string, Matrix4>,
    thickness: number,
  ): FoldPreviewFullScanNonAdjacentWitnessJob | null
  /**
   * Synchronously drains createFullScanNonAdjacentWitnessSetJob(). This keeps
   * the legacy output and ordering but does not provide a frame-time bound.
   *
   * Positive thickness is required. A returned unavailable result is still a
   * complete classification scan, but must not seed a global correction.
   */
  collectFullScanNonAdjacentWitnessSet(
    faceTransforms: ReadonlyMap<string, Matrix4>,
    thickness: number,
  ): FoldPreviewFullScanNonAdjacentWitnessSet | null
}>

type PreparedFoldPreviewNarrowPhaseFace = Readonly<{
  id: string
  polygon: FoldPreviewCollisionPoseFace['polygon']
  triangles: readonly FoldPreviewTriangleIndices[]
}>

type FoldPreviewNarrowPhaseFace = Readonly<{
  id: string
  polygon: FoldPreviewCollisionPoseFace['polygon']
  triangles?: readonly FoldPreviewTriangleIndices[]
}>

type FoldPreviewTopologyPoseMismatch = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  hingeEdgeIds: readonly string[]
}>

type TrianglePrism = Readonly<{
  triangleIndex: number
  /** Authoritative transformed material mid-surface used for topology proofs. */
  midSurfaceVertices: readonly Vector3[]
  vertices: readonly Vector3[]
  topologyVertices: readonly Readonly<{
    vertexId: string | null
    x: number
    z: number
  }>[]
  faceAxes: readonly Vector3[]
  edgeDirections: readonly Vector3[]
  zeroThickness: boolean
  witnessFrame: FoldPreviewWitnessFrame | null
  bounds: Readonly<{
    minX: number
    minY: number
    minZ: number
    maxX: number
    maxY: number
    maxZ: number
  }>
}>

type PrismIntersection = 'separated' | 'touching' | 'penetrating' | 'indeterminate'

type FoldPreviewAllowedSharedVertexPair = Readonly<{
  policyVersion: typeof FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
  topology: 'shared_vertex'
  evidence: 'shared_feature_contact' | 'shared_feature_thickness_overlap'
  decision: 'allowed_shared_vertex_contact'
  sharedVertexId: string
}>

/**
 * Private proof capability issued only at the exact-geometry call site.
 * The public string policy table cannot construct or forge this identity.
 */
type FoldPreviewCertifiedSharedVertexGeometry = Readonly<{
  policyVersion: typeof FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
  topology: 'shared_vertex'
  proof:
    'binary64_shared_vertex_only_with_cooriented_material_normals_v1'
  sharedVertexId: string
}>

type FoldPreviewSharedVertexGeometryProvenance = Readonly<{
  first: TrianglePrism
  second: TrianglePrism
  thicknessClass: 0 | 1
}>

type FoldPreviewCertifiedSharedVertexRuntimeEvidence = Readonly<{
  policyVersion: typeof FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
  topology: 'shared_vertex'
  evidence: 'shared_feature_contact' | 'shared_feature_thickness_overlap'
  proof: 'certified_shared_vertex_runtime_evidence_v1'
  sharedVertexId: string
}>

type FoldPreviewSharedVertexRuntimeProvenance = Readonly<{
  geometryCertificate: FoldPreviewCertifiedSharedVertexGeometry
  first: TrianglePrism
  second: TrianglePrism
  thicknessClass: 0 | 1
  rawGeometryClass: PrismIntersection
}>

type FoldPreviewAllowedHingeDecision = Extract<
  FoldPreviewHingeContactDecision,
  { kind: 'allowed_by_hinge_model' }
>

type FoldPreviewCertifiedHingeRuntimeEvidence = Readonly<{
  policyVersion: typeof FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
  topology: 'shared_hinge_edge'
  evidence:
    | 'shared_feature_contact'
    | 'shared_feature_thickness_overlap'
    | 'shared_feature_flat_stack'
  proof: 'centered_hinge_model_runtime_evidence_v1'
  hingeEdgeId: string
  geometry: FoldPreviewAllowedHingeDecision['geometry']
}>

type FoldPreviewHingeRuntimeProvenance = Readonly<{
  hingeDecision: FoldPreviewAllowedHingeDecision
  hingeEdgeIds: readonly string[]
  inputGeometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass']
}>

type PrismIntersectionClassification = Readonly<{
  /** Effective class consumed by collision-stop and presentation layers. */
  geometryClass: PrismIntersection
  /** Unmodified surface/SAT result retained for diagnostics and audit. */
  rawGeometryClass: PrismIntersection
  topologyContact?: FoldPreviewAllowedSharedVertexPair
}>

type MutableFoldPreviewTopologyContactSummary = {
  sharedVertexIds: Set<string>
  featureContactPairCount: number
  thicknessOverlapPairCount: number
  rawTouchingPairCount: number
  rawPenetratingPairCount: number
  rawIndeterminatePairCount: number
}

type FoldPreviewExactTransversalProofBudget = {
  attempted: number
  skippedByLimit: number
}

type FoldPreviewExactTransversalProofDecision =
  | 'proved'
  | 'not_proved'
  | 'budget_exhausted'

type WitnessPairSeed = Readonly<{
  first: TrianglePrism
  second: TrianglePrism
}>

type EligibleWitnessPairSeed = WitnessPairSeed & Readonly<{
  firstFaceId: string
  secondFaceId: string
  geometryClass: 'touching' | 'penetrating'
}>

type PreparedFullScanCandidate = Readonly<{
  firstFaceId: string
  secondFaceId: string
  firstPrisms: readonly TrianglePrism[]
  secondPrisms: readonly TrianglePrism[]
}>

type PreparedNarrowPhaseCandidate = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: FoldPreviewBroadPhaseCandidate['relation']
  hingeEdgeIds: readonly string[]
  firstPrisms: readonly TrianglePrism[]
  secondPrisms: readonly TrianglePrism[]
  potentialTrianglePairCount: number
}>

type FullScanNonAdjacentWitnessCounts = Readonly<{
  broadPhaseCandidateCount: number
  expectedTrianglePairCount: number
  trianglePairTests: number
  aabbRejectedPairCount: number
  satTests: number
  satSeparatedPairCount: number
  allowedSharedVertexPairCount: number
  touchingPairCount: number
  penetratingPairCount: number
  indeterminatePairCount: number
}>

function createMutableTopologyContactSummary():
  MutableFoldPreviewTopologyContactSummary {
  return {
    sharedVertexIds: new Set(),
    featureContactPairCount: 0,
    thicknessOverlapPairCount: 0,
    rawTouchingPairCount: 0,
    rawPenetratingPairCount: 0,
    rawIndeterminatePairCount: 0,
  }
}

function recordAllowedSharedVertexPair(
  summary: MutableFoldPreviewTopologyContactSummary,
  pair: FoldPreviewAllowedSharedVertexPair,
  rawGeometryClass: PrismIntersection,
) {
  summary.sharedVertexIds.add(pair.sharedVertexId)
  if (pair.evidence === 'shared_feature_contact') {
    summary.featureContactPairCount += 1
  } else {
    summary.thicknessOverlapPairCount += 1
  }
  if (rawGeometryClass === 'touching') summary.rawTouchingPairCount += 1
  else if (rawGeometryClass === 'penetrating') {
    summary.rawPenetratingPairCount += 1
  } else if (rawGeometryClass === 'indeterminate') {
    summary.rawIndeterminatePairCount += 1
  }
}

function snapshotTopologyContactSummary(
  value: MutableFoldPreviewTopologyContactSummary,
  exclusive: boolean,
): FoldPreviewTopologyContactSummary | undefined {
  const pairCount = value.featureContactPairCount
    + value.thicknessOverlapPairCount
  if (pairCount === 0) return undefined
  const allIds = [...value.sharedVertexIds]
  return Object.freeze({
    policyVersion: FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
    decision: 'allowed_shared_vertex_contact',
    exclusive,
    sharedVertexIds: Object.freeze(
      allIds.slice(0, MAX_FOLD_PREVIEW_TOPOLOGY_CONTACT_VERTEX_IDS),
    ),
    omittedSharedVertexIdCount: Math.max(
      0,
      allIds.length - MAX_FOLD_PREVIEW_TOPOLOGY_CONTACT_VERTEX_IDS,
    ),
    featureContactPairCount: value.featureContactPairCount,
    thicknessOverlapPairCount: value.thicknessOverlapPairCount,
    rawTouchingPairCount: value.rawTouchingPairCount,
    rawPenetratingPairCount: value.rawPenetratingPairCount,
    rawIndeterminatePairCount: value.rawIndeterminatePairCount,
  })
}

function createFoldPreviewExactTransversalProofBudget():
  FoldPreviewExactTransversalProofBudget {
  return {
    attempted: 0,
    skippedByLimit: 0,
  }
}

function snapshotFoldPreviewExactTransversalProofWork(
  budget: FoldPreviewExactTransversalProofBudget,
): FoldPreviewExactTransversalProofWork {
  return Object.freeze({
    algorithm: 'binary64_transversal_triangle_intersection_v1',
    maximumAttempts:
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    attempted: budget.attempted,
    skippedByLimit: budget.skippedByLimit,
  })
}

function sameFoldPreviewExactTransversalProofWork(
  first: FoldPreviewExactTransversalProofWork,
  second: FoldPreviewExactTransversalProofWork,
) {
  return first.algorithm === second.algorithm
    && first.maximumAttempts === second.maximumAttempts
    && first.attempted === second.attempted
    && first.skippedByLimit === second.skippedByLimit
}

function validFoldPreviewExactTransversalProofWork(
  value: FoldPreviewExactTransversalProofWork,
) {
  return value.algorithm
      === 'binary64_transversal_triangle_intersection_v1'
    && value.maximumAttempts
      === MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
    && Number.isSafeInteger(value.attempted)
    && value.attempted >= 0
    && value.attempted
      <= MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
    && Number.isSafeInteger(value.skippedByLimit)
    && value.skippedByLimit >= 0
    && value.skippedByLimit
      <= MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
}

function exactProofLimitHingeDecision(
  hingeEdgeIds: readonly string[],
): FoldPreviewHingeContactDecision {
  return indeterminateHingeDecision(hingeEdgeIds, 'numerical_geometry')
}

function indeterminateHingeDecision(
  hingeEdgeIds: readonly string[],
  reason: Extract<
    FoldPreviewHingeContactDecision,
    { kind: 'indeterminate' }
  >['reason'],
): FoldPreviewHingeContactDecision {
  return Object.freeze({
    kind: 'indeterminate',
    hingeEdgeIds: Object.freeze([...hingeEdgeIds]),
    reason,
  })
}

/**
 * The hinge model remains the geometric authority.  This gate only verifies
 * that its analyzer-issued result occupies a realizable shared-hinge policy
 * cell before face aggregation is allowed to publish that exception.
 */
function applyCertifiedHingeTopologyDecision(
  inputGeometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass'],
  hingeDecision: FoldPreviewHingeContactDecision,
  hingeEdgeIds: readonly string[],
): Readonly<{
  geometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass']
  hingeDecision: FoldPreviewHingeContactDecision
}> {
  if (hingeDecision.kind !== 'allowed_by_hinge_model') {
    return { geometryClass: inputGeometryClass, hingeDecision }
  }
  const runtimeEvidence = issueHingeRuntimeEvidence(
    hingeDecision,
    hingeEdgeIds,
    inputGeometryClass,
  )
  if (
    !runtimeEvidence
    || !dispatchHingeRuntimeEvidence(
      runtimeEvidence,
      hingeDecision,
      hingeEdgeIds,
      inputGeometryClass,
    )
  ) {
    return {
      geometryClass: 'indeterminate',
      hingeDecision: exactProofLimitHingeDecision(hingeEdgeIds),
    }
  }
  return {
    geometryClass: hingeDecision.geometry === 'boundary_contact'
      ? 'touching'
      : 'penetrating',
    hingeDecision,
  }
}

function issueHingeRuntimeEvidence(
  hingeDecision: FoldPreviewAllowedHingeDecision,
  hingeEdgeIds: readonly string[],
  inputGeometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass'],
): FoldPreviewCertifiedHingeRuntimeEvidence | null {
  try {
    if (
      !Array.isArray(hingeEdgeIds)
      || hingeEdgeIds.length !== 1
      || hingeEdgeIds[0] !== hingeDecision.hingeEdgeId
      || !validId(hingeDecision.hingeEdgeId)
      || hingeDecision.thicknessRule !== 'centered_mid_surface_v1'
    ) return null
    const evidence = hingeTopologyEvidence(hingeDecision.geometry)
    if (!evidence) return null
    const runtimeEvidence = Object.freeze({
      policyVersion: FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
      topology: 'shared_hinge_edge' as const,
      evidence,
      proof: 'centered_hinge_model_runtime_evidence_v1' as const,
      hingeEdgeId: hingeDecision.hingeEdgeId,
      geometry: hingeDecision.geometry,
    })
    const provenance = Object.freeze({
      hingeDecision,
      hingeEdgeIds,
      inputGeometryClass,
    })
    reflectApplyIntrinsic(
      weakSetAddIntrinsic,
      trustedHingeRuntimeEvidence,
      [runtimeEvidence],
    )
    reflectApplyIntrinsic(
      weakMapSetIntrinsic,
      trustedHingeRuntimeProvenance,
      [runtimeEvidence, provenance],
    )
    return runtimeEvidence
  } catch {
    return null
  }
}

function dispatchHingeRuntimeEvidence(
  value: FoldPreviewCertifiedHingeRuntimeEvidence,
  hingeDecision: FoldPreviewAllowedHingeDecision,
  hingeEdgeIds: readonly string[],
  inputGeometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass'],
) {
  try {
    if (
      typeof value !== 'object'
      || value === null
      || !reflectApplyIntrinsic(
        weakSetHasIntrinsic,
        trustedHingeRuntimeEvidence,
        [value],
      )
    ) return false
    const provenance = reflectApplyIntrinsic(
      weakMapGetIntrinsic,
      trustedHingeRuntimeProvenance,
      [value],
    ) as FoldPreviewHingeRuntimeProvenance | undefined
    if (
      value.policyVersion
        !== FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
      || value.topology !== 'shared_hinge_edge'
      || value.proof !== 'centered_hinge_model_runtime_evidence_v1'
      || !validId(value.hingeEdgeId)
      || value.hingeEdgeId !== hingeDecision.hingeEdgeId
      || value.geometry !== hingeDecision.geometry
      || value.evidence !== hingeTopologyEvidence(value.geometry)
      || !Object.isFrozen(value)
      || provenance?.hingeDecision !== hingeDecision
      || provenance.hingeEdgeIds !== hingeEdgeIds
      || provenance.inputGeometryClass !== inputGeometryClass
    ) return false
    return classifyFoldPreviewTopologyContact(
      value.topology,
      value.evidence,
    ) === 'requires_hinge_model'
  } catch {
    return false
  }
}

function hingeTopologyEvidence(
  geometry: FoldPreviewAllowedHingeDecision['geometry'],
): FoldPreviewCertifiedHingeRuntimeEvidence['evidence'] | null {
  if (geometry === 'boundary_contact') return 'shared_feature_contact'
  if (geometry === 'corridor_overlap') {
    return 'shared_feature_thickness_overlap'
  }
  if (geometry === 'flat_surface_stack') return 'shared_feature_flat_stack'
  return null
}

/**
 * Refines conservative face AABBs with SAT tests between triangulated paper
 * prisms. The output is still geometric, not an origami legality decision:
 * shared-hinge interactions remain explicitly tagged for the contact-policy
 * layer instead of being silently accepted or rejected here. This evaluates
 * one immutable pose only; continuous collision detection is a later stage.
 */
export function findFoldPreviewNarrowPhaseInteractions(
  faces: readonly FoldPreviewCollisionPoseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
): FoldPreviewNarrowPhaseResult | null {
  try {
    const transformSnapshot = snapshotRigidFaceTransforms(
      faces,
      faceTransforms,
    )
    if (!transformSnapshot) return null
    const broadPhase = findFoldPreviewPoseBroadPhaseCandidates(
      faces,
      transformSnapshot,
      thickness,
      adjacencies,
    )
    if (!broadPhase) return null
    const topologyPoseMismatches = findTopologyPoseMismatches(
      faces,
      transformSnapshot,
      thickness,
      adjacencies,
      broadPhase,
    )
    if (!topologyPoseMismatches) return null
    if (topologyPoseMismatches.length > 0) {
      return topologyPoseMismatchResult(
        broadPhase,
        topologyPoseMismatches,
      )
    }
    return refineFoldPreviewNarrowPhase(
      faces,
      transformSnapshot,
      thickness,
      broadPhase,
      null,
    )
  } catch {
    return null
  }
}

/**
 * Snapshots and triangulates pose-independent collision inputs once.
 *
 * The returned analyzer deliberately does not retain a pose or thickness:
 * every synchronous call validates one exact immutable transform map, rebuilds
 * world bounds, and reruns the broad and narrow phases.
 */
export function prepareFoldPreviewNarrowPhase(
  faces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  hingeConstraints?: readonly FoldPreviewHingeContactConstraint[],
): FoldPreviewNarrowPhaseAnalyzer | null {
  const prepared = snapshotNarrowPhaseInputs(faces, adjacencies, hingeConstraints)
  if (!prepared) return null
  const {
    preparedFaces,
    poseFaces,
    adjacencySnapshot,
    hingeContactPolicy,
  } = prepared

  return Object.freeze({
    analyze(
      faceTransforms: ReadonlyMap<string, Matrix4>,
      thickness: number,
    ): FoldPreviewNarrowPhaseResult | null {
      try {
        if (!Number.isFinite(thickness) || thickness < 0) return null
        return collectPreparedFoldPreviewNarrowPhaseAnalysis(
          preparedFaces,
          poseFaces,
          adjacencySnapshot,
          faceTransforms,
          thickness,
          hingeContactPolicy,
        )
      } catch {
        return null
      }
    },
    createAnalysisJob(
      faceTransforms: ReadonlyMap<string, Matrix4>,
      thickness: number,
    ): FoldPreviewNarrowPhaseAnalysisJob | null {
      try {
        if (!Number.isFinite(thickness) || thickness < 0) return null
        return createPreparedFoldPreviewNarrowPhaseAnalysisJob(
          preparedFaces,
          poseFaces,
          adjacencySnapshot,
          faceTransforms,
          thickness,
          hingeContactPolicy,
        )
      } catch {
        return null
      }
    },
    createFullScanNonAdjacentWitnessSetJob(
      faceTransforms: ReadonlyMap<string, Matrix4>,
      thickness: number,
    ): FoldPreviewFullScanNonAdjacentWitnessJob | null {
      try {
        if (!Number.isFinite(thickness) || thickness <= 0) return null
        return createPreparedFullScanNonAdjacentWitnessSetJob(
          preparedFaces,
          poseFaces,
          adjacencySnapshot,
          faceTransforms,
          thickness,
        )
      } catch {
        return null
      }
    },
    collectFullScanNonAdjacentWitnessSet(
      faceTransforms: ReadonlyMap<string, Matrix4>,
      thickness: number,
    ): FoldPreviewFullScanNonAdjacentWitnessSet | null {
      try {
        if (!Number.isFinite(thickness) || thickness <= 0) return null
        return collectPreparedFullScanNonAdjacentWitnessSet(
          preparedFaces,
          poseFaces,
          adjacencySnapshot,
          faceTransforms,
          thickness,
        )
      } catch {
        return null
      }
    },
  })
}

function collectPreparedFoldPreviewNarrowPhaseAnalysis(
  preparedFaces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  poseFaces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  hingeContactPolicy: FoldPreviewHingeContactPolicy | null,
): FoldPreviewNarrowPhaseResult | null {
  const job = createPreparedFoldPreviewNarrowPhaseAnalysisJob(
    preparedFaces,
    poseFaces,
    adjacencies,
    faceTransforms,
    thickness,
    hingeContactPolicy,
  )
  if (!job) return null
  const step = job.step(MAX_FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_WORK_UNITS)
  if (step.kind === 'complete') return step.result
  job.cancel()
  return null
}

function createPreparedFoldPreviewNarrowPhaseAnalysisJob(
  preparedFaces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  poseFaces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  hingeContactPolicy: FoldPreviewHingeContactPolicy | null,
): FoldPreviewNarrowPhaseAnalysisJob | null {
  const transformSnapshot = snapshotRigidFaceTransforms(
    preparedFaces,
    faceTransforms,
  )
  if (!transformSnapshot) return null
  const broadPhase = findFoldPreviewPoseBroadPhaseCandidates(
    poseFaces,
    transformSnapshot,
    thickness,
    adjacencies,
  )
  if (!broadPhase) return null
  const topologyPoseMismatches = findTopologyPoseMismatches(
    preparedFaces,
    transformSnapshot,
    thickness,
    adjacencies,
    broadPhase,
  )
  if (!topologyPoseMismatches) return null
  if (topologyPoseMismatches.length > 0) {
    return createTopologyPoseMismatchAnalysisJob(
      broadPhase,
      topologyPoseMismatches,
    )
  }
  return createFoldPreviewNarrowPhaseAnalysisJob(
    preparedFaces,
    transformSnapshot,
    thickness,
    broadPhase,
    hingeContactPolicy,
  )
}

function collectPreparedFullScanNonAdjacentWitnessSet(
  preparedFaces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  poseFaces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
): FoldPreviewFullScanNonAdjacentWitnessSet | null {
  const job = createPreparedFullScanNonAdjacentWitnessSetJob(
    preparedFaces,
    poseFaces,
    adjacencies,
    faceTransforms,
    thickness,
  )
  if (!job) return null
  const step = job.step(MAX_FOLD_PREVIEW_FULL_SCAN_JOB_WORK_UNITS)
  if (step.kind === 'complete') return step.result
  job.cancel()
  return null
}

function createPreparedFullScanNonAdjacentWitnessSetJob(
  preparedFaces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  poseFaces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
): FoldPreviewFullScanNonAdjacentWitnessJob | null {
  const transformSnapshot = snapshotRigidFaceTransforms(
    preparedFaces,
    faceTransforms,
  )
  if (!transformSnapshot) return null
  const broadPhase = findFoldPreviewPoseBroadPhaseCandidates(
    poseFaces,
    transformSnapshot,
    thickness,
    adjacencies,
  )
  if (!broadPhase) return null
  const topologyPoseMismatches = findTopologyPoseMismatches(
    preparedFaces,
    transformSnapshot,
    thickness,
    adjacencies,
    broadPhase,
  )
  if (!topologyPoseMismatches || topologyPoseMismatches.length > 0) {
    return null
  }
  return createFullScanNonAdjacentWitnessSetJob(
    preparedFaces,
    transformSnapshot,
    thickness,
    broadPhase,
  )
}

/**
 * Validates topology identity independently of broad-phase overlap. A
 * disconnected current copy of one topology vertex is an invalid pose, not
 * affirmative separation evidence. Every declared hinge must also reach the
 * broad phase, even if its faces omit or corrupt their shared vertex IDs,
 * because otherwise the finite-axis hinge validator cannot run.
 */
function findTopologyPoseMismatches(
  faces: readonly FoldPreviewNarrowPhaseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  broadPhase: FoldPreviewBroadPhaseResult,
): readonly FoldPreviewTopologyPoseMismatch[] | null {
  try {
    if (
      !Number.isFinite(thickness)
      || thickness < 0
      || !Array.isArray(faces)
      || !Array.isArray(adjacencies)
    ) return null
    const adjacencyByPair = new Map<string, {
      firstFaceId: string
      secondFaceId: string
      edgeIds: string[]
    }>()
    for (const adjacency of adjacencies) {
      const key = facePairKey(
        adjacency.firstFaceId,
        adjacency.secondFaceId,
      )
      const [firstFaceId, secondFaceId] =
        adjacency.firstFaceId < adjacency.secondFaceId
          ? [adjacency.firstFaceId, adjacency.secondFaceId]
          : [adjacency.secondFaceId, adjacency.firstFaceId]
      const group = adjacencyByPair.get(key) ?? {
        firstFaceId,
        secondFaceId,
        edgeIds: [] as string[],
      }
      group.edgeIds.push(adjacency.edgeId)
      group.edgeIds.sort()
      adjacencyByPair.set(key, group)
    }
    const broadPhasePairs = new Set(broadPhase.candidates.map((candidate) =>
      facePairKey(candidate.firstFaceId, candidate.secondFaceId)))

    type Occurrence = Readonly<{
      faceId: string
      world: Vector3
      localScale: number
    }>
    const restByVertexId = new Map<string, Readonly<{
      x: number
      z: number
    }>>()
    const occurrences = new Map<string, Occurrence[]>()
    let occurrenceCount = 0
    for (const face of faces) {
      const transform = faceTransforms.get(face.id)
      if (!transform || !Array.isArray(face.polygon)) return null
      let minimumX = Number.POSITIVE_INFINITY
      let maximumX = Number.NEGATIVE_INFINITY
      let minimumZ = Number.POSITIVE_INFINITY
      let maximumZ = Number.NEGATIVE_INFINITY
      for (const point of face.polygon) {
        if (!point || !Number.isFinite(point.x) || !Number.isFinite(point.z)) {
          return null
        }
        minimumX = Math.min(minimumX, point.x)
        maximumX = Math.max(maximumX, point.x)
        minimumZ = Math.min(minimumZ, point.z)
        maximumZ = Math.max(maximumZ, point.z)
      }
      const localScale = Math.max(
        1,
        thickness,
        maximumX - minimumX,
        maximumZ - minimumZ,
      )
      if (!Number.isFinite(localScale)) return null
      const seenOnFace = new Set<string>()
      for (const point of face.polygon) {
        const vertexId = point.vertexId
        if (vertexId === undefined || seenOnFace.has(vertexId)) continue
        if (!validId(vertexId)) return null
        seenOnFace.add(vertexId)
        const knownRest = restByVertexId.get(vertexId)
        if (
          knownRest
          && (knownRest.x !== point.x || knownRest.z !== point.z)
        ) return null
        if (!knownRest) {
          restByVertexId.set(vertexId, { x: point.x, z: point.z })
        }
        const world = transformedPoint(point.x, 0, point.z, transform)
        if (!world) return null
        const group = occurrences.get(vertexId) ?? []
        group.push({ faceId: face.id, world, localScale })
        occurrences.set(vertexId, group)
        occurrenceCount += 1
        if (
          !Number.isSafeInteger(occurrenceCount)
          || occurrenceCount
            > MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES
        ) return null
      }
    }

    const mismatches = new Map<string, FoldPreviewTopologyPoseMismatch>()
    for (const [key, adjacency] of adjacencyByPair) {
      if (broadPhasePairs.has(key)) continue
      mismatches.set(key, {
        firstFaceId: adjacency.firstFaceId,
        secondFaceId: adjacency.secondFaceId,
        relation: 'hinge_adjacent',
        hingeEdgeIds: Object.freeze([...adjacency.edgeIds]),
      })
      if (
        mismatches.size > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
      ) return null
    }
    let comparisonCount = 0
    for (const group of occurrences.values()) {
      for (let firstIndex = 0; firstIndex < group.length; firstIndex += 1) {
        const first = group[firstIndex]
        for (
          let secondIndex = firstIndex + 1;
          secondIndex < group.length;
          secondIndex += 1
        ) {
          const second = group[secondIndex]
          if (first.faceId === second.faceId) continue
          const key = facePairKey(first.faceId, second.faceId)
          comparisonCount += 1
          if (
            comparisonCount > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
          ) return null
          const localMargin = calculateFoldPreviewNarrowPhaseNumericalMargin(
            Math.max(first.localScale, second.localScale),
          )
          const distance = first.world.distanceTo(second.world)
          if (
            localMargin === null
            || !Number.isFinite(distance)
          ) return null
          if (
            distance <= localMargin
            && broadPhasePairs.has(key)
          ) continue
          if (mismatches.has(key)) continue
          const [firstFaceId, secondFaceId] =
            first.faceId < second.faceId
              ? [first.faceId, second.faceId]
              : [second.faceId, first.faceId]
          const hingeEdgeIds = adjacencyByPair.get(key)?.edgeIds ?? []
          mismatches.set(key, {
            firstFaceId,
            secondFaceId,
            relation: hingeEdgeIds.length > 0
              ? 'hinge_adjacent'
              : 'non_adjacent',
            hingeEdgeIds: Object.freeze([...hingeEdgeIds]),
          })
          if (
            mismatches.size > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
          ) return null
        }
      }
    }
    return Object.freeze([...mismatches.values()].sort((first, second) =>
      first.firstFaceId < second.firstFaceId
        ? -1
        : first.firstFaceId > second.firstFaceId
          ? 1
          : first.secondFaceId < second.secondFaceId
            ? -1
            : first.secondFaceId > second.secondFaceId
              ? 1
              : 0))
  } catch {
    return null
  }
}

function facePairKey(firstFaceId: string, secondFaceId: string) {
  return JSON.stringify(firstFaceId < secondFaceId
    ? [firstFaceId, secondFaceId]
    : [secondFaceId, firstFaceId])
}

function topologyPoseMismatchResult(
  broadPhase: FoldPreviewBroadPhaseResult,
  mismatches: readonly FoldPreviewTopologyPoseMismatch[],
): FoldPreviewNarrowPhaseResult | null {
  const numericalMargin = broadPhase.numericalMargin * SAT_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin) || mismatches.length === 0) return null
  const broadPhaseHingeAdjacentCandidates = broadPhase.candidates.reduce(
    (count, candidate) =>
      count + Number(candidate.relation === 'hinge_adjacent'),
    0,
  )
  const exactTransversalProofWork =
    snapshotFoldPreviewExactTransversalProofWork(
      createFoldPreviewExactTransversalProofBudget(),
    )
  return freezeNarrowPhaseResultSnapshot({
    broadPhaseCandidates: broadPhase.candidates.length,
    broadPhaseNonAdjacentCandidates:
      broadPhase.candidates.length - broadPhaseHingeAdjacentCandidates,
    broadPhaseHingeAdjacentCandidates,
    interactions: mismatches.map((mismatch) =>
      mismatch.relation === 'hinge_adjacent'
        ? {
            firstFaceId: mismatch.firstFaceId,
            secondFaceId: mismatch.secondFaceId,
            relation: mismatch.relation,
            hingeEdgeIds: mismatch.hingeEdgeIds,
            geometryClass: 'indeterminate' as const,
            hingeDecision: indeterminateHingeDecision(
              mismatch.hingeEdgeIds,
              'pose_mismatch',
            ),
          }
        : {
            firstFaceId: mismatch.firstFaceId,
            secondFaceId: mismatch.secondFaceId,
            relation: mismatch.relation,
            hingeEdgeIds: mismatch.hingeEdgeIds,
            geometryClass: 'indeterminate' as const,
          }),
    trianglePairTests: 0,
    satTests: 0,
    numericalMargin,
    exactTransversalProofWork,
    witnessSamples: Object.freeze([]),
    witnessCoverage: freezeWitnessCoverage({
      eligiblePairCount: 0,
      attemptedPairCount: 0,
      unavailablePairCount: 0,
      omittedByLimitCount: 0,
      authoritativePairScanComplete: false,
    }),
  })
}

function createTopologyPoseMismatchAnalysisJob(
  broadPhase: FoldPreviewBroadPhaseResult,
  mismatches: readonly FoldPreviewTopologyPoseMismatch[],
): FoldPreviewNarrowPhaseAnalysisJob | null {
  const result = topologyPoseMismatchResult(broadPhase, mismatches)
  if (!result) return null
  const work = Object.freeze({
    totalWorkUnits: 0,
    trianglePairTests: 0,
    witnessDerivations: 0,
  })
  const workBounds = Object.freeze({
    entireStepTimeBounded: false as const,
    synchronousFactoryPreparation: true as const,
    synchronousHingePolicyFinalization: true as const,
    synchronousResultFinalization: true as const,
    potentialTrianglePairCount: 0,
    maximumTrianglePairTests: 0,
    maximumWitnessDerivations: 0,
    maximumTotalWorkUnits: 0,
  })
  const exactTransversalProofWork = result.exactTransversalProofWork
  let cancelled = false
  let terminal: FoldPreviewNarrowPhaseAnalysisJobStep | null = null
  const publish = (step: FoldPreviewNarrowPhaseAnalysisJobStep) => {
    terminal ??= Object.freeze(step)
    return terminal
  }
  return Object.freeze({
    workBounds,
    step(workBudget: number): FoldPreviewNarrowPhaseAnalysisJobStep {
      if (terminal) return terminal
      if (cancelled) {
        return publish({
          version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
          kind: 'cancelled',
          work,
          workBounds,
          exactTransversalProofWork,
        })
      }
      if (!Number.isSafeInteger(workBudget) || workBudget <= 0) {
        return publish({
          version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
          kind: 'indeterminate',
          reason: 'invalid_work_budget',
          work,
          workBounds,
          exactTransversalProofWork,
        })
      }
      return publish({
        version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
        kind: 'complete',
        result,
        work,
        workBounds,
        exactTransversalProofWork,
      })
    },
    cancel() {
      if (!terminal) cancelled = true
    },
  })
}

function refineFoldPreviewNarrowPhase(
  faces: readonly FoldPreviewNarrowPhaseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  broadPhase: FoldPreviewBroadPhaseResult,
  hingeContactPolicy: FoldPreviewHingeContactPolicy | null,
): FoldPreviewNarrowPhaseResult | null {
  const facesById = new Map(faces.map((face) => [face.id, face]))
  if (facesById.size !== faces.length) return null
  const numericalMargin = broadPhase.numericalMargin * SAT_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin)) return null
  const broadPhaseHingeAdjacentCandidates = broadPhase.candidates.reduce(
    (count, candidate) => count + Number(candidate.relation === 'hinge_adjacent'),
    0,
  )
  const broadPhaseNonAdjacentCandidates = broadPhase.candidates.length
    - broadPhaseHingeAdjacentCandidates
  const prismCache = new Map<string, readonly TrianglePrism[]>()
  let trianglePairTests = 0
  let satTests = 0
  const interactions: FoldPreviewNarrowPhaseInteraction[] = []
  const witnessSamples: FoldPreviewNarrowPhaseWitnessSample[] = []
  const penetratingEligibleSeeds: EligibleWitnessPairSeed[] = []
  const touchingEligibleSeeds: EligibleWitnessPairSeed[] = []
  let eligibleWitnessPairCount = 0
  let attemptedWitnessPairCount = 0
  let unavailableWitnessPairCount = 0
  let performedNonAdjacentSatScan = false
  let nonAdjacentPairScansComplete = true
  const exactTransversalProofBudget =
    createFoldPreviewExactTransversalProofBudget()

  try {
    const prismsForFace = (faceId: string) => {
      const cached = prismCache.get(faceId)
      if (cached) return cached
      const face = facesById.get(faceId)
      const transform = faceTransforms.get(faceId)
      if (!face || !transform) return null
      const prisms = buildTrianglePrisms(face, transform, thickness)
      if (!prisms) return null
      prismCache.set(faceId, prisms)
      return prisms
    }

    for (const candidate of broadPhase.candidates) {
      const firstPrisms = prismsForFace(candidate.firstFaceId)
      const secondPrisms = prismsForFace(candidate.secondFaceId)
      if (!firstPrisms || !secondPrisms) return null

      let geometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass'] | null = null
      const hingePairs: FoldPreviewHingeContactPair[] = []
      const touchingWitnessSeeds: WitnessPairSeed[] = []
      const penetratingWitnessSeeds: WitnessPairSeed[] = []
      let touchingWitnessPairCount = 0
      let penetratingWitnessPairCount = 0
      let candidateTrianglePairTests = 0
      let candidateExactProofSkippedByLimit = false
      let candidateHasNonAllowedInteraction = false
      const topologyContacts = createMutableTopologyContactSummary()
      pairSearch:
      for (const first of firstPrisms) {
        for (const second of secondPrisms) {
          candidateTrianglePairTests += 1
          trianglePairTests += 1
          if (trianglePairTests > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS) return null
          if (!boundsOverlap(first.bounds, second.bounds, numericalMargin)) continue
          satTests += 1
          if (candidate.relation === 'non_adjacent') {
            performedNonAdjacentSatScan = true
          }
          const skippedBefore =
            exactTransversalProofBudget.skippedByLimit
          const intersection = classifyTrianglePrisms(
            first,
            second,
            numericalMargin,
            exactTransversalProofBudget,
          )
          if (
            exactTransversalProofBudget.skippedByLimit
              > skippedBefore
          ) candidateExactProofSkippedByLimit = true
          if (!intersection) return null
          const intersectionClass = intersection.geometryClass
          if (intersectionClass === 'separated') continue
          if (intersection.topologyContact) {
            recordAllowedSharedVertexPair(
              topologyContacts,
              intersection.topologyContact,
              intersection.rawGeometryClass,
            )
          } else {
            candidateHasNonAllowedInteraction = true
          }
          if (candidate.relation === 'non_adjacent') {
            if (
              intersectionClass === 'touching'
              && !intersection.topologyContact
            ) {
              touchingWitnessPairCount += 1
              if (
                touchingWitnessSeeds.length
                < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
              ) {
                touchingWitnessSeeds.push(Object.freeze({ first, second }))
              }
            } else if (intersectionClass === 'penetrating') {
              penetratingWitnessPairCount += 1
              if (
                penetratingWitnessSeeds.length
                < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
              ) {
                penetratingWitnessSeeds.push(Object.freeze({ first, second }))
              }
            }
          }
          if (candidate.relation === 'hinge_adjacent' && hingeContactPolicy) {
            if (intersection.rawGeometryClass === 'separated') return null
            hingePairs.push({
              firstTriangleIndex: first.triangleIndex,
              secondTriangleIndex: second.triangleIndex,
              firstVertices: first.vertices,
              secondVertices: second.vertices,
              geometryClass: intersection.rawGeometryClass,
            })
          }
          if (intersectionClass === 'penetrating') {
            geometryClass = 'penetrating'
            if (!hingeContactPolicy || candidate.relation !== 'hinge_adjacent') {
              break pairSearch
            }
            continue
          }
          if (
            intersectionClass === 'indeterminate'
            && geometryClass !== 'penetrating'
            && geometryClass !== 'indeterminate'
          ) {
            geometryClass = 'indeterminate'
          } else if (intersectionClass === 'touching' && !geometryClass) {
            geometryClass = 'touching'
          }
        }
      }
      if (candidate.relation === 'non_adjacent') {
        const candidatePairCount = firstPrisms.length * secondPrisms.length
        if (
          !Number.isSafeInteger(candidatePairCount)
          || candidatePairCount < candidateTrianglePairTests
        ) return null
        if (candidateTrianglePairTests !== candidatePairCount) {
          nonAdjacentPairScansComplete = false
        }
        const definitiveClass = geometryClass === 'touching'
          || geometryClass === 'penetrating'
          ? geometryClass
          : null
        if (definitiveClass) {
          const pairCount = definitiveClass === 'penetrating'
            ? penetratingWitnessPairCount
            : touchingWitnessPairCount
          const seeds = definitiveClass === 'penetrating'
            ? penetratingWitnessSeeds
            : touchingWitnessSeeds
          eligibleWitnessPairCount += pairCount
          const eligibleSeeds = definitiveClass === 'penetrating'
            ? penetratingEligibleSeeds
            : touchingEligibleSeeds
          for (const seed of seeds) {
            if (
              eligibleSeeds.length
              >= MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
            ) break
            eligibleSeeds.push(Object.freeze({
              firstFaceId: candidate.firstFaceId,
              secondFaceId: candidate.secondFaceId,
              geometryClass: definitiveClass,
              first: seed.first,
              second: seed.second,
            }))
          }
        }
      }
      if (candidate.relation === 'hinge_adjacent' && !geometryClass) {
        // A declared shared hinge with no intersecting triangle pair is a
        // topology/evidence contradiction, not affirmative separation.
        geometryClass = 'indeterminate'
      }
      if (geometryClass) {
        let interaction: FoldPreviewNarrowPhaseInteraction = {
          firstFaceId: candidate.firstFaceId,
          secondFaceId: candidate.secondFaceId,
          relation: candidate.relation,
          hingeEdgeIds: candidate.hingeEdgeIds,
          geometryClass,
        }
        const topologyContact = snapshotTopologyContactSummary(
          topologyContacts,
          !candidateHasNonAllowedInteraction,
        )
        if (topologyContact) {
          interaction = { ...interaction, topologyContact }
        }
        if (candidate.relation === 'hinge_adjacent') {
          if (!hingeContactPolicy) {
            interaction = {
              ...interaction,
              geometryClass: 'indeterminate',
              hingeDecision: indeterminateHingeDecision(
                candidate.hingeEdgeIds,
                'missing_constraint',
              ),
            }
          } else {
            const hingeDecision = candidateExactProofSkippedByLimit
              ? exactProofLimitHingeDecision(candidate.hingeEdgeIds)
              : hingeContactPolicy.classify({
                  firstFaceId: candidate.firstFaceId,
                  secondFaceId: candidate.secondFaceId,
                  hingeEdgeIds: candidate.hingeEdgeIds,
                  faceTransforms,
                  thickness,
                  numericalMargin,
                  testedTrianglePairs: candidateTrianglePairTests,
                  pairs: hingePairs,
                })
            const certifiedHinge = applyCertifiedHingeTopologyDecision(
              interaction.geometryClass,
              hingeDecision,
              candidate.hingeEdgeIds,
            )
            interaction = {
              ...interaction,
              geometryClass: certifiedHinge.geometryClass,
              hingeDecision: certifiedHinge.hingeDecision,
            }
          }
        }
        interactions.push(interaction)
      }
    }

    for (const eligibleSeeds of [
      penetratingEligibleSeeds,
      touchingEligibleSeeds,
    ]) {
      const remainingAttempts =
        MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
        - attemptedWitnessPairCount
      const attemptCount = Math.min(eligibleSeeds.length, remainingAttempts)
      attemptedWitnessPairCount += attemptCount
      for (let index = 0; index < attemptCount; index += 1) {
        const seed = eligibleSeeds[index]
        if (!seed.first.witnessFrame) {
          unavailableWitnessPairCount += 1
          continue
        }
        const witness = deriveFoldPreviewTrianglePrismWitness({
          firstVertices: seed.first.vertices,
          secondVertices: seed.second.vertices,
          firstFrame: seed.first.witnessFrame,
          numericalMargin,
          authoritativeGeometryClass: seed.geometryClass,
        })
        if (!witness) {
          unavailableWitnessPairCount += 1
          continue
        }
        witnessSamples.push(Object.freeze({
          firstFaceId: seed.firstFaceId,
          secondFaceId: seed.secondFaceId,
          relation: 'non_adjacent',
          firstTriangleIndex: seed.first.triangleIndex,
          secondTriangleIndex: seed.second.triangleIndex,
          geometryClass: seed.geometryClass,
          witness,
        }))
      }
    }
  } catch {
    return null
  }

  return {
    broadPhaseCandidates: broadPhase.candidates.length,
    broadPhaseNonAdjacentCandidates,
    broadPhaseHingeAdjacentCandidates,
    interactions,
    trianglePairTests,
    satTests,
    numericalMargin,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
    witnessSamples: Object.freeze(witnessSamples),
    witnessCoverage: freezeWitnessCoverage({
      eligiblePairCount: eligibleWitnessPairCount,
      attemptedPairCount: attemptedWitnessPairCount,
      unavailablePairCount: unavailableWitnessPairCount,
      omittedByLimitCount:
        eligibleWitnessPairCount - attemptedWitnessPairCount,
      authoritativePairScanComplete:
        performedNonAdjacentSatScan && nonAdjacentPairScansComplete,
    }),
  }
}

function createFoldPreviewNarrowPhaseAnalysisJob(
  faces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  broadPhase: FoldPreviewBroadPhaseResult,
  hingeContactPolicy: FoldPreviewHingeContactPolicy | null,
): FoldPreviewNarrowPhaseAnalysisJob | null {
  const facesById = new Map(faces.map((face) => [face.id, face]))
  if (facesById.size !== faces.length) return null
  const numericalMargin = broadPhase.numericalMargin * SAT_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin) || thickness < 0) return null
  const broadPhaseHingeAdjacentCandidates = broadPhase.candidates.reduce(
    (count, candidate) => count + Number(candidate.relation === 'hinge_adjacent'),
    0,
  )
  const broadPhaseNonAdjacentCandidates = broadPhase.candidates.length
    - broadPhaseHingeAdjacentCandidates

  const preparedCandidates: PreparedNarrowPhaseCandidate[] = []
  let potentialTrianglePairCount = 0
  let potentialNonAdjacentTrianglePairCount = 0

  try {
    const prismCache = new Map<string, readonly TrianglePrism[]>()
    const prismsForFace = (faceId: string) => {
      const cached = prismCache.get(faceId)
      if (cached) return cached
      const face = facesById.get(faceId)
      const transform = faceTransforms.get(faceId)
      if (!face || !transform) return null
      const prisms = buildTrianglePrisms(face, transform, thickness)
      if (!prisms) return null
      prismCache.set(faceId, prisms)
      return prisms
    }

    for (const candidate of broadPhase.candidates) {
      const firstPrisms = prismsForFace(candidate.firstFaceId)
      const secondPrisms = prismsForFace(candidate.secondFaceId)
      if (!firstPrisms || !secondPrisms) return null
      const candidatePairCount =
        firstPrisms.length * secondPrisms.length
      if (!Number.isSafeInteger(candidatePairCount)) return null
      potentialTrianglePairCount += candidatePairCount
      if (!Number.isSafeInteger(potentialTrianglePairCount)) return null
      if (candidate.relation === 'non_adjacent') {
        potentialNonAdjacentTrianglePairCount += candidatePairCount
        if (
          !Number.isSafeInteger(potentialNonAdjacentTrianglePairCount)
        ) return null
      }
      preparedCandidates.push(Object.freeze({
        firstFaceId: candidate.firstFaceId,
        secondFaceId: candidate.secondFaceId,
        relation: candidate.relation,
        hingeEdgeIds: candidate.hingeEdgeIds,
        firstPrisms,
        secondPrisms,
        potentialTrianglePairCount: candidatePairCount,
      }))
    }
  } catch {
    return null
  }

  const maximumTrianglePairTests = Math.min(
    potentialTrianglePairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
  )
  const maximumWitnessDerivations = Math.min(
    potentialNonAdjacentTrianglePairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  const maximumTotalWorkUnits =
    maximumTrianglePairTests + maximumWitnessDerivations
  if (!Number.isSafeInteger(maximumTotalWorkUnits)) return null
  const workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds =
    Object.freeze({
      entireStepTimeBounded: false,
      synchronousFactoryPreparation: true,
      synchronousHingePolicyFinalization: true,
      synchronousResultFinalization: true,
      potentialTrianglePairCount,
      maximumTrianglePairTests,
      maximumWitnessDerivations,
      maximumTotalWorkUnits,
    })
  const exactTransversalProofBudget =
    createFoldPreviewExactTransversalProofBudget()

  let candidateIndex = 0
  let firstTriangleIndex = 0
  let secondTriangleIndex = 0
  let phase: 'triangle_pair_scan' | 'witness_derivation' =
    'triangle_pair_scan'
  let trianglePairTests = 0
  let satTests = 0
  const interactions: FoldPreviewNarrowPhaseInteraction[] = []
  const witnessSamples: FoldPreviewNarrowPhaseWitnessSample[] = []
  const penetratingEligibleSeeds: EligibleWitnessPairSeed[] = []
  const touchingEligibleSeeds: EligibleWitnessPairSeed[] = []
  let eligibleWitnessPairCount = 0
  let unavailableWitnessPairCount = 0
  let performedNonAdjacentSatScan = false
  let nonAdjacentPairScansComplete = true

  let candidateGeometryClass:
    FoldPreviewNarrowPhaseInteraction['geometryClass'] | null = null
  let candidateTrianglePairTests = 0
  let hingePairs: FoldPreviewHingeContactPair[] = []
  let touchingWitnessSeeds: WitnessPairSeed[] = []
  let penetratingWitnessSeeds: WitnessPairSeed[] = []
  let touchingWitnessPairCount = 0
  let penetratingWitnessPairCount = 0
  let candidateExactProofSkippedByLimit = false
  let candidateHasNonAllowedInteraction = false
  let topologyContacts = createMutableTopologyContactSummary()

  let selectedSeeds: readonly EligibleWitnessPairSeed[] = []
  let witnessIndex = 0
  let witnessDerivations = 0
  let cancelled = false
  let stepping = false
  let terminal: FoldPreviewNarrowPhaseAnalysisJobStep | null = null

  const work = (): FoldPreviewNarrowPhaseAnalysisJobWork => Object.freeze({
    totalWorkUnits: trianglePairTests + witnessDerivations,
    trianglePairTests,
    witnessDerivations,
  })

  const freezeUnpublishedStep = (
    value: FoldPreviewNarrowPhaseAnalysisJobStep,
  ): FoldPreviewNarrowPhaseAnalysisJobStep => {
    if (terminal) return terminal
    return Object.freeze(value)
  }

  const publish = (
    value: FoldPreviewNarrowPhaseAnalysisJobStep,
  ): FoldPreviewNarrowPhaseAnalysisJobStep => {
    if (terminal) return terminal
    if (value.kind === 'pending') return value
    terminal = value
    return terminal
  }

  const indeterminateStep = (
    reason: Extract<
      FoldPreviewNarrowPhaseAnalysisJobStep,
      { kind: 'indeterminate' }
    >['reason'],
  ) => freezeUnpublishedStep({
    version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
    kind: 'indeterminate',
    reason,
    work: work(),
    workBounds,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
  })

  const cancelledStep = () => freezeUnpublishedStep({
    version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
    kind: 'cancelled',
    work: work(),
    workBounds,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
  })

  const result = (): FoldPreviewNarrowPhaseResult => ({
      broadPhaseCandidates: broadPhase.candidates.length,
      broadPhaseNonAdjacentCandidates,
      broadPhaseHingeAdjacentCandidates,
      interactions,
      trianglePairTests,
      satTests,
      numericalMargin,
      exactTransversalProofWork:
        snapshotFoldPreviewExactTransversalProofWork(
          exactTransversalProofBudget,
        ),
      witnessSamples: Object.freeze(witnessSamples),
      witnessCoverage: freezeWitnessCoverage({
        eligiblePairCount: eligibleWitnessPairCount,
        attemptedPairCount: witnessDerivations,
        unavailablePairCount: unavailableWitnessPairCount,
        omittedByLimitCount:
          eligibleWitnessPairCount - witnessDerivations,
        authoritativePairScanComplete:
          performedNonAdjacentSatScan && nonAdjacentPairScansComplete,
      }),
    })

  const completeStep = () => {
    const resultSnapshot = freezeNarrowPhaseResultSnapshot(result())
    return freezeUnpublishedStep({
      version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
      kind: 'complete',
      result: resultSnapshot,
      work: work(),
      workBounds,
      exactTransversalProofWork:
        snapshotFoldPreviewExactTransversalProofWork(
          exactTransversalProofBudget,
        ),
    })
  }

  const resetCandidateState = () => {
    firstTriangleIndex = 0
    secondTriangleIndex = 0
    candidateGeometryClass = null
    candidateTrianglePairTests = 0
    hingePairs = []
    touchingWitnessSeeds = []
    penetratingWitnessSeeds = []
    touchingWitnessPairCount = 0
    penetratingWitnessPairCount = 0
    candidateExactProofSkippedByLimit = false
    candidateHasNonAllowedInteraction = false
    topologyContacts = createMutableTopologyContactSummary()
  }

  const enterWitnessDerivationPhase = () => {
    phase = 'witness_derivation'
    const seeds: EligibleWitnessPairSeed[] = []
    for (const eligibleSeeds of [
      penetratingEligibleSeeds,
      touchingEligibleSeeds,
    ]) {
      const remaining =
        MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES - seeds.length
      if (remaining <= 0) break
      seeds.push(...eligibleSeeds.slice(0, remaining))
    }
    selectedSeeds = Object.freeze(seeds)
    if (
      selectedSeeds.length > maximumWitnessDerivations
      || selectedSeeds.length
        > MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
    ) return indeterminateStep('work_accounting_error')
    return selectedSeeds.length === 0 ? completeStep() : null
  }

  const finishCandidate = ():
    FoldPreviewNarrowPhaseAnalysisJobStep | null => {
    const candidate = preparedCandidates[candidateIndex]
    if (!candidate) return indeterminateStep('scan_error')
    if (candidate.relation === 'non_adjacent') {
      if (
        candidateTrianglePairTests
        !== candidate.potentialTrianglePairCount
      ) {
        nonAdjacentPairScansComplete = false
      }
      const definitiveClass = candidateGeometryClass === 'touching'
        || candidateGeometryClass === 'penetrating'
        ? candidateGeometryClass
        : null
      if (definitiveClass) {
        const pairCount = definitiveClass === 'penetrating'
          ? penetratingWitnessPairCount
          : touchingWitnessPairCount
        const candidateSeeds = definitiveClass === 'penetrating'
          ? penetratingWitnessSeeds
          : touchingWitnessSeeds
        eligibleWitnessPairCount += pairCount
        if (!Number.isSafeInteger(eligibleWitnessPairCount)) {
          return indeterminateStep('scan_error')
        }
        const eligibleSeeds = definitiveClass === 'penetrating'
          ? penetratingEligibleSeeds
          : touchingEligibleSeeds
        for (const seed of candidateSeeds) {
          if (
            eligibleSeeds.length
            >= MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
          ) break
          eligibleSeeds.push(Object.freeze({
            firstFaceId: candidate.firstFaceId,
            secondFaceId: candidate.secondFaceId,
            geometryClass: definitiveClass,
            first: seed.first,
            second: seed.second,
          }))
        }
      }
    }

    if (
      candidate.relation === 'hinge_adjacent'
      && !candidateGeometryClass
    ) {
      // See the synchronous path: shared-hinge separation is contradictory
      // until the finite hinge model proves the declared topology.
      candidateGeometryClass = 'indeterminate'
    }
    if (candidateGeometryClass) {
      let interaction: FoldPreviewNarrowPhaseInteraction = {
        firstFaceId: candidate.firstFaceId,
        secondFaceId: candidate.secondFaceId,
        relation: candidate.relation,
        hingeEdgeIds: candidate.hingeEdgeIds,
        geometryClass: candidateGeometryClass,
      }
      const topologyContact = snapshotTopologyContactSummary(
        topologyContacts,
        !candidateHasNonAllowedInteraction,
      )
      if (topologyContact) interaction = { ...interaction, topologyContact }
      if (candidate.relation === 'hinge_adjacent') {
        if (!hingeContactPolicy) {
          interaction = {
            ...interaction,
            geometryClass: 'indeterminate',
            hingeDecision: indeterminateHingeDecision(
              candidate.hingeEdgeIds,
              'missing_constraint',
            ),
          }
        } else {
          const hingeDecision = candidateExactProofSkippedByLimit
            ? exactProofLimitHingeDecision(candidate.hingeEdgeIds)
            : hingeContactPolicy.classify({
                firstFaceId: candidate.firstFaceId,
                secondFaceId: candidate.secondFaceId,
                hingeEdgeIds: candidate.hingeEdgeIds,
                faceTransforms,
                thickness,
                numericalMargin,
                testedTrianglePairs: candidateTrianglePairTests,
                pairs: hingePairs,
              })
          if (terminal) return terminal
          if (cancelled) return cancelledStep()
          const certifiedHinge = applyCertifiedHingeTopologyDecision(
            interaction.geometryClass,
            hingeDecision,
            candidate.hingeEdgeIds,
          )
          interaction = {
            ...interaction,
            geometryClass: certifiedHinge.geometryClass,
            hingeDecision: certifiedHinge.hingeDecision,
          }
        }
      }
      interactions.push(interaction)
    }

    candidateIndex += 1
    resetCandidateState()
    return candidateIndex === preparedCandidates.length
      ? enterWitnessDerivationPhase()
      : null
  }

  const advanceTrianglePairCursor = () => {
    const candidate = preparedCandidates[candidateIndex]
    if (!candidate) return false
    secondTriangleIndex += 1
    if (secondTriangleIndex < candidate.secondPrisms.length) return true
    secondTriangleIndex = 0
    firstTriangleIndex += 1
    return firstTriangleIndex < candidate.firstPrisms.length
  }

  const processTrianglePair = ():
    FoldPreviewNarrowPhaseAnalysisJobStep | null => {
    const candidate = preparedCandidates[candidateIndex]
    const first = candidate?.firstPrisms[firstTriangleIndex]
    const second = candidate?.secondPrisms[secondTriangleIndex]
    if (!candidate || !first || !second) {
      return indeterminateStep('scan_error')
    }

    trianglePairTests += 1
    candidateTrianglePairTests += 1
    if (
      trianglePairTests > maximumTrianglePairTests
      || trianglePairTests > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
    ) return indeterminateStep('work_accounting_error')

    let stopCandidateEarly = false
    if (boundsOverlap(first.bounds, second.bounds, numericalMargin)) {
      satTests += 1
      if (candidate.relation === 'non_adjacent') {
        performedNonAdjacentSatScan = true
      }
      const skippedBefore =
        exactTransversalProofBudget.skippedByLimit
      const intersection = classifyTrianglePrisms(
        first,
        second,
        numericalMargin,
        exactTransversalProofBudget,
      )
      if (
        exactTransversalProofBudget.skippedByLimit > skippedBefore
      ) candidateExactProofSkippedByLimit = true
      if (terminal) return terminal
      if (cancelled) return cancelledStep()
      if (!intersection) return indeterminateStep('scan_error')
      const intersectionClass = intersection.geometryClass
      if (intersectionClass !== 'separated') {
        if (intersection.topologyContact) {
          recordAllowedSharedVertexPair(
            topologyContacts,
            intersection.topologyContact,
            intersection.rawGeometryClass,
          )
        } else {
          candidateHasNonAllowedInteraction = true
        }
        if (candidate.relation === 'non_adjacent') {
          if (
            intersectionClass === 'touching'
            && !intersection.topologyContact
          ) {
            touchingWitnessPairCount += 1
            if (
              touchingWitnessSeeds.length
              < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
            ) {
              touchingWitnessSeeds.push(Object.freeze({ first, second }))
            }
          } else if (intersectionClass === 'penetrating') {
            penetratingWitnessPairCount += 1
            if (
              penetratingWitnessSeeds.length
              < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
            ) {
              penetratingWitnessSeeds.push(Object.freeze({ first, second }))
            }
          }
        }
        if (
          candidate.relation === 'hinge_adjacent'
          && hingeContactPolicy
        ) {
          if (intersection.rawGeometryClass === 'separated') {
            return indeterminateStep('scan_error')
          }
          hingePairs.push({
            firstTriangleIndex: first.triangleIndex,
            secondTriangleIndex: second.triangleIndex,
            firstVertices: first.vertices,
            secondVertices: second.vertices,
            geometryClass: intersection.rawGeometryClass,
          })
        }
        if (intersectionClass === 'penetrating') {
          candidateGeometryClass = 'penetrating'
          stopCandidateEarly =
            !hingeContactPolicy || candidate.relation !== 'hinge_adjacent'
        } else if (
          intersectionClass === 'indeterminate'
          && candidateGeometryClass !== 'penetrating'
          && candidateGeometryClass !== 'indeterminate'
        ) {
          candidateGeometryClass = 'indeterminate'
        } else if (
          intersectionClass === 'touching'
          && !candidateGeometryClass
        ) {
          candidateGeometryClass = 'touching'
        }
      }
    }

    if (stopCandidateEarly || !advanceTrianglePairCursor()) {
      return finishCandidate()
    }
    return null
  }

  const processWitnessDerivation = ():
    FoldPreviewNarrowPhaseAnalysisJobStep | null => {
    const seed = selectedSeeds[witnessIndex]
    if (!seed) return indeterminateStep('scan_error')
    witnessDerivations += 1
    if (witnessDerivations > maximumWitnessDerivations) {
      return indeterminateStep('work_accounting_error')
    }

    let witness: FoldPreviewTrianglePrismWitness | null = null
    if (seed.first.witnessFrame) {
      witness = deriveFoldPreviewTrianglePrismWitness({
        firstVertices: seed.first.vertices,
        secondVertices: seed.second.vertices,
        firstFrame: seed.first.witnessFrame,
        numericalMargin,
        authoritativeGeometryClass: seed.geometryClass,
      })
      if (terminal) return terminal
      if (cancelled) return cancelledStep()
    }
    if (!witness) {
      unavailableWitnessPairCount += 1
    } else {
      witnessSamples.push(Object.freeze({
        firstFaceId: seed.firstFaceId,
        secondFaceId: seed.secondFaceId,
        relation: 'non_adjacent',
        firstTriangleIndex: seed.first.triangleIndex,
        secondTriangleIndex: seed.second.triangleIndex,
        geometryClass: seed.geometryClass,
        witness,
      }))
    }
    witnessIndex += 1
    return witnessIndex === selectedSeeds.length ? completeStep() : null
  }

  const pending = () => Object.freeze({
    version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
    kind: 'pending' as const,
    phase,
    work: work(),
    workBounds,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
  })

  const checkedStep = (
    value: FoldPreviewNarrowPhaseAnalysisJobStep,
    previousWork: FoldPreviewNarrowPhaseAnalysisJobWork,
    previousExactWork: FoldPreviewExactTransversalProofWork,
    workBudget: number,
    processed: number,
  ) => {
    const currentWork = work()
    const currentExactWork =
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      )
    const totalDelta =
      currentWork.totalWorkUnits - previousWork.totalWorkUnits
    const trianglePairDelta =
      currentWork.trianglePairTests - previousWork.trianglePairTests
    const witnessDelta =
      currentWork.witnessDerivations - previousWork.witnessDerivations
    const exactAttemptDelta =
      currentExactWork.attempted - previousExactWork.attempted
    const exactSkippedDelta =
      currentExactWork.skippedByLimit - previousExactWork.skippedByLimit
    const exactRequestDelta = exactAttemptDelta + exactSkippedDelta
    if (
      !Number.isSafeInteger(currentWork.totalWorkUnits)
      || currentWork.totalWorkUnits !== currentWork.trianglePairTests
        + currentWork.witnessDerivations
      || totalDelta < 0
      || trianglePairDelta < 0
      || witnessDelta < 0
      || exactAttemptDelta < 0
      || exactSkippedDelta < 0
      || exactRequestDelta > trianglePairDelta
      || totalDelta !== trianglePairDelta + witnessDelta
      || totalDelta !== processed
      || totalDelta > workBudget
      || trianglePairDelta > workBudget
      || currentWork.trianglePairTests
        > workBounds.maximumTrianglePairTests
      || currentWork.witnessDerivations
        > workBounds.maximumWitnessDerivations
      || currentWork.totalWorkUnits > workBounds.maximumTotalWorkUnits
      || currentExactWork.attempted
        > MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
      || currentExactWork.skippedByLimit > currentWork.trianglePairTests
      || value.work.totalWorkUnits !== currentWork.totalWorkUnits
      || value.work.trianglePairTests !== currentWork.trianglePairTests
      || value.work.witnessDerivations !== currentWork.witnessDerivations
      || value.workBounds !== workBounds
      || !sameFoldPreviewExactTransversalProofWork(
        value.exactTransversalProofWork,
        currentExactWork,
      )
    ) return publish(indeterminateStep('work_accounting_error'))
    return publish(value)
  }

  const runStep = (
    workBudget: number,
    previousWork: FoldPreviewNarrowPhaseAnalysisJobWork,
    previousExactWork: FoldPreviewExactTransversalProofWork,
  ): FoldPreviewNarrowPhaseAnalysisJobStep => {
    let processed = 0
    while (processed < workBudget) {
      if (terminal) {
        return checkedStep(
          terminal,
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
      if (cancelled) {
        return checkedStep(
          cancelledStep(),
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
      if (
        phase === 'triangle_pair_scan'
        && candidateIndex >= preparedCandidates.length
      ) {
        const phaseResult = enterWitnessDerivationPhase()
        if (phaseResult) {
          return checkedStep(
            phaseResult,
            previousWork,
            previousExactWork,
            workBudget,
            processed,
          )
        }
        continue
      }
      if (
        phase === 'triangle_pair_scan'
        && trianglePairTests >= maximumTrianglePairTests
      ) {
        return checkedStep(
          indeterminateStep('work_limit_exceeded'),
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }

      const stepResult = phase === 'triangle_pair_scan'
        ? processTrianglePair()
        : processWitnessDerivation()
      processed += 1
      if (stepResult) {
        return checkedStep(
          stepResult,
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
      if (
        phase === 'triangle_pair_scan'
        && trianglePairTests >= maximumTrianglePairTests
      ) {
        return checkedStep(
          indeterminateStep('work_limit_exceeded'),
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
    }
    return checkedStep(
      pending(),
      previousWork,
      previousExactWork,
      workBudget,
      processed,
    )
  }

  return Object.freeze({
    workBounds,
    step(
      workBudget: number,
    ): FoldPreviewNarrowPhaseAnalysisJobStep {
      if (terminal) return terminal
      if (cancelled) return publish(cancelledStep())
      if (stepping) {
        cancelled = true
        return publish(cancelledStep())
      }
      stepping = true
      let previousWork: FoldPreviewNarrowPhaseAnalysisJobWork | null = null
      let previousExactWork:
        FoldPreviewExactTransversalProofWork | null = null
      let validatedWorkBudget: number | null = null
      try {
        previousWork = work()
        previousExactWork =
          snapshotFoldPreviewExactTransversalProofWork(
            exactTransversalProofBudget,
          )
        if (terminal) return terminal
        if (cancelled) return publish(cancelledStep())
        const validWorkBudget =
          Number.isSafeInteger(workBudget) && workBudget > 0
        if (terminal) return terminal
        if (cancelled) return publish(cancelledStep())
        if (!validWorkBudget) {
          return publish(indeterminateStep('invalid_work_budget'))
        }
        validatedWorkBudget = workBudget
        return runStep(
          validatedWorkBudget,
          previousWork,
          previousExactWork,
        )
      } catch {
        if (terminal) return terminal
        if (cancelled) {
          if (
            previousWork
            && previousExactWork
            && validatedWorkBudget !== null
          ) {
            return checkedStep(
              cancelledStep(),
              previousWork,
              previousExactWork,
              validatedWorkBudget,
              work().totalWorkUnits - previousWork.totalWorkUnits,
            )
          }
          return publish(cancelledStep())
        }
        if (
          !previousWork
          || !previousExactWork
          || validatedWorkBudget === null
        ) {
          return publish(indeterminateStep('scan_error'))
        }
        return checkedStep(
          indeterminateStep('scan_error'),
          previousWork,
          previousExactWork,
          validatedWorkBudget,
          work().totalWorkUnits - previousWork.totalWorkUnits,
        )
      } finally {
        stepping = false
      }
    },
    cancel() {
      if (!terminal) cancelled = true
    },
  })
}

function createFullScanNonAdjacentWitnessSetJob(
  faces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  broadPhase: FoldPreviewBroadPhaseResult,
): FoldPreviewFullScanNonAdjacentWitnessJob | null {
  const facesById = new Map(faces.map((face) => [face.id, face]))
  if (facesById.size !== faces.length) return null
  const numericalMargin = broadPhase.numericalMargin * SAT_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin) || thickness <= 0) return null

  const prismCache = new Map<string, readonly TrianglePrism[]>()
  const preparedCandidates: PreparedFullScanCandidate[] = []
  let broadPhaseCandidateCount = 0
  let expectedTrianglePairCount = 0

  try {
    const prismsForFace = (faceId: string) => {
      const cached = prismCache.get(faceId)
      if (cached) return cached
      const face = facesById.get(faceId)
      const transform = faceTransforms.get(faceId)
      if (!face || !transform) return null
      const prisms = buildTrianglePrisms(face, transform, thickness)
      if (!prisms) return null
      prismCache.set(faceId, prisms)
      return prisms
    }

    for (const candidate of broadPhase.candidates) {
      if (candidate.relation !== 'non_adjacent') continue
      broadPhaseCandidateCount += 1
      const firstPrisms = prismsForFace(candidate.firstFaceId)
      const secondPrisms = prismsForFace(candidate.secondFaceId)
      if (!firstPrisms || !secondPrisms) return null
      const candidatePairCount = firstPrisms.length * secondPrisms.length
      if (!Number.isSafeInteger(candidatePairCount)) return null
      expectedTrianglePairCount += candidatePairCount
      if (
        !Number.isSafeInteger(expectedTrianglePairCount)
        || expectedTrianglePairCount
          > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
      ) return null
      if (candidatePairCount === 0) continue
      preparedCandidates.push(Object.freeze({
        firstFaceId: candidate.firstFaceId,
        secondFaceId: candidate.secondFaceId,
        firstPrisms,
        secondPrisms,
      }))
    }
  } catch {
    return null
  }

  const maximumWitnessDerivations = Math.min(
    expectedTrianglePairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  const maximumTotalWorkUnits =
    expectedTrianglePairCount + maximumWitnessDerivations
  if (!Number.isSafeInteger(maximumTotalWorkUnits)) return null
  const workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds =
    Object.freeze({
      expectedTrianglePairCount,
      maximumWitnessDerivations,
      maximumTotalWorkUnits,
    })
  const exactTransversalProofBudget =
    createFoldPreviewExactTransversalProofBudget()

  const eligibleSeeds: EligibleWitnessPairSeed[] = []
  let candidateIndex = 0
  let firstTriangleIndex = 0
  let secondTriangleIndex = 0
  let phase: 'triangle_pair_scan' | 'witness_derivation' =
    'triangle_pair_scan'
  let trianglePairTests = 0
  let aabbRejectedPairCount = 0
  let satTests = 0
  let satSeparatedPairCount = 0
  let allowedSharedVertexPairCount = 0
  let touchingPairCount = 0
  let penetratingPairCount = 0
  let indeterminatePairCount = 0
  let selectedSeeds: readonly EligibleWitnessPairSeed[] = []
  let witnessIndex = 0
  let witnessDerivations = 0
  const witnessSamples: FoldPreviewNarrowPhaseWitnessSample[] = []
  let unavailablePairCount = 0
  let cancelled = false
  let stepping = false
  let terminal: FoldPreviewFullScanNonAdjacentWitnessJobStep | null = null

  const work = (): FoldPreviewFullScanNonAdjacentWitnessJobWork =>
    Object.freeze({
      totalWorkUnits: trianglePairTests + witnessDerivations,
      trianglePairTests,
      witnessDerivations,
    })

  const freezeUnpublishedStep = (
    value: FoldPreviewFullScanNonAdjacentWitnessJobStep,
  ): FoldPreviewFullScanNonAdjacentWitnessJobStep => {
    if (terminal) return terminal
    return Object.freeze(value)
  }

  const publish = (
    value: FoldPreviewFullScanNonAdjacentWitnessJobStep,
  ): FoldPreviewFullScanNonAdjacentWitnessJobStep => {
    if (terminal) return terminal
    if (value.kind === 'pending') return value
    terminal = value
    return terminal
  }

  const indeterminateStep = (
    reason: Extract<
      FoldPreviewFullScanNonAdjacentWitnessJobStep,
      { kind: 'indeterminate' }
    >['reason'],
  ) => freezeUnpublishedStep({
    version: FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
    kind: 'indeterminate',
    reason,
    work: work(),
    workBounds,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
  })

  const cancelledStep = () => freezeUnpublishedStep({
    version: FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
    kind: 'cancelled',
    work: work(),
    workBounds,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
  })

  const completeStep = () => {
    const result = createFullScanNonAdjacentWitnessSet({
      thickness,
      numericalMargin,
      counts: {
        broadPhaseCandidateCount,
        expectedTrianglePairCount,
        trianglePairTests,
        aabbRejectedPairCount,
        satTests,
        satSeparatedPairCount,
        allowedSharedVertexPairCount,
        touchingPairCount,
        penetratingPairCount,
        indeterminatePairCount,
      },
      witnessSamples,
      unavailablePairCount,
      exactTransversalProofWork:
        snapshotFoldPreviewExactTransversalProofWork(
          exactTransversalProofBudget,
        ),
    })
    if (!result) return indeterminateStep('scan_error')
    return freezeUnpublishedStep({
      version: FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
      kind: 'complete',
      result,
      work: work(),
      workBounds,
      exactTransversalProofWork:
        snapshotFoldPreviewExactTransversalProofWork(
          exactTransversalProofBudget,
        ),
    })
  }

  const enterWitnessDerivationPhase = () => {
    phase = 'witness_derivation'
    const eligiblePairCount = touchingPairCount + penetratingPairCount
    if (!Number.isSafeInteger(eligiblePairCount)) {
      return indeterminateStep('scan_error')
    }
    const attemptedPairCount = Math.min(
      eligiblePairCount,
      MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
    )
    selectedSeeds = Object.freeze(eligibleSeeds.slice(0, attemptedPairCount))
    if (selectedSeeds.length !== attemptedPairCount) {
      return indeterminateStep('scan_error')
    }
    return selectedSeeds.length === 0 ? completeStep() : null
  }

  const advanceTrianglePairCursor = () => {
    const candidate = preparedCandidates[candidateIndex]
    if (!candidate) return false
    secondTriangleIndex += 1
    if (secondTriangleIndex < candidate.secondPrisms.length) return true
    secondTriangleIndex = 0
    firstTriangleIndex += 1
    if (firstTriangleIndex < candidate.firstPrisms.length) return true
    firstTriangleIndex = 0
    candidateIndex += 1
    return candidateIndex < preparedCandidates.length
  }

  const processTrianglePair = () => {
    const candidate = preparedCandidates[candidateIndex]
    const first = candidate?.firstPrisms[firstTriangleIndex]
    const second = candidate?.secondPrisms[secondTriangleIndex]
    if (!candidate || !first || !second) {
      return indeterminateStep('scan_error')
    }

    trianglePairTests += 1
    if (
      trianglePairTests > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
    ) return indeterminateStep('work_accounting_error')

    if (!boundsOverlap(first.bounds, second.bounds, numericalMargin)) {
      aabbRejectedPairCount += 1
    } else {
      satTests += 1
      const intersection = classifyTrianglePrisms(
        first,
        second,
        numericalMargin,
        exactTransversalProofBudget,
      )
      if (terminal) return terminal
      if (cancelled) return cancelledStep()
      if (!intersection) return indeterminateStep('scan_error')
      const intersectionClass = intersection.geometryClass
      if (intersectionClass === 'separated') {
        satSeparatedPairCount += 1
      } else if (intersection.topologyContact) {
        allowedSharedVertexPairCount += 1
      } else if (intersectionClass === 'indeterminate') {
        indeterminatePairCount += 1
      } else {
        if (intersectionClass === 'penetrating') {
          penetratingPairCount += 1
        } else {
          touchingPairCount += 1
        }
        if (
          eligibleSeeds.length
          < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
        ) {
          eligibleSeeds.push(Object.freeze({
            firstFaceId: candidate.firstFaceId,
            secondFaceId: candidate.secondFaceId,
            geometryClass: intersectionClass,
            first,
            second,
          }))
        }
      }
    }

    return advanceTrianglePairCursor()
      ? null
      : enterWitnessDerivationPhase()
  }

  const processWitnessDerivation = () => {
    const seed = selectedSeeds[witnessIndex]
    if (!seed) return indeterminateStep('scan_error')
    witnessDerivations += 1

    let witness: FoldPreviewTrianglePrismWitness | null = null
    if (seed.first.witnessFrame) {
      witness = deriveFoldPreviewTrianglePrismWitness({
        firstVertices: seed.first.vertices,
        secondVertices: seed.second.vertices,
        firstFrame: seed.first.witnessFrame,
        numericalMargin,
        authoritativeGeometryClass: seed.geometryClass,
      })
      if (terminal) return terminal
      if (cancelled) return cancelledStep()
    }
    if (!witness) {
      unavailablePairCount += 1
    } else {
      witnessSamples.push(Object.freeze({
        firstFaceId: seed.firstFaceId,
        secondFaceId: seed.secondFaceId,
        relation: 'non_adjacent',
        firstTriangleIndex: seed.first.triangleIndex,
        secondTriangleIndex: seed.second.triangleIndex,
        geometryClass: seed.geometryClass,
        witness,
      }))
    }

    witnessIndex += 1
    return witnessIndex === selectedSeeds.length ? completeStep() : null
  }

  const pending = () => Object.freeze({
    version: FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
    kind: 'pending' as const,
    phase,
    work: work(),
    workBounds,
    exactTransversalProofWork:
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      ),
  })

  const checkedStep = (
    value: FoldPreviewFullScanNonAdjacentWitnessJobStep,
    previousWork: FoldPreviewFullScanNonAdjacentWitnessJobWork,
    previousExactWork: FoldPreviewExactTransversalProofWork,
    workBudget: number,
    processed: number,
  ) => {
    const currentWork = work()
    const currentExactWork =
      snapshotFoldPreviewExactTransversalProofWork(
        exactTransversalProofBudget,
      )
    const totalDelta =
      currentWork.totalWorkUnits - previousWork.totalWorkUnits
    const trianglePairDelta =
      currentWork.trianglePairTests - previousWork.trianglePairTests
    const witnessDelta =
      currentWork.witnessDerivations - previousWork.witnessDerivations
    const exactAttemptDelta =
      currentExactWork.attempted - previousExactWork.attempted
    const exactSkippedDelta =
      currentExactWork.skippedByLimit - previousExactWork.skippedByLimit
    const exactRequestDelta = exactAttemptDelta + exactSkippedDelta
    if (
      !Number.isSafeInteger(currentWork.totalWorkUnits)
      || currentWork.totalWorkUnits !== currentWork.trianglePairTests
        + currentWork.witnessDerivations
      || totalDelta < 0
      || trianglePairDelta < 0
      || witnessDelta < 0
      || exactAttemptDelta < 0
      || exactSkippedDelta < 0
      || exactRequestDelta > trianglePairDelta
      || totalDelta !== trianglePairDelta + witnessDelta
      || totalDelta !== processed
      || totalDelta > workBudget
      || trianglePairDelta > workBudget
      || currentWork.trianglePairTests
        > workBounds.expectedTrianglePairCount
      || currentWork.witnessDerivations
        > workBounds.maximumWitnessDerivations
      || currentWork.totalWorkUnits > workBounds.maximumTotalWorkUnits
      || currentExactWork.attempted
        > MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
      || currentExactWork.skippedByLimit > currentWork.trianglePairTests
      || value.work.totalWorkUnits !== currentWork.totalWorkUnits
      || value.work.trianglePairTests !== currentWork.trianglePairTests
      || value.work.witnessDerivations !== currentWork.witnessDerivations
      || value.workBounds !== workBounds
      || !sameFoldPreviewExactTransversalProofWork(
        value.exactTransversalProofWork,
        currentExactWork,
      )
    ) return publish(indeterminateStep('work_accounting_error'))
    return publish(value)
  }

  const runStep = (
    workBudget: number,
    previousWork: FoldPreviewFullScanNonAdjacentWitnessJobWork,
    previousExactWork: FoldPreviewExactTransversalProofWork,
  ): FoldPreviewFullScanNonAdjacentWitnessJobStep => {
    let processed = 0
    while (processed < workBudget) {
      if (terminal) {
        return checkedStep(
          terminal,
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
      if (cancelled) {
        return checkedStep(
          cancelledStep(),
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
      if (
        phase === 'triangle_pair_scan'
        && candidateIndex >= preparedCandidates.length
      ) {
        const result = enterWitnessDerivationPhase()
        if (result) {
          return checkedStep(
            result,
            previousWork,
            previousExactWork,
            workBudget,
            processed,
          )
        }
        continue
      }

      const result = phase === 'triangle_pair_scan'
        ? processTrianglePair()
        : processWitnessDerivation()
      processed += 1
      if (result) {
        return checkedStep(
          result,
          previousWork,
          previousExactWork,
          workBudget,
          processed,
        )
      }
    }
    return checkedStep(
      pending(),
      previousWork,
      previousExactWork,
      workBudget,
      processed,
    )
  }

  return Object.freeze({
    workBounds,
    step(
      workBudget: number,
    ): FoldPreviewFullScanNonAdjacentWitnessJobStep {
      if (terminal) return terminal
      if (cancelled) return publish(cancelledStep())
      if (stepping) {
        cancelled = true
        return publish(cancelledStep())
      }
      stepping = true
      let previousWork:
        FoldPreviewFullScanNonAdjacentWitnessJobWork | null = null
      let previousExactWork:
        FoldPreviewExactTransversalProofWork | null = null
      let validatedWorkBudget: number | null = null
      try {
        previousWork = work()
        previousExactWork =
          snapshotFoldPreviewExactTransversalProofWork(
            exactTransversalProofBudget,
          )
        if (terminal) return terminal
        if (cancelled) return publish(cancelledStep())
        const validWorkBudget =
          Number.isSafeInteger(workBudget) && workBudget > 0
        if (terminal) return terminal
        if (cancelled) return publish(cancelledStep())
        if (!validWorkBudget) {
          return publish(indeterminateStep('invalid_work_budget'))
        }
        validatedWorkBudget = workBudget
        return runStep(
          validatedWorkBudget,
          previousWork,
          previousExactWork,
        )
      } catch {
        if (terminal) return terminal
        if (cancelled) {
          if (
            previousWork
            && previousExactWork
            && validatedWorkBudget !== null
          ) {
            return checkedStep(
              cancelledStep(),
              previousWork,
              previousExactWork,
              validatedWorkBudget,
              work().totalWorkUnits - previousWork.totalWorkUnits,
            )
          }
          return publish(cancelledStep())
        }
        if (
          !previousWork
          || !previousExactWork
          || validatedWorkBudget === null
        ) {
          return publish(indeterminateStep('scan_error'))
        }
        return checkedStep(
          indeterminateStep('scan_error'),
          previousWork,
          previousExactWork,
          validatedWorkBudget,
          work().totalWorkUnits - previousWork.totalWorkUnits,
        )
      } finally {
        stepping = false
      }
    },
    cancel() {
      if (!terminal) cancelled = true
    },
  })
}

function createFullScanNonAdjacentWitnessSet({
  thickness,
  numericalMargin,
  counts,
  witnessSamples,
  unavailablePairCount,
  exactTransversalProofWork,
}: Readonly<{
  thickness: number
  numericalMargin: number
  counts: FullScanNonAdjacentWitnessCounts
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
  unavailablePairCount: number
  exactTransversalProofWork: FoldPreviewExactTransversalProofWork
}>): FoldPreviewFullScanNonAdjacentWitnessSet | null {
  if (
    !validFoldPreviewExactTransversalProofWork(
      exactTransversalProofWork,
    )
    || exactTransversalProofWork.skippedByLimit
      > counts.indeterminatePairCount
  ) return null
  const eligiblePairCount =
    counts.touchingPairCount + counts.penetratingPairCount
  if (!Number.isSafeInteger(eligiblePairCount)) return null
  const attemptedPairCount = Math.min(
    eligiblePairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  const availablePairCount = witnessSamples.length
  const omittedByLimitCount = eligiblePairCount - attemptedPairCount
  const allCollisionConstraintsRepresented =
    counts.indeterminatePairCount === 0
    && unavailablePairCount === 0
    && omittedByLimitCount === 0
    && availablePairCount === eligiblePairCount
  const coverage = freezeFullScanNonAdjacentWitnessCoverage({
    ...counts,
    eligiblePairCount,
    attemptedPairCount,
    availablePairCount,
    unavailablePairCount,
    omittedByLimitCount,
    authoritativePairScanComplete: true,
    allCollisionConstraintsRepresented,
  })
  if (!coverage) return null

  const base = {
    algorithm: 'full_non_adjacent_prism_witness_scan_v2' as const,
    sourcePose: 'analyzed_input_pose' as const,
    requestIdentityBound: false as const,
    collisionThickness: thickness,
    numericalMargin,
    exactTransversalProofWork: Object.freeze({
      algorithm: exactTransversalProofWork.algorithm,
      maximumAttempts: exactTransversalProofWork.maximumAttempts,
      attempted: exactTransversalProofWork.attempted,
      skippedByLimit: exactTransversalProofWork.skippedByLimit,
    }),
    autoApplicable: false as const,
  }
  if (allCollisionConstraintsRepresented) {
    return Object.freeze({
      ...base,
      kind: 'complete',
      coverage:
        coverage as FoldPreviewFullScanNonAdjacentWitnessCoverage<true>,
      witnessSamples: Object.freeze([...witnessSamples]),
    })
  }

  const reasons: FoldPreviewFullScanNonAdjacentWitnessUnavailableReason[] = []
  if (counts.indeterminatePairCount > 0) reasons.push('indeterminate_pair')
  if (omittedByLimitCount > 0) reasons.push('witness_limit_exceeded')
  if (unavailablePairCount > 0) reasons.push('witness_derivation_failed')
  if (reasons.length === 0) return null
  return Object.freeze({
    ...base,
    kind: 'unavailable',
    coverage:
      coverage as FoldPreviewFullScanNonAdjacentWitnessCoverage<false>,
    reasons: Object.freeze(reasons),
    witnessSamples: Object.freeze([]),
  })
}

function snapshotNarrowPhaseInputs(
  faces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  hingeConstraints: readonly FoldPreviewHingeContactConstraint[] | undefined,
) {
  try {
    if (
      !Array.isArray(faces)
      || !Array.isArray(adjacencies)
      || faces.length > MAX_FOLD_PREVIEW_COLLISION_FACES
      || adjacencies.length > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
    ) return null

    const faceIds = new Set<string>()
    const topologyVertexPositions = new Map<string, Readonly<{
      x: number
      z: number
    }>>()
    const preparedFaces: PreparedFoldPreviewNarrowPhaseFace[] = []
    let vertexCount = 0
    for (const face of faces) {
      if (
        !face
        || !validId(face.id)
        || faceIds.has(face.id)
        || !Array.isArray(face.polygon)
        || face.polygon.length < 3
      ) return null
      vertexCount += face.polygon.length
      if (
        !Number.isSafeInteger(vertexCount)
        || vertexCount > MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES
      ) return null

      const polygon = face.polygon.map((
        point: FoldPreviewCollisionPoseFace['polygon'][number],
      ) => {
        if (
          !point
          || !Number.isFinite(point.x)
          || !Number.isFinite(point.z)
          || (point.vertexId !== undefined && !validId(point.vertexId))
        ) {
          throw new RangeError('invalid collision polygon point')
        }
        if (point.vertexId !== undefined) {
          const known = topologyVertexPositions.get(point.vertexId)
          if (known && (known.x !== point.x || known.z !== point.z)) {
            throw new RangeError('topology vertex position mismatch')
          }
          if (!known) {
            topologyVertexPositions.set(point.vertexId, {
              x: point.x,
              z: point.z,
            })
          }
        }
        return point.vertexId === undefined
          ? { x: point.x, z: point.z }
          : { vertexId: point.vertexId, x: point.x, z: point.z }
      })
      const triangles = triangulateFoldPreviewPolygon(polygon).map((triangle) =>
        [...triangle] as FoldPreviewTriangleIndices)
      preparedFaces.push({
        id: face.id,
        polygon,
        triangles,
      })
      faceIds.add(face.id)
    }

    const edgeIds = new Set<string>()
    const adjacencySnapshot: FoldPreviewCollisionAdjacency[] = []
    for (const adjacency of adjacencies) {
      if (
        !adjacency
        || !validId(adjacency.edgeId)
        || edgeIds.has(adjacency.edgeId)
        || !faceIds.has(adjacency.firstFaceId)
        || !faceIds.has(adjacency.secondFaceId)
        || adjacency.firstFaceId === adjacency.secondFaceId
      ) return null
      adjacencySnapshot.push({
        edgeId: adjacency.edgeId,
        firstFaceId: adjacency.firstFaceId,
        secondFaceId: adjacency.secondFaceId,
      })
      edgeIds.add(adjacency.edgeId)
    }

    const poseFaces = preparedFaces.map((face) => ({
      id: face.id,
      polygon: face.polygon,
    }))
    const hingeContactPolicy = hingeConstraints === undefined
      ? null
      : prepareFoldPreviewHingeContactPolicy(
          preparedFaces,
          adjacencySnapshot,
          hingeConstraints,
        )
    if (hingeConstraints !== undefined && !hingeContactPolicy) return null
    return {
      preparedFaces,
      poseFaces,
      adjacencySnapshot,
      hingeContactPolicy,
    }
  } catch {
    return null
  }
}

function snapshotRigidFaceTransforms(
  faces: readonly Readonly<{ id: string }>[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
): ReadonlyMap<string, Matrix4> | null {
  try {
    if (!faceTransforms) return null
    const size = faceTransforms.size
    const get = faceTransforms.get
    if (
      size !== faces.length
      || typeof get !== 'function'
    ) return null

    const snapshot = new Map<string, Matrix4>()
    for (const face of faces) {
      const rawTransform = get.call(faceTransforms, face.id)
      if (!rawTransform) return null
      const rawElements = rawTransform.elements
      if (!Array.isArray(rawElements) || rawElements.length !== 16) return null
      const elements: number[] = []
      for (let index = 0; index < 16; index += 1) {
        const element = rawElements[index]
        if (typeof element !== 'number' || !Number.isFinite(element)) return null
        elements.push(element)
      }
      const transform = new Matrix4().fromArray(elements)
      if (!rigidTransform(transform)) return null
      snapshot.set(face.id, transform)
    }
    return snapshot.size === faces.length ? snapshot : null
  } catch {
    return null
  }
}

function buildTrianglePrisms(
  face: FoldPreviewNarrowPhaseFace,
  transform: Matrix4,
  thickness: number,
): readonly TrianglePrism[] | null {
  const triangles = face.triangles ?? triangulateFoldPreviewPolygon(face.polygon)
  const halfThickness = thickness / 2
  if (!Number.isFinite(halfThickness) || halfThickness < 0) return null
  const witnessFrame = witnessFrameForTransform(transform)
  const prisms: TrianglePrism[] = []
  for (let triangleIndex = 0; triangleIndex < triangles.length; triangleIndex += 1) {
    const triangle = triangles[triangleIndex]
    const midSurface = triangle.map((index) => transformedPoint(
      face.polygon[index].x,
      0,
      face.polygon[index].z,
      transform,
    ))
    const top = triangle.map((index) => transformedPoint(
      face.polygon[index].x,
      halfThickness,
      face.polygon[index].z,
      transform,
    ))
    const bottom = triangle.map((index) => transformedPoint(
      face.polygon[index].x,
      -halfThickness,
      face.polygon[index].z,
      transform,
    ))
    if ([...midSurface, ...top, ...bottom].some((point) => !point)) return null
    const midSurfaceVertices = midSurface as Vector3[]
    const topVertices = top as Vector3[]
    const bottomVertices = bottom as Vector3[]
    const vertices = [...topVertices, ...bottomVertices]
    const canonicalPositions = canonicalTriangleVertexPositions(
      face,
      triangle,
    )
    if (!canonicalPositions) return null
    const canonicalPoints = canonicalPositions.map((position) =>
      face.polygon[triangle[position]])
    if (canonicalPoints.some((point) => !point)) return null
    const firstEdge = transformedRestEdgeDirection(
      canonicalPoints[0],
      canonicalPoints[1],
      transform,
    )
    const secondEdge = transformedRestEdgeDirection(
      canonicalPoints[1],
      canonicalPoints[2],
      transform,
    )
    const thirdEdge = transformedRestEdgeDirection(
      canonicalPoints[2],
      canonicalPoints[0],
      transform,
    )
    const baseNormal = transformedLocalYAxis(transform)
    if (!firstEdge || !secondEdge || !thirdEdge) return null
    if (!baseNormal) return null
    const extrusionDirection = thickness > 0
      ? baseNormal.clone().multiplyScalar(-1)
      : null
    const edgeDirections = [firstEdge, secondEdge, thirdEdge]
    if (extrusionDirection) edgeDirections.push(extrusionDirection)

    const faceAxes = [baseNormal]
    for (const edge of edgeDirections.slice(0, 3)) {
      const sideAxis = normalized(thickness > 0
        ? edge.clone().cross(extrusionDirection as Vector3)
        : edge.clone().cross(baseNormal))
      if (!sideAxis) return null
      faceAxes.push(sideAxis)
    }
    const bounds = boundsForVertices(vertices)
    if (!bounds) return null
    prisms.push({
      triangleIndex,
      midSurfaceVertices,
      vertices,
      topologyVertices: triangle.map((index) => ({
        vertexId: face.polygon[index].vertexId ?? null,
        x: face.polygon[index].x,
        z: face.polygon[index].z,
      })),
      faceAxes,
      edgeDirections,
      zeroThickness: thickness === 0,
      witnessFrame,
      bounds,
    })
  }
  return prisms.length === triangles.length && prisms.length > 0 ? prisms : null
}

/**
 * Produces one geometry-derived triangle order for SAT axis construction.
 *
 * Polygon start position and winding are serialization choices, not geometry.
 * Computing nearly parallel cross axes from those arbitrary orders can make
 * one permutation round to an exact redundant axis while another rounds to a
 * subnormal non-zero axis. The material and diagnostic vertex snapshots stay
 * in their authoritative input order; only the mathematically unordered SAT
 * axis set uses this canonical order.
 */
function canonicalTriangleVertexPositions(
  face: FoldPreviewNarrowPhaseFace,
  triangle: FoldPreviewTriangleIndices,
): readonly [number, number, number] | null {
  const positions = [0, 1, 2] as [number, number, number]
  for (const position of positions) {
    const vertexIndex = triangle[position]
    if (!Number.isSafeInteger(vertexIndex) || !face.polygon[vertexIndex]) {
      return null
    }
  }
  positions.sort((firstPosition, secondPosition) => {
    const first = face.polygon[triangle[firstPosition]]
    const second = face.polygon[triangle[secondPosition]]
    if (!first || !second) return firstPosition - secondPosition
    if (first.x !== second.x) return first.x < second.x ? -1 : 1
    if (first.z !== second.z) return first.z < second.z ? -1 : 1
    const firstId = first.vertexId ?? ''
    const secondId = second.vertexId ?? ''
    if (firstId !== secondId) return firstId < secondId ? -1 : 1
    return firstPosition - secondPosition
  })
  return positions
}

function transformedRestEdgeDirection(
  first: FoldPreviewNarrowPhaseFace['polygon'][number],
  second: FoldPreviewNarrowPhaseFace['polygon'][number],
  transform: Matrix4,
) {
  if (!first || !second) return null
  const x = second.x - first.x
  const z = second.z - first.z
  const elements = transform.elements
  if (
    !Number.isFinite(x)
    || !Number.isFinite(z)
    || !Array.isArray(elements)
    || elements.length !== 16
  ) return null
  // Apply only the already validated rigid transform's linear component.
  // Re-subtracting two translated world points would reintroduce cancellation.
  return normalized(new Vector3(
    elements[0] * x + elements[8] * z,
    elements[1] * x + elements[9] * z,
    elements[2] * x + elements[10] * z,
  ))
}

function transformedLocalYAxis(transform: Matrix4) {
  const elements = transform.elements
  if (!Array.isArray(elements) || elements.length !== 16) return null
  return normalized(new Vector3(
    elements[4],
    elements[5],
    elements[6],
  ))
}

function classifyTrianglePrisms(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
  exactTransversalProofBudget: FoldPreviewExactTransversalProofBudget,
): PrismIntersectionClassification | null {
  const projectionFrame = prismPairProjectionFrame(first, second, margin)
  if (!projectionFrame) return null
  const topologyMargin = projectionFrame.topologyMargin
  const sharedVertexSurface = classifySharedVertexMidSurfaceContact(
    first,
    second,
    topologyMargin,
    exactTransversalProofBudget,
  )
  const sharedVertexExactAttempted = sharedVertexSurface !== null
  if (sharedVertexSurface?.kind === 'transversal') {
    return prismIntersectionClassification('penetrating')
  }

  if (first.zeroThickness || second.zeroThickness) {
    const surfaceIntersection = first.zeroThickness && second.zeroThickness
      ? classifyZeroThicknessTriangles(first, second, margin)
      : zeroThicknessIntersection('indeterminate')
    if (!surfaceIntersection) return null
    // A tolerance-based contact/unknown cannot overrule a strict intersection
    // proof over the exact stored coordinates.
    if (
      surfaceIntersection.geometryClass === 'touching'
      || surfaceIntersection.geometryClass === 'indeterminate'
    ) {
      if (!sharedVertexExactAttempted) {
        const exactDecision = attemptTransversalTriangleIntersectionProof(
          first,
          second,
          topologyMargin,
          exactTransversalProofBudget,
        )
        if (exactDecision === 'proved') {
          return prismIntersectionClassification('penetrating')
        }
        if (exactDecision === 'budget_exhausted') {
          return prismIntersectionClassification('indeterminate')
        }
      }
    }
    return combineSharedVertexTopologyDecision(
      surfaceIntersection.geometryClass,
      0,
      sharedVertexSurface,
      first,
      second,
    )
  }

  const axes = [...first.faceAxes, ...second.faceAxes]
  let uncertainAxis = false
  for (const firstEdge of first.edgeDirections) {
    for (const secondEdge of second.edgeDirections) {
      const cross = firstEdge.clone().cross(secondEdge)
      const length = cross.length()
      if (!Number.isFinite(length)) return null
      if (length === 0) continue
      if (length <= PARALLEL_AXIS_TOLERANCE) {
        uncertainAxis = true
        continue
      }
      axes.push(cross.multiplyScalar(1 / length))
    }
  }
  if (axes.length === 0) return null

  let exactBoundaryContact = false
  let subMarginUncertainty = false
  for (const axis of axes) {
    const firstProjection = projectVertices(
      first.vertices,
      axis,
      projectionFrame.origin,
    )
    const secondProjection = projectVertices(
      second.vertices,
      axis,
      projectionFrame.origin,
    )
    if (!firstProjection || !secondProjection) return null
    const gap = Math.max(
      secondProjection.min - firstProjection.max,
      firstProjection.min - secondProjection.max,
    )
    if (gap > projectionFrame.margin) {
      return prismIntersectionClassification('separated')
    }
    const overlap = Math.min(firstProjection.max, secondProjection.max)
      - Math.max(firstProjection.min, secondProjection.min)
    if (!Number.isFinite(gap) || !Number.isFinite(overlap)) return null
    if (overlap < 0) subMarginUncertainty = true
    else if (overlap === 0) {
      // Once absolute-world binary64 rounding dominates the pair's local
      // error scale, a stored zero may be a collapsed positive gap or overlap.
      // It is no longer affirmative evidence of true boundary contact.
      if (
        projectionAxisWorldRoundingMargin(
          projectionFrame.worldCoordinateScale,
          axis,
        ) > projectionFrame.localMargin
      ) {
        subMarginUncertainty = true
      } else {
        exactBoundaryContact = true
      }
    } else if (overlap <= projectionFrame.margin) {
      subMarginUncertainty = true
    }
  }
  if (uncertainAxis) {
    const rawGeometryClass = sharedVertexExactAttempted
      ? 'indeterminate'
      : attemptTransversalTriangleIntersectionProof(
          first,
          second,
          topologyMargin,
          exactTransversalProofBudget,
        ) === 'proved'
        ? 'penetrating'
        : 'indeterminate'
    return combineSharedVertexTopologyDecision(
      rawGeometryClass,
      1,
      sharedVertexSurface,
      first,
      second,
    )
  }
  // Exact zero is contact. A signed non-zero gap or overlap inside the error
  // band is unresolved, never contact. Crossing central surfaces can still
  // prove positive volume without widening that band.
  if (exactBoundaryContact || subMarginUncertainty) {
    if (!sharedVertexExactAttempted) {
      const exactDecision = attemptTransversalTriangleIntersectionProof(
        first,
        second,
        topologyMargin,
        exactTransversalProofBudget,
      )
      if (exactDecision === 'proved') {
        return prismIntersectionClassification('penetrating')
      }
      if (exactDecision === 'budget_exhausted') {
        return prismIntersectionClassification('indeterminate')
      }
    }
  }
  return combineSharedVertexTopologyDecision(
    subMarginUncertainty
      ? 'indeterminate'
      : exactBoundaryContact
        ? 'touching'
        : 'penetrating',
    1,
    sharedVertexSurface,
    first,
    second,
  )
}

function prismIntersectionClassification(
  geometryClass: PrismIntersection,
  topologyContact?: FoldPreviewAllowedSharedVertexPair,
  rawGeometryClass = geometryClass,
): PrismIntersectionClassification {
  return topologyContact
    ? { geometryClass, rawGeometryClass, topologyContact }
    : { geometryClass, rawGeometryClass }
}

type SharedVertexMidSurfaceEvidence =
  | Readonly<{
      kind: 'certified_shared_vertex_only'
      certificate: FoldPreviewCertifiedSharedVertexGeometry
    }>
  | Readonly<{ kind: 'transversal'; sharedVertexId: string }>
  | Readonly<{ kind: 'not_proved'; sharedVertexId: string }>
  | Readonly<{ kind: 'indeterminate'; sharedVertexId: string }>

function combineSharedVertexTopologyDecision(
  rawGeometryClass: PrismIntersection,
  thicknessClass: 0 | 1,
  evidence: SharedVertexMidSurfaceEvidence | null,
  first: TrianglePrism,
  second: TrianglePrism,
): PrismIntersectionClassification {
  if (!evidence || evidence.kind === 'not_proved') {
    return prismIntersectionClassification(rawGeometryClass)
  }
  if (evidence.kind === 'transversal') {
    return prismIntersectionClassification('penetrating')
  }
  if (evidence.kind === 'indeterminate') {
    return prismIntersectionClassification(
      'indeterminate',
      undefined,
      rawGeometryClass,
    )
  }
  return dispatchCertifiedSharedVertexTopologyContact(
    evidence.certificate,
    rawGeometryClass,
    thicknessClass,
    first,
    second,
  )
}

/**
 * Converts an analyzer-issued geometry proof into one policy decision.
 * A copied object, a pure string-table result, or a contradictory separated
 * SAT result cannot authorize a collision exemption.
 */
function dispatchCertifiedSharedVertexTopologyContact(
  certificate: FoldPreviewCertifiedSharedVertexGeometry,
  rawGeometryClass: PrismIntersection,
  thicknessClass: 0 | 1,
  first: TrianglePrism,
  second: TrianglePrism,
): PrismIntersectionClassification {
  // An exact shared point and a separating axis cannot both describe the same
  // canonical triangle pair. Preserve the contradiction as blocking evidence.
  if (
    rawGeometryClass === 'separated'
    || (rawGeometryClass === 'indeterminate' && thicknessClass === 0)
  ) {
    return prismIntersectionClassification(
      'indeterminate',
      undefined,
      rawGeometryClass,
    )
  }

  const runtimeEvidence = issueSharedVertexRuntimeEvidence(
    certificate,
    first,
    second,
    thicknessClass,
    rawGeometryClass,
  )
  if (!runtimeEvidence) {
    return prismIntersectionClassification(
      'indeterminate',
      undefined,
      rawGeometryClass,
    )
  }
  return dispatchSharedVertexRuntimeEvidence(
    runtimeEvidence,
    first,
    second,
    thicknessClass,
    rawGeometryClass,
  )
}

function dispatchSharedVertexRuntimeEvidence(
  runtimeEvidence: FoldPreviewCertifiedSharedVertexRuntimeEvidence,
  first: TrianglePrism,
  second: TrianglePrism,
  thicknessClass: 0 | 1,
  rawGeometryClass: PrismIntersection,
): PrismIntersectionClassification {
  if (
    !isTrustedSharedVertexRuntimeEvidence(
      runtimeEvidence,
      first,
      second,
      thicknessClass,
      rawGeometryClass,
    )
  ) {
    return prismIntersectionClassification(
      'indeterminate',
      undefined,
      rawGeometryClass,
    )
  }
  const decision = classifyFoldPreviewTopologyContact(
    runtimeEvidence.topology,
    runtimeEvidence.evidence,
  )
  if (decision !== 'allowed_shared_vertex_contact') {
    return prismIntersectionClassification(
      'indeterminate',
      undefined,
      rawGeometryClass,
    )
  }
  return prismIntersectionClassification(
    'touching',
    {
      policyVersion: FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
      topology: runtimeEvidence.topology,
      evidence: runtimeEvidence.evidence,
      decision,
      sharedVertexId: runtimeEvidence.sharedVertexId,
    },
    rawGeometryClass,
  )
}

function isTrustedSharedVertexGeometryCertificate(
  value: FoldPreviewCertifiedSharedVertexGeometry,
  first: TrianglePrism,
  second: TrianglePrism,
  thicknessClass: 0 | 1,
) {
  try {
    if (
      typeof value !== 'object'
      || value === null
      || !reflectApplyIntrinsic(
        weakSetHasIntrinsic,
        trustedSharedVertexGeometryCertificates,
        [value],
      )
    ) return false
    const provenance = reflectApplyIntrinsic(
      weakMapGetIntrinsic,
      trustedSharedVertexGeometryProvenance,
      [value],
    ) as FoldPreviewSharedVertexGeometryProvenance | undefined
    return value.policyVersion
        === FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
      && value.topology === 'shared_vertex'
      && value.proof
        === 'binary64_shared_vertex_only_with_cooriented_material_normals_v1'
      && validId(value.sharedVertexId)
      && Object.isFrozen(value)
      && provenance?.first === first
      && provenance.second === second
      && provenance.thicknessClass === thicknessClass
  } catch {
    return false
  }
}

function issueSharedVertexRuntimeEvidence(
  geometryCertificate: FoldPreviewCertifiedSharedVertexGeometry,
  first: TrianglePrism,
  second: TrianglePrism,
  thicknessClass: 0 | 1,
  rawGeometryClass: PrismIntersection,
): FoldPreviewCertifiedSharedVertexRuntimeEvidence | null {
  try {
    if (
      !isTrustedSharedVertexGeometryCertificate(
        geometryCertificate,
        first,
        second,
        thicknessClass,
      )
    ) return null
    const topologyEvidence = thicknessClass === 1
        && (
          rawGeometryClass === 'penetrating'
          || rawGeometryClass === 'indeterminate'
        )
      ? 'shared_feature_thickness_overlap'
      : 'shared_feature_contact'
    const runtimeEvidence = Object.freeze({
      policyVersion: FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
      topology: 'shared_vertex' as const,
      evidence: topologyEvidence,
      proof: 'certified_shared_vertex_runtime_evidence_v1' as const,
      sharedVertexId: geometryCertificate.sharedVertexId,
    })
    const provenance = Object.freeze({
      geometryCertificate,
      first,
      second,
      thicknessClass,
      rawGeometryClass,
    })
    reflectApplyIntrinsic(
      weakSetAddIntrinsic,
      trustedSharedVertexRuntimeEvidence,
      [runtimeEvidence],
    )
    reflectApplyIntrinsic(
      weakMapSetIntrinsic,
      trustedSharedVertexRuntimeProvenance,
      [runtimeEvidence, provenance],
    )
    return runtimeEvidence
  } catch {
    return null
  }
}

function isTrustedSharedVertexRuntimeEvidence(
  value: FoldPreviewCertifiedSharedVertexRuntimeEvidence,
  first: TrianglePrism,
  second: TrianglePrism,
  thicknessClass: 0 | 1,
  rawGeometryClass: PrismIntersection,
) {
  try {
    if (
      typeof value !== 'object'
      || value === null
      || !reflectApplyIntrinsic(
        weakSetHasIntrinsic,
        trustedSharedVertexRuntimeEvidence,
        [value],
      )
    ) return false
    const provenance = reflectApplyIntrinsic(
      weakMapGetIntrinsic,
      trustedSharedVertexRuntimeProvenance,
      [value],
    ) as FoldPreviewSharedVertexRuntimeProvenance | undefined
    return value.policyVersion
        === FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION
      && value.topology === 'shared_vertex'
      && (
        value.evidence === 'shared_feature_contact'
        || value.evidence === 'shared_feature_thickness_overlap'
      )
      && value.proof === 'certified_shared_vertex_runtime_evidence_v1'
      && validId(value.sharedVertexId)
      && Object.isFrozen(value)
      && provenance?.first === first
      && provenance.second === second
      && provenance.thicknessClass === thicknessClass
      && provenance.rawGeometryClass === rawGeometryClass
      && provenance.geometryCertificate.sharedVertexId
        === value.sharedVertexId
      && isTrustedSharedVertexGeometryCertificate(
        provenance.geometryCertificate,
        first,
        second,
        thicknessClass,
      )
  } catch {
    return false
  }
}

/**
 * Called only after the exact binary64 singleton proof has succeeded.  The
 * material normals are derived from each rigid transform's local +Y axis, so
 * this proof is invariant to triangle serialization order and world position.
 */
function issueSharedVertexGeometryCertificate(
  first: TrianglePrism,
  second: TrianglePrism,
  sharedVertexId: string,
): FoldPreviewCertifiedSharedVertexGeometry | null {
  try {
    const firstNormal = first.faceAxes[0]
    const secondNormal = second.faceAxes[0]
    if (
      !firstNormal
      || !secondNormal
      || !validId(sharedVertexId)
      || !finiteVectors([firstNormal, secondNormal])
      || first.zeroThickness !== second.zeroThickness
    ) return null
    const materialNormalDot = firstNormal.dot(secondNormal)
    if (
      !Number.isFinite(materialNormalDot)
      || materialNormalDot
        <= SHARED_VERTEX_COORIENTED_NORMAL_DOT_TOLERANCE
    ) return null
    const certificate = Object.freeze({
      policyVersion: FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
      topology: 'shared_vertex' as const,
      proof: (
        'binary64_shared_vertex_only_with_cooriented_material_normals_v1'
      ) as const,
      sharedVertexId,
    })
    reflectApplyIntrinsic(
      weakSetAddIntrinsic,
      trustedSharedVertexGeometryCertificates,
      [certificate],
    )
    reflectApplyIntrinsic(
      weakMapSetIntrinsic,
      trustedSharedVertexGeometryProvenance,
      [
        certificate,
        Object.freeze({
          first,
          second,
          thicknessClass: first.zeroThickness ? 0 as const : 1 as const,
        }),
      ],
    )
    return certificate
  } catch {
    return null
  }
}

function attemptTransversalTriangleIntersectionProof(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
  budget: FoldPreviewExactTransversalProofBudget,
): FoldPreviewExactTransversalProofDecision {
  if (!reserveExactIntersectionCertificateAttempt(budget)) {
    return 'budget_exhausted'
  }
  const triangles = topologyCanonicalMidSurfaceTriangles(
    first,
    second,
    margin,
  )
  if (!triangles) return 'not_proved'
  return provesFoldPreviewBinary64TransversalTriangleIntersection(
    triangles.first,
    triangles.second,
  )
    ? 'proved'
    : 'not_proved'
}

function reserveExactIntersectionCertificateAttempt(
  budget: FoldPreviewExactTransversalProofBudget,
) {
  if (
    budget.attempted
    >= MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
  ) {
    budget.skippedByLimit += 1
    return false
  }
  budget.attempted += 1
  return true
}

type MatchingTopologyVertex = Readonly<{
  vertexId: string
  firstIndex: number
  secondIndex: number
}>

/**
 * Finds topology vertices that are genuinely identical in the immutable
 * crease-pattern snapshot. A matching ID with different rest coordinates is
 * malformed input, not evidence for contact.
 */
function matchingTopologyVertices(
  first: TrianglePrism,
  second: TrianglePrism,
): readonly MatchingTopologyVertex[] | null {
  if (
    first.topologyVertices.length !== 3
    || second.topologyVertices.length !== 3
  ) return null
  const matches: MatchingTopologyVertex[] = []
  const matchedIds = new Set<string>()
  for (
    let firstIndex = 0;
    firstIndex < first.topologyVertices.length;
    firstIndex += 1
  ) {
    const firstTopology = first.topologyVertices[firstIndex]
    if (!firstTopology || firstTopology.vertexId === null) continue
    for (
      let secondIndex = 0;
      secondIndex < second.topologyVertices.length;
      secondIndex += 1
    ) {
      const secondTopology = second.topologyVertices[secondIndex]
      if (
        !secondTopology
        || secondTopology.vertexId !== firstTopology.vertexId
      ) continue
      if (
        secondTopology.x !== firstTopology.x
        || secondTopology.z !== firstTopology.z
        || matchedIds.has(firstTopology.vertexId)
      ) return null
      matchedIds.add(firstTopology.vertexId)
      matches.push({
        vertexId: firstTopology.vertexId,
        firstIndex,
        secondIndex,
      })
    }
  }
  return matches
}

/**
 * Returns central-surface triangles with matching topology vertices represented
 * by one identical stored point when their independently transformed values
 * differ only inside the certified numerical margin.
 *
 * The representative is selected from the two existing binary64 points in a
 * symmetric lexical order. No averaged coordinate is invented, and a shared ID
 * farther apart than the margin is deliberately left unsnapped.
 */
function topologyCanonicalMidSurfaceTriangles(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
): Readonly<{
  first: readonly Vector3[]
  second: readonly Vector3[]
}> | null {
  if (
    !Number.isFinite(margin)
    || margin < 0
    || first.midSurfaceVertices.length !== 3
    || second.midSurfaceVertices.length !== 3
    || !finiteVectors([
      ...first.midSurfaceVertices,
      ...second.midSurfaceVertices,
    ])
  ) return null
  const matches = matchingTopologyVertices(first, second)
  if (!matches) return null
  const firstVertices = first.midSurfaceVertices.map((point) => point.clone())
  const secondVertices = second.midSurfaceVertices.map((point) => point.clone())
  for (const match of matches) {
    const firstPoint = firstVertices[match.firstIndex]
    const secondPoint = secondVertices[match.secondIndex]
    if (!firstPoint || !secondPoint) return null
    const distance = firstPoint.distanceTo(secondPoint)
    if (!Number.isFinite(distance)) return null
    if (distance > margin) continue
    const canonical = lexicographicallyEarlierPoint(firstPoint, secondPoint)
      .clone()
    firstVertices[match.firstIndex] = canonical.clone()
    secondVertices[match.secondIndex] = canonical
  }
  return { first: firstVertices, second: secondVertices }
}

function lexicographicallyEarlierPoint(first: Vector3, second: Vector3) {
  if (first.x !== second.x) return first.x < second.x ? first : second
  if (first.y !== second.y) return first.y < second.y ? first : second
  if (first.z !== second.z) return first.z < second.z ? first : second
  return first
}

/**
 * Grants the centered-thickness shared-vertex allowance only after exact
 * binary64 arithmetic positively proves that the complete central-surface
 * intersection is the one matching topology vertex. A false proof result is
 * unresolved and falls through to the ordinary SAT/crossing classifier.
 */
function classifySharedVertexMidSurfaceContact(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
  budget: FoldPreviewExactTransversalProofBudget,
): SharedVertexMidSurfaceEvidence | null {
  const matches = matchingTopologyVertices(first, second)
  if (!matches || matches.length !== 1) return null
  const match = matches[0]
  const firstPoint = first.midSurfaceVertices[match.firstIndex]
  const secondPoint = second.midSurfaceVertices[match.secondIndex]
  if (!firstPoint || !secondPoint) return null
  const sharedPointDistance = firstPoint.distanceTo(secondPoint)
  if (!Number.isFinite(sharedPointDistance)) {
    return { kind: 'indeterminate', sharedVertexId: match.vertexId }
  }
  if (sharedPointDistance > margin) {
    return { kind: 'indeterminate', sharedVertexId: match.vertexId }
  }

  const canonical = topologyCanonicalMidSurfaceTriangles(
    first,
    second,
    margin,
  )
  if (!canonical) {
    return { kind: 'indeterminate', sharedVertexId: match.vertexId }
  }
  if (!reserveExactIntersectionCertificateAttempt(budget)) {
    return { kind: 'indeterminate', sharedVertexId: match.vertexId }
  }
  if (
    provesFoldPreviewBinary64SharedVertexOnlyIntersection(
      canonical.first,
      canonical.second,
      match.firstIndex,
      match.secondIndex,
    )
  ) {
    const certificate = issueSharedVertexGeometryCertificate(
      first,
      second,
      match.vertexId,
    )
    return certificate
      ? { kind: 'certified_shared_vertex_only', certificate }
      : { kind: 'indeterminate', sharedVertexId: match.vertexId }
  }
  if (
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      canonical.first,
      canonical.second,
    )
  ) return { kind: 'transversal', sharedVertexId: match.vertexId }
  return { kind: 'not_proved', sharedVertexId: match.vertexId }
}

type TrianglePlaneSection = Readonly<{
  points: readonly Vector3[]
  crossesInterior: boolean
  crossesInteriorByRawSigns: boolean
  hasSubMarginPlaneDistance: boolean
  mergedNumericallyDistinctPoint: boolean
}>

type ZeroThicknessTriangleIntersection = Readonly<{
  geometryClass: PrismIntersection
  sharedTopologyContact: 'vertex' | 'edge' | null
}>

function zeroThicknessIntersection(
  geometryClass: PrismIntersection,
  sharedTopologyContact: ZeroThicknessTriangleIntersection[
    'sharedTopologyContact'
  ] = null,
): ZeroThicknessTriangleIntersection {
  return { geometryClass, sharedTopologyContact }
}

type ProjectionRange = Readonly<{
  min: number
  max: number
}>

/**
 * Classifies the intersection dimension of two ideal paper triangles.
 *
 * A point or boundary segment is contact. A coplanar area overlap, or a
 * positive-length line segment where both triangle interiors cross the other
 * plane, is penetration. This geometric proof is intentionally independent of
 * face-level shared vertex IDs: merely sharing a topology vertex never grants
 * an exemption.
 */
function classifyZeroThicknessTriangles(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
): ZeroThicknessTriangleIntersection | null {
  const firstVertices = first.midSurfaceVertices.slice(0, 3)
  const secondVertices = second.midSurfaceVertices.slice(0, 3)
  const firstNormal = first.faceAxes[0]?.clone()
  const secondNormal = second.faceAxes[0]?.clone()
  if (
    firstVertices.length !== 3
    || secondVertices.length !== 3
    || !firstNormal
    || !secondNormal
    || !finiteVectors([...firstVertices, ...secondVertices])
    || !finiteVector(firstNormal)
    || !finiteVector(secondNormal)
  ) return null

  const firstNormalLength = firstNormal.length()
  const secondNormalLength = secondNormal.length()
  const minimumEdgeLength = Math.min(
    minimumTriangleEdgeLength(firstVertices),
    minimumTriangleEdgeLength(secondVertices),
  )
  const minimumConditionRatio = Math.min(
    triangleConditionRatio(firstVertices),
    triangleConditionRatio(secondVertices),
  )
  if (
    !Number.isFinite(firstNormalLength)
    || !Number.isFinite(secondNormalLength)
    || !Number.isFinite(minimumEdgeLength)
    || !Number.isFinite(minimumConditionRatio)
    || firstNormalLength === 0
    || secondNormalLength === 0
    || minimumEdgeLength === 0
    || minimumConditionRatio <= 0
  ) return null
  const surfaceMargin = margin / Math.max(
    minimumConditionRatio,
    Number.EPSILON,
  )
  if (!Number.isFinite(surfaceMargin)) {
    return zeroThicknessIntersection('indeterminate')
  }
  if (surfaceMargin * 16 >= minimumEdgeLength) {
    return zeroThicknessIntersection('indeterminate')
  }
  firstNormal.multiplyScalar(1 / firstNormalLength)
  secondNormal.multiplyScalar(1 / secondNormalLength)

  const firstDistances = signedPlaneDistances(
    firstVertices,
    secondVertices,
    secondNormal,
  )
  const secondDistances = signedPlaneDistances(
    secondVertices,
    firstVertices,
    firstNormal,
  )
  if (!firstDistances || !secondDistances) return null
  if (
    strictlyOnOneSide(firstDistances, surfaceMargin)
    || strictlyOnOneSide(secondDistances, surfaceMargin)
  ) return zeroThicknessIntersection('separated')

  const intersectionDirection = firstNormal.clone().cross(secondNormal)
  const directionLength = intersectionDirection.length()
  if (!Number.isFinite(directionLength)) return null
  if (directionLength <= PARALLEL_AXIS_TOLERANCE) {
    const maximumPlaneDistance = Math.max(
      ...firstDistances.map(Math.abs),
      ...secondDistances.map(Math.abs),
    )
    if (!Number.isFinite(maximumPlaneDistance)) return null
    // Near-parallel binary64 normals can manufacture a one-sided distance
    // even when the exact stored triangles cross. Keep this unresolved so the
    // exact transversal certificate runs before any separated verdict.
    if (maximumPlaneDistance > surfaceMargin) {
      return zeroThicknessIntersection('indeterminate')
    }
    if (maximumPlaneDistance > 0) {
      return zeroThicknessIntersection('indeterminate')
    }
    const exactlyParallel = intersectionDirection.x === 0
      && intersectionDirection.y === 0
      && intersectionDirection.z === 0
    if (!exactlyParallel) return zeroThicknessIntersection('indeterminate')
    const coplanar = classifyCoplanarTriangleOverlap(
      firstVertices,
      secondVertices,
      firstNormal,
      surfaceMargin,
    )
    return coplanar ? zeroThicknessIntersection(coplanar) : null
  }
  intersectionDirection.multiplyScalar(1 / directionLength)

  const firstSection = trianglePlaneSection(
    firstVertices,
    firstDistances,
    intersectionDirection,
    surfaceMargin,
  )
  const secondSection = trianglePlaneSection(
    secondVertices,
    secondDistances,
    intersectionDirection,
    surfaceMargin,
  )
  if (!firstSection || !secondSection) return null
  if (firstSection.points.length === 0 || secondSection.points.length === 0) {
    return firstSection.hasSubMarginPlaneDistance
      || secondSection.hasSubMarginPlaneDistance
      || firstSection.mergedNumericallyDistinctPoint
      || secondSection.mergedNumericallyDistinctPoint
      ? zeroThicknessIntersection('indeterminate')
      : zeroThicknessIntersection('separated')
  }
  const firstRange = projectPointRange(
    firstSection.points,
    intersectionDirection,
    firstVertices[0],
  )
  const secondRange = projectPointRange(
    secondSection.points,
    intersectionDirection,
    firstVertices[0],
  )
  if (!firstRange || !secondRange) return null
  const gap = Math.max(
    secondRange.min - firstRange.max,
    firstRange.min - secondRange.max,
  )
  if (!Number.isFinite(gap)) return null
  if (gap > surfaceMargin) return zeroThicknessIntersection('separated')
  const overlap = Math.min(firstRange.max, secondRange.max)
    - Math.max(firstRange.min, secondRange.min)
  const overlapMinimum = Math.max(firstRange.min, secondRange.min)
  const overlapMaximum = Math.min(firstRange.max, secondRange.max)
  if (!Number.isFinite(overlap)) return null
  const mergedNumericallyDistinctPoint =
    firstSection.mergedNumericallyDistinctPoint
    || secondSection.mergedNumericallyDistinctPoint
  const hasSubMarginPlaneDistance =
    firstSection.hasSubMarginPlaneDistance
    || secondSection.hasSubMarginPlaneDistance
  if (overlap === 0) {
    const contactCoordinate = Math.max(firstRange.min, secondRange.min)
    const sharedTopologyPoint = sharedTopologyPointProvesContact(
      first,
      second,
      firstDistances,
      secondDistances,
      intersectionDirection,
      firstVertices[0],
      contactCoordinate,
      margin,
    )
    const sharedVertexOnly =
      sharedTopologyPoint
      && !firstSection.crossesInteriorByRawSigns
      && !secondSection.crossesInteriorByRawSigns
    if (mergedNumericallyDistinctPoint) {
      return zeroThicknessIntersection('indeterminate')
    }
    if (!hasSubMarginPlaneDistance) {
      return zeroThicknessIntersection(
        'touching',
        sharedVertexOnly ? 'vertex' : null,
      )
    }
    return sharedVertexOnly
      ? zeroThicknessIntersection('touching', 'vertex')
      : zeroThicknessIntersection('indeterminate')
  }
  if (overlap <= surfaceMargin) {
    return zeroThicknessIntersection('indeterminate')
  }
  if (
    !mergedNumericallyDistinctPoint
    && hasSubMarginPlaneDistance
    && !firstSection.crossesInterior
    && !secondSection.crossesInterior
    && sharedTopologySegmentProvesContact(
      first,
      second,
      firstDistances,
      secondDistances,
      intersectionDirection,
      firstVertices[0],
      overlapMinimum,
      overlapMaximum,
      margin,
    )
  ) return zeroThicknessIntersection('touching', 'edge')
  if (mergedNumericallyDistinctPoint || hasSubMarginPlaneDistance) {
    return zeroThicknessIntersection('indeterminate')
  }
  return firstSection.crossesInterior && secondSection.crossesInterior
    ? zeroThicknessIntersection('penetrating')
    : zeroThicknessIntersection('touching')
}

function classifyCoplanarTriangleOverlap(
  first: readonly Vector3[],
  second: readonly Vector3[],
  normal: Vector3,
  margin: number,
): PrismIntersection | null {
  const basisU = first[1].clone().sub(first[0])
  const basisULength = basisU.length()
  if (!Number.isFinite(basisULength) || basisULength === 0) return null
  basisU.multiplyScalar(1 / basisULength)
  const basisV = normal.clone().cross(basisU)
  const basisVLength = basisV.length()
  if (!Number.isFinite(basisVLength) || basisVLength === 0) return null
  basisV.multiplyScalar(1 / basisVLength)

  const first2d = projectTriangleToPlane(
    first,
    basisU,
    basisV,
    first[0],
  )
  const second2d = projectTriangleToPlane(
    second,
    basisU,
    basisV,
    first[0],
  )
  if (!first2d || !second2d) return null
  let boundaryContact = false
  let uncertainBoundary = false
  for (const triangle of [first2d, second2d]) {
    for (let index = 0; index < 3; index += 1) {
      const start = triangle[index]
      const end = triangle[(index + 1) % 3]
      const deltaX = end.x - start.x
      const deltaY = end.y - start.y
      const axisLength = Math.hypot(deltaX, deltaY)
      if (!Number.isFinite(axisLength) || axisLength === 0) return null
      const axisX = -deltaY / axisLength
      const axisY = deltaX / axisLength
      const firstRange = projectPoints2d(first2d, axisX, axisY)
      const secondRange = projectPoints2d(second2d, axisX, axisY)
      if (!firstRange || !secondRange) return null
      const gap = Math.max(
        secondRange.min - firstRange.max,
        firstRange.min - secondRange.max,
      )
      if (!Number.isFinite(gap)) return null
      if (gap > margin) return 'separated'
      const overlap = Math.min(firstRange.max, secondRange.max)
        - Math.max(firstRange.min, secondRange.min)
      if (!Number.isFinite(overlap)) return null
      if (overlap === 0) boundaryContact = true
      else if (overlap <= margin) uncertainBoundary = true
    }
  }
  if (uncertainBoundary) return 'indeterminate'
  return boundaryContact ? 'touching' : 'penetrating'
}

function trianglePlaneSection(
  vertices: readonly Vector3[],
  distances: readonly number[],
  lineDirection: Vector3,
  margin: number,
): TrianglePlaneSection | null {
  if (vertices.length !== 3 || distances.length !== 3) return null
  const hasNumericallyAmbiguousDistance = distances.some((distance) =>
    distance !== 0 && Math.abs(distance) <= margin)
  let mergedNumericallyDistinctPoint = false
  const points: Vector3[] = []
  for (let index = 0; index < 3; index += 1) {
    if (Math.abs(distances[index]) <= margin) {
      mergedNumericallyDistinctPoint =
        addDistinctPoint(points, vertices[index], margin)
        || mergedNumericallyDistinctPoint
    }
  }
  for (let index = 0; index < 3; index += 1) {
    const nextIndex = (index + 1) % 3
    const firstDistance = distances[index]
    const secondDistance = distances[nextIndex]
    if (
      (firstDistance > margin && secondDistance < -margin)
      || (firstDistance < -margin && secondDistance > margin)
    ) {
      const denominator = firstDistance - secondDistance
      if (!Number.isFinite(denominator) || denominator === 0) return null
      const interpolation = firstDistance / denominator
      if (!Number.isFinite(interpolation)) return null
      const point = vertices[index].clone().lerp(
        vertices[nextIndex],
        interpolation,
      )
      if (!finiteVector(point)) return null
      mergedNumericallyDistinctPoint =
        addDistinctPoint(points, point, margin)
        || mergedNumericallyDistinctPoint
    }
  }
  if (points.length > 2) {
    const origin = points[0]
    const range = projectPointRange(points, lineDirection, origin)
    if (!range) return null
    const minimum = points.reduce((best, point) =>
      point.clone().sub(origin).dot(lineDirection)
        < best.clone().sub(origin).dot(lineDirection)
        ? point
        : best)
    const maximum = points.reduce((best, point) =>
      point.clone().sub(origin).dot(lineDirection)
        > best.clone().sub(origin).dot(lineDirection)
        ? point
        : best)
    points.splice(0, points.length, minimum, maximum)
  }
  const hasPositive = distances.some((distance) => distance > margin)
  const hasNegative = distances.some((distance) => distance < -margin)
  return {
    points,
    crossesInterior: hasPositive && hasNegative,
    crossesInteriorByRawSigns:
      distances.some((distance) => distance > 0)
      && distances.some((distance) => distance < 0),
    hasSubMarginPlaneDistance: hasNumericallyAmbiguousDistance,
    mergedNumericallyDistinctPoint,
  }
}

function sharedTopologyPointProvesContact(
  first: TrianglePrism,
  second: TrianglePrism,
  firstDistances: readonly number[],
  secondDistances: readonly number[],
  lineDirection: Vector3,
  projectionOrigin: Vector3,
  contactCoordinate: number,
  margin: number,
) {
  for (
    let firstIndex = 0;
    firstIndex < first.topologyVertices.length;
    firstIndex += 1
  ) {
    const firstTopology = first.topologyVertices[firstIndex]
    const firstWorld = first.midSurfaceVertices[firstIndex]
    if (
      !firstTopology
      || !firstWorld
      || firstTopology.vertexId === null
      || Math.abs(firstDistances[firstIndex] ?? Number.POSITIVE_INFINITY)
        > margin
    ) continue
    for (
      let secondIndex = 0;
      secondIndex < second.topologyVertices.length;
      secondIndex += 1
    ) {
      const secondTopology = second.topologyVertices[secondIndex]
      const secondWorld = second.midSurfaceVertices[secondIndex]
      if (
        !secondTopology
        || !secondWorld
        || secondTopology.vertexId !== firstTopology.vertexId
        || secondTopology.x !== firstTopology.x
        || secondTopology.z !== firstTopology.z
        || Math.abs(secondDistances[secondIndex] ?? Number.POSITIVE_INFINITY)
          > margin
        || firstWorld.distanceTo(secondWorld) > margin
      ) continue
      const firstCoordinate = firstWorld.clone()
        .sub(projectionOrigin)
        .dot(lineDirection)
      const secondCoordinate = secondWorld.clone()
        .sub(projectionOrigin)
        .dot(lineDirection)
      if (
        Number.isFinite(firstCoordinate)
        && Number.isFinite(secondCoordinate)
        && Math.abs(firstCoordinate - contactCoordinate) <= margin
        && Math.abs(secondCoordinate - contactCoordinate) <= margin
      ) return true
    }
  }
  return false
}

function sharedTopologySegmentProvesContact(
  first: TrianglePrism,
  second: TrianglePrism,
  firstDistances: readonly number[],
  secondDistances: readonly number[],
  lineDirection: Vector3,
  projectionOrigin: Vector3,
  overlapMinimum: number,
  overlapMaximum: number,
  margin: number,
) {
  const coordinates: number[] = []
  for (
    let firstIndex = 0;
    firstIndex < first.topologyVertices.length;
    firstIndex += 1
  ) {
    const firstTopology = first.topologyVertices[firstIndex]
    const firstWorld = first.midSurfaceVertices[firstIndex]
    if (
      !firstTopology
      || !firstWorld
      || firstTopology.vertexId === null
      || Math.abs(firstDistances[firstIndex] ?? Number.POSITIVE_INFINITY)
        > margin
    ) continue
    for (
      let secondIndex = 0;
      secondIndex < second.topologyVertices.length;
      secondIndex += 1
    ) {
      const secondTopology = second.topologyVertices[secondIndex]
      const secondWorld = second.midSurfaceVertices[secondIndex]
      if (
        !secondTopology
        || !secondWorld
        || secondTopology.vertexId !== firstTopology.vertexId
        || secondTopology.x !== firstTopology.x
        || secondTopology.z !== firstTopology.z
        || Math.abs(secondDistances[secondIndex] ?? Number.POSITIVE_INFINITY)
          > margin
        || firstWorld.distanceTo(secondWorld) > margin
      ) continue
      const coordinate = firstWorld.clone()
        .sub(projectionOrigin)
        .dot(lineDirection)
      if (Number.isFinite(coordinate)) coordinates.push(coordinate)
    }
  }
  if (coordinates.length < 2) return false
  const minimum = Math.min(...coordinates)
  const maximum = Math.max(...coordinates)
  return maximum - minimum > margin
    && minimum <= overlapMinimum + margin
    && maximum >= overlapMaximum - margin
}

function signedPlaneDistances(
  vertices: readonly Vector3[],
  planeVertices: readonly Vector3[],
  planeNormal: Vector3,
) {
  if (planeVertices.length !== 3) return null
  const distances = vertices.map((vertex) => {
    let closest = planeVertices[0]
    let closestDistanceSquared = vertex.distanceToSquared(closest)
    for (let index = 1; index < planeVertices.length; index += 1) {
      const distanceSquared = vertex.distanceToSquared(planeVertices[index])
      if (
        !Number.isFinite(distanceSquared)
        || distanceSquared >= closestDistanceSquared
      ) continue
      closest = planeVertices[index]
      closestDistanceSquared = distanceSquared
    }
    return vertex.clone().sub(closest).dot(planeNormal)
  })
  return distances.every(Number.isFinite) ? distances : null
}

function strictlyOnOneSide(distances: readonly number[], margin: number) {
  return distances.every((distance) => distance > margin)
    || distances.every((distance) => distance < -margin)
}

function addDistinctPoint(
  target: Vector3[],
  point: Vector3,
  margin: number,
) {
  const duplicateMargin = Math.max(margin, Number.EPSILON)
  for (const candidate of target) {
    const distanceSquared = candidate.distanceToSquared(point)
    if (distanceSquared <= duplicateMargin * duplicateMargin) {
      return distanceSquared > 0
    }
  }
  target.push(point.clone())
  return false
}

function projectPointRange(
  points: readonly Vector3[],
  axis: Vector3,
  origin: Vector3,
): ProjectionRange | null {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const point of points) {
    const projection = point.clone().sub(origin).dot(axis)
    if (!Number.isFinite(projection)) return null
    min = Math.min(min, projection)
    max = Math.max(max, projection)
  }
  return Number.isFinite(min) && Number.isFinite(max) ? { min, max } : null
}

function projectTriangleToPlane(
  vertices: readonly Vector3[],
  basisU: Vector3,
  basisV: Vector3,
  origin: Vector3,
) {
  const points = vertices.map((vertex) => ({
    x: vertex.clone().sub(origin).dot(basisU),
    y: vertex.clone().sub(origin).dot(basisV),
  }))
  return points.every((point) =>
    Number.isFinite(point.x) && Number.isFinite(point.y))
    ? points
    : null
}

function projectPoints2d(
  points: readonly Readonly<{ x: number; y: number }>[],
  axisX: number,
  axisY: number,
): ProjectionRange | null {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const point of points) {
    const projection = point.x * axisX + point.y * axisY
    if (!Number.isFinite(projection)) return null
    min = Math.min(min, projection)
    max = Math.max(max, projection)
  }
  return Number.isFinite(min) && Number.isFinite(max) ? { min, max } : null
}

function finiteVectors(vectors: readonly Vector3[]) {
  return vectors.every(finiteVector)
}

function finiteVector(vector: Vector3) {
  return Number.isFinite(vector.x)
    && Number.isFinite(vector.y)
    && Number.isFinite(vector.z)
}

function minimumTriangleEdgeLength(vertices: readonly Vector3[]) {
  if (vertices.length !== 3) return Number.NaN
  return Math.min(
    vertices[0].distanceTo(vertices[1]),
    vertices[1].distanceTo(vertices[2]),
    vertices[2].distanceTo(vertices[0]),
  )
}

function triangleConditionRatio(vertices: readonly Vector3[]) {
  if (vertices.length !== 3) return Number.NaN
  const firstEdge = vertices[1].clone().sub(vertices[0])
  const secondEdge = vertices[2].clone().sub(vertices[0])
  const maximumEdgeLength = Math.max(
    firstEdge.length(),
    secondEdge.length(),
    vertices[2].distanceTo(vertices[1]),
  )
  const doubledArea = firstEdge.cross(secondEdge).length()
  const squaredMaximumEdgeLength = maximumEdgeLength * maximumEdgeLength
  if (
    !Number.isFinite(doubledArea)
    || !Number.isFinite(squaredMaximumEdgeLength)
    || doubledArea <= 0
    || squaredMaximumEdgeLength <= 0
  ) return Number.NaN
  return doubledArea / squaredMaximumEdgeLength
}

function boundsForVertices(vertices: readonly Vector3[]) {
  const bounds = {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    minZ: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
    maxZ: Number.NEGATIVE_INFINITY,
  }
  for (const vertex of vertices) {
    bounds.minX = Math.min(bounds.minX, vertex.x)
    bounds.minY = Math.min(bounds.minY, vertex.y)
    bounds.minZ = Math.min(bounds.minZ, vertex.z)
    bounds.maxX = Math.max(bounds.maxX, vertex.x)
    bounds.maxY = Math.max(bounds.maxY, vertex.y)
    bounds.maxZ = Math.max(bounds.maxZ, vertex.z)
  }
  return Object.values(bounds).every(Number.isFinite) ? bounds : null
}

function boundsOverlap(
  first: TrianglePrism['bounds'],
  second: TrianglePrism['bounds'],
  margin: number,
) {
  return second.minX - first.maxX <= margin
    && first.minX - second.maxX <= margin
    && second.minY - first.maxY <= margin
    && first.minY - second.maxY <= margin
    && second.minZ - first.maxZ <= margin
    && first.minZ - second.maxZ <= margin
}

function prismPairProjectionFrame(
  first: TrianglePrism,
  second: TrianglePrism,
  authoritativeMargin: number,
): Readonly<{
  origin: Vector3
  margin: number
  topologyMargin: number
  localMargin: number
  worldCoordinateScale: Vector3
}> | null {
  if (!Number.isFinite(authoritativeMargin) || authoritativeMargin < 0) {
    return null
  }
  const vertices = [...first.vertices, ...second.vertices]
  if (vertices.length === 0 || !finiteVectors(vertices)) return null
  let origin = vertices[0]
  let worldScale = 1
  const worldCoordinateScale = new Vector3(1, 1, 1)
  for (const vertex of vertices) {
    origin = lexicographicallyEarlierPoint(origin, vertex)
    worldCoordinateScale.set(
      Math.max(worldCoordinateScale.x, Math.abs(vertex.x)),
      Math.max(worldCoordinateScale.y, Math.abs(vertex.y)),
      Math.max(worldCoordinateScale.z, Math.abs(vertex.z)),
    )
    worldScale = Math.max(
      worldScale,
      Math.abs(vertex.x),
      Math.abs(vertex.y),
      Math.abs(vertex.z),
    )
  }
  let localScale = 1
  for (const vertex of vertices) {
    localScale = Math.max(
      localScale,
      Math.abs(vertex.x - origin.x),
      Math.abs(vertex.y - origin.y),
      Math.abs(vertex.z - origin.z),
    )
  }
  const localMargin =
    calculateFoldPreviewNarrowPhaseNumericalMargin(localScale)
  const storedWorldRoundingMargin = worldScale
    * Number.EPSILON
    * PAIR_WORLD_ROUNDING_MARGIN_FACTOR
  if (
    localMargin === null
    || !Number.isFinite(storedWorldRoundingMargin)
  ) return null
  const pairMargin = Math.min(
    authoritativeMargin,
    Math.max(localMargin, storedWorldRoundingMargin),
  )
  // SAT must cover the rounding already stored in large world coordinates.
  // Topology certification is stricter: a non-zero current shared-feature
  // mismatch cannot be excused by absolute translation alone because this
  // generic pose boundary cannot distinguish kinematic round-off from a
  // physically disconnected input.
  const topologyMargin = Math.min(authoritativeMargin, localMargin)
  return Number.isFinite(pairMargin) && Number.isFinite(topologyMargin)
    ? {
        origin,
        margin: pairMargin,
        topologyMargin,
        localMargin,
        worldCoordinateScale,
      }
    : null
}

function projectionAxisWorldRoundingMargin(
  worldCoordinateScale: Vector3,
  axis: Vector3,
) {
  const margin = Number.EPSILON
    * PAIR_WORLD_CONTACT_ZERO_ULP_FACTOR
    * (
      Math.abs(axis.x) * worldCoordinateScale.x
      + Math.abs(axis.y) * worldCoordinateScale.y
      + Math.abs(axis.z) * worldCoordinateScale.z
    )
  return Number.isFinite(margin) ? margin : Number.POSITIVE_INFINITY
}

function projectVertices(
  vertices: readonly Vector3[],
  axis: Vector3,
  origin: Vector3,
) {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  const originProjection = origin.dot(axis)
  if (!Number.isFinite(originProjection)) return null
  for (const vertex of vertices) {
    const projection = vertex.dot(axis) - originProjection
    if (!Number.isFinite(projection)) return null
    min = Math.min(min, projection)
    max = Math.max(max, projection)
  }
  return Number.isFinite(min) && Number.isFinite(max) ? { min, max } : null
}

function transformedPoint(x: number, y: number, z: number, transform: Matrix4) {
  const point = new Vector3(x, y, z).applyMatrix4(transform)
  return [point.x, point.y, point.z].every(Number.isFinite) ? point : null
}

function witnessFrameForTransform(
  transform: Matrix4,
): FoldPreviewWitnessFrame | null {
  try {
    const elements = transform.elements
    if (!Array.isArray(elements) || elements.length !== 16) return null
    const xAxisX = elements[0]
    const xAxisY = elements[1]
    const xAxisZ = elements[2]
    const yAxisX = elements[4]
    const yAxisY = elements[5]
    const yAxisZ = elements[6]
    const zAxisX = elements[8]
    const zAxisY = elements[9]
    const zAxisZ = elements[10]
    const values = [
      xAxisX,
      xAxisY,
      xAxisZ,
      yAxisX,
      yAxisY,
      yAxisZ,
      zAxisX,
      zAxisY,
      zAxisZ,
    ]
    if (!values.every(Number.isFinite)) return null
    return Object.freeze({
      xAxis: Object.freeze({
        x: xAxisX,
        y: xAxisY,
        z: xAxisZ,
      }),
      yAxis: Object.freeze({
        x: yAxisX,
        y: yAxisY,
        z: yAxisZ,
      }),
      zAxis: Object.freeze({
        x: zAxisX,
        y: zAxisY,
        z: zAxisZ,
      }),
    })
  } catch {
    return null
  }
}

function freezeHingeContactDecisionSnapshot(
  value: FoldPreviewHingeContactDecision,
): FoldPreviewHingeContactDecision {
  if (value.kind === 'allowed_by_hinge_model') {
    return Object.freeze({
      kind: value.kind,
      hingeEdgeId: value.hingeEdgeId,
      geometry: value.geometry,
      thicknessRule: value.thicknessRule,
    })
  }
  if (
    value.kind === 'outside_hinge_penetration'
    || value.kind === 'outside_hinge_contact'
  ) {
    return Object.freeze({
      kind: value.kind,
      hingeEdgeId: value.hingeEdgeId,
    })
  }
  return Object.freeze({
    kind: value.kind,
    hingeEdgeIds: Object.freeze([...value.hingeEdgeIds]),
    reason: value.reason,
  })
}

function freezeNarrowPhaseResultSnapshot(
  value: FoldPreviewNarrowPhaseResult,
): FoldPreviewNarrowPhaseResult {
  const interactions = Object.freeze(value.interactions.map((interaction) => {
    const base = {
      firstFaceId: interaction.firstFaceId,
      secondFaceId: interaction.secondFaceId,
      relation: interaction.relation,
      hingeEdgeIds: Object.freeze([...interaction.hingeEdgeIds]),
      geometryClass: interaction.geometryClass,
    }
    const withTopologyContact = interaction.topologyContact
      ? {
          ...base,
          topologyContact: Object.freeze({
            policyVersion: interaction.topologyContact.policyVersion,
            decision: interaction.topologyContact.decision,
            exclusive: interaction.topologyContact.exclusive,
            sharedVertexIds: Object.freeze([
              ...interaction.topologyContact.sharedVertexIds,
            ]),
            omittedSharedVertexIdCount:
              interaction.topologyContact.omittedSharedVertexIdCount,
            featureContactPairCount:
              interaction.topologyContact.featureContactPairCount,
            thicknessOverlapPairCount:
              interaction.topologyContact.thicknessOverlapPairCount,
            rawTouchingPairCount:
              interaction.topologyContact.rawTouchingPairCount,
            rawPenetratingPairCount:
              interaction.topologyContact.rawPenetratingPairCount,
            rawIndeterminatePairCount:
              interaction.topologyContact.rawIndeterminatePairCount,
          }),
        }
      : base
    const snapshot = Object.freeze(interaction.hingeDecision
      ? {
          ...withTopologyContact,
          hingeDecision:
            freezeHingeContactDecisionSnapshot(interaction.hingeDecision),
        }
      : withTopologyContact)
    if (interaction.topologyContact) {
      reflectApplyIntrinsic(
        weakSetAddIntrinsic,
        trustedAllowedSharedVertexInteractions,
        [snapshot],
      )
    }
    return snapshot
  }))
  const witnessSamples = Object.freeze(value.witnessSamples.map((sample) =>
    Object.freeze({
      firstFaceId: sample.firstFaceId,
      secondFaceId: sample.secondFaceId,
      relation: sample.relation,
      firstTriangleIndex: sample.firstTriangleIndex,
      secondTriangleIndex: sample.secondTriangleIndex,
      geometryClass: sample.geometryClass,
      witness: sample.witness,
    })))
  const witnessCoverage = freezeWitnessCoverage({
    eligiblePairCount: value.witnessCoverage.eligiblePairCount,
    attemptedPairCount: value.witnessCoverage.attemptedPairCount,
    unavailablePairCount: value.witnessCoverage.unavailablePairCount,
    omittedByLimitCount: value.witnessCoverage.omittedByLimitCount,
    authoritativePairScanComplete:
      value.witnessCoverage.authoritativePairScanComplete,
  })
  return Object.freeze({
    broadPhaseCandidates: value.broadPhaseCandidates,
    broadPhaseNonAdjacentCandidates: value.broadPhaseNonAdjacentCandidates,
    broadPhaseHingeAdjacentCandidates:
      value.broadPhaseHingeAdjacentCandidates,
    interactions,
    trianglePairTests: value.trianglePairTests,
    satTests: value.satTests,
    numericalMargin: value.numericalMargin,
    exactTransversalProofWork: Object.freeze({
      algorithm: value.exactTransversalProofWork.algorithm,
      maximumAttempts:
        value.exactTransversalProofWork.maximumAttempts,
      attempted: value.exactTransversalProofWork.attempted,
      skippedByLimit:
        value.exactTransversalProofWork.skippedByLimit,
    }),
    witnessSamples,
    witnessCoverage,
  })
}

function freezeWitnessCoverage(
  value: Omit<FoldPreviewNarrowPhaseWitnessCoverage, 'scope'>,
): FoldPreviewNarrowPhaseWitnessCoverage {
  return Object.freeze({
    scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
    ...value,
  })
}

function freezeFullScanNonAdjacentWitnessCoverage(
  value: Omit<FoldPreviewFullScanNonAdjacentWitnessCoverage, 'scope'>,
): FoldPreviewFullScanNonAdjacentWitnessCoverage | null {
  const counts = [
    value.broadPhaseCandidateCount,
    value.expectedTrianglePairCount,
    value.trianglePairTests,
    value.aabbRejectedPairCount,
    value.satTests,
    value.satSeparatedPairCount,
    value.allowedSharedVertexPairCount,
    value.touchingPairCount,
    value.penetratingPairCount,
    value.indeterminatePairCount,
    value.eligiblePairCount,
    value.attemptedPairCount,
    value.availablePairCount,
    value.unavailablePairCount,
    value.omittedByLimitCount,
  ]
  const expectedComplete =
    value.indeterminatePairCount === 0
    && value.unavailablePairCount === 0
    && value.omittedByLimitCount === 0
    && value.availablePairCount === value.eligiblePairCount
  if (
    counts.some((count) =>
      !Number.isSafeInteger(count)
      || count < 0
      || count > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
    )
    || value.expectedTrianglePairCount !== value.trianglePairTests
    || value.trianglePairTests
      !== value.aabbRejectedPairCount + value.satTests
    || value.satTests !== value.satSeparatedPairCount
      + value.allowedSharedVertexPairCount
      + value.touchingPairCount
      + value.penetratingPairCount
      + value.indeterminatePairCount
    || value.eligiblePairCount
      !== value.touchingPairCount + value.penetratingPairCount
    || value.eligiblePairCount
      !== value.attemptedPairCount + value.omittedByLimitCount
    || value.attemptedPairCount
      !== value.availablePairCount + value.unavailablePairCount
    || value.attemptedPairCount
      > MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
    || value.authoritativePairScanComplete !== true
    || value.allCollisionConstraintsRepresented !== expectedComplete
  ) return null
  return Object.freeze({
    scope: 'all_broad_phase_non_adjacent_triangle_pairs_full_scan_v2',
    ...value,
  })
}

function normalized(vector: Vector3) {
  const length = vector.length()
  return Number.isFinite(length) && length > 0
    ? vector.multiplyScalar(1 / length)
    : null
}

function rigidTransform(transform: Matrix4) {
  const elements = transform.elements
  if (
    !Array.isArray(elements)
    || elements.length !== 16
    || !elements.every(Number.isFinite)
  ) return false
  const first = new Vector3(elements[0], elements[1], elements[2])
  const second = new Vector3(elements[4], elements[5], elements[6])
  const third = new Vector3(elements[8], elements[9], elements[10])
  const determinant = first.dot(second.clone().cross(third))
  return Math.abs(first.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(second.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(third.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.dot(second)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.dot(third)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(second.dot(third)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(determinant - 1) <= RIGID_TRANSFORM_TOLERANCE
}

function validId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}
