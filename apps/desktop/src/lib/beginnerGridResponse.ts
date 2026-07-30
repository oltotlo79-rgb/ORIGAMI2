import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import {
  beginnerGenericFeatureBindingIdentityIsCanonicalV1,
  MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1,
  MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
  MIN_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1,
} from './beginnerGeneratedPlanContract.ts'
import {
  beginnerGeneratedPlanTopologyMatchesProfileV1,
} from './beginnerGeneratedPlanTopologyContract.ts'
import type {
  BeginnerDesignProfileV1,
  BeginnerGeneratedPlanAssessmentV1,
  BeginnerGeneratedPlanV1,
  BeginnerGridEvaluationResponse,
  BeginnerParameterGridPointV1,
} from './coreClient.ts'
type NormalizeCandidateResponse = (
  value: unknown, expectedProjectInstanceId: string, expectedProjectId: string,
  expectedRevision: number, requestedCandidateCount: number,
  instructionContext: 'grid',
  expectedProfile: BeginnerDesignProfileV1,
) => Readonly<{
  generated_plans: BeginnerGeneratedPlanV1[]
  plan_assessments: BeginnerGeneratedPlanAssessmentV1[]
}> | null

type GridExpectation = Readonly<{
  expectedProjectInstanceId: string, expectedProjectId: string
  expectedRevision: number, requestGenerationId: string
  expectedProfile: BeginnerDesignProfileV1
}>

const INVALID_GRID_RESPONSE = 'invalid beginner parameter grid response'
const invalidGridResponse = (): never => { throw new Error(INVALID_GRID_RESPONSE) }
const BEGINNER_CANONICAL_GRID_HASH_V1 = Object.freeze([
  224, 59, 9, 238, 119, 51, 70, 177,
  12, 139, 19, 69, 142, 139, 157, 2,
  55, 85, 134, 120, 49, 93, 4, 65,
  125, 141, 52, 157, 74, 39, 236, 192,
])

function snapshotRecord(value: unknown): Record<string, unknown> | null {
  try {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of Reflect.ownKeys(descriptors)) {
      if (typeof key !== 'string') return null
      const descriptor = descriptors[key]
      if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) return null
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch {
    return null
  }
}

function exactRecord<const Keys extends readonly string[]>(
  value: unknown, expectedKeys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = snapshotRecord(value)
  if (!record) return null
  const actualKeys = Object.keys(record)
  return actualKeys.length === expectedKeys.length
    && expectedKeys.every((key) => Object.hasOwn(record, key))
    ? record as Readonly<Record<Keys[number], unknown>>
    : null
}

function snapshotArray(value: unknown, maximumLength: number):
readonly unknown[] | null {
  try {
    if (!Array.isArray(value)
      || Object.getPrototypeOf(value) !== Array.prototype) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const lengthDescriptor = Reflect.getOwnPropertyDescriptor(value, 'length')
    const lengthValue = lengthDescriptor && 'value' in lengthDescriptor
      ? lengthDescriptor.value
      : null
    if (typeof lengthValue !== 'number' || !Number.isSafeInteger(lengthValue)
      || lengthValue < 0 || lengthValue > maximumLength) return null
    const keys = Reflect.ownKeys(descriptors)
    if (keys.length !== lengthValue + 1 || keys.some((key) =>
      typeof key !== 'string' || (key !== 'length'
        && (!/^(?:0|[1-9][0-9]*)$/u.test(key) || Number(key) >= lengthValue)))) {
      return null
    }
    const snapshot: unknown[] = []
    for (let index = 0; index < lengthValue; index += 1) {
      const descriptor = descriptors[String(index)]
      if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) return null
      snapshot.push(descriptor.value)
    }
    return Object.freeze(snapshot)
  } catch {
    return null
  }
}

function snapshotSha256Bytes(value: unknown): ReadonlyArray<number> | null {
  const snapshot = snapshotArray(value, 32)
  if (snapshot?.length !== 32
    || snapshot.some((item) =>
      !Number.isInteger(item) || Number(item) < 0 || Number(item) > 255)) {
    return null
  }
  return Object.freeze(snapshot.map(Number))
}

function expectedGridTemporaryProtrusionIdsV1(
  expectedProfile: BeginnerDesignProfileV1,
  expectedEndpointCount: number,
): ReadonlyArray<number> {
  const sourceProtrusions =
    expectedProfile.generation_constraints.protrusions ?? []
  const preservesSourceIds =
    sourceProtrusions.length === expectedEndpointCount
    && sourceProtrusions.every((protrusion, index) =>
      Number.isInteger(protrusion.id)
      && protrusion.id >= 0
      && protrusion.id <= 65_535
      && (index === 0
        || sourceProtrusions[index - 1]!.id < protrusion.id))
  return Object.freeze(preservesSourceIds
    ? sourceProtrusions.map((protrusion) => protrusion.id)
    : Array.from({ length: expectedEndpointCount }, (_, index) => index + 1))
}

function expectedGridContourContractV1(
  expectedProfile: BeginnerDesignProfileV1,
  planKind: BeginnerGeneratedPlanV1['kind'],
  expectedEndpointCount: number,
): Readonly<{
  bodyPoints: number
  localBindings: ReadonlyArray<Readonly<{
    protrusionId: number
    contourPoints: number
  }>>
  contourLengths: ReadonlyArray<number>
}> {
  const constraints = expectedProfile.generation_constraints
  const sourceProtrusions = constraints.protrusions ?? []
  const generic = planKind === 'composite_generic_target_base'
  const preservesGenericLocalOutlines =
    sourceProtrusions.length === expectedEndpointCount
    && new Set(sourceProtrusions.map((protrusion) => protrusion.id)).size
      === sourceProtrusions.length
    && sourceProtrusions.every((protrusion) =>
      protrusion.count === 1 && protrusion.symmetry === 'none')
  const localBindings = (
    !generic || preservesGenericLocalOutlines
      ? sourceProtrusions
      : []
  ).flatMap((protrusion) =>
    protrusion.local_outline_tenths_mm === undefined
      ? []
      : [{
          protrusionId: protrusion.id,
          contourPoints: protrusion.local_outline_tenths_mm.length,
        }])
  const bodyPoints =
    constraints.generic_body_outline_tenths_mm?.length ?? 0
  return Object.freeze({
    bodyPoints,
    localBindings: Object.freeze(localBindings.map(
      (binding) => Object.freeze(binding),
    )),
    contourLengths: Object.freeze([
      ...(bodyPoints === 0 ? [] : [bodyPoints]),
      ...localBindings.map((binding) => binding.contourPoints),
    ]),
  })
}

function canonicalBeginnerGridSeedV1(
  id: unknown,
): BeginnerParameterGridPointV1 | null {
  if (!Number.isInteger(id) || Number(id) < 0 || Number(id) > 26) {
    return null
  }
  const canonicalId = Number(id)
  const scales = [10, 27, 45] as const
  const spacings = [20, 50, 80] as const
  const details = ['simple', 'standard', 'detailed'] as const
  return Object.freeze({
    id: canonicalId,
    scale_percent: scales[Math.floor((canonicalId % 9) / 3)]!,
    spacing_percent: spacings[canonicalId % 3]!,
    detail_level: details[Math.floor(canonicalId / 9)]!,
  })
}

function expectedBeginnerGridEstimateV1(
  expectedProfile: BeginnerDesignProfileV1,
  kind: BeginnerGeneratedPlanV1['kind'],
): Readonly<{ scale: number, spacing: number }> | null {
  const constraints = expectedProfile.generation_constraints
  const semanticEndpointCount = constraints.target_parts.reduce(
    (sum, part) =>
      part.kind === 'head' || part.kind === 'torso'
        ? sum
        : sum + part.count,
    0,
  )
  const physicalEndpointCount = (constraints.protrusions ?? []).reduce(
    (sum, protrusion) => sum + protrusion.count,
    0,
  )
  const protrusionCount =
    kind === 'asymmetric_insect_landmark_base'
      ? 7
      : constraints.target_category === 'custom_object'
          && semanticEndpointCount === 0
        ? physicalEndpointCount
        : semanticEndpointCount
  if (
    !Number.isInteger(protrusionCount)
    || protrusionCount < 1
    || protrusionCount > MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
  ) return null
  const scale = constraints.detail_level === 'simple'
    ? 20
    : constraints.detail_level === 'standard'
      ? 25
      : 30
  return Object.freeze({
    scale,
    spacing: protrusionCount === 4 ? 35 : 50,
  })
}

function beginnerGridRefinementMetadataIsCanonicalV1(
  seed: BeginnerParameterGridPointV1,
  point: Readonly<Record<
    'id' | 'scale_percent' | 'spacing_percent' | 'detail_level',
    unknown
  >>,
  refinementIterations: unknown,
  strictImprovements: unknown,
  refinementStarts: unknown,
  referenceMayBeAvailable: boolean,
): boolean {
  if (
    !Number.isInteger(refinementIterations)
    || Number(refinementIterations) < 0
    || Number(refinementIterations) > 8
    || !Number.isInteger(strictImprovements)
    || Number(strictImprovements) < 0
    || Number(strictImprovements) > Number(refinementIterations) + 1
  ) return false
  const scale = Number(point.scale_percent)
  const spacing = Number(point.spacing_percent)
  const coordinatesChanged =
    scale !== seed.scale_percent || spacing !== seed.spacing_percent
  const unchangedPath =
    refinementStarts === 1
    && refinementIterations === 0
    && strictImprovements === 0
    && !coordinatesChanged
  const referencePath =
    referenceMayBeAvailable
    && refinementStarts === 5
    && coordinatesChanged === (Number(strictImprovements) > 0)
    && Math.abs(scale - seed.scale_percent)
      <= 4 + Number(refinementIterations) * 2
    && Math.abs(spacing - seed.spacing_percent)
      <= 6 + Number(refinementIterations) * 3
  return point.id === seed.id
    && point.detail_level === seed.detail_level
    && (unchangedPath || referencePath)
}

export function normalizeBeginnerGridEvaluationResponseV1(
  value: unknown,
  expectation: GridExpectation,
  normalizeCandidateResponse: NormalizeCandidateResponse,
): BeginnerGridEvaluationResponse {
  const { expectedProjectInstanceId, expectedProjectId,
    expectedRevision, requestGenerationId, expectedProfile } = expectation
  const response = exactRecord(value, [
    'request_generation_id', 'authority_token', 'project_instance_id',
    'project_id', 'revision', 'grid_hash', 'evaluated_grid_points',
    'global_checked_candidates', 'refinement_iterations', 'candidates',
  ] as const)
  const gridHashInput = snapshotArray(response?.grid_hash, 32)
  const gridCandidateInputs = snapshotArray(response?.candidates, 3)
  if (!response || response.request_generation_id !== requestGenerationId
    || !isCanonicalNonNilUuid(response.authority_token)
    || response.project_instance_id !== expectedProjectInstanceId
    || response.project_id !== expectedProjectId
    || response.revision !== expectedRevision
    || response.evaluated_grid_points !== 27
    || response.global_checked_candidates !== 3
    || !Number.isInteger(response.refinement_iterations)
    || Number(response.refinement_iterations) < 0
    || Number(response.refinement_iterations) > 24
    || !gridHashInput || gridHashInput.length !== 32
    || gridHashInput.some((byte) =>
      !Number.isInteger(byte) || Number(byte) < 0 || Number(byte) > 255)
    || gridHashInput.some((byte, index) =>
      byte !== BEGINNER_CANONICAL_GRID_HASH_V1[index])
    || !gridCandidateInputs || gridCandidateInputs.length < 1) {
    return invalidGridResponse()
  }
  const rawCandidates = gridCandidateInputs.map((candidate) => exactRecord(
    candidate, [
      'point', 'primary_score', 'plan', 'assessment', 'local_proof_scope',
      'global_proof_scope', 'complexity_score', 'scale_deviation_penalty',
      'paper_efficiency_score', 'spacing_deviation_penalty',
      'detail_mismatch_penalty', 'outcome_reason', 'contour_witness',
      'refinement_iterations', 'strict_improvements', 'refinement_starts',
    ] as const,
  ))
  if (rawCandidates.some((candidate) => candidate === null)) {
    return invalidGridResponse()
  }
  const admitted = rawCandidates as NonNullable<(typeof rawCandidates)[number]>[]
  const normalizedPlans = normalizeCandidateResponse({
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
    multi_reference_fusion: null,
    reference_consensus_analysis: null,
    candidates: [0, 1, 2].map((index) => ({
      schema_version: 1,
      kind: ['recommended', 'shape_focused', 'foldability_focused'][index],
      rank: index + 1,
      total_score: 100 - index,
      shape_score: 100 - index,
      target_approximation_score: 100 - index,
      foldability_score: 100 - index,
      step_count_score: 100 - index,
      paper_efficiency_score: 100 - index,
    })),
  }, expectedProjectInstanceId, expectedProjectId, expectedRevision, 3, 'grid',
  expectedProfile)
  if (!normalizedPlans) return invalidGridResponse()

  const candidates = admitted.map((candidate, index) => {
    const point = exactRecord(candidate.point,
      ['id', 'scale_percent', 'spacing_percent', 'detail_level'] as const)
    const witness = exactRecord(candidate.contour_witness, [
      'body_contour_points', 'local_bindings', 'generic_feature_bindings',
      'skeleton_branch_bindings', 'skeleton_tree_authority_sha256',
      'witnessed_vertices', 'witnessed_creases', 'topology_authority_hash',
      'max_contour_error_millionths',
    ] as const)
    const localInputs = witness
      ? snapshotArray(
          witness.local_bindings,
          MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
        )
      : null
    const featureInputs = witness
      ? snapshotArray(
          witness.generic_feature_bindings,
          MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
        )
      : null
    const branchInputs = witness
      ? snapshotArray(witness.skeleton_branch_bindings, 16)
      : null
    const bindings = localInputs
      ? localInputs.map((binding) => exactRecord(binding, [
          'protrusion_id', 'contour_points', 'generated_face_id',
          'vertex_start', 'crease_start',
        ] as const))
      : []
    const featureBindings = featureInputs
      ? featureInputs.map((binding) => {
        const record = exactRecord(binding, [
            'protrusion_id', 'generated_feature_id', 'endpoint_count',
            'crease_start', 'crease_authority_sha256', 'skeleton_segment_id',
            'skeleton_endpoint', 'mount_distance_squared_tenths_mm',
          ] as const)
        const creaseAuthoritySha256 = snapshotSha256Bytes(
          record?.crease_authority_sha256,
        )
        return record && creaseAuthoritySha256
          ? { ...record, crease_authority_sha256: creaseAuthoritySha256 }
          : null
      })
      : []
    const branchBindings = branchInputs
      ? branchInputs.map((value) => {
        const branch = exactRecord(value, [
          'segment_id', 'parent_segment_id', 'parent_endpoint',
          'child_endpoint', 'generated_feature_ids',
        ] as const)
        const featureIds = snapshotArray(
          branch?.generated_feature_ids,
          MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
        )
        return branch && featureIds
          ? { ...branch, generated_feature_ids: featureIds }
          : null
      })
      : []
    const witnessPointCount = witness
      && bindings.every((binding) => binding !== null)
      ? Number(witness.body_contour_points) + bindings.reduce(
          (sum, binding) => sum + Number(binding?.contour_points),
          0,
        )
      : -1
    const plan = normalizedPlans.generated_plans[index]
    const genericPlan = plan.kind
      === 'composite_generic_target_base'
    const seed = canonicalBeginnerGridSeedV1(point?.id)
    const estimate = expectedBeginnerGridEstimateV1(
      expectedProfile,
      plan.kind,
    )
    const expectedScalePenalty = point && estimate
      ? Math.abs(Number(point.scale_percent) - estimate.scale) * 10
      : -1
    const expectedSpacingPenalty = point && estimate
      ? Math.abs(Number(point.spacing_percent) - estimate.spacing) * 5
      : -1
    const expectedDetailPenalty = point
      ? point.detail_level
          === expectedProfile.generation_constraints.detail_level
        ? 0
        : 10
      : -1
    const expectedPrimaryScore = Math.max(
      0,
      1_000
        - expectedScalePenalty
        - expectedSpacingPenalty
        - expectedDetailPenalty,
    )
    const detailComplexity = point?.detail_level === 'simple'
      ? 10
      : point?.detail_level === 'standard'
        ? 20
        : point?.detail_level === 'detailed'
          ? 30
          : -1
    const expectedComplexity = detailComplexity < 0
      ? -1
      : Math.min(
          100,
          plan.crease_pattern.edges.length * 10 + detailComplexity,
        )
    const skeletonTreeAuthoritySha256 = snapshotSha256Bytes(
      witness?.skeleton_tree_authority_sha256,
    )
    const topologyAuthorityHash = snapshotSha256Bytes(
      witness?.topology_authority_hash,
    )
    if (!point || !witness
      || !Number.isInteger(point.id) || Number(point.id) < 0 || Number(point.id) > 26
      || !seed
      || !estimate
      || !Number.isInteger(point.scale_percent)
      || Number(point.scale_percent) < 10 || Number(point.scale_percent) > 45
      || !Number.isInteger(point.spacing_percent)
      || Number(point.spacing_percent) < 20 || Number(point.spacing_percent) > 80
      || !['simple', 'standard', 'detailed'].includes(String(point.detail_level))
      || !Number.isInteger(candidate.primary_score)
      || Number(candidate.primary_score) < 0 || Number(candidate.primary_score) > 1000
      || candidate.local_proof_scope !== 'necessary'
      || candidate.global_proof_scope
        !== normalizedPlans.plan_assessments[index].proof_scope
      || candidate.outcome_reason !== normalizedPlans.plan_assessments[index].reason
      || !Number.isInteger(witness.body_contour_points)
      || Number(witness.body_contour_points) < 0
      || Number(witness.body_contour_points) > 16
      || !localInputs || bindings.length !== localInputs.length
      || !featureInputs || featureBindings.length !== featureInputs.length
      || !branchInputs || branchBindings.length !== branchInputs.length
      || (genericPlan
        ? featureBindings.length < 1
        : featureBindings.length !== 0 || branchBindings.length !== 0)
      || bindings.some((binding, bindingIndex) => !binding
        || !Number.isInteger(binding.protrusion_id)
        || Number(binding.protrusion_id) < 0
        || Number(binding.protrusion_id) > 65_535
        || !Number.isInteger(binding.contour_points)
        || Number(binding.contour_points) < 3
        || Number(binding.contour_points) > 8
        || binding.generated_face_id !== bindingIndex + 1
        || !Number.isInteger(binding.vertex_start)
        || Number(binding.vertex_start) < 0
        || !Number.isInteger(binding.crease_start)
        || Number(binding.crease_start) < 0
        || (bindingIndex > 0 && (
          Number(binding.vertex_start)
            !== Number(bindings[bindingIndex - 1]?.vertex_start)
              + Number(bindings[bindingIndex - 1]?.contour_points)
          || Number(binding.crease_start)
            !== Number(bindings[bindingIndex - 1]?.crease_start)
              + Number(bindings[bindingIndex - 1]?.contour_points)
          || Number(bindings[bindingIndex - 1]?.protrusion_id)
            >= Number(binding.protrusion_id))))
      || featureBindings.some((binding, bindingIndex) => !binding
        || !Number.isInteger(binding.protrusion_id)
        || Number(binding.protrusion_id) < 0
        || Number(binding.protrusion_id) > 65_535
        || !Number.isInteger(binding.generated_feature_id)
        || Number(binding.generated_feature_id) !== bindingIndex + 1
        || !Number.isInteger(binding.endpoint_count)
        || Number(binding.endpoint_count) < 1 || Number(binding.endpoint_count) > 8
        || !Number.isInteger(binding.crease_start)
        || Number(binding.crease_start) < 0
        || !Number.isInteger(binding.skeleton_segment_id)
        || Number(binding.skeleton_segment_id) < 0
        || Number(binding.skeleton_segment_id) > 65_535
        || !['start', 'end'].includes(String(binding.skeleton_endpoint))
        || !Number.isSafeInteger(binding.mount_distance_squared_tenths_mm)
        || Number(binding.mount_distance_squared_tenths_mm) < 0
        || Number(binding.crease_start) + Number(binding.endpoint_count)
          > normalizedPlans.generated_plans[index].crease_pattern.edges.length
        || (bindingIndex > 0
          && Number(featureBindings[bindingIndex - 1]?.protrusion_id)
            >= Number(binding.protrusion_id)))
      || featureBindings.some((binding, bindingIndex) =>
        featureBindings.some((other, otherIndex) =>
          bindingIndex !== otherIndex && binding && other
          && Number(binding.crease_start)
            < Number(other.crease_start) + Number(other.endpoint_count)
          && Number(other.crease_start)
            < Number(binding.crease_start) + Number(binding.endpoint_count)))
      || branchBindings.some((branch, branchIndex) => !branch
        || !Number.isInteger(branch.segment_id)
        || Number(branch.segment_id) < 0 || Number(branch.segment_id) > 65_535
        || (branchIndex === 0
          ? branch.parent_segment_id !== null
          : !Number.isInteger(branch.parent_segment_id)
            || Number(branch.parent_segment_id) < 0
            || Number(branch.parent_segment_id) > 65_535)
        || (branchIndex === 0
          ? branch.parent_endpoint !== null || branch.child_endpoint !== null
          : !['start', 'end'].includes(String(branch.parent_endpoint))
            || !['start', 'end'].includes(String(branch.child_endpoint))
            || !branchBindings.slice(0, branchIndex).some(
              (parent) => parent?.segment_id === branch.parent_segment_id))
        || branchBindings.slice(0, branchIndex).some(
          (previous) => previous?.segment_id === branch.segment_id)
        || new Set(branch.generated_feature_ids).size
          !== branch.generated_feature_ids.length
        || branch.generated_feature_ids.some((id) =>
          !featureBindings.some(
            (binding) => binding?.generated_feature_id === id)))
      || !skeletonTreeAuthoritySha256
      || !Number.isInteger(witness.witnessed_vertices)
      || Number(witness.witnessed_vertices) !== witnessPointCount
      || !Number.isInteger(witness.witnessed_creases)
      || Number(witness.witnessed_creases) !== witnessPointCount
      || !topologyAuthorityHash
      || !Number.isInteger(witness.max_contour_error_millionths)
      || Number(witness.max_contour_error_millionths) < 0
      || Number(witness.max_contour_error_millionths) > 1
      || normalizedPlans.generated_plans[index].crease_pattern.vertices.length
        < witnessPointCount
      || normalizedPlans.generated_plans[index].crease_pattern.edges.length
        < witnessPointCount
      || bindings.some((binding) =>
        Number(binding?.vertex_start) + Number(binding?.contour_points)
          > normalizedPlans.generated_plans[index].crease_pattern.vertices.length
        || Number(binding?.crease_start) + Number(binding?.contour_points)
          > normalizedPlans.generated_plans[index].crease_pattern.edges.length)
      || !Number.isInteger(candidate.complexity_score)
      || Number(candidate.complexity_score) !== expectedComplexity
      || !Number.isInteger(candidate.paper_efficiency_score)
      || Number(candidate.paper_efficiency_score) < 0
      || Number(candidate.paper_efficiency_score) > 100
      || !Number.isInteger(candidate.refinement_iterations)
      || Number(candidate.refinement_iterations) < 0
      || Number(candidate.refinement_iterations) > 8
      || !Number.isInteger(candidate.strict_improvements)
      || Number(candidate.strict_improvements) < 0
      || Number(candidate.strict_improvements)
        > Number(candidate.refinement_iterations) + 1
      || !Number.isInteger(candidate.refinement_starts)
      || !beginnerGridRefinementMetadataIsCanonicalV1(
        seed,
        point,
        candidate.refinement_iterations,
        candidate.strict_improvements,
        candidate.refinement_starts,
        expectedProfile.generation_constraints.target_asset?.kind
          === 'reference_model',
      )
      || ![
        candidate.scale_deviation_penalty,
        candidate.spacing_deviation_penalty,
        candidate.detail_mismatch_penalty,
      ].every((penalty) =>
        Number.isInteger(penalty)
        && Number(penalty) >= 0
        && Number(penalty) <= 1000)
      || Number(candidate.scale_deviation_penalty)
        !== expectedScalePenalty
      || Number(candidate.spacing_deviation_penalty)
        !== expectedSpacingPenalty
      || Number(candidate.detail_mismatch_penalty)
        !== expectedDetailPenalty
      || Number(candidate.primary_score) !== expectedPrimaryScore
      || (index > 0 && (
        Number(admitted[index - 1].primary_score)
          < Number(candidate.primary_score)
        || (Number(admitted[index - 1].primary_score)
          === Number(candidate.primary_score)
          && Number(exactRecord(admitted[index - 1].point, [
            'id', 'scale_percent', 'spacing_percent', 'detail_level',
          ] as const)?.id) >= Number(point.id))))) {
      return invalidGridResponse()
    }
    const admittedBindings = bindings as ReadonlyArray<
      NonNullable<(typeof bindings)[number]>
    >
    const admittedFeatureBindings = featureBindings as ReadonlyArray<
      NonNullable<(typeof featureBindings)[number]>
    >
    const admittedBranchBindings = branchBindings as ReadonlyArray<
      NonNullable<(typeof branchBindings)[number]>
    >
    const expectedBodyContourPoints =
      expectedProfile.generation_constraints
        .generic_body_outline_tenths_mm?.length ?? 0
    const graphEdgeCount = genericPlan
      ? plan.skeleton_segments.length
      : 0
    const graphVertexCount = genericPlan
      ? plan.skeleton_segments.length + 1
      : 0
    const contourEdgeEnd =
      plan.crease_pattern.edges.length - graphEdgeCount
    const contourVertexEnd =
      plan.crease_pattern.vertices.length - graphVertexCount
    let expectedLocalVertexStart =
      contourVertexEnd - witnessPointCount + expectedBodyContourPoints
    let expectedLocalCreaseStart =
      contourEdgeEnd - witnessPointCount + expectedBodyContourPoints
    const localBindingsAreCanonical = expectedLocalVertexStart >= 0
      && expectedLocalCreaseStart >= 0
      && admittedBindings.every((binding) => {
        const matches =
          Number(binding.vertex_start) === expectedLocalVertexStart
          && Number(binding.crease_start) === expectedLocalCreaseStart
        expectedLocalVertexStart += Number(binding.contour_points)
        expectedLocalCreaseStart += Number(binding.contour_points)
        return matches
      })
      && expectedLocalVertexStart === contourVertexEnd
      && expectedLocalCreaseStart === contourEdgeEnd
    const witnessedFeatureEndpointCount = admittedFeatureBindings.reduce(
      (sum, binding) => sum + Number(binding.endpoint_count),
      0,
    )
    const featureCreaseEnd = contourEdgeEnd - witnessPointCount
    let expectedFeatureCreaseStart =
      featureCreaseEnd - witnessedFeatureEndpointCount
    const featureCreaseBlocksAreCanonical =
      expectedFeatureCreaseStart >= 0
      && admittedFeatureBindings.every((binding) => {
        const matches =
          Number(binding.crease_start) === expectedFeatureCreaseStart
        expectedFeatureCreaseStart += Number(binding.endpoint_count)
        return matches
      })
      && expectedFeatureCreaseStart === featureCreaseEnd
    const expectedConstraints = expectedProfile.generation_constraints
    const semanticFeatureEndpointCount =
      expectedConstraints.target_parts.reduce(
        (sum, part) =>
          part.kind === 'head' || part.kind === 'torso'
            ? sum
            : sum + part.count,
        0,
      )
    const physicalFeatureEndpointCount =
      (expectedConstraints.protrusions ?? []).reduce(
        (sum, protrusion) => sum + protrusion.count,
        0,
      )
    const expectedFeatureEndpointCount =
      expectedConstraints.target_category === 'custom_object'
        && semanticFeatureEndpointCount === 0
        ? physicalFeatureEndpointCount
        : semanticFeatureEndpointCount
    const contourContract = expectedGridContourContractV1(
      expectedProfile,
      plan.kind,
      expectedFeatureEndpointCount,
    )
    const expectedTemporaryProtrusionIds =
      expectedGridTemporaryProtrusionIdsV1(
        expectedProfile,
        expectedFeatureEndpointCount,
      )
    if (
      Number(witness.body_contour_points)
        !== contourContract.bodyPoints
      || admittedBindings.length
        !== contourContract.localBindings.length
      || admittedBindings.some((binding, bindingIndex) =>
        Number(binding.protrusion_id)
          !== contourContract.localBindings[bindingIndex]?.protrusionId
        || Number(binding.contour_points)
          !== contourContract.localBindings[bindingIndex]?.contourPoints)
      || !localBindingsAreCanonical
      || !beginnerGeneratedPlanTopologyMatchesProfileV1(
        plan,
        expectedProfile,
        0,
        contourContract.contourLengths,
      )
      || (genericPlan && (
        expectedFeatureEndpointCount
          < MIN_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
        || expectedFeatureEndpointCount
          > MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
        || admittedFeatureBindings.length !== expectedFeatureEndpointCount
        || witnessedFeatureEndpointCount !== expectedFeatureEndpointCount
        || !featureCreaseBlocksAreCanonical
        || admittedFeatureBindings.some((binding, bindingIndex) =>
          Number(binding.endpoint_count) !== 1
          || Number(binding.protrusion_id)
            !== expectedTemporaryProtrusionIds[bindingIndex])
        || !beginnerGenericFeatureBindingIdentityIsCanonicalV1(
          admittedFeatureBindings.map((binding) => ({
            protrusion_id: Number(binding.protrusion_id),
            generated_feature_id: Number(binding.generated_feature_id),
            endpoint_count: Number(binding.endpoint_count),
            skeleton_segment_id: Number(binding.skeleton_segment_id),
          })),
          admittedBranchBindings.map((branch) => ({
            segment_id: Number(branch.segment_id),
            parent_segment_id: branch.parent_segment_id === null
              ? null
              : Number(branch.parent_segment_id),
            parent_endpoint:
              branch.parent_endpoint as 'start' | 'end' | null,
            child_endpoint: branch.child_endpoint as 'start' | 'end' | null,
            generated_feature_ids:
              (branch.generated_feature_ids as number[]).map(Number),
          })),
          normalizedPlans.generated_plans[index].skeleton_segments,
        )
      ))
    ) return invalidGridResponse()

    return Object.freeze({
      point: Object.freeze(point) as BeginnerParameterGridPointV1,
      primary_score: Number(candidate.primary_score),
      plan: normalizedPlans.generated_plans[index],
      assessment: normalizedPlans.plan_assessments[index],
      local_proof_scope: 'necessary' as const,
      global_proof_scope:
        candidate.global_proof_scope as
          BeginnerGeneratedPlanAssessmentV1['proof_scope'],
      complexity_score: Number(candidate.complexity_score),
      paper_efficiency_score: Number(candidate.paper_efficiency_score),
      scale_deviation_penalty: Number(candidate.scale_deviation_penalty),
      spacing_deviation_penalty: Number(candidate.spacing_deviation_penalty),
      detail_mismatch_penalty: Number(candidate.detail_mismatch_penalty),
      outcome_reason:
        candidate.outcome_reason as BeginnerGeneratedPlanAssessmentV1['reason'],
      refinement_iterations: Number(candidate.refinement_iterations),
      strict_improvements: Number(candidate.strict_improvements),
      refinement_starts: Number(candidate.refinement_starts),
      contour_witness: Object.freeze({
        body_contour_points: Number(witness.body_contour_points),
        local_bindings: Object.freeze(admittedBindings.map((binding) =>
          Object.freeze({
            protrusion_id: Number(binding.protrusion_id),
            contour_points: Number(binding.contour_points),
            generated_face_id: Number(binding.generated_face_id),
            vertex_start: Number(binding.vertex_start),
            crease_start: Number(binding.crease_start),
          }))),
        generic_feature_bindings: Object.freeze(
          admittedFeatureBindings.map((binding) => Object.freeze({
            protrusion_id: Number(binding.protrusion_id),
            generated_feature_id: Number(binding.generated_feature_id),
            crease_authority_sha256: binding.crease_authority_sha256,
            endpoint_count: Number(binding.endpoint_count) as
              1 | 2 | 3 | 4 | 5 | 6 | 7 | 8,
            crease_start: Number(binding.crease_start),
            skeleton_segment_id: Number(binding.skeleton_segment_id),
            skeleton_endpoint:
              String(binding.skeleton_endpoint) as 'start' | 'end',
            mount_distance_squared_tenths_mm:
              Number(binding.mount_distance_squared_tenths_mm),
          })),
        ),
        skeleton_branch_bindings: Object.freeze(
          admittedBranchBindings.map((branch) => Object.freeze({
            segment_id: Number(branch.segment_id),
            parent_segment_id: branch.parent_segment_id === null
              ? null
              : Number(branch.parent_segment_id),
            parent_endpoint:
              branch.parent_endpoint as 'start' | 'end' | null,
            child_endpoint: branch.child_endpoint as 'start' | 'end' | null,
            generated_feature_ids: Object.freeze(
              (branch.generated_feature_ids as number[]).slice(),
            ),
          })),
        ),
        skeleton_tree_authority_sha256: skeletonTreeAuthoritySha256,
        witnessed_vertices: Number(witness.witnessed_vertices),
        witnessed_creases: Number(witness.witnessed_creases),
        topology_authority_hash: topologyAuthorityHash,
        max_contour_error_millionths:
          Number(witness.max_contour_error_millionths),
      }),
    })
  })
  if (
    new Set(candidates.map((candidate) => candidate.point.id)).size
      !== candidates.length
    || new Set(candidates.map((candidate) => [
      candidate.point.scale_percent,
      candidate.point.spacing_percent,
      candidate.point.detail_level,
    ].join(':'))).size !== candidates.length
    || Number(response.refinement_iterations)
      < candidates.reduce(
        (sum, candidate) => sum + candidate.refinement_iterations,
        0,
      )
    || Number(response.refinement_iterations)
      > candidates.reduce(
        (sum, candidate) => sum + candidate.refinement_iterations,
        0,
      ) + (3 - candidates.length) * 8
  ) {
    return invalidGridResponse()
  }
  return Object.freeze({
    request_generation_id: requestGenerationId,
    authority_token: response.authority_token,
    project_instance_id: expectedProjectInstanceId, project_id: expectedProjectId,
    revision: expectedRevision, evaluated_grid_points: 27,
    global_checked_candidates: 3,
    grid_hash: Object.freeze(gridHashInput.slice()) as ReadonlyArray<number>,
    refinement_iterations: Number(response.refinement_iterations),
    candidates: Object.freeze(candidates),
  })
}
