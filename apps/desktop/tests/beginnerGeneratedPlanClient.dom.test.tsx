import { beforeEach, describe, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import {
  evaluateBeginnerCandidates,
  evaluateBeginnerParameterGrid,
  type BeginnerDesignProfileV1,
} from '../src/lib/coreClient.ts'
import {
  beginnerExpectedTargetApproximationScoreV1,
  beginnerReferenceConsensusPairDigestV1,
} from '../src/lib/beginnerCandidateScoreContract.ts'

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const VERTEX_A = '33333333-3333-4333-8333-333333333333'
const VERTEX_B = '44444444-4444-4444-8444-444444444444'
const EDGE_ID = '55555555-5555-4555-8555-555555555555'
const GRID_GENERATION_ID = '66666666-6666-4666-8666-666666666666'
const GRID_AUTHORITY_ID = '77777777-7777-4777-8777-777777777777'
const TARGET_ASSET_ID = '88888888-8888-4888-8888-888888888888'
const OTHER_ASSET_ID = '99999999-9999-4999-8999-999999999999'
const CANDIDATE_GENERATION_ID =
  'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
const THIRD_CONSENSUS_ASSET_ID =
  'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
const FOURTH_CONSENSUS_ASSET_ID =
  'cccccccc-cccc-4ccc-8ccc-cccccccccccc'
const CANONICAL_GRID_HASH_V1 = [
  224, 59, 9, 238, 119, 51, 70, 177,
  12, 139, 19, 69, 142, 139, 157, 2,
  55, 85, 134, 120, 49, 93, 4, 65,
  125, 141, 52, 157, 74, 39, 236, 192,
] as const

const ASYMMETRIC_INSECT_RAY_DIGESTS = [
  [213, 100, 5, 8, 192, 66, 152, 160, 194, 233, 1, 213, 122, 93, 223, 98, 40, 90, 120, 82, 11, 67, 162, 155, 111, 87, 115, 210, 17, 24, 20, 214],
  [129, 45, 3, 220, 103, 100, 168, 77, 239, 198, 183, 47, 163, 199, 110, 178, 201, 166, 66, 26, 155, 17, 241, 21, 87, 84, 107, 98, 136, 35, 51, 92],
  [23, 164, 6, 77, 87, 18, 29, 42, 246, 60, 210, 220, 59, 34, 167, 44, 157, 174, 12, 81, 10, 0, 226, 138, 153, 54, 51, 73, 94, 193, 23, 250],
  [229, 127, 126, 18, 52, 160, 111, 196, 175, 230, 97, 142, 9, 79, 197, 232, 238, 88, 70, 214, 0, 195, 94, 118, 124, 163, 45, 91, 174, 243, 198, 219],
] as const

const ASYMMETRIC_FISH_RAY_DIGESTS = [
  [75, 41, 210, 152, 136, 151, 46, 106, 24, 123, 23, 184, 30, 114, 42, 135, 137, 104, 245, 152, 132, 24, 91, 70, 94, 24, 236, 17, 27, 2, 50, 160],
  [161, 248, 204, 0, 96, 167, 32, 29, 69, 192, 109, 11, 216, 173, 136, 184, 254, 168, 75, 149, 4, 228, 224, 106, 4, 131, 187, 25, 183, 13, 1, 159],
  [202, 241, 97, 235, 226, 126, 156, 158, 161, 24, 8, 56, 7, 121, 174, 191, 34, 49, 180, 97, 195, 114, 200, 217, 150, 23, 163, 150, 142, 77, 176, 173],
  [244, 237, 179, 47, 153, 216, 77, 228, 12, 216, 247, 224, 124, 44, 111, 86, 85, 226, 67, 79, 22, 1, 187, 119, 64, 146, 75, 8, 53, 62, 112, 224],
] as const

function validResponse() {
  const plan = {
    schema_version: 1,
    kind: 'center_axis_tail_base',
    crease_pattern: {
      vertices: [
        { id: VERTEX_A, position: { x: 0, y: 0 } },
        { id: VERTEX_B, position: { x: 1, y: 0 } },
      ],
      edges: [{
        id: EDGE_ID,
        start: VERTEX_A,
        end: VERTEX_B,
        kind: 'valley',
      }],
    },
    instruction_codes: ['center_axis_tail_base'],
    target_parts: [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'tail', count: 1 },
    ],
    skeleton_segments: [],
    target_asset: null,
  }
  const response = {
    schema_version: 1,
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: 7,
    requested_candidate_count: 1,
    bulge_treatment: 'target_shape_approximation',
    elasticity_model: 'not_computed',
    generation_status: 'ready',
    generated_plans: [plan],
    plan_assessments: [{
      kind: plan.kind,
      expected_candidate_edge_id: EDGE_ID,
      proof_scope: 'sufficient',
      apply_allowed: true,
      reason: 'native_fold_path_certified',
      shape_approximation_score: null,
      shape_difference_reason: null,
      component_shape_comparison: null,
    }],
    candidates: [{
      schema_version: 1,
      kind: 'recommended',
      rank: 1,
      total_score: 100,
      shape_score: 100,
      target_approximation_score: 75,
      foldability_score: 100,
      step_count_score: 100,
      paper_efficiency_score: 100,
    }],
    multi_reference_fusion: null,
    reference_consensus_analysis: null,
  }
  return response
}

function expectedTailProfile(): BeginnerDesignProfileV1 {
  return {
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
        { kind: 'tail', count: 1 },
      ],
      skeleton_segments: [],
      protrusions: [],
      bulge_targets: [],
      target_asset: null,
      allowed_techniques: ['valley_fold'],
    },
  }
}

function referenceModelTailFixture() {
  const profile = expectedTailProfile()
  const skeletonSegments = [
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
  ]
  profile.generation_constraints.skeleton_segments =
    structuredClone(skeletonSegments)
  profile.generation_constraints.target_asset = {
    kind: 'reference_model',
    asset_id: TARGET_ASSET_ID,
  }
  const response = validResponse()
  response.generated_plans[0]!.skeleton_segments =
    structuredClone(skeletonSegments)
  response.generated_plans[0]!.target_asset = {
    kind: 'reference_model',
    asset_id: TARGET_ASSET_ID,
  } as never
  return { profile, response }
}

function consensusExpectedProfile(
  bindingCount = 2,
  excludedAssetId?: string,
): BeginnerDesignProfileV1 {
  const profile = expectedTailProfile()
  const assetIds = [
    TARGET_ASSET_ID,
    OTHER_ASSET_ID,
    THIRD_CONSENSUS_ASSET_ID,
    FOURTH_CONSENSUS_ASSET_ID,
  ].slice(0, bindingCount)
  profile.reference_consensus_v1 = {
    schema_version: 1,
    bindings: assetIds.map((assetId, index) => ({
      kind: index === 0 ? 'image' as const : 'reference_model' as const,
      asset_id: assetId,
      sha256: Array(32).fill(index + 1),
      quality: 90 - index,
    })),
    ...(excludedAssetId === undefined
      ? {}
      : { excluded_asset_id: excludedAssetId }),
  }
  return profile
}

function consensusCandidateFixture(
  profile: BeginnerDesignProfileV1 = consensusExpectedProfile(),
) {
  const response = validResponse()
  const expected = profile.reference_consensus_v1!
  const excludedAssetId = expected.excluded_asset_id ?? null
  const active = expected.bindings.filter(
    (binding) => binding.asset_id !== excludedAssetId,
  )
  const pairs = []
  for (let left = 0; left < active.length; left += 1) {
    for (let right = left + 1; right < active.length; right += 1) {
      const leftBranchCount =
        active[left]!.kind === 'image' ? 1 : 3
      const rightBranchCount =
        active[right]!.kind === 'image' ? 1 : 3
      const branchError = Math.abs(
        leftBranchCount - rightBranchCount,
      )
      const pair = {
        left_asset_id: active[left]!.asset_id,
        right_asset_id: active[right]!.asset_id,
        component_error: 0,
        normalized_extent_error: 0,
        branch_error: branchError,
        agreement_score: 100 - branchError * 10,
        disagrees: false,
        pair_digest_sha256: [] as number[],
        left_component_count: 1,
        right_component_count: 1,
        left_normalized_extents: [50, 100],
        right_normalized_extents: [50, 100],
        left_branch_count: leftBranchCount,
        right_branch_count: rightBranchCount,
      }
      pair.pair_digest_sha256 = Array.from(
        beginnerReferenceConsensusPairDigestV1(
          active[left]!.asset_id,
          active[left]!.sha256,
          active[right]!.asset_id,
          active[right]!.sha256,
          {
            componentError: pair.component_error,
            normalizedExtentError: pair.normalized_extent_error,
            branchError: pair.branch_error,
            agreementScore: pair.agreement_score,
            disagrees: pair.disagrees,
          },
        )!,
      )
      pairs.push(pair)
    }
  }
  const disagreementCount =
    pairs.filter((pair) => pair.disagrees).length
  const applyAllowed = disagreementCount < 2
  const analysis = {
    schema_version: 1,
    revision: 7,
    source_count: active.length,
    excluded_asset_id: excludedAssetId,
    pair_count: pairs.length,
    disagreement_count: disagreementCount,
    agreement_score: pairs.length === 0
      ? 0
      : Math.floor(
          pairs.reduce(
            (sum, pair) => sum + pair.agreement_score,
            0,
          ) / pairs.length,
        ),
    apply_allowed: applyAllowed,
    reason: applyAllowed
      ? 'reference_consensus_agreement_v1'
      : 'reference_consensus_multiple_disagreements_v1',
    pairs,
  }
  Object.assign(response, { reference_consensus_analysis: analysis })
  return { profile, response, analysis, pairs }
}

function synchronizeConsensusFixture(
  fixture: ReturnType<typeof consensusCandidateFixture>,
) {
  for (const pair of fixture.pairs) {
    pair.component_error = Math.abs(
      pair.left_component_count - pair.right_component_count,
    )
    pair.branch_error = Math.abs(
      pair.left_branch_count - pair.right_branch_count,
    )
    pair.normalized_extent_error = Math.max(
      Math.abs(
        pair.left_normalized_extents[0]
          - pair.right_normalized_extents[0],
      ),
      Math.abs(
        pair.left_normalized_extents[1]
          - pair.right_normalized_extents[1],
      ),
    )
    pair.disagrees =
      pair.component_error > 1
      || pair.branch_error > 2
      || pair.normalized_extent_error > 20
    pair.agreement_score = Math.max(
      0,
      100 - Math.min(
        100,
        pair.normalized_extent_error * 2
          + pair.component_error * 20
          + pair.branch_error * 10,
      ),
    )
    const leftBinding = fixture.profile.reference_consensus_v1!.bindings.find(
      (binding) => binding.asset_id === pair.left_asset_id,
    )!
    const rightBinding = fixture.profile.reference_consensus_v1!.bindings.find(
      (binding) => binding.asset_id === pair.right_asset_id,
    )!
    pair.pair_digest_sha256 = Array.from(
      beginnerReferenceConsensusPairDigestV1(
        leftBinding.asset_id,
        leftBinding.sha256,
        rightBinding.asset_id,
        rightBinding.sha256,
        {
          componentError: pair.component_error,
          normalizedExtentError: pair.normalized_extent_error,
          branchError: pair.branch_error,
          agreementScore: pair.agreement_score,
          disagrees: pair.disagrees,
        },
      )!,
    )
  }
  fixture.analysis.disagreement_count = fixture.pairs.filter(
    (pair) => pair.disagrees,
  ).length
  fixture.analysis.agreement_score = Math.floor(
    fixture.pairs.reduce(
      (sum, pair) => sum + pair.agreement_score,
      0,
    ) / fixture.pairs.length,
  )
  fixture.analysis.apply_allowed =
    fixture.analysis.disagreement_count < 2
  fixture.analysis.reason = fixture.analysis.apply_allowed
    ? 'reference_consensus_agreement_v1'
    : 'reference_consensus_multiple_disagreements_v1'
}

function genericProtrusion(id: number, count: number) {
  return {
    id,
    count,
    length_tenths_mm: 100,
    thickness_tenths_mm: 10,
    position_tenths_mm: [id * 10, 0, 0] as [number, number, number],
    direction_milli: [1_000, 0, 0] as [number, number, number],
    symmetry: 'none' as const,
    curvature_degrees: 0,
    joint: 'fixed' as const,
    motion_degrees: [0, 0] as [number, number],
    side: 'either' as const,
    priority: 50,
  }
}

function genericFeatureParts(endpointCount: number) {
  const kinds = [
    'fin', 'tail', 'wing', 'leg', 'ear', 'horn', 'antenna',
  ] as const
  let remaining = endpointCount
  return kinds.flatMap((kind) => {
    if (remaining === 0) return []
    const count = Math.min(remaining, 8)
    remaining -= count
    return [{ kind, count }]
  })
}

function physicalCountsForEndpoints(endpointCount: number) {
  const counts: number[] = []
  let remaining = endpointCount
  while (remaining > 0) {
    const count = Math.min(remaining, 8)
    counts.push(count)
    remaining -= count
  }
  return counts
}

function genericCandidateProfile(
  category: 'animal' | 'insect' | 'custom_object',
  semanticEndpointCount: number,
  physicalCounts: readonly number[],
): BeginnerDesignProfileV1 {
  const body = category === 'custom_object'
    ? []
    : [
        { kind: 'head' as const, count: 1 },
        { kind: 'torso' as const, count: 1 },
      ]
  return {
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
      target_category: category,
      ...(category === 'custom_object'
        ? { custom_object_display_name: 'Direct custom target' }
        : {}),
      target_parts: [
        ...body,
        ...genericFeatureParts(semanticEndpointCount),
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
      protrusions: physicalCounts.map((count, index) =>
        genericProtrusion(index + 1, count)),
      bulge_targets: [],
      target_asset: null,
      allowed_techniques: ['valley_fold'],
    },
  }
}

function genericCandidateResponse(profile: BeginnerDesignProfileV1) {
  const response = validResponse()
  const plan = response.generated_plans[0]!
  const physicalEndpointCount = (
    profile.generation_constraints.protrusions ?? []
  ).reduce((sum, protrusion) => sum + protrusion.count, 0)
  plan.kind = 'composite_generic_target_base'
  plan.crease_pattern.vertices = [{
    id: VERTEX_A,
    position: { x: 0, y: 0 },
  }]
  plan.crease_pattern.edges = []
  for (let index = 0; index < physicalEndpointCount; index += 1) {
    const vertexId = index === 0
      ? VERTEX_B
      : indexedUuid(1, index + 2)
    plan.crease_pattern.vertices.push({
      id: vertexId,
      position: { x: index + 1, y: 1 },
    })
    plan.crease_pattern.edges.push({
      id: index === 0 ? EDGE_ID : indexedUuid(2, index + 1),
      start: VERTEX_A,
      end: vertexId,
      kind: 'valley',
    })
  }
  const radialSupportAdded =
    [2, 4].includes(physicalEndpointCount)
      ? 4
      : [3, 5, 7, 9, 11, 13].includes(physicalEndpointCount)
      ? 1
      : physicalEndpointCount >= 6 && physicalEndpointCount % 2 === 0
        ? 0
        : null
  if (radialSupportAdded !== null && radialSupportAdded > 0) {
    const supportEdges = []
    for (let index = 0; index < radialSupportAdded; index += 1) {
      const supportVertexId = indexedUuid(6, index + 1)
      plan.crease_pattern.vertices.push({
        id: supportVertexId,
        position: { x: index, y: 2 },
      })
      supportEdges.push({
        id: indexedUuid(7, index + 1),
        start: VERTEX_A,
        end: supportVertexId,
        kind: 'valley',
      })
    }
    plan.crease_pattern.edges.unshift(...supportEdges)
  }
  const graphVertices = [
    {
      id: indexedUuid(3, 1),
      position: { x: 10, y: 10 },
    },
    {
      id: indexedUuid(3, 2),
      position: { x: 11, y: 10 },
    },
    {
      id: indexedUuid(3, 3),
      position: { x: 11, y: 11 },
    },
  ]
  plan.crease_pattern.vertices.push(...graphVertices)
  plan.crease_pattern.edges.push(
    {
      id: indexedUuid(4, 1),
      start: graphVertices[0]!.id,
      end: graphVertices[1]!.id,
      kind: 'auxiliary',
    },
    {
      id: indexedUuid(4, 2),
      start: graphVertices[1]!.id,
      end: graphVertices[2]!.id,
      kind: 'auxiliary',
    },
  )
  plan.instruction_codes = [
    'bounded_tree_river_axial_v1:4000000,1000000',
    ...(radialSupportAdded === null ? [] : [
      `bounded_radial_corner_support_v1:added=${radialSupportAdded}:covered=4`,
    ]),
    'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
  ]
  plan.target_parts = profile.generation_constraints.target_parts.map(
    (part) => ({ ...part }),
  )
  plan.skeleton_segments =
    profile.generation_constraints.skeleton_segments.map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
  const targetAsset = profile.generation_constraints.target_asset
  plan.target_asset = (
    targetAsset === null ? null : { ...targetAsset }
  ) as never
  response.plan_assessments[0]!.kind = plan.kind
  response.plan_assessments[0]!.expected_candidate_edge_id =
    plan.crease_pattern.edges[0]!.id
  response.candidates[0]!.target_approximation_score =
    beginnerExpectedTargetApproximationScoreV1(
      profile,
      'composite_generic_target_base',
    )
  return response
}

type PairedCandidateKind =
  | 'symmetric_four_leg_base'
  | 'asymmetric_four_leg_landmark_base'
  | 'symmetric_bird_base'
  | 'asymmetric_bird_landmark_base'
  | 'asymmetric_insect_landmark_base'
  | 'asymmetric_fish_landmark_base'

function pairedCandidateResponse(
  profile: BeginnerDesignProfileV1,
  kind: PairedCandidateKind,
) {
  const response = validResponse()
  const plan = response.generated_plans[0]!
  plan.kind = kind
  const symmetricFourLegSupport =
    kind === 'symmetric_four_leg_base'
  plan.instruction_codes = [
    kind,
    ...(symmetricFourLegSupport
      ? ['bounded_radial_corner_support_v1:added=4:covered=4']
      : []),
  ]
  plan.target_parts = profile.generation_constraints.target_parts.map(
    (part) => ({ ...part }),
  )
  plan.skeleton_segments =
    profile.generation_constraints.skeleton_segments.map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
  const asymmetric = kind.startsWith('asymmetric_')
  plan.crease_pattern.vertices = [{
    id: VERTEX_A,
    position: { x: 0, y: 0 },
  }]
  const semanticEdges = []
  for (let index = 0; index < 4; index += 1) {
    const vertexId = index === 0
      ? VERTEX_B
      : indexedUuid(1, index + 2)
    plan.crease_pattern.vertices.push({
      id: vertexId,
      position: { x: index + 1, y: 1 },
    })
    semanticEdges.push({
      id: index === 0 ? EDGE_ID : indexedUuid(2, index + 1),
      start: asymmetric ? vertexId : VERTEX_A,
      end: asymmetric ? VERTEX_A : vertexId,
      kind: asymmetric && index === 3 ? 'mountain' : 'valley',
    })
  }
  const supportEdges = []
  if (symmetricFourLegSupport) {
    for (let index = 0; index < 4; index += 1) {
      const vertexId = indexedUuid(21, index + 1)
      plan.crease_pattern.vertices.push({
        id: vertexId,
        position: {
          x: index === 0 || index === 3 ? -1 : 1,
          y: index < 2 ? -1 : 1,
        },
      })
      supportEdges.push({
        id: indexedUuid(22, index + 1),
        start: VERTEX_A,
        end: vertexId,
        kind: 'valley',
      })
    }
  }
  plan.crease_pattern.edges = [...supportEdges, ...semanticEdges]
  const semanticRoles = kind === 'asymmetric_insect_landmark_base'
    ? [
        'head',
        'tail',
        'wing_left',
        'wing_right',
        'leg_front_left',
        'leg_front_right',
        'leg_middle_left',
        'leg_middle_right',
        'leg_rear_left',
        'leg_rear_right',
      ]
    : kind === 'asymmetric_fish_landmark_base'
      ? ['head', 'tail', 'fin_left', 'fin_right']
      : null
  if (semanticRoles) {
    Object.assign(plan, {
      semantic_landmark_provenance: {
        schema_version: 1,
        ordered_bindings: semanticRoles.map((role, ordinal) => ({
          ordinal,
          role,
          physical_ray: ordinal % 4,
        })),
        physical_ray_group_sha256: (
          kind === 'asymmetric_insect_landmark_base'
            ? ASYMMETRIC_INSECT_RAY_DIGESTS
            : ASYMMETRIC_FISH_RAY_DIGESTS
        ).map((digest) => Array.from(digest)),
      },
    })
  }
  response.plan_assessments[0]!.kind = kind
  response.plan_assessments[0]!.expected_candidate_edge_id =
    plan.crease_pattern.edges[0]!.id
  response.candidates[0]!.target_approximation_score =
    beginnerExpectedTargetApproximationScoreV1(profile, kind)
  return response
}

type AsymmetricSemanticKind =
  | 'asymmetric_insect_landmark_base'
  | 'asymmetric_fish_landmark_base'

function asymmetricSemanticProfile(
  kind: AsymmetricSemanticKind,
): BeginnerDesignProfileV1 {
  const insect = kind === 'asymmetric_insect_landmark_base'
  const profile = genericCandidateProfile(
    insect ? 'insect' : 'animal',
    insect ? 9 : 3,
    Array.from({ length: insect ? 7 : 3 }, () => 1),
  )
  profile.generation_constraints.target_parts = insect
    ? [
        { kind: 'head', count: 1 },
        { kind: 'torso', count: 1 },
        { kind: 'tail', count: 1 },
        { kind: 'wing', count: 2 },
        { kind: 'leg', count: 6 },
      ]
    : [
        { kind: 'head', count: 1 },
        { kind: 'torso', count: 1 },
        { kind: 'tail', count: 1 },
        { kind: 'fin', count: 2 },
      ]
  return profile
}

function pairedGridResponse(
  profile: BeginnerDesignProfileV1,
  kind: AsymmetricSemanticKind,
) {
  const response = gridResponseWithLocalBindings(0)
  const candidate = response.candidates[0]!
  candidate.plan = pairedCandidateResponse(
    profile,
    kind,
  ).generated_plans[0]!
  candidate.assessment.kind = kind
  candidate.assessment.expected_candidate_edge_id =
    candidate.plan.crease_pattern.edges[0]!.id
  synchronizeGridCandidateScalars(candidate, profile)
  return response
}

function mutateAsymmetricSemanticProfile(
  profile: BeginnerDesignProfileV1,
  mutation: 'non-singleton' | 'symmetry' | 'direction' | 'local-outline',
) {
  const protrusion = profile.generation_constraints.protrusions![0]!
  if (mutation === 'non-singleton') protrusion.count = 2
  if (mutation === 'symmetry') protrusion.symmetry = 'bilateral'
  if (mutation === 'direction') {
    protrusion.direction_milli = [0, 0, 0]
  }
  if (mutation === 'local-outline') {
    protrusion.local_outline_tenths_mm = [
      [2_000, 2_000],
      [2_100, 2_000],
      [2_000, 2_100],
    ]
  }
}

function completeAnimalProfile(): BeginnerDesignProfileV1 {
  const profile = expectedTailProfile()
  profile.generation_constraints.target_parts = [
    { kind: 'head', count: 1 },
    { kind: 'torso', count: 1 },
    { kind: 'horn', count: 1 },
    { kind: 'tail', count: 1 },
    { kind: 'ear', count: 2 },
    { kind: 'leg', count: 4 },
  ]
  return profile
}

function completeAnimalCandidateResponse(
  profile: BeginnerDesignProfileV1,
  supportAdded = 0,
) {
  const response = validResponse()
  const plan = response.generated_plans[0]!
  plan.kind = 'composite_complete_animal_base'
  plan.instruction_codes = [
    'composite_complete_animal_base',
    `bounded_radial_corner_support_v1:added=${supportAdded}:covered=4`,
  ]
  plan.target_parts = profile.generation_constraints.target_parts.map(
    (part) => ({ ...part }),
  )
  plan.skeleton_segments =
    profile.generation_constraints.skeleton_segments.map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
  plan.crease_pattern.vertices = [{
    id: VERTEX_A,
    position: { x: 0, y: 0 },
  }]
  const baseEdges = []
  for (let index = 0; index < 10; index += 1) {
    const vertexId = index === 0
      ? VERTEX_B
      : indexedUuid(11, index + 1)
    plan.crease_pattern.vertices.push({
      id: vertexId,
      position: { x: index + 1, y: 1 },
    })
    baseEdges.push({
      id: index === 0 ? EDGE_ID : indexedUuid(12, index + 1),
      start: VERTEX_A,
      end: vertexId,
      kind: 'valley',
    })
  }
  const supportEdges = []
  for (let index = 0; index < supportAdded; index += 1) {
    const vertexId = indexedUuid(15, index + 1)
    plan.crease_pattern.vertices.push({
      id: vertexId,
      position: { x: index + 1, y: 2 },
    })
    supportEdges.push({
      id: indexedUuid(16, index + 1),
      start: VERTEX_A,
      end: vertexId,
      kind: 'valley',
    })
  }
  plan.crease_pattern.edges = [...supportEdges, ...baseEdges]
  response.plan_assessments[0]!.kind = plan.kind
  response.plan_assessments[0]!.expected_candidate_edge_id =
    plan.crease_pattern.edges[0]!.id
  response.candidates[0]!.target_approximation_score =
    beginnerExpectedTargetApproximationScoreV1(
      profile,
      'composite_complete_animal_base',
    )
  return response
}

type SpecializedRadialKind =
  | 'composite_horn_tail_ear_base'
  | 'composite_wing_antenna_base'

function specializedRadialProfile(
  kind: SpecializedRadialKind,
): BeginnerDesignProfileV1 {
  const profile = expectedTailProfile()
  profile.generation_constraints.target_category =
    kind === 'composite_wing_antenna_base' ? 'insect' : 'animal'
  profile.generation_constraints.target_parts = [
    { kind: 'head', count: 1 },
    { kind: 'torso', count: 1 },
    ...(kind === 'composite_wing_antenna_base'
      ? [
          { kind: 'wing' as const, count: 2 },
          { kind: 'antenna' as const, count: 2 },
        ]
      : [
          { kind: 'horn' as const, count: 1 },
          { kind: 'tail' as const, count: 1 },
          { kind: 'ear' as const, count: 2 },
        ]),
  ]
  return profile
}

function specializedRadialCandidateResponse(
  profile: BeginnerDesignProfileV1,
  kind: SpecializedRadialKind,
  basePhysicalEdgeCount: 6 | 8,
  supportAdded: number | null,
) {
  const response = validResponse()
  const plan = response.generated_plans[0]!
  plan.kind = kind
  plan.instruction_codes = [
    kind,
    ...(supportAdded === null ? [] : [
      `bounded_radial_corner_support_v1:added=${supportAdded}:covered=4`,
    ]),
  ]
  plan.target_parts = profile.generation_constraints.target_parts.map(
    (part) => ({ ...part }),
  )
  plan.skeleton_segments =
    profile.generation_constraints.skeleton_segments.map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
  plan.crease_pattern.vertices = [{
    id: VERTEX_A,
    position: { x: 0, y: 0 },
  }]
  const baseEdges = []
  for (let index = 0; index < basePhysicalEdgeCount; index += 1) {
    const vertexId = index === 0
      ? VERTEX_B
      : indexedUuid(13, index + 1)
    plan.crease_pattern.vertices.push({
      id: vertexId,
      position: { x: index + 1, y: 1 },
    })
    baseEdges.push({
      id: index === 0 ? EDGE_ID : indexedUuid(14, index + 1),
      start: VERTEX_A,
      end: vertexId,
      kind: 'valley',
    })
  }
  const supportEdges = []
  for (let index = 0; index < (supportAdded ?? 0); index += 1) {
    const vertexId = indexedUuid(17, index + 1)
    plan.crease_pattern.vertices.push({
      id: vertexId,
      position: { x: index + 1, y: 2 },
    })
    supportEdges.push({
      id: indexedUuid(18, index + 1),
      start: VERTEX_A,
      end: vertexId,
      kind: 'valley',
    })
  }
  plan.crease_pattern.edges = [...supportEdges, ...baseEdges]
  response.plan_assessments[0]!.kind = kind
  response.plan_assessments[0]!.expected_candidate_edge_id =
    plan.crease_pattern.edges[0]!.id
  response.candidates[0]!.target_approximation_score =
    beginnerExpectedTargetApproximationScoreV1(profile, kind)
  return response
}

function specializedRadialGridResponse(
  profile: BeginnerDesignProfileV1,
  kind: SpecializedRadialKind,
  basePhysicalEdgeCount: 6 | 8,
  supportAdded: number | null,
) {
  const response = gridResponseWithLocalBindings(0)
  const candidate = response.candidates[0]!
  candidate.plan = specializedRadialCandidateResponse(
    profile,
    kind,
    basePhysicalEdgeCount,
    supportAdded,
  ).generated_plans[0]!
  candidate.assessment.kind = kind
  candidate.assessment.expected_candidate_edge_id =
    candidate.plan.crease_pattern.edges[0]!.id
  synchronizeGridCandidateScalars(candidate, profile)
  return response
}

async function evaluate(
  response: unknown,
  expectedProfile: BeginnerDesignProfileV1 = expectedTailProfile(),
  requestedCandidateCount = 1,
) {
  nativeInvoke.mockResolvedValueOnce(response)
  return evaluateBeginnerCandidates(
    PROJECT_ID,
    7,
    INSTANCE_ID,
    requestedCandidateCount,
    CANDIDATE_GENERATION_ID,
    expectedProfile,
  )
}

function appendFoldVariant(
  response: ReturnType<typeof validResponse>,
  kind: 'vertical_book_fold' | 'horizontal_book_fold' | 'diagonal_fold',
) {
  const instruction = kind === 'vertical_book_fold'
    ? 'book_fold_vertical'
    : kind === 'horizontal_book_fold'
      ? 'book_fold_horizontal'
      : 'diagonal_fold'
  const plan = structuredClone(response.generated_plans[0]!)
  plan.kind = kind
  plan.instruction_codes = [instruction]
  plan.crease_pattern = {
    vertices: [
      { id: VERTEX_A, position: { x: 0, y: 0 } },
      { id: VERTEX_B, position: { x: 1, y: 0 } },
    ],
    edges: [{
      id: EDGE_ID,
      start: VERTEX_A,
      end: VERTEX_B,
      kind: 'valley',
    }],
  }
  const assessment = structuredClone(response.plan_assessments[0]!)
  assessment.kind = kind
  assessment.expected_candidate_edge_id = EDGE_ID
  const rank = response.generated_plans.length + 1
  response.requested_candidate_count = rank
  response.generated_plans.push(plan)
  response.plan_assessments.push(assessment)
  response.candidates.push(balancedCandidateScore(
    rank === 2 ? 'shape_focused' : 'foldability_focused',
    rank,
  ))
}

function indexedUuid(namespace: number, index: number) {
  return `${namespace.toString(16).padStart(8, '0')}`
    + `-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
}

function balancedCandidateScore(
  kind: 'recommended' | 'shape_focused' | 'foldability_focused',
  rank: number,
  targetApproximationScore = 75,
) {
  const shapeScore =
    kind === 'foldability_focused' ? 90 : 100
  const foldabilityScore =
    kind === 'shape_focused' ? 90 : 100
  return {
    schema_version: 1,
    kind,
    rank,
    total_score: Math.floor(
      (
        shapeScore * 35
        + foldabilityScore * 35
        + 100 * 15
        + 100 * 15
      ) / 100,
    ),
    shape_score: shapeScore,
    target_approximation_score: targetApproximationScore,
    foldability_score: foldabilityScore,
    step_count_score: 100,
    paper_efficiency_score: 100,
  }
}

function gridResponseWithLocalBindings(bindingCount = 12) {
  const vertices = [
    { id: indexedUuid(1, 1), position: { x: 0, y: 0 } },
    { id: indexedUuid(1, 2), position: { x: 1, y: 0 } },
  ]
  const edges = [{
    id: indexedUuid(2, 1),
    start: vertices[0]!.id,
    end: vertices[1]!.id,
    kind: 'valley',
  }]
  const localBindings = []
  for (let index = 0; index < bindingCount; index += 1) {
    const vertexStart = vertices.length
    const creaseStart = edges.length
    const block = [0, 1, 2].map((offset) => ({
      id: indexedUuid(1, vertexStart + offset + 1),
      position: { x: index * 10 + offset, y: index + 1 },
    }))
    vertices.push(...block)
    for (let offset = 0; offset < block.length; offset += 1) {
      edges.push({
        id: indexedUuid(2, creaseStart + offset + 1),
        start: block[offset]!.id,
        end: block[(offset + 1) % block.length]!.id,
        kind: 'auxiliary',
      })
    }
    localBindings.push({
      protrusion_id: index,
      contour_points: 3,
      generated_face_id: index + 1,
      vertex_start: vertexStart,
      crease_start: creaseStart,
    })
  }
  const plan = {
    schema_version: 1,
    kind: 'center_axis_tail_base',
    crease_pattern: { vertices, edges },
    instruction_codes: ['center_axis_tail_base'],
    target_parts: [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'tail', count: 1 },
    ],
    skeleton_segments: [],
    target_asset: null,
  }
  const assessment = {
    kind: plan.kind,
    expected_candidate_edge_id: edges[0]!.id,
    proof_scope: 'sufficient',
    apply_allowed: true,
    reason: 'native_fold_path_certified',
    shape_approximation_score: null,
    shape_difference_reason: null,
    component_shape_comparison: null,
  }
  const response = {
    request_generation_id: GRID_GENERATION_ID,
    authority_token: GRID_AUTHORITY_ID,
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: 7,
    grid_hash: Array.from(CANONICAL_GRID_HASH_V1),
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
      complexity_score: 0,
      paper_efficiency_score: 50,
      scale_deviation_penalty: 20,
      spacing_deviation_penalty: 0,
      detail_mismatch_penalty: 0,
      outcome_reason: assessment.reason,
      contour_witness: {
        body_contour_points: 0,
        local_bindings: localBindings,
        generic_feature_bindings: [],
        skeleton_branch_bindings: [],
        skeleton_tree_authority_sha256: Array(32).fill(2),
        witnessed_vertices: bindingCount * 3,
        witnessed_creases: bindingCount * 3,
        topology_authority_hash: Array(32).fill(3),
        max_contour_error_millionths: 0,
      },
      refinement_iterations: 0,
      strict_improvements: 0,
      refinement_starts: 1,
    }],
  }
  synchronizeGridCandidateScalars(
    response.candidates[0]!,
    expectedTailProfile(),
  )
  return response
}

function synchronizeGridCandidateScalars(
  candidate: ReturnType<
    typeof gridResponseWithLocalBindings
  >['candidates'][number],
  profile: BeginnerDesignProfileV1,
) {
  const semanticEndpointCount =
    profile.generation_constraints.target_parts.reduce(
      (sum, part) =>
        part.kind === 'head' || part.kind === 'torso'
          ? sum
          : sum + part.count,
      0,
    )
  const physicalEndpointCount =
    (profile.generation_constraints.protrusions ?? []).reduce(
      (sum, protrusion) => sum + protrusion.count,
      0,
    )
  const protrusionCount =
    candidate.plan.kind === 'asymmetric_insect_landmark_base'
      ? 7
      : profile.generation_constraints.target_category === 'custom_object'
          && semanticEndpointCount === 0
        ? physicalEndpointCount
        : semanticEndpointCount
  const estimateScale =
    profile.generation_constraints.detail_level === 'simple'
      ? 20
      : profile.generation_constraints.detail_level === 'standard'
        ? 25
        : 30
  const estimateSpacing = protrusionCount === 4 ? 35 : 50
  candidate.scale_deviation_penalty =
    Math.abs(candidate.point.scale_percent - estimateScale) * 10
  candidate.spacing_deviation_penalty =
    Math.abs(candidate.point.spacing_percent - estimateSpacing) * 5
  candidate.detail_mismatch_penalty =
    candidate.point.detail_level
      === profile.generation_constraints.detail_level
      ? 0
      : 10
  candidate.primary_score = Math.max(
    0,
    1_000
      - candidate.scale_deviation_penalty
      - candidate.spacing_deviation_penalty
      - candidate.detail_mismatch_penalty,
  )
  const detailComplexity =
    candidate.point.detail_level === 'simple'
      ? 10
      : candidate.point.detail_level === 'standard'
        ? 20
        : 30
  candidate.complexity_score = Math.min(
    100,
    candidate.plan.crease_pattern.edges.length * 10
      + detailComplexity,
  )
}

function genericGridResponse(
  witnessedEndpointCount = 2,
  semanticEndpointCount: number | null = null,
) {
  const response = gridResponseWithLocalBindings(0)
  const candidate = response.candidates[0]!
  candidate.plan.kind = 'composite_generic_target_base'
  const radialSupportAdded =
    witnessedEndpointCount >= 2
      && witnessedEndpointCount <= 14
      ? witnessedEndpointCount % 2 === 0 ? 4 : 5
      : null
  candidate.plan.instruction_codes = [
    'bounded_tree_river_axial_v1:4000000,1000000',
    ...(radialSupportAdded === null ? [] : [
      `bounded_radial_corner_support_v1:added=${radialSupportAdded}:covered=4`,
    ]),
    'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
    'bounded_tree_paper_orientation_v1:horizontal',
  ]
  candidate.plan.target_parts = semanticEndpointCount === null
    ? []
    : [
        { kind: 'head', count: 1 },
        { kind: 'torso', count: 1 },
        {
          kind: 'fin',
          count: Math.min(semanticEndpointCount, 8),
        },
        ...(semanticEndpointCount > 8
          ? [{ kind: 'tail', count: semanticEndpointCount - 8 }]
          : []),
      ]
  candidate.plan.skeleton_segments = [
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
  ]
  while (
    candidate.plan.crease_pattern.edges.length < witnessedEndpointCount
  ) {
    const vertex = {
      id: indexedUuid(
        1,
        candidate.plan.crease_pattern.vertices.length + 1,
      ),
      position: {
        x: candidate.plan.crease_pattern.vertices.length,
        y: 1,
      },
    }
    candidate.plan.crease_pattern.vertices.push(vertex)
    candidate.plan.crease_pattern.edges.push({
      id: indexedUuid(2, candidate.plan.crease_pattern.edges.length + 1),
      start: candidate.plan.crease_pattern.vertices[0]!.id,
      end: vertex.id,
      kind: 'valley',
    })
  }
  if (radialSupportAdded !== null && radialSupportAdded > 0) {
    const supportEdges = []
    for (let index = 0; index < radialSupportAdded; index += 1) {
      const vertex = {
        id: indexedUuid(
          5,
          candidate.plan.crease_pattern.vertices.length + 1,
        ),
        position: { x: index, y: 2 },
      }
      candidate.plan.crease_pattern.vertices.push(vertex)
      supportEdges.push({
        id: indexedUuid(6, index + 1),
        start: candidate.plan.crease_pattern.vertices[0]!.id,
        end: vertex.id,
        kind: 'valley',
      })
    }
    candidate.plan.crease_pattern.edges.unshift(...supportEdges)
    candidate.assessment.expected_candidate_edge_id =
      candidate.plan.crease_pattern.edges[0]!.id
  }
  const graphVertices = [
    { id: indexedUuid(15, 1), position: { x: 10, y: 10 } },
    { id: indexedUuid(15, 2), position: { x: 11, y: 10 } },
    { id: indexedUuid(15, 3), position: { x: 11, y: 11 } },
  ]
  candidate.plan.crease_pattern.vertices.push(...graphVertices)
  candidate.plan.crease_pattern.edges.push(
    {
      id: indexedUuid(16, 1),
      start: graphVertices[0]!.id,
      end: graphVertices[1]!.id,
      kind: 'auxiliary',
    },
    {
      id: indexedUuid(16, 2),
      start: graphVertices[1]!.id,
      end: graphVertices[2]!.id,
      kind: 'auxiliary',
    },
  )
  candidate.assessment.kind = candidate.plan.kind
  candidate.contour_witness.generic_feature_bindings = Array.from(
    { length: witnessedEndpointCount },
    (_, index) => {
      const binding = {
        protrusion_id: index + 1,
        generated_feature_id: index + 1,
        endpoint_count: 1,
        crease_start: (radialSupportAdded ?? 0) + index,
        crease_authority_sha256: Array(32).fill(4 + index),
        skeleton_segment_id: index === 0 ? 10 : 20,
        skeleton_endpoint: index === 0 ? 'start' : 'end',
        mount_distance_squared_tenths_mm: 0,
      }
      return binding
    },
  )
  candidate.contour_witness.skeleton_branch_bindings = [
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
        { length: Math.max(0, witnessedEndpointCount - 1) },
        (_, index) => index + 2,
      ),
    },
  ]
  synchronizeGridCandidateScalars(
    candidate,
    expectedProfileForGridResponse(response),
  )
  return response
}

function genericGridResponseWithLocalBindings(bindingCount = 14) {
  const profile = genericCandidateProfile(
    'custom_object',
    0,
    Array.from({ length: bindingCount }, () => 1),
  )
  for (
    const protrusion of
    profile.generation_constraints.protrusions ?? []
  ) {
    protrusion.local_outline_tenths_mm = [
      [0, 0],
      [1, 0],
      [0, 1],
    ]
  }
  const response = genericGridResponse(bindingCount)
  const candidate = response.candidates[0]!
  const plan = candidate.plan
  const graphVertices = plan.crease_pattern.vertices.splice(-3)
  const graphEdges = plan.crease_pattern.edges.splice(-2)
  candidate.contour_witness.local_bindings = []
  for (let index = 0; index < bindingCount; index += 1) {
    const vertexStart = plan.crease_pattern.vertices.length
    const creaseStart = plan.crease_pattern.edges.length
    const block = [0, 1, 2].map((offset) => ({
      id: indexedUuid(30 + index, offset + 1),
      position: { x: index * 10 + offset, y: index + 3 },
    }))
    plan.crease_pattern.vertices.push(...block)
    for (let offset = 0; offset < block.length; offset += 1) {
      plan.crease_pattern.edges.push({
        id: indexedUuid(50 + index, offset + 1),
        start: block[offset]!.id,
        end: block[(offset + 1) % block.length]!.id,
        kind: 'auxiliary',
      })
    }
    candidate.contour_witness.local_bindings.push({
      protrusion_id:
        profile.generation_constraints.protrusions![index]!.id,
      contour_points: 3,
      generated_face_id: index + 1,
      vertex_start: vertexStart,
      crease_start: creaseStart,
    })
  }
  plan.crease_pattern.vertices.push(...graphVertices)
  plan.crease_pattern.edges.push(...graphEdges)
  candidate.contour_witness.witnessed_vertices = bindingCount * 3
  candidate.contour_witness.witnessed_creases = bindingCount * 3
  synchronizeGridCandidateScalars(candidate, profile)
  return { profile, response }
}

function gridResponseWithTwoCandidates() {
  const response = gridResponseWithLocalBindings(0)
  const second = structuredClone(response.candidates[0]!)
  second.point.id = 12
  second.point.scale_percent = 27
  second.point.spacing_percent = 20
  second.point.detail_level = 'standard'
  synchronizeGridCandidateScalars(second, expectedTailProfile())
  response.candidates.push(second)
  return response
}

function expectedProfileForGridResponse(
  response: ReturnType<typeof gridResponseWithLocalBindings>,
): BeginnerDesignProfileV1 {
  const candidate = response.candidates[0]
  if (candidate?.plan.kind !== 'composite_generic_target_base') {
    return expectedTailProfile()
  }
  const semanticEndpointCount = candidate.plan.target_parts.reduce(
    (sum, part) =>
      part.kind === 'head' || part.kind === 'torso'
        ? sum
        : sum + part.count,
    0,
  )
  const witnessedEndpointCount =
    candidate.contour_witness.generic_feature_bindings.reduce(
      (sum, binding) => sum + binding.endpoint_count,
      0,
    )
  const profile = genericCandidateProfile(
    candidate.plan.target_parts.length === 0
      ? 'custom_object'
      : 'animal',
    semanticEndpointCount,
    physicalCountsForEndpoints(
      semanticEndpointCount === 0
        ? witnessedEndpointCount
        : semanticEndpointCount,
    ),
  )
  profile.generation_constraints.target_parts =
    candidate.plan.target_parts.map((part) => ({ ...part })) as
      BeginnerDesignProfileV1['generation_constraints']['target_parts']
  profile.generation_constraints.skeleton_segments =
    candidate.plan.skeleton_segments.map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
  return profile
}

async function evaluateGrid(
  response: unknown,
  expectedProfile?: BeginnerDesignProfileV1,
) {
  nativeInvoke.mockResolvedValueOnce(response)
  return evaluateBeginnerParameterGrid(
    PROJECT_ID,
    7,
    INSTANCE_ID,
    GRID_GENERATION_ID,
    expectedProfile ?? expectedProfileForGridResponse(
      response as ReturnType<typeof gridResponseWithLocalBindings>,
    ),
  )
}

describe('beginner generated-plan response contract', () => {
  beforeEach(() => nativeInvoke.mockReset())

  it('accepts the native omitted semantic field and native fold-path reason', async () => {
    const result = await evaluate(validResponse())

    expect(result.generated_plans[0]).not.toHaveProperty(
      'semantic_landmark_provenance',
    )
    expect(result.plan_assessments[0]).toMatchObject({
      proof_scope: 'sufficient',
      apply_allowed: true,
      reason: 'native_fold_path_certified',
    })
  })

  it('admits bounded folded-pose landmark scoring without a component comparison', async () => {
    const { profile, response } = referenceModelTailFixture()
    Object.assign(response.plan_assessments[0]!, {
      shape_approximation_score: 84,
      shape_difference_reason: 'bounded_folded_pose_landmarks_v1',
    })

    const result = await evaluate(response, profile)
    expect(result.plan_assessments[0]).toMatchObject({
      shape_approximation_score: 84,
      shape_difference_reason: 'bounded_folded_pose_landmarks_v1',
      component_shape_comparison: null,
    })

    const unknownFixture = referenceModelTailFixture()
    const unknown = unknownFixture.response
    Object.assign(unknown.plan_assessments[0]!, {
      shape_approximation_score: 84,
      shape_difference_reason: 'future_landmark_model',
    })
    await expect(evaluate(
      unknown,
      unknownFixture.profile,
    )).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('rejects shape evidence without a live reference-model target', async () => {
    const response = validResponse()
    Object.assign(response.plan_assessments[0]!, {
      shape_approximation_score: 84,
      shape_difference_reason: 'bounded_folded_pose_landmarks_v1',
    })

    await expect(evaluate(response)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('requires the native folded-pose failure proof state exactly', async () => {
    const { profile, response } = referenceModelTailFixture()
    Object.assign(response.plan_assessments[0]!, {
      proof_scope: 'indeterminate',
      apply_allowed: false,
      reason: 'folded_pose_simulation_failed',
      shape_approximation_score: 84,
      shape_difference_reason: 'bounded_folded_pose_landmarks_v1',
    })
    await evaluate(response, profile)

    const wrongScopeFixture = referenceModelTailFixture()
    const wrongScope = wrongScopeFixture.response
    Object.assign(wrongScope.plan_assessments[0]!, {
      proof_scope: 'necessary',
      apply_allowed: false,
      reason: 'folded_pose_simulation_failed',
      shape_approximation_score: 84,
      shape_difference_reason: 'bounded_folded_pose_landmarks_v1',
    })
    await expect(evaluate(
      wrongScope,
      wrongScopeFixture.profile,
    )).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const wrongEvidenceFixture = referenceModelTailFixture()
    const wrongEvidence = wrongEvidenceFixture.response
    Object.assign(wrongEvidence.plan_assessments[0]!, {
      proof_scope: 'indeterminate',
      apply_allowed: false,
      reason: 'folded_pose_simulation_failed',
      shape_approximation_score: 84,
      shape_difference_reason: 'crease_preview_has_no_surface_mesh',
    })
    await expect(evaluate(
      wrongEvidence,
      wrongEvidenceFixture.profile,
    )).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('binds certified flat-surface evidence to native terminal outcomes', async () => {
    const provenFixture = referenceModelTailFixture()
    Object.assign(provenFixture.response.plan_assessments[0]!, {
      proof_scope: 'sufficient',
      apply_allowed: true,
      reason: 'global_flat_foldability_proven',
      shape_approximation_score: 84,
      shape_difference_reason: 'certified_flat_surface_v1',
    })
    await evaluate(provenFixture.response, provenFixture.profile)

    const unavailableFixture = referenceModelTailFixture()
    Object.assign(unavailableFixture.response.plan_assessments[0]!, {
      proof_scope: 'necessary',
      apply_allowed: false,
      reason: 'fold_path_certificate_unavailable',
      shape_approximation_score: 84,
      shape_difference_reason: 'certified_flat_surface_v1',
    })
    await evaluate(unavailableFixture.response, unavailableFixture.profile)

    const forgedFixture = referenceModelTailFixture()
    Object.assign(forgedFixture.response.plan_assessments[0]!, {
      proof_scope: 'sufficient',
      apply_allowed: true,
      reason: 'native_fold_path_certified',
      shape_approximation_score: 84,
      shape_difference_reason: 'certified_flat_surface_v1',
    })
    await expect(evaluate(
      forgedFixture.response,
      forgedFixture.profile,
    )).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it.each([-1, 0, 2, 4])(
    'rejects component comparison work_units=%i',
    async (workUnits) => {
      const { profile, response } = referenceModelTailFixture()
      Object.assign(response.plan_assessments[0]!, {
        shape_approximation_score: 64,
        shape_difference_reason: 'component_aware_quantized_shape_v1',
        component_shape_comparison: {
          component_count: 2,
          matched_branch_count: 2,
          work_units: workUnits,
          extent_score: 80,
          branch_score: 80,
          bridge_score: 0,
          extent_weight: 45,
          branch_weight: 35,
          bridge_weight: 20,
        },
      })

      await expect(evaluate(response, profile)).rejects.toThrow(
        'invalid beginner candidate response',
      )
    },
  )

  it('admits only the native component comparison work and bridge tuple', async () => {
    const { profile, response } = referenceModelTailFixture()
    const componentComparison = {
      component_count: 2,
      matched_branch_count: 2,
      work_units: 3,
      extent_score: 80,
      branch_score: 80,
      bridge_score: 0,
      extent_weight: 45,
      branch_weight: 35,
      bridge_weight: 20,
    }
    Object.assign(response.plan_assessments[0]!, {
      shape_approximation_score: 64,
      shape_difference_reason: 'component_aware_quantized_shape_v1',
      component_shape_comparison: componentComparison,
    })

    await evaluate(response, profile)

    componentComparison.bridge_score = 1
    await expect(evaluate(response, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    componentComparison.bridge_score = 0
    componentComparison.matched_branch_count = 1
    await expect(evaluate(response, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )

  })

  it('snapshots and freezes admitted multi-reference digests', async () => {
    const response = validResponse()
    const profile = expectedTailProfile()
    profile.generation_constraints.target_asset = {
      kind: 'reference_model',
      asset_id: TARGET_ASSET_ID,
    }
    response.generated_plans[0]!.target_asset = {
      kind: 'reference_model',
      asset_id: TARGET_ASSET_ID,
    } as never
    const fusion = {
      revision: 7,
      image_sha256: Array(32).fill(1),
      reference_sha256: Array(32).fill(2),
      source_count: 2,
      image_component_count: 1,
      reference_component_count: 1,
      image_branch_count: 1,
      reference_branch_count: 3,
      normalized_extent_error: 0,
      agreement_score: 80,
      apply_allowed: true,
      reason: 'image_glb_agreement_v1',
    }
    Object.assign(response, { multi_reference_fusion: fusion })

    const result = await evaluate(response, profile)
    const admitted = result.multi_reference_fusion
    expect(admitted).not.toBeNull()
    expect(Object.isFrozen(admitted?.image_sha256)).toBe(true)
    fusion.image_sha256[0] = 9
    expect(admitted?.image_sha256[0]).toBe(1)

    const invalid = validResponse()
    invalid.generated_plans[0]!.target_asset = {
      kind: 'reference_model',
      asset_id: TARGET_ASSET_ID,
    } as never
    Object.assign(invalid, {
      multi_reference_fusion: {
        ...fusion,
        image_sha256: [-1, ...Array(31).fill(1)],
      },
    })
    await expect(evaluate(invalid, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('admits the native image and GLB fusion boundary exactly', async () => {
    const { profile, response } = referenceModelTailFixture()
    const fusion = {
      revision: 7,
      image_sha256: Array(32).fill(1),
      reference_sha256: Array(32).fill(2),
      source_count: 2,
      image_component_count: 8,
      reference_component_count: 8,
      image_branch_count: 15,
      reference_branch_count: 15,
      normalized_extent_error: 0,
      agreement_score: 100,
      apply_allowed: true,
      reason: 'image_glb_agreement_v1',
    }
    Object.assign(response, { multi_reference_fusion: fusion })
    await evaluate(response, profile)

    fusion.image_component_count = 9
    fusion.image_branch_count = 17
    fusion.agreement_score = 60
    await expect(evaluate(response, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const referenceBoundary = referenceModelTailFixture()
    Object.assign(referenceBoundary.response, {
      multi_reference_fusion: {
        ...fusion,
        image_component_count: 8,
        image_branch_count: 15,
        reference_component_count: 9,
        reference_branch_count: 17,
      },
    })
    await expect(evaluate(
      referenceBoundary.response,
      referenceBoundary.profile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('snapshots and freezes admitted consensus pair evidence', async () => {
    const fixture = consensusCandidateFixture()
    const pair = fixture.pairs[0]!
    const expectedFirstDigestByte = pair.pair_digest_sha256[0]!
    const result = await evaluate(fixture.response, fixture.profile)
    const admitted = result.reference_consensus_analysis
    expect(admitted).not.toBeNull()
    expect(Object.isFrozen(
      admitted?.pairs[0]?.pair_digest_sha256,
    )).toBe(true)
    pair.pair_digest_sha256[0] = 9
    pair.left_normalized_extents[0] = 99
    expect(admitted?.pairs[0]?.pair_digest_sha256[0]).toBe(
      expectedFirstDigestByte,
    )
    expect(admitted?.pairs[0]?.left_normalized_extents[0]).toBe(50)

    const invalid = consensusCandidateFixture()
    invalid.pairs[0]!.pair_digest_sha256 = Array(32).fill(-1)
    await expect(evaluate(
      invalid.response,
      invalid.profile,
    )).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const forged = consensusCandidateFixture()
    forged.pairs[0]!.pair_digest_sha256[0] =
      forged.pairs[0]!.pair_digest_sha256[0]! ^ 1
    await expect(evaluate(
      forged.response,
      forged.profile,
    )).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('admits kind-specific consensus descriptor boundaries', async () => {
    const fixture = consensusCandidateFixture()
    fixture.pairs[0]!.left_component_count = 16
    fixture.pairs[0]!.left_branch_count = 31
    fixture.pairs[0]!.right_component_count = 8
    fixture.pairs[0]!.right_branch_count = 15
    fixture.pairs[0]!.left_normalized_extents = [0, 100]
    fixture.pairs[0]!.right_normalized_extents = [0, 100]
    synchronizeConsensusFixture(fixture)
    await evaluate(fixture.response, fixture.profile)
  })

  it('rejects a reference consensus descriptor above its native boundary', async () => {
    const fixture = consensusCandidateFixture()
    fixture.pairs[0]!.right_component_count = 9
    fixture.pairs[0]!.right_branch_count = 17
    synchronizeConsensusFixture(fixture)
    await expect(evaluate(
      fixture.response,
      fixture.profile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it.each([
    ['component count 17', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs[0]!.left_component_count = 17
    }],
    ['branch count 32', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs[0]!.left_branch_count = 32
    }],
    ['negative normalized extent', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs[0]!.left_normalized_extents = [-1, 100]
    }],
  ] as const)(
    'rejects consensus pair %s',
    async (_label, mutate) => {
      const fixture = consensusCandidateFixture()
      mutate(fixture)
      await expect(evaluate(
        fixture.response,
        fixture.profile,
      )).rejects.toThrow('invalid beginner candidate response')
    },
  )

  it.each([
    ['arbitrary asset', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs[0]!.left_asset_id = FOURTH_CONSENSUS_ASSET_ID
    }],
    ['duplicate pair', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs[1]!.left_asset_id =
        fixture.pairs[0]!.left_asset_id
      fixture.pairs[1]!.right_asset_id =
        fixture.pairs[0]!.right_asset_id
    }],
    ['missing pair', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs.pop()
    }],
    ['source count', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.analysis.source_count -= 1
    }],
    ['pair count', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.analysis.pair_count -= 1
    }],
    ['disagreement aggregate', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.pairs[0]!.disagrees = true
    }],
    ['agreement average', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.analysis.agreement_score -= 1
    }],
    ['reason', (fixture: ReturnType<
      typeof consensusCandidateFixture
    >) => {
      fixture.analysis.reason =
        'reference_consensus_multiple_disagreements_v1'
    }],
  ] as const)(
    'rejects consensus analysis with mutated %s',
    async (_label, mutate) => {
      const fixture = consensusCandidateFixture(
        consensusExpectedProfile(3),
      )
      mutate(fixture)
      await expect(evaluate(
        fixture.response,
        fixture.profile,
      )).rejects.toThrow('invalid beginner candidate response')
    },
  )

  it('binds consensus analysis to the exact excluded asset', async () => {
    const profile = consensusExpectedProfile(
      3,
      OTHER_ASSET_ID,
    )
    const fixture = consensusCandidateFixture(profile)
    await evaluate(fixture.response, profile)

    fixture.analysis.excluded_asset_id = null
    await expect(evaluate(fixture.response, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('requires null analysis when exclusion leaves fewer than two sources', async () => {
    const profile = consensusExpectedProfile(2, OTHER_ASSET_ID)
    await evaluate(validResponse(), profile)

    const forged = consensusCandidateFixture()
    await expect(evaluate(
      forged.response,
      profile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('uses the snapshotted consensus binding order after invoke', async () => {
    const fixture = consensusCandidateFixture()
    const pending = evaluate(fixture.response, fixture.profile)
    const firstBinding =
      fixture.profile.reference_consensus_v1!.bindings[0] as {
        asset_id: string
      }
    firstBinding.asset_id = FOURTH_CONSENSUS_ASSET_ID

    await expect(pending).resolves.toMatchObject({
      reference_consensus_analysis: {
        source_count: 2,
      },
    })
  })

  it('requires every assessment to mirror the consensus blocker iff active', async () => {
    const fixture = consensusCandidateFixture(
      consensusExpectedProfile(3),
    )
    for (const pair of fixture.pairs) {
      if (pair.left_asset_id !== TARGET_ASSET_ID) {
        pair.left_component_count = 3
        pair.left_branch_count = 5
      }
      if (pair.right_asset_id !== TARGET_ASSET_ID) {
        pair.right_component_count = 3
        pair.right_branch_count = 5
      }
    }
    synchronizeConsensusFixture(fixture)
    Object.assign(fixture.response.plan_assessments[0]!, {
      proof_scope: 'indeterminate',
      apply_allowed: false,
      reason: 'multi_reference_disagreement',
    })

    await evaluate(fixture.response, fixture.profile)

    fixture.response.plan_assessments[0]!.proof_scope = 'sufficient'
    fixture.response.plan_assessments[0]!.apply_allowed = true
    fixture.response.plan_assessments[0]!.reason =
      'native_fold_path_certified'
    await expect(evaluate(
      fixture.response,
      fixture.profile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('rejects an invalid expected profile before invoking native evaluation', async () => {
    const profile = expectedTailProfile()
    profile.shape_fidelity_weight = 34

    await expect(evaluate(validResponse(), profile)).rejects.toThrow(
      'invalid expected beginner profile',
    )
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it.each([
    'candidate-generation',
    '00000000-0000-0000-0000-000000000000',
    'AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA',
  ])(
    'rejects non-canonical candidate generation ID %s before invoke',
    async (requestGenerationId) => {
      await expect(evaluateBeginnerCandidates(
        PROJECT_ID,
        7,
        INSTANCE_ID,
        1,
        requestGenerationId,
        expectedTailProfile(),
      )).rejects.toThrow('invalid candidate generation')
      expect(nativeInvoke).not.toHaveBeenCalled()
    },
  )

  it('snapshots the expected profile before the native response settles', async () => {
    const profile = expectedTailProfile()
    const pending = evaluate(validResponse(), profile)
    profile.generation_constraints.target_parts[2] = {
      kind: 'horn',
      count: 1,
    }

    await expect(pending).resolves.toMatchObject({
      generated_plans: [{
        kind: 'center_axis_tail_base',
      }],
    })
  })

  it('binds candidate target parts in request order and binds the target asset', async () => {
    const reordered = validResponse()
    reordered.generated_plans[0]!.target_parts = [
      { kind: 'torso', count: 1 },
      { kind: 'head', count: 1 },
      { kind: 'tail', count: 1 },
    ]
    await expect(evaluate(reordered)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const assetProfile = genericCandidateProfile(
      'custom_object',
      0,
      [2],
    )
    assetProfile.generation_constraints.target_asset = {
      kind: 'reference_model',
      asset_id: TARGET_ASSET_ID,
    }
    await evaluate(genericCandidateResponse(assetProfile), assetProfile)

    const forgedAsset = genericCandidateResponse(assetProfile)
    forgedAsset.generated_plans[0]!.target_asset = {
      kind: 'reference_model',
      asset_id: OTHER_ASSET_ID,
    } as never
    await expect(evaluate(forgedAsset, assetProfile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('rejects a specialized plan forged for a custom request category', async () => {
    const customProfile = genericCandidateProfile('custom_object', 0, [1])
    customProfile.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'tail', count: 1 },
    ]
    await expect(evaluate(
      validResponse(),
      customProfile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('preserves the exact native animal fold-variant order', async () => {
    const response = validResponse()
    appendFoldVariant(response, 'vertical_book_fold')
    appendFoldVariant(response, 'horizontal_book_fold')

    const result = await evaluate(response, expectedTailProfile(), 3)

    expect(result.generated_plans.map((plan) => plan.kind)).toEqual([
      'center_axis_tail_base',
      'vertical_book_fold',
      'horizontal_book_fold',
    ])
  })

  it('rejects candidate scores that violate native weighting or kind uniqueness', async () => {
    const forgedTotal = validResponse()
    forgedTotal.candidates[0]!.total_score = 99
    await expect(evaluate(forgedTotal)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const duplicateKind = validResponse()
    appendFoldVariant(duplicateKind, 'vertical_book_fold')
    duplicateKind.candidates[1]!.kind = 'recommended'
    await expect(evaluate(
      duplicateKind,
      expectedTailProfile(),
      2,
    )).rejects.toThrow('invalid beginner candidate response')

    const forgedTargetApproximation = validResponse()
    forgedTargetApproximation.candidates[0]!
      .target_approximation_score = 76
    await expect(evaluate(forgedTargetApproximation)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('rejects a noncanonical fold variant order for an animal request', async () => {
    const response = validResponse()
    appendFoldVariant(response, 'diagonal_fold')

    await expect(evaluate(
      response,
      expectedTailProfile(),
      2,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('preserves the native insect diagonal fold as the second plan', async () => {
    const profile = expectedTailProfile()
    profile.generation_constraints.target_category = 'insect'
    profile.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'antenna', count: 1 },
    ]
    profile.generation_constraints.protrusions = [
      genericProtrusion(1, 1),
    ]
    const response = validResponse()
    response.generated_plans[0]!.kind = 'center_axis_antenna_base'
    response.generated_plans[0]!.instruction_codes = [
      'center_axis_antenna_base',
    ]
    response.generated_plans[0]!.target_parts =
      profile.generation_constraints.target_parts.map((part) => ({ ...part }))
    response.plan_assessments[0]!.kind = 'center_axis_antenna_base'
    appendFoldVariant(response, 'diagonal_fold')
    for (const candidate of response.candidates) {
      candidate.target_approximation_score =
        beginnerExpectedTargetApproximationScoreV1(
          profile,
          'center_axis_antenna_base',
        )
    }

    const result = await evaluate(response, profile, 2)

    expect(result.generated_plans.map((plan) => plan.kind)).toEqual([
      'center_axis_antenna_base',
      'diagonal_fold',
    ])
  })

  it('rejects a fold variant forged for a custom request category', async () => {
    const profile = genericCandidateProfile('custom_object', 0, [1])
    profile.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'tail', count: 1 },
    ]
    const response = genericCandidateResponse(profile)
    appendFoldVariant(response, 'vertical_book_fold')

    await expect(evaluate(response, profile, 2)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it.each([9, 10, 11, 12, 13, 14])(
    'accepts an exact %i-endpoint animal general candidate',
    async (endpointCount) => {
      const profile = genericCandidateProfile(
        'animal',
        endpointCount,
        physicalCountsForEndpoints(endpointCount),
      )
      const result = await evaluate(
        genericCandidateResponse(profile),
        profile,
      )
      expect(result.generated_plans[0]?.kind)
        .toBe('composite_generic_target_base')
    },
  )

  it('accepts an exact insect general candidate outside specialized signatures', async () => {
    const profile = genericCandidateProfile('insect', 3, [3])
    await evaluate(genericCandidateResponse(profile), profile)
  })

  it('binds a generic plan to the native-canonical request skeleton', async () => {
    const profile = genericCandidateProfile('animal', 9, [8, 1])
    profile.generation_constraints.skeleton_segments = [
      {
        id: 20,
        start: { x_tenths_mm: 1_000, y_tenths_mm: 500 },
        end: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
        thickness_tenths_mm: 10,
      },
      {
        id: 10,
        start: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
        end: { x_tenths_mm: 0, y_tenths_mm: 0 },
        thickness_tenths_mm: 10,
      },
    ]
    const response = genericCandidateResponse(profile)
    response.generated_plans[0]!.skeleton_segments = [
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
    ]

    await evaluate(response, profile)

    response.generated_plans[0]!.skeleton_segments[1]!.end.y_tenths_mm = 501
    await expect(evaluate(response, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('rejects a specialized plan with a forged request skeleton', async () => {
    const profile = expectedTailProfile()
    profile.generation_constraints.skeleton_segments = [{
      id: 10,
      start: { x_tenths_mm: 0, y_tenths_mm: 0 },
      end: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
      thickness_tenths_mm: 10,
    }]
    const response = validResponse()
    response.generated_plans[0]!.skeleton_segments = [{
      ...profile.generation_constraints.skeleton_segments[0]!,
      start: { x_tenths_mm: 0, y_tenths_mm: 0 },
      end: { x_tenths_mm: 999, y_tenths_mm: 0 },
    }]

    await expect(evaluate(response, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('binds the animal four-leg primary kind to asymmetric landmarks', async () => {
    const profile = genericCandidateProfile(
      'animal',
      4,
      [1, 1, 1, 1],
    )
    profile.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'leg', count: 4 },
    ]
    profile.generation_constraints.skeleton_segments.push({
      id: 30,
      start: { x_tenths_mm: 1_000, y_tenths_mm: 500 },
      end: { x_tenths_mm: 0, y_tenths_mm: 500 },
      thickness_tenths_mm: 10,
    })
    const asymmetric = pairedCandidateResponse(
      profile,
      'asymmetric_four_leg_landmark_base',
    )
    await evaluate(asymmetric, profile)

    const forgedSymmetric = pairedCandidateResponse(
      profile,
      'symmetric_four_leg_base',
    )
    await expect(evaluate(forgedSymmetric, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('binds bilateral four-leg and asymmetric wing primary kinds exactly', async () => {
    const bilateralLegs = genericCandidateProfile('animal', 4, [4])
    bilateralLegs.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'leg', count: 4 },
    ]
    bilateralLegs.generation_constraints.protrusions![0]!.symmetry =
      'bilateral'
    await evaluate(
      pairedCandidateResponse(
        bilateralLegs,
        'symmetric_four_leg_base',
      ),
      bilateralLegs,
    )
    const missingFourLegSupport = pairedCandidateResponse(
      bilateralLegs,
      'symmetric_four_leg_base',
    )
    missingFourLegSupport.generated_plans[0]!.instruction_codes = [
      'symmetric_four_leg_base',
    ]
    await expect(evaluate(
      missingFourLegSupport,
      bilateralLegs,
    )).rejects.toThrow('invalid beginner candidate response')
    const oddFourLegSupport = pairedCandidateResponse(
      bilateralLegs,
      'symmetric_four_leg_base',
    )
    oddFourLegSupport.generated_plans[0]!.instruction_codes[1] =
      'bounded_radial_corner_support_v1:added=3:covered=4'
    oddFourLegSupport.generated_plans[0]!.crease_pattern.edges.splice(3, 1)
    oddFourLegSupport.generated_plans[0]!.crease_pattern.vertices.pop()
    await expect(evaluate(
      oddFourLegSupport,
      bilateralLegs,
    )).rejects.toThrow('invalid beginner candidate response')
    await expect(evaluate(
      pairedCandidateResponse(
        bilateralLegs,
        'asymmetric_four_leg_landmark_base',
      ),
      bilateralLegs,
    )).rejects.toThrow('invalid beginner candidate response')

    const asymmetricWings = genericCandidateProfile(
      'animal',
      2,
      [1, 1],
    )
    asymmetricWings.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'wing', count: 2 },
    ]
    await evaluate(
      pairedCandidateResponse(
        asymmetricWings,
        'asymmetric_bird_landmark_base',
      ),
      asymmetricWings,
    )
    await expect(evaluate(
      pairedCandidateResponse(
        asymmetricWings,
        'symmetric_bird_base',
      ),
      asymmetricWings,
    )).rejects.toThrow('invalid beginner candidate response')

    const bilateralWings = genericCandidateProfile('animal', 2, [2])
    bilateralWings.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'wing', count: 2 },
    ]
    bilateralWings.generation_constraints.protrusions![0]!.symmetry =
      'bilateral'
    await evaluate(
      pairedCandidateResponse(
        bilateralWings,
        'symmetric_bird_base',
      ),
      bilateralWings,
    )
    await expect(evaluate(
      pairedCandidateResponse(
        bilateralWings,
        'asymmetric_bird_landmark_base',
      ),
      bilateralWings,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('snapshots asymmetric protrusion tuples before native evaluation settles', async () => {
    const profile = genericCandidateProfile('animal', 2, [1, 1])
    profile.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'wing', count: 2 },
    ]
    const pending = evaluate(
      pairedCandidateResponse(
        profile,
        'asymmetric_bird_landmark_base',
      ),
      profile,
    )
    profile.generation_constraints.protrusions![0]!.symmetry =
      'bilateral'
    profile.generation_constraints.protrusions![0]!
      .direction_milli[0] = 0

    await expect(pending).resolves.toMatchObject({
      generated_plans: [{
        kind: 'asymmetric_bird_landmark_base',
      }],
    })
  })

  it.each([
    'asymmetric_insect_landmark_base',
    'asymmetric_fish_landmark_base',
  ] as const)(
    'binds %s to exact asymmetric physical landmarks',
    async (kind) => {
      const profile = asymmetricSemanticProfile(kind)
      await evaluate(pairedCandidateResponse(profile, kind), profile)

      for (const mutation of [
        'non-singleton',
        'symmetry',
        'local-outline',
      ] as const) {
        const invalidProfile = asymmetricSemanticProfile(kind)
        mutateAsymmetricSemanticProfile(invalidProfile, mutation)
        await expect(evaluate(
          pairedCandidateResponse(invalidProfile, kind),
          invalidProfile,
        )).rejects.toThrow('invalid beginner candidate response')
      }

      const invalidDirection = asymmetricSemanticProfile(kind)
      mutateAsymmetricSemanticProfile(invalidDirection, 'direction')
      nativeInvoke.mockReset()
      await expect(evaluateBeginnerCandidates(
        PROJECT_ID,
        7,
        INSTANCE_ID,
        1,
        CANDIDATE_GENERATION_ID,
        invalidDirection,
      )).rejects.toThrow('invalid expected beginner profile')
      expect(nativeInvoke).not.toHaveBeenCalled()
    },
  )

  it('rejects forged asymmetric semantic ray assignment and digest', async () => {
    const profile = asymmetricSemanticProfile(
      'asymmetric_fish_landmark_base',
    )
    const wrongRay = pairedCandidateResponse(
      profile,
      'asymmetric_fish_landmark_base',
    )
    const wrongRaySemantic = Reflect.get(
      wrongRay.generated_plans[0]!,
      'semantic_landmark_provenance',
    ) as {
      ordered_bindings: Array<{ physical_ray: number }>
    }
    wrongRaySemantic.ordered_bindings[0]!.physical_ray = 1
    await expect(evaluate(wrongRay, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const wrongDigest = pairedCandidateResponse(
      profile,
      'asymmetric_fish_landmark_base',
    )
    const wrongDigestSemantic = Reflect.get(
      wrongDigest.generated_plans[0]!,
      'semantic_landmark_provenance',
    ) as {
      physical_ray_group_sha256: number[][]
    }
    const firstDigest = wrongDigestSemantic.physical_ray_group_sha256[0]!
    firstDigest[0] = firstDigest[0]! ^ 1
    await expect(evaluate(wrongDigest, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('rejects non-native physical assignment, orientation, and fan topology', async () => {
    const wrongAssignment = validResponse()
    wrongAssignment.generated_plans[0]!.crease_pattern
      .edges[0]!.kind = 'mountain'
    await expect(evaluate(wrongAssignment)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const reversed = validResponse()
    const edge = reversed.generated_plans[0]!.crease_pattern.edges[0]!
    ;[edge.start, edge.end] = [edge.end, edge.start]
    await expect(evaluate(reversed)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const completeProfile = completeAnimalProfile()
    const nonStar = completeAnimalCandidateResponse(
      completeProfile,
      1,
    )
    nonStar.generated_plans[0]!.crease_pattern.edges[1]!.start =
      nonStar.generated_plans[0]!.crease_pattern.vertices[1]!.id
    await expect(evaluate(
      nonStar,
      completeProfile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it('requires exact radial support edges for eligible specialized plans', async () => {
    const profile = completeAnimalProfile()
    await evaluate(completeAnimalCandidateResponse(profile), profile)
    await evaluate(completeAnimalCandidateResponse(profile, 1), profile)

    const missingInstruction =
      completeAnimalCandidateResponse(profile)
    missingInstruction.generated_plans[0]!.instruction_codes = [
      'composite_complete_animal_base',
    ]
    await expect(evaluate(missingInstruction, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const missingSupportEdge =
      completeAnimalCandidateResponse(profile)
    missingSupportEdge.generated_plans[0]!.instruction_codes[1] =
      'bounded_radial_corner_support_v1:added=1:covered=4'
    await expect(evaluate(missingSupportEdge, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it.each([
    ['composite_horn_tail_ear_base', 6],
    ['composite_wing_antenna_base', 8],
  ] as const)(
    'strictly admits %s radial support in candidate responses',
    async (kind, basePhysicalEdgeCount) => {
      const profile = specializedRadialProfile(kind)
      await evaluate(specializedRadialCandidateResponse(
        profile,
        kind,
        basePhysicalEdgeCount,
        0,
      ), profile)

      await expect(evaluate(specializedRadialCandidateResponse(
        profile,
        kind,
        basePhysicalEdgeCount,
        null,
      ), profile)).rejects.toThrow('invalid beginner candidate response')
      await expect(evaluate(specializedRadialCandidateResponse(
        profile,
        kind,
        basePhysicalEdgeCount,
        5,
      ), profile)).rejects.toThrow('invalid beginner candidate response')
    },
  )

  it('rejects generic plans whose physical crease count is not endpoint-bound', async () => {
    const profile = genericCandidateProfile('animal', 9, [8, 1])
    const missingEndpoint = genericCandidateResponse(profile)
    missingEndpoint.generated_plans[0]!.crease_pattern.edges.pop()
    missingEndpoint.generated_plans[0]!.crease_pattern.vertices.pop()
    await expect(evaluate(missingEndpoint, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const excessiveSupport = genericCandidateResponse(profile)
    const plan = excessiveSupport.generated_plans[0]!
    for (let index = 0; index < 6; index += 1) {
      const vertexId = indexedUuid(3, index + 1)
      plan.crease_pattern.vertices.push({
        id: vertexId,
        position: { x: index + 1, y: 2 },
      })
      plan.crease_pattern.edges.push({
        id: indexedUuid(4, index + 1),
        start: VERTEX_A,
        end: vertexId,
        kind: 'mountain',
      })
    }
    await expect(evaluate(excessiveSupport, profile)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it.each([1, 4, 12, 32])(
    'preserves a custom direct generic candidate with %i exact endpoints',
    async (endpointCount) => {
      const profile = genericCandidateProfile(
        'custom_object',
        endpointCount,
        physicalCountsForEndpoints(endpointCount),
      )
      await evaluate(genericCandidateResponse(profile), profile)
    },
  )

  it.each([2, 3])(
    'accepts one custom plan for a %i-candidate score request',
    async (requestedCandidateCount) => {
      const profile = genericCandidateProfile(
        'custom_object',
        0,
        [2],
      )
      const response = genericCandidateResponse(profile)
      response.requested_candidate_count = requestedCandidateCount
      while (response.candidates.length < requestedCandidateCount) {
        const rank = response.candidates.length + 1
        response.candidates.push(balancedCandidateScore(
          rank === 2 ? 'shape_focused' : 'foldability_focused',
          rank,
          beginnerExpectedTargetApproximationScoreV1(
            profile,
            'composite_generic_target_base',
          ),
        ))
      }

      const result = await evaluate(
        response,
        profile,
        requestedCandidateCount,
      )
      expect(result.generated_plans).toHaveLength(1)
      expect(result.candidates).toHaveLength(requestedCandidateCount)
    },
  )

  it('preserves all fourteen bounded custom physical bindings', async () => {
    const profile = genericCandidateProfile(
      'custom_object',
      14,
      Array.from({ length: 14 }, () => 1),
    )
    await evaluate(genericCandidateResponse(profile), profile)
  })

  it('preserves a custom feature-empty direct candidate at the endpoint bounds', async () => {
    for (const endpointCount of [1, 32]) {
      const profile = genericCandidateProfile(
        'custom_object',
        0,
        physicalCountsForEndpoints(endpointCount),
      )
      await evaluate(genericCandidateResponse(profile), profile)
    }
  })

  it.each([
    ['animal semantic/physical mismatch', () => (
      genericCandidateProfile('animal', 11, [8, 2])
    )],
    ['animal specialized downgrade', () => (
      genericCandidateProfile('animal', 2, [2])
    )],
    ['custom semantic/physical mismatch', () => (
      genericCandidateProfile('custom_object', 12, [8, 3])
    )],
    ['custom fifteen bindings', () => (
      genericCandidateProfile(
        'custom_object',
        0,
        Array.from({ length: 15 }, () => 1),
      )
    )],
    ['custom thirty-three physical endpoints', () => (
      genericCandidateProfile('custom_object', 0, [8, 8, 8, 8, 1])
    )],
  ] as const)(
    'rejects generic candidate profile %s',
    async (_label, createProfile) => {
      const profile = createProfile()
      await expect(evaluate(
        genericCandidateResponse(profile),
        profile,
      )).rejects.toThrow('invalid beginner candidate response')
    },
  )

  it('rejects a non-custom generic candidate without the exact body pair', async () => {
    const profile = genericCandidateProfile('insect', 9, [8, 1])
    profile.generation_constraints.target_parts =
      profile.generation_constraints.target_parts.filter(
        (part) => part.kind !== 'head',
      )
    await expect(evaluate(
      genericCandidateResponse(profile),
      profile,
    )).rejects.toThrow('invalid beginner candidate response')
  })

  it.each([
    ['empty specialized parts', (response: ReturnType<typeof validResponse>) => {
      response.generated_plans[0]!.target_parts = []
    }],
    ['wrong specialized signature', (response: ReturnType<typeof validResponse>) => {
      response.generated_plans[0]!.target_parts[2] = {
        kind: 'horn',
        count: 1,
      }
    }],
    ['overlarge target record list', (response: ReturnType<typeof validResponse>) => {
      response.generated_plans[0]!.target_parts = Array.from(
        { length: 11 },
        () => ({ kind: 'tail', count: 1 }),
      )
    }],
    ['unknown plan kind', (response: ReturnType<typeof validResponse>) => {
      response.generated_plans[0]!.kind = 'unknown_plan_kind'
      response.plan_assessments[0]!.kind = 'unknown_plan_kind'
    }],
    ['oversized fold plan', (response: ReturnType<typeof validResponse>) => {
      const plan = response.generated_plans[0]!
      plan.kind = 'vertical_book_fold'
      plan.instruction_codes = ['book_fold_vertical']
      plan.crease_pattern.vertices.push({
        id: '66666666-6666-4666-8666-666666666666',
        position: { x: 2, y: 0 },
      })
      plan.crease_pattern.edges.push({
        id: '77777777-7777-4777-8777-777777777777',
        start: VERTEX_B,
        end: '66666666-6666-4666-8666-666666666666',
        kind: 'valley',
      })
      response.plan_assessments[0]!.kind = 'vertical_book_fold'
    }],
  ])('rejects %s', async (_label, mutate) => {
    const response = validResponse()
    mutate(response)

    await expect(evaluate(response)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })

  it('rejects accessors, hostile prototypes, and huge arrays without reading them', async () => {
    const accessorResponse = validResponse()
    let getterCalled = false
    const hostilePart = { kind: 'tail', count: 1 }
    Object.defineProperty(hostilePart, 'count', {
      enumerable: true,
      get() {
        getterCalled = true
        return 1
      },
    })
    accessorResponse.generated_plans[0]!.target_parts[2] =
      hostilePart as never
    await expect(evaluate(accessorResponse)).rejects.toThrow(
      'invalid beginner candidate response',
    )
    expect(getterCalled).toBe(false)

    const prototypeResponse = validResponse()
    Object.setPrototypeOf(
      prototypeResponse.generated_plans[0]!.target_parts,
      null,
    )
    await expect(evaluate(prototypeResponse)).rejects.toThrow(
      'invalid beginner candidate response',
    )

    const hugeResponse = validResponse()
    const hugeVertices: unknown[] = []
    hugeVertices.length = 1_000_000
    hugeResponse.generated_plans[0]!.crease_pattern.vertices =
      hugeVertices as never
    await expect(evaluate(hugeResponse)).rejects.toThrow(
      'invalid beginner candidate response',
    )
  })
})

describe('beginner grid contour witness contract', () => {
  beforeEach(() => nativeInvoke.mockReset())

  it('rejects an invalid expected grid profile before invoking native', async () => {
    const profile = expectedTailProfile()
    profile.foldability_weight = 34

    await expect(evaluateGrid(
      gridResponseWithLocalBindings(),
      profile,
    )).rejects.toThrow('invalid expected beginner profile')
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('requires the canonical grid hash and bounded aggregate refinement work', async () => {
    const forgedHash = gridResponseWithLocalBindings(0)
    forgedHash.grid_hash[0] = forgedHash.grid_hash[0]! ^ 1
    await expect(evaluateGrid(forgedHash)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )

    const excessiveWork = gridResponseWithLocalBindings(0)
    excessiveWork.refinement_iterations = 17
    await expect(evaluateGrid(excessiveWork)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('accepts all fourteen native general local contour records', async () => {
    const { profile, response } =
      genericGridResponseWithLocalBindings()
    const candidateResponse = validResponse()
    const candidatePlan = structuredClone(
      response.candidates[0]!.plan,
    )
    candidatePlan.instruction_codes =
      candidatePlan.instruction_codes.filter((code) =>
        !code.startsWith('bounded_tree_paper_orientation_v1:'))
    candidateResponse.generated_plans = [
      candidatePlan,
    ]
    candidateResponse.plan_assessments = [
      structuredClone(response.candidates[0]!.assessment),
    ]
    candidateResponse.candidates[0]!.target_approximation_score =
      beginnerExpectedTargetApproximationScoreV1(
        profile,
        'composite_generic_target_base',
      )
    await evaluate(candidateResponse, profile)
    const result = await evaluateGrid(response, profile)

    expect(
      result.candidates[0]?.contour_witness.local_bindings,
    ).toHaveLength(14)
  })

  it('rejects a fifteenth or overlapping general local contour block', async () => {
    const excessive = genericGridResponseWithLocalBindings(15)
    await expect(evaluateGrid(
      excessive.response,
      excessive.profile,
    )).rejects.toThrow('invalid beginner parameter grid response')

    const overlappingFixture =
      genericGridResponseWithLocalBindings()
    const overlapping = overlappingFixture.response
    overlapping.candidates[0]!.contour_witness
      .local_bindings[1]!.vertex_start -= 1
    await expect(evaluateGrid(
      overlapping,
      overlappingFixture.profile,
    )).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('requires non-generic plans to carry no generic tree branches', async () => {
    const response = gridResponseWithLocalBindings(0)
    response.candidates[0]!.contour_witness.skeleton_branch_bindings.push({
      segment_id: 0,
      parent_segment_id: null,
      parent_endpoint: null,
      child_endpoint: null,
      generated_feature_ids: [],
    })

    await expect(evaluateGrid(response)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('replays grouped general bindings without preserving discarded local outlines', async () => {
    const response = genericGridResponse(2)
    const profile = expectedProfileForGridResponse(response)
    profile.generation_constraints.protrusions![0]!
      .local_outline_tenths_mm = [
        [0, 0],
        [1, 0],
        [0, 1],
      ]

    const result = await evaluateGrid(response, profile)
    expect(result.candidates[0]?.contour_witness.local_bindings)
      .toEqual([])

    const forged = genericGridResponse(2)
    forged.candidates[0]!.contour_witness.local_bindings.push({
      protrusion_id: 1,
      contour_points: 3,
      generated_face_id: 1,
      vertex_start: 7,
      crease_start: 6,
    })
    await expect(evaluateGrid(forged, profile)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('rejects noncanonical generic support and tree edge order', async () => {
    const supportOrder = genericGridResponse(2)
    const supportPlan = supportOrder.candidates[0]!.plan
    ;[
      supportPlan.crease_pattern.edges[0],
      supportPlan.crease_pattern.edges[4],
    ] = [
      supportPlan.crease_pattern.edges[4]!,
      supportPlan.crease_pattern.edges[0]!,
    ]
    supportOrder.candidates[0]!.assessment
      .expected_candidate_edge_id =
        supportPlan.crease_pattern.edges[0]!.id
    await expect(evaluateGrid(supportOrder)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )

    const treeOrder = genericGridResponse(2)
    const treeEdges = treeOrder.candidates[0]!.plan
      .crease_pattern.edges
    const firstTreeEdge = treeEdges[treeEdges.length - 2]!
    ;[firstTreeEdge.start, firstTreeEdge.end] = [
      firstTreeEdge.end,
      firstTreeEdge.start,
    ]
    await expect(evaluateGrid(treeOrder)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('accepts canonical candidate score and point-ID order', async () => {
    const result = await evaluateGrid(gridResponseWithTwoCandidates())

    expect(result.candidates.map((candidate) => candidate.point.id))
      .toEqual([13, 12])
  })

  it('rejects duplicate point IDs and noncanonical candidate order', async () => {
    const duplicate = gridResponseWithTwoCandidates()
    duplicate.candidates[1]!.point = {
      ...duplicate.candidates[0]!.point,
    }
    synchronizeGridCandidateScalars(
      duplicate.candidates[1]!,
      expectedTailProfile(),
    )
    await expect(evaluateGrid(duplicate)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )

    const scoreOrder = gridResponseWithTwoCandidates()
    scoreOrder.candidates.reverse()
    await expect(evaluateGrid(scoreOrder)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )

    const tieOrder = gridResponseWithTwoCandidates()
    tieOrder.candidates[0]!.point = {
      id: 14,
      scale_percent: 27,
      spacing_percent: 80,
      detail_level: 'standard',
    }
    synchronizeGridCandidateScalars(
      tieOrder.candidates[0]!,
      expectedTailProfile(),
    )
    await expect(evaluateGrid(tieOrder)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('accepts the exact native generic branch tree and unchanged seed metadata', async () => {
    const response = genericGridResponse()

    const result = await evaluateGrid(response)

    expect(
      result.candidates[0]?.contour_witness.skeleton_branch_bindings,
    ).toHaveLength(2)
    expect(result.candidates[0]?.strict_improvements).toBe(0)
    expect(result.candidates[0]?.refinement_iterations).toBe(0)
  })

  it.each([9, 10, 11, 12, 13, 14])(
    'accepts a %i-endpoint semantic general witness without counting body parts',
    async (endpointCount) => {
      const response = genericGridResponse(endpointCount, endpointCount)
      const result = await evaluateGrid(response)
      const candidate = result.candidates[0]!

      expect(candidate.plan.target_parts.reduce(
        (sum, part) => sum + part.count,
        0,
      )).toBe(endpointCount + 2)
      expect(candidate.contour_witness.generic_feature_bindings.reduce(
        (sum, binding) => sum + binding.endpoint_count,
        0,
      )).toBe(endpointCount)
    },
  )

  it('binds every grid primary plan to the expected profile snapshot', async () => {
    const response = genericGridResponse(9, 9)
    const expectedProfile = expectedProfileForGridResponse(response)
    const [head, torso, fin, tail] =
      response.candidates[0]!.plan.target_parts
    response.candidates[0]!.plan.target_parts = [
      head!,
      torso!,
      tail!,
      fin!,
    ]

    await expect(evaluateGrid(
      response,
      expectedProfile,
    )).rejects.toThrow('invalid beginner parameter grid response')
  })

  it.each([13, 15])(
    'rejects a 14-endpoint semantic target with a %i-endpoint witness',
    async (witnessedEndpointCount) => {
      await expect(evaluateGrid(
        genericGridResponse(witnessedEndpointCount, 14),
      )).rejects.toThrow('invalid beginner parameter grid response')
    },
  )

  it.each([2, 4, 11, 12, 13, 14])(
    'accepts a custom feature-empty target with %i witnessed endpoints',
    async (witnessedEndpointCount) => {
      const result = await evaluateGrid(
        genericGridResponse(witnessedEndpointCount),
      )
      expect(result.candidates[0]?.plan.target_parts).toEqual([])
    },
  )

  it.each([1, 15])(
    'rejects a custom feature-empty target with %i witnessed endpoints',
    async (witnessedEndpointCount) => {
      await expect(evaluateGrid(
        genericGridResponse(witnessedEndpointCount),
      )).rejects.toThrow('invalid beginner parameter grid response')
    },
  )

  it('uses physical endpoints only for an expected custom body-only target', async () => {
    const response = genericGridResponse(2)
    response.candidates[0]!.plan.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
    ]
    await expect(evaluateGrid(response)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )

    const customProfile = genericCandidateProfile(
      'custom_object',
      0,
      [2],
    )
    customProfile.generation_constraints.target_parts = [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
    ]
    const admitted = await evaluateGrid(response, customProfile)
    expect(admitted.candidates[0]?.plan.target_parts).toEqual(
      customProfile.generation_constraints.target_parts,
    )
  })

  it('clones the native generic instruction grammar and auxiliary witness', async () => {
    const response = genericGridResponse(13)
    const plan = response.candidates[0]!.plan
    const result = await evaluateGrid(response)
    const admitted = result.candidates[0]!.plan
    plan.instruction_codes[1] =
      'bounded_radial_corner_support_v1:added=0:covered=4'
    plan.crease_pattern.edges[18]!.kind = 'valley'

    expect(admitted.instruction_codes).toEqual([
      'bounded_tree_river_axial_v1:4000000,1000000',
      'bounded_radial_corner_support_v1:added=5:covered=4',
      'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
      'bounded_tree_paper_orientation_v1:horizontal',
    ])
    expect(admitted.crease_pattern.edges[18]?.kind).toBe('auxiliary')
  })

  it('rejects generic radial support counts without matching physical edges', async () => {
    const response = genericGridResponse()
    response.candidates[0]!.plan.instruction_codes = [
      'bounded_tree_river_axial_v1:4000000,1000000',
      'bounded_radial_corner_support_v1:added=5:covered=4',
      'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
      'bounded_tree_paper_orientation_v1:horizontal',
    ]

    await expect(evaluateGrid(response)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it.each([
    'asymmetric_insect_landmark_base',
    'asymmetric_fish_landmark_base',
  ] as const)(
    'binds grid %s to exact asymmetric physical landmarks',
    async (kind) => {
      const profile = asymmetricSemanticProfile(kind)
      await evaluateGrid(pairedGridResponse(profile, kind), profile)

      for (const mutation of [
        'non-singleton',
        'symmetry',
        'local-outline',
      ] as const) {
        const invalidProfile = asymmetricSemanticProfile(kind)
        mutateAsymmetricSemanticProfile(invalidProfile, mutation)
        await expect(evaluateGrid(
          pairedGridResponse(invalidProfile, kind),
          invalidProfile,
        )).rejects.toThrow(
          'invalid beginner parameter grid response',
        )
      }

      const invalidDirection = asymmetricSemanticProfile(kind)
      mutateAsymmetricSemanticProfile(invalidDirection, 'direction')
      nativeInvoke.mockReset()
      await expect(evaluateBeginnerParameterGrid(
        PROJECT_ID,
        7,
        INSTANCE_ID,
        GRID_GENERATION_ID,
        invalidDirection,
      )).rejects.toThrow('invalid expected beginner profile')
      expect(nativeInvoke).not.toHaveBeenCalled()
    },
  )

  it.each([
    ['composite_horn_tail_ear_base', 6],
    ['composite_wing_antenna_base', 8],
  ] as const)(
    'strictly admits %s radial support in grid responses',
    async (kind, basePhysicalEdgeCount) => {
      const profile = specializedRadialProfile(kind)
      await evaluateGrid(specializedRadialGridResponse(
        profile,
        kind,
        basePhysicalEdgeCount,
        0,
      ), profile)

      await expect(evaluateGrid(specializedRadialGridResponse(
        profile,
        kind,
        basePhysicalEdgeCount,
        null,
      ), profile)).rejects.toThrow(
        'invalid beginner parameter grid response',
      )
      await expect(evaluateGrid(specializedRadialGridResponse(
        profile,
        kind,
        basePhysicalEdgeCount,
        5,
      ), profile)).rejects.toThrow(
        'invalid beginner parameter grid response',
      )
    },
  )

  it.each([
    ['wrong parent endpoint', (response: ReturnType<typeof genericGridResponse>) => {
      response.candidates[0]!.contour_witness
        .skeleton_branch_bindings[1]!.parent_endpoint = 'start'
    }],
    ['out-of-range segment id', (response: ReturnType<typeof genericGridResponse>) => {
      response.candidates[0]!.contour_witness
        .skeleton_branch_bindings[1]!.segment_id = 65_536
    }],
    ['wrong feature ownership', (response: ReturnType<typeof genericGridResponse>) => {
      response.candidates[0]!.contour_witness
        .skeleton_branch_bindings[0]!.generated_feature_ids = []
      response.candidates[0]!.contour_witness
        .skeleton_branch_bindings[1]!.generated_feature_ids = [1]
    }],
    ['missing generic feature', (response: ReturnType<typeof genericGridResponse>) => {
      response.candidates[0]!.contour_witness
        .generic_feature_bindings = []
    }],
    ['too many strict improvements', (response: ReturnType<typeof genericGridResponse>) => {
      response.candidates[0]!.strict_improvements = 2
    }],
  ])('rejects generic witness %s', async (_label, mutate) => {
    const response = genericGridResponse()
    mutate(response)

    await expect(evaluateGrid(response)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('rejects hostile generic witness accessors and prototypes without reading them', async () => {
    const response = genericGridResponse()
    let getterCalled = false
    const binding = response.candidates[0]!.contour_witness
      .generic_feature_bindings[0]!
    Object.defineProperty(binding, 'skeleton_segment_id', {
      enumerable: true,
      get() {
        getterCalled = true
        return 10
      },
    })

    await expect(evaluateGrid(response)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
    expect(getterCalled).toBe(false)

    const hostilePrototype = genericGridResponse()
    Object.setPrototypeOf(
      hostilePrototype.candidates[0]!.contour_witness
        .skeleton_branch_bindings,
      null,
    )
    await expect(evaluateGrid(hostilePrototype)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })
})
