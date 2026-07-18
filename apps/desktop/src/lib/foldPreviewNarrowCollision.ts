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

export const MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS = 1_000_000
/** Bounds synchronous deep-copy and triangulation during preview setup. */
export const MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES = 100_000
/** Bounds explanatory derivation work independently of collision classification. */
export const MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES = 16

const SAT_MARGIN_FACTOR = 4
const PARALLEL_AXIS_TOLERANCE = Number.EPSILON * 128
const RIGID_TRANSFORM_TOLERANCE = 1e-10
const MAX_FOLD_PREVIEW_FULL_SCAN_JOB_WORK_UNITS =
  MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
  + MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
const MAX_FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_WORK_UNITS =
  MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
  + MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES

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
  hingeDecision?: FoldPreviewHingeContactDecision
}>

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
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      kind: 'complete'
      result: FoldPreviewFullScanNonAdjacentWitnessSet
      work: FoldPreviewFullScanNonAdjacentWitnessJobWork
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
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
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      kind: 'cancelled'
      work: FoldPreviewFullScanNonAdjacentWitnessJobWork
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJobWorkBounds
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
    }>
  | Readonly<{
      version: typeof FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
      kind: 'complete'
      result: FoldPreviewNarrowPhaseResult
      work: FoldPreviewNarrowPhaseAnalysisJobWork
      workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
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
    }>
  | Readonly<{
      version: typeof FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
      kind: 'cancelled'
      work: FoldPreviewNarrowPhaseAnalysisJobWork
      workBounds: FoldPreviewNarrowPhaseAnalysisJobWorkBounds
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

type TrianglePrism = Readonly<{
  triangleIndex: number
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
  touchingPairCount: number
  penetratingPairCount: number
  indeterminatePairCount: number
}>

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
    const broadPhase = findFoldPreviewPoseBroadPhaseCandidates(
      faces,
      faceTransforms,
      thickness,
      adjacencies,
    )
    if (!broadPhase) return null
    for (const face of faces) {
      const transform = faceTransforms.get(face.id)
      if (!transform || !rigidTransform(transform)) return null
    }
    return refineFoldPreviewNarrowPhase(
      faces,
      faceTransforms,
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
  return createFullScanNonAdjacentWitnessSetJob(
    preparedFaces,
    transformSnapshot,
    thickness,
    broadPhase,
  )
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
          const intersection = classifyTrianglePrisms(first, second, numericalMargin)
          if (!intersection) return null
          if (intersection === 'separated') continue
          if (candidate.relation === 'non_adjacent') {
            if (intersection === 'touching') {
              touchingWitnessPairCount += 1
              if (
                touchingWitnessSeeds.length
                < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
              ) {
                touchingWitnessSeeds.push(Object.freeze({ first, second }))
              }
            } else if (intersection === 'penetrating') {
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
            hingePairs.push({
              firstTriangleIndex: first.triangleIndex,
              secondTriangleIndex: second.triangleIndex,
              firstVertices: first.vertices,
              secondVertices: second.vertices,
              geometryClass: intersection,
            })
          }
          if (intersection === 'penetrating') {
            geometryClass = 'penetrating'
            if (!hingeContactPolicy || candidate.relation !== 'hinge_adjacent') {
              break pairSearch
            }
            continue
          }
          if (
            intersection === 'indeterminate'
            && geometryClass !== 'penetrating'
            && geometryClass !== 'indeterminate'
          ) {
            geometryClass = 'indeterminate'
          } else if (intersection === 'touching' && !geometryClass) {
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
      if (geometryClass) {
        let interaction: FoldPreviewNarrowPhaseInteraction = {
          firstFaceId: candidate.firstFaceId,
          secondFaceId: candidate.secondFaceId,
          relation: candidate.relation,
          hingeEdgeIds: candidate.hingeEdgeIds,
          geometryClass,
        }
        if (candidate.relation === 'hinge_adjacent' && hingeContactPolicy) {
          const hingeDecision = hingeContactPolicy.classify({
            firstFaceId: candidate.firstFaceId,
            secondFaceId: candidate.secondFaceId,
            hingeEdgeIds: candidate.hingeEdgeIds,
            faceTransforms,
            thickness,
            numericalMargin,
            testedTrianglePairs: candidateTrianglePairTests,
            pairs: hingePairs,
          })
          interaction = {
            ...interaction,
            geometryClass: hingeDecision.kind === 'allowed_by_hinge_model'
              ? hingeDecision.geometry === 'boundary_contact'
                ? 'touching'
                : 'penetrating'
              : interaction.geometryClass,
            hingeDecision,
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
  })

  const cancelledStep = () => freezeUnpublishedStep({
    version: FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
    kind: 'cancelled',
    work: work(),
    workBounds,
  })

  const result = (): FoldPreviewNarrowPhaseResult => ({
      broadPhaseCandidates: broadPhase.candidates.length,
      broadPhaseNonAdjacentCandidates,
      broadPhaseHingeAdjacentCandidates,
      interactions,
      trianglePairTests,
      satTests,
      numericalMargin,
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

    if (candidateGeometryClass) {
      let interaction: FoldPreviewNarrowPhaseInteraction = {
        firstFaceId: candidate.firstFaceId,
        secondFaceId: candidate.secondFaceId,
        relation: candidate.relation,
        hingeEdgeIds: candidate.hingeEdgeIds,
        geometryClass: candidateGeometryClass,
      }
      if (candidate.relation === 'hinge_adjacent' && hingeContactPolicy) {
        const hingeDecision = hingeContactPolicy.classify({
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
        interaction = {
          ...interaction,
          geometryClass: hingeDecision.kind === 'allowed_by_hinge_model'
            ? hingeDecision.geometry === 'boundary_contact'
              ? 'touching'
              : 'penetrating'
            : interaction.geometryClass,
          hingeDecision,
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
      const intersection = classifyTrianglePrisms(
        first,
        second,
        numericalMargin,
      )
      if (terminal) return terminal
      if (cancelled) return cancelledStep()
      if (!intersection) return indeterminateStep('scan_error')
      if (intersection !== 'separated') {
        if (candidate.relation === 'non_adjacent') {
          if (intersection === 'touching') {
            touchingWitnessPairCount += 1
            if (
              touchingWitnessSeeds.length
              < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
            ) {
              touchingWitnessSeeds.push(Object.freeze({ first, second }))
            }
          } else if (intersection === 'penetrating') {
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
          hingePairs.push({
            firstTriangleIndex: first.triangleIndex,
            secondTriangleIndex: second.triangleIndex,
            firstVertices: first.vertices,
            secondVertices: second.vertices,
            geometryClass: intersection,
          })
        }
        if (intersection === 'penetrating') {
          candidateGeometryClass = 'penetrating'
          stopCandidateEarly =
            !hingeContactPolicy || candidate.relation !== 'hinge_adjacent'
        } else if (
          intersection === 'indeterminate'
          && candidateGeometryClass !== 'penetrating'
          && candidateGeometryClass !== 'indeterminate'
        ) {
          candidateGeometryClass = 'indeterminate'
        } else if (
          intersection === 'touching'
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
  })

  const checkedStep = (
    value: FoldPreviewNarrowPhaseAnalysisJobStep,
    previousWork: FoldPreviewNarrowPhaseAnalysisJobWork,
    workBudget: number,
    processed: number,
  ) => {
    const currentWork = work()
    const totalDelta =
      currentWork.totalWorkUnits - previousWork.totalWorkUnits
    const trianglePairDelta =
      currentWork.trianglePairTests - previousWork.trianglePairTests
    const witnessDelta =
      currentWork.witnessDerivations - previousWork.witnessDerivations
    if (
      !Number.isSafeInteger(currentWork.totalWorkUnits)
      || currentWork.totalWorkUnits !== currentWork.trianglePairTests
        + currentWork.witnessDerivations
      || totalDelta < 0
      || trianglePairDelta < 0
      || witnessDelta < 0
      || totalDelta !== trianglePairDelta + witnessDelta
      || totalDelta !== processed
      || totalDelta > workBudget
      || trianglePairDelta > workBudget
      || currentWork.trianglePairTests
        > workBounds.maximumTrianglePairTests
      || currentWork.witnessDerivations
        > workBounds.maximumWitnessDerivations
      || currentWork.totalWorkUnits > workBounds.maximumTotalWorkUnits
      || value.work.totalWorkUnits !== currentWork.totalWorkUnits
      || value.work.trianglePairTests !== currentWork.trianglePairTests
      || value.work.witnessDerivations !== currentWork.witnessDerivations
      || value.workBounds !== workBounds
    ) return publish(indeterminateStep('work_accounting_error'))
    return publish(value)
  }

  const runStep = (
    workBudget: number,
    previousWork: FoldPreviewNarrowPhaseAnalysisJobWork,
  ): FoldPreviewNarrowPhaseAnalysisJobStep => {
    let processed = 0
    while (processed < workBudget) {
      if (terminal) {
        return checkedStep(terminal, previousWork, workBudget, processed)
      }
      if (cancelled) {
        return checkedStep(
          cancelledStep(),
          previousWork,
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
          workBudget,
          processed,
        )
      }
    }
    return checkedStep(pending(), previousWork, workBudget, processed)
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
      let validatedWorkBudget: number | null = null
      try {
        previousWork = work()
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
        return runStep(validatedWorkBudget, previousWork)
      } catch {
        if (terminal) return terminal
        if (cancelled) {
          if (previousWork && validatedWorkBudget !== null) {
            return checkedStep(
              cancelledStep(),
              previousWork,
              validatedWorkBudget,
              work().totalWorkUnits - previousWork.totalWorkUnits,
            )
          }
          return publish(cancelledStep())
        }
        if (!previousWork || validatedWorkBudget === null) {
          return publish(indeterminateStep('scan_error'))
        }
        return checkedStep(
          indeterminateStep('scan_error'),
          previousWork,
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
  })

  const cancelledStep = () => freezeUnpublishedStep({
    version: FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
    kind: 'cancelled',
    work: work(),
    workBounds,
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
        touchingPairCount,
        penetratingPairCount,
        indeterminatePairCount,
      },
      witnessSamples,
      unavailablePairCount,
    })
    if (!result) return indeterminateStep('scan_error')
    return freezeUnpublishedStep({
      version: FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
      kind: 'complete',
      result,
      work: work(),
      workBounds,
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
      )
      if (terminal) return terminal
      if (cancelled) return cancelledStep()
      if (!intersection) return indeterminateStep('scan_error')
      if (intersection === 'separated') {
        satSeparatedPairCount += 1
      } else if (intersection === 'indeterminate') {
        indeterminatePairCount += 1
      } else {
        if (intersection === 'penetrating') {
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
            geometryClass: intersection,
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
  })

  const checkedStep = (
    value: FoldPreviewFullScanNonAdjacentWitnessJobStep,
    previousWork: FoldPreviewFullScanNonAdjacentWitnessJobWork,
    workBudget: number,
    processed: number,
  ) => {
    const currentWork = work()
    const totalDelta =
      currentWork.totalWorkUnits - previousWork.totalWorkUnits
    const trianglePairDelta =
      currentWork.trianglePairTests - previousWork.trianglePairTests
    const witnessDelta =
      currentWork.witnessDerivations - previousWork.witnessDerivations
    if (
      !Number.isSafeInteger(currentWork.totalWorkUnits)
      || currentWork.totalWorkUnits !== currentWork.trianglePairTests
        + currentWork.witnessDerivations
      || totalDelta < 0
      || trianglePairDelta < 0
      || witnessDelta < 0
      || totalDelta !== trianglePairDelta + witnessDelta
      || totalDelta !== processed
      || totalDelta > workBudget
      || trianglePairDelta > workBudget
      || currentWork.trianglePairTests
        > workBounds.expectedTrianglePairCount
      || currentWork.witnessDerivations
        > workBounds.maximumWitnessDerivations
      || currentWork.totalWorkUnits > workBounds.maximumTotalWorkUnits
      || value.work.totalWorkUnits !== currentWork.totalWorkUnits
      || value.work.trianglePairTests !== currentWork.trianglePairTests
      || value.work.witnessDerivations !== currentWork.witnessDerivations
      || value.workBounds !== workBounds
    ) return publish(indeterminateStep('work_accounting_error'))
    return publish(value)
  }

  const runStep = (
    workBudget: number,
    previousWork: FoldPreviewFullScanNonAdjacentWitnessJobWork,
  ): FoldPreviewFullScanNonAdjacentWitnessJobStep => {
    let processed = 0
    while (processed < workBudget) {
      if (terminal) {
        return checkedStep(terminal, previousWork, workBudget, processed)
      }
      if (cancelled) {
        return checkedStep(
          cancelledStep(),
          previousWork,
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
          return checkedStep(result, previousWork, workBudget, processed)
        }
        continue
      }

      const result = phase === 'triangle_pair_scan'
        ? processTrianglePair()
        : processWitnessDerivation()
      processed += 1
      if (result) {
        return checkedStep(result, previousWork, workBudget, processed)
      }
    }
    return checkedStep(pending(), previousWork, workBudget, processed)
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
      let validatedWorkBudget: number | null = null
      try {
        previousWork = work()
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
        return runStep(validatedWorkBudget, previousWork)
      } catch {
        if (terminal) return terminal
        if (cancelled) {
          if (previousWork && validatedWorkBudget !== null) {
            return checkedStep(
              cancelledStep(),
              previousWork,
              validatedWorkBudget,
              work().totalWorkUnits - previousWork.totalWorkUnits,
            )
          }
          return publish(cancelledStep())
        }
        if (!previousWork || validatedWorkBudget === null) {
          return publish(indeterminateStep('scan_error'))
        }
        return checkedStep(
          indeterminateStep('scan_error'),
          previousWork,
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
}: Readonly<{
  thickness: number
  numericalMargin: number
  counts: FullScanNonAdjacentWitnessCounts
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
  unavailablePairCount: number
}>): FoldPreviewFullScanNonAdjacentWitnessSet | null {
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
  faces: readonly PreparedFoldPreviewNarrowPhaseFace[],
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
    if ([...top, ...bottom].some((point) => !point)) return null
    const vertices = [...top, ...bottom] as Vector3[]
    const firstEdge = vertices[1].clone().sub(vertices[0])
    const secondEdge = vertices[2].clone().sub(vertices[1])
    const thirdEdge = vertices[0].clone().sub(vertices[2])
    const baseNormal = normalized(firstEdge.clone().cross(
      vertices[2].clone().sub(vertices[0]),
    ))
    if (!baseNormal) return null
    const extrusion = vertices[3].clone().sub(vertices[0])
    const extrusionDirection = normalized(extrusion)
    if (thickness > 0 && !extrusionDirection) return null

    const baseEdges = [firstEdge, secondEdge, thirdEdge]
      .map(normalized)
    if (baseEdges.some((edge) => !edge)) return null
    const edgeDirections = baseEdges as Vector3[]
    if (extrusionDirection) edgeDirections.push(extrusionDirection)

    const faceAxes = [baseNormal]
    for (const edge of edgeDirections.slice(0, 3)) {
      const sideAxis = normalized(thickness > 0
        ? edge.clone().cross(extrusion)
        : edge.clone().cross(baseNormal))
      if (!sideAxis) return null
      faceAxes.push(sideAxis)
    }
    const bounds = boundsForVertices(vertices)
    if (!bounds) return null
    prisms.push({
      triangleIndex,
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

function classifyTrianglePrisms(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
): PrismIntersection | null {
  if (first.zeroThickness || second.zeroThickness) {
    return first.zeroThickness && second.zeroThickness
      ? classifyZeroThicknessTriangles(first, second, margin)
      : 'indeterminate'
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

  let boundaryContact = false
  for (const axis of axes) {
    const firstProjection = projectVertices(first.vertices, axis)
    const secondProjection = projectVertices(second.vertices, axis)
    if (!firstProjection || !secondProjection) return null
    const gap = Math.max(
      secondProjection.min - firstProjection.max,
      firstProjection.min - secondProjection.max,
    )
    if (gap > margin) return 'separated'
    const overlap = Math.min(firstProjection.max, secondProjection.max)
      - Math.max(firstProjection.min, secondProjection.min)
    if (!Number.isFinite(gap) || !Number.isFinite(overlap)) return null
    if (overlap <= margin) boundaryContact = true
  }
  if (uncertainAxis) return 'indeterminate'
  return boundaryContact ? 'touching' : 'penetrating'
}

type TrianglePlaneSection = Readonly<{
  points: readonly Vector3[]
  crossesInterior: boolean
  crossesInteriorByRawSigns: boolean
  hasSubMarginPlaneDistance: boolean
  mergedNumericallyDistinctPoint: boolean
}>

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
): PrismIntersection | null {
  const firstVertices = first.vertices.slice(0, 3)
  const secondVertices = second.vertices.slice(0, 3)
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
  if (
    !Number.isFinite(firstNormalLength)
    || !Number.isFinite(secondNormalLength)
    || !Number.isFinite(minimumEdgeLength)
    || firstNormalLength === 0
    || secondNormalLength === 0
    || minimumEdgeLength === 0
  ) return null
  if (margin * 16 >= minimumEdgeLength) return 'indeterminate'
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
    strictlyOnOneSide(firstDistances, margin)
    || strictlyOnOneSide(secondDistances, margin)
  ) return 'separated'

  const intersectionDirection = firstNormal.clone().cross(secondNormal)
  const directionLength = intersectionDirection.length()
  if (!Number.isFinite(directionLength)) return null
  if (directionLength <= PARALLEL_AXIS_TOLERANCE) {
    const maximumPlaneDistance = Math.max(
      ...firstDistances.map(Math.abs),
      ...secondDistances.map(Math.abs),
    )
    if (!Number.isFinite(maximumPlaneDistance)) return null
    if (maximumPlaneDistance > margin) return 'separated'
    if (maximumPlaneDistance > 0) return 'indeterminate'
    const exactlyParallel = intersectionDirection.x === 0
      && intersectionDirection.y === 0
      && intersectionDirection.z === 0
    if (!exactlyParallel) return 'indeterminate'
    return classifyCoplanarTriangleOverlap(
      firstVertices,
      secondVertices,
      firstNormal,
      margin,
    )
  }
  intersectionDirection.multiplyScalar(1 / directionLength)

  const firstSection = trianglePlaneSection(
    firstVertices,
    firstDistances,
    intersectionDirection,
    margin,
  )
  const secondSection = trianglePlaneSection(
    secondVertices,
    secondDistances,
    intersectionDirection,
    margin,
  )
  if (!firstSection || !secondSection) return null
  if (firstSection.points.length === 0 || secondSection.points.length === 0) {
    return firstSection.hasSubMarginPlaneDistance
      || secondSection.hasSubMarginPlaneDistance
      || firstSection.mergedNumericallyDistinctPoint
      || secondSection.mergedNumericallyDistinctPoint
      ? 'indeterminate'
      : 'separated'
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
  if (gap > margin) return 'separated'
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
    if (mergedNumericallyDistinctPoint) return 'indeterminate'
    if (!hasSubMarginPlaneDistance) return 'touching'
    const contactCoordinate = Math.max(firstRange.min, secondRange.min)
    return sharedTopologyPointProvesContact(
      first,
      second,
      firstDistances,
      secondDistances,
      intersectionDirection,
      firstVertices[0],
      contactCoordinate,
      margin,
    )
      && !firstSection.crossesInteriorByRawSigns
      && !secondSection.crossesInteriorByRawSigns
      ? 'touching'
      : 'indeterminate'
  }
  if (overlap <= margin) return 'indeterminate'
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
  ) return 'touching'
  if (mergedNumericallyDistinctPoint || hasSubMarginPlaneDistance) {
    return 'indeterminate'
  }
  return firstSection.crossesInterior && secondSection.crossesInterior
    ? 'penetrating'
    : 'touching'
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
    const firstWorld = first.vertices[firstIndex]
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
      const secondWorld = second.vertices[secondIndex]
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
    const firstWorld = first.vertices[firstIndex]
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
      const secondWorld = second.vertices[secondIndex]
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

function projectVertices(vertices: readonly Vector3[], axis: Vector3) {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const vertex of vertices) {
    const projection = vertex.dot(axis)
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
    return Object.freeze(interaction.hingeDecision
      ? {
          ...base,
          hingeDecision:
            freezeHingeContactDecisionSnapshot(interaction.hingeDecision),
        }
      : base)
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
