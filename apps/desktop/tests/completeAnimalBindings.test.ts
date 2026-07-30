import assert from 'node:assert/strict'
import test from 'node:test'

import {
  resolveCompleteAnimalBindings,
} from '../src/lib/completeAnimalBindings.ts'
import { normalizeBeginnerDesignProfile } from '../src/lib/coreClient.ts'

type Target = ReturnType<typeof target>

function target(
  id: number,
  count: number,
  direction: [number, number, number],
  symmetry: 'none' | 'bilateral',
) {
  return {
    id,
    count,
    length_tenths_mm: 100,
    thickness_tenths_mm: 10,
    position_tenths_mm: [0, 0, 0] as [number, number, number],
    direction_milli: direction,
    symmetry,
    curvature_degrees: 0,
    joint: 'fixed' as const,
    motion_degrees: [0, 0] as [number, number],
    side: 'either' as const,
    priority: 50,
  }
}

const horn = target(1, 1, [0, -1_000, 0], 'none')
const tail = target(2, 1, [1_000, 0, 0], 'none')
const ears = target(3, 2, [1_000, 0, 0], 'bilateral')
const legs = target(4, 4, [0, 1_000, 0], 'bilateral')
const wing = target(5, 2, [1_000, 0, 0], 'bilateral')
const animal = [horn, tail, ears, legs]
const FISH_RAY_GROUP_DIGESTS_V1 = [
  [75, 41, 210, 152, 136, 151, 46, 106, 24, 123, 23, 184, 30, 114, 42, 135, 137, 104, 245, 152, 132, 24, 91, 70, 94, 24, 236, 17, 27, 2, 50, 160],
  [161, 248, 204, 0, 96, 167, 32, 29, 69, 192, 109, 11, 216, 173, 136, 184, 254, 168, 75, 149, 4, 228, 224, 106, 4, 131, 187, 25, 183, 13, 1, 159],
  [202, 241, 97, 235, 226, 126, 156, 158, 161, 24, 8, 56, 7, 121, 174, 191, 34, 49, 180, 97, 195, 114, 200, 217, 150, 23, 163, 150, 142, 77, 176, 173],
  [244, 237, 179, 47, 153, 216, 77, 228, 12, 216, 247, 224, 124, 44, 111, 86, 85, 226, 67, 79, 22, 1, 187, 119, 64, 146, 75, 8, 53, 62, 112, 224],
] as const

function permutations<T>(values: readonly T[]): T[][] {
  if (values.length === 0) return [[]]
  return values.flatMap((value, index) => permutations([
    ...values.slice(0, index),
    ...values.slice(index + 1),
  ]).map((rest) => [value, ...rest]))
}

function profile(protrusions: readonly Target[], hasWing: boolean) {
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
        { kind: 'horn', count: 1 },
        { kind: 'tail', count: 1 },
        { kind: 'ear', count: 2 },
        { kind: 'leg', count: 4 },
        ...(hasWing ? [{ kind: 'wing', count: 2 }] : []),
      ],
      skeleton_segments: [],
      protrusions,
      bulge_targets: [],
      target_asset: null,
      allowed_techniques: ['valley_fold'],
    },
  }
}

const IMAGE_ASSET_ID = '11111111-1111-4111-8111-111111111111'
const MODEL_ASSET_ID = '22222222-2222-4222-8222-222222222222'

function consensusSummary() {
  return {
    schema_version: 1,
    model: 'component_extent_branch_v1',
    source_count: 2,
    excluded_count: 1,
    agreement_score: 82,
    component_subscore: 80,
    extent_subscore: 81,
    branch_subscore: 85,
  }
}

function profileWithRichProvenance() {
  const summary = consensusSummary()
  const topology = Array(32).fill(3)
  const bindings = [
    {
      kind: 'image',
      asset_id: IMAGE_ASSET_ID,
      sha256: Array(32).fill(4),
      quality: 91,
    },
    {
      kind: 'reference_model',
      asset_id: MODEL_ASSET_ID,
      sha256: Array(32).fill(5),
      quality: 93,
    },
  ]
  return {
    ...profile(animal, false),
    reference_consensus_v1: {
      schema_version: 1,
      bindings: bindings.map((binding) => ({
        ...binding,
        sha256: binding.sha256.slice(),
      })),
    },
    generation_provenance: {
      schema_version: 1,
      topology_authority_sha256: Array(32).fill(1),
      fold_path_certificate_sha256: Array(32).fill(2),
      document_authority_sha256: Array(32).fill(7),
      confidence_score: 90,
      confidence_reasons: ['bounded_native_fold_path_v2'],
      explicit_override: false,
      source_asset_fingerprint: 'none',
      semantic_landmark_provenance: {
        schema_version: 1,
        ordered_bindings: [
          { ordinal: 0, role: 'head', physical_ray: 0 },
          { ordinal: 1, role: 'tail', physical_ray: 1 },
          { ordinal: 2, role: 'fin_left', physical_ray: 2 },
          { ordinal: 3, role: 'fin_right', physical_ray: 3 },
        ],
        physical_ray_group_sha256: FISH_RAY_GROUP_DIGESTS_V1.map(
          (digest) => Array.from(digest),
        ),
      },
      reference_consensus: {
        schema_version: 1,
        source_revision: 4,
        bindings,
        excluded_asset_id: MODEL_ASSET_ID,
        pair_digests_sha256: [Array(32).fill(6)],
        summary,
      },
      reference_consensus_summary: { ...summary },
      generic_tree: {
        schema_version: 1,
        target_category: 'custom_object',
        source: 'manual_skeleton',
        asset_content_sha256: Array(32).fill(8),
        tree_topology_sha256: topology,
        normalized_length_ratios: [1_000_000, 4_294_967_295],
        orientation: 'horizontal',
        generator_version: 1,
        authorizes_apply: false,
        instruction_proposal: {
          schema_version: 1,
          topology_sha256: topology.slice(),
          generator_version: 1,
          authorizes_apply: false,
          physical_motion_proof: false,
          steps: [
            {
              canonical_crease_id: 'crease-a',
              tree_depth: 0,
              assignment: 'mountain',
              target_branch: 'root-a',
              fixed_side: 'root',
              caution: 'keep the root fixed',
            },
            {
              canonical_crease_id: 'crease-b',
              tree_depth: 1,
              assignment: 'valley',
              target_branch: 'branch-b',
              fixed_side: 'leaf',
              caution: 'do not overfold',
            },
          ],
        },
      },
    },
  }
}

type RichProfile = ReturnType<typeof profileWithRichProvenance>

function cloneRichProfile(): RichProfile {
  return JSON.parse(JSON.stringify(profileWithRichProvenance())) as RichProfile
}

function aggregateGeneralProfile(count: 9 | 10 | 11 | 12 | 13 | 14) {
  const base = profile([], false)
  return {
    ...base,
    generation_constraints: {
      ...base.generation_constraints,
      target_category: 'custom_object',
      custom_object_display_name: 'Aggregate general target',
      target_parts: [
        { kind: 'fin', count: 8 },
        { kind: 'tail', count: count - 8 },
      ],
      protrusions: Array.from(
        { length: count },
        (_, index) => target(
          index + 1,
          1,
          [index % 2 === 0 ? 1_000 : -1_000, 0, 0],
          'none',
        ),
      ),
    },
  }
}

test('all storage permutations resolve the same four complete-animal roles', () => {
  for (const permutation of permutations(animal)) {
    const bindings = resolveCompleteAnimalBindings(permutation, false)
    assert.deepEqual(bindings?.ordered.map(({ id }) => id), [1, 2, 3, 4])
    assert.ok(normalizeBeginnerDesignProfile(profile(permutation, false)))

    const winged = [...permutation, wing]
    const wingedBindings = resolveCompleteAnimalBindings(winged, true)
    assert.deepEqual(
      wingedBindings?.ordered.map(({ id }) => id),
      [1, 2, 3, 4, 5],
    )
    assert.ok(normalizeBeginnerDesignProfile(profile(winged, true)))
  }
})

test('semantic-only complete-animal profiles remain normalizable before bindings exist', () => {
  assert.ok(normalizeBeginnerDesignProfile(profile([], false)))
  assert.ok(normalizeBeginnerDesignProfile(profile([], true)))
})

test('missing, duplicate, ambiguous, and non-final wing bindings fail closed', () => {
  const ambiguousHorn = target(6, 1, [0, 1_000, 0], 'none')
  for (const invalid of [
    animal.slice(0, 3),
    [horn, tail, ears, { ...legs, id: ears.id }],
    [horn, ambiguousHorn, ears, legs],
  ]) {
    assert.equal(resolveCompleteAnimalBindings(invalid, false), null)
    assert.equal(normalizeBeginnerDesignProfile(profile(invalid, false)), null)
  }

  const nonFinalWing = [wing, horn, tail, ears, legs]
  assert.equal(resolveCompleteAnimalBindings(nonFinalWing, true), null)
  assert.equal(
    normalizeBeginnerDesignProfile(profile(nonFinalWing, true)),
    null,
  )
})

test('generation provenance preserves only an exact document authority digest', () => {
  const withAuthority = profileWithRichProvenance()
  const authority =
    withAuthority.generation_provenance.document_authority_sha256
  const normalized = normalizeBeginnerDesignProfile(withAuthority)
  assert.deepEqual(
    normalized?.generation_provenance?.document_authority_sha256,
    authority,
  )
  assert.notEqual(
    normalized?.generation_provenance?.document_authority_sha256,
    authority,
  )
  assert.deepEqual(
    normalized?.generation_provenance?.semantic_landmark_provenance
      ?.ordered_bindings.map((binding) => binding.role),
    ['head', 'tail', 'fin_left', 'fin_right'],
  )
  assert.equal(
    normalized?.generation_provenance?.generic_tree?.source,
    'manual_skeleton',
  )
  assert.equal(
    normalized?.generation_provenance?.generic_tree
      ?.normalized_length_ratios[1],
    4_294_967_295,
  )
  assert.deepEqual(
    normalized?.generation_provenance?.reference_consensus_summary,
    normalized?.generation_provenance?.reference_consensus?.summary,
  )
  assert.equal(
    normalized?.reference_consensus_v1?.excluded_asset_id,
    undefined,
  )

  for (const forged of [
    authority.slice(1),
    [...authority.slice(0, 31), 256],
    [...authority.slice(0, 31), 1.5],
  ]) {
    assert.equal(normalizeBeginnerDesignProfile({
      ...withAuthority,
      generation_provenance: {
        ...withAuthority.generation_provenance,
        document_authority_sha256: forged,
      },
    }), null)
  }
})

test('rich provenance is deeply snapshotted and frozen', () => {
  const source = profileWithRichProvenance()
  const normalized = normalizeBeginnerDesignProfile(source)
  assert.ok(normalized)
  const provenance = normalized.generation_provenance
  const consensus = provenance?.reference_consensus
  const genericTree = provenance?.generic_tree
  const proposal = genericTree?.instruction_proposal
  const exportedConsensus = normalized.reference_consensus_v1
  const semantic = provenance?.semantic_landmark_provenance
  assert.ok(
    provenance
    && consensus
    && genericTree
    && proposal
    && exportedConsensus
    && semantic,
  )

  assert.ok(Object.isFrozen(normalized))
  assert.ok(Object.isFrozen(provenance))
  assert.ok(Object.isFrozen(provenance.topology_authority_sha256))
  assert.ok(Object.isFrozen(provenance.confidence_reasons))
  assert.ok(Object.isFrozen(consensus.bindings))
  assert.ok(Object.isFrozen(consensus.bindings[0]))
  assert.ok(Object.isFrozen(consensus.bindings[0]?.sha256))
  assert.ok(Object.isFrozen(consensus.pair_digests_sha256))
  assert.ok(Object.isFrozen(consensus.pair_digests_sha256[0]))
  assert.ok(Object.isFrozen(consensus.summary))
  assert.ok(Object.isFrozen(genericTree.normalized_length_ratios))
  assert.ok(Object.isFrozen(proposal.steps))
  assert.ok(Object.isFrozen(proposal.steps[0]))
  assert.ok(Object.isFrozen(semantic.ordered_bindings))
  assert.ok(Object.isFrozen(semantic.ordered_bindings[0]))
  assert.ok(Object.isFrozen(semantic.physical_ray_group_sha256))
  assert.ok(Object.isFrozen(semantic.physical_ray_group_sha256[0]))
  assert.ok(Object.isFrozen(exportedConsensus))
  assert.ok(Object.isFrozen(exportedConsensus.bindings))
  assert.ok(Object.isFrozen(exportedConsensus.bindings[0]))
  assert.ok(Object.isFrozen(exportedConsensus.bindings[0]?.sha256))

  source.generation_provenance.topology_authority_sha256[0] = 99
  source.generation_provenance.confidence_reasons[0] = 'forged'
  source.generation_provenance.reference_consensus.bindings[0]!.sha256[0] = 99
  source.generation_provenance.reference_consensus.summary.agreement_score = 1
  source.generation_provenance.generic_tree.normalized_length_ratios[0] = 2
  source.generation_provenance.generic_tree.instruction_proposal.steps[0]!
    .caution = 'forged'
  source.reference_consensus_v1.bindings[0]!.quality = 1

  assert.equal(provenance.topology_authority_sha256[0], 1)
  assert.equal(provenance.confidence_reasons[0], 'bounded_native_fold_path_v2')
  assert.equal(consensus.bindings[0]?.sha256[0], 4)
  assert.equal(consensus.summary.agreement_score, 82)
  assert.equal(genericTree.normalized_length_ratios[0], 1_000_000)
  assert.equal(proposal.steps[0]?.caution, 'keep the root fixed')
  assert.equal(exportedConsensus.bindings[0]?.quality, 91)
})

test('provenance digest arrays fail closed for signed, sparse, or accessor data', () => {
  const digestMutations: Array<(candidate: RichProfile) => void> = [
    (candidate) => {
      candidate.generation_provenance.topology_authority_sha256[0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.fold_path_certificate_sha256[0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.document_authority_sha256[0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.semantic_landmark_provenance
        .physical_ray_group_sha256[0]![0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus.bindings[0]!
        .sha256[0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus
        .pair_digests_sha256[0]![0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .asset_content_sha256[0] = -1
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.topology_sha256[0] = -1
    },
    (candidate) => {
      candidate.reference_consensus_v1.bindings[0]!.sha256[0] = -1
    },
  ]
  for (const mutate of digestMutations) {
    const candidate = cloneRichProfile()
    mutate(candidate)
    assert.equal(normalizeBeginnerDesignProfile(candidate), null)
  }

  const sparse = cloneRichProfile()
  sparse.generation_provenance.document_authority_sha256 =
    new Array<number>(32)
  assert.equal(normalizeBeginnerDesignProfile(sparse), null)

  const accessor = cloneRichProfile()
  const accessorDigest =
    accessor.generation_provenance.topology_authority_sha256
  let accessorReads = 0
  Object.defineProperty(accessorDigest, '0', {
    configurable: true,
    enumerable: true,
    get() {
      accessorReads += 1
      return 1
    },
  })
  assert.equal(normalizeBeginnerDesignProfile(accessor), null)
  assert.equal(accessorReads, 0)
})

test('provenance collections and optional records require dense exact data', () => {
  const sparseMutations: Array<(candidate: RichProfile) => void> = [
    (candidate) => {
      candidate.generation_provenance.confidence_reasons =
        new Array<string>(1)
    },
    (candidate) => {
      candidate.generation_provenance.semantic_landmark_provenance
        .ordered_bindings = new Array(4)
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus.bindings =
        new Array(2)
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus
        .pair_digests_sha256 = new Array(1)
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .normalized_length_ratios = new Array<number>(1)
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps = new Array(1)
    },
  ]
  for (const mutate of sparseMutations) {
    const candidate = cloneRichProfile()
    mutate(candidate)
    assert.equal(normalizeBeginnerDesignProfile(candidate), null)
  }

  const accessor = cloneRichProfile()
  const bindings =
    accessor.generation_provenance.reference_consensus.bindings.slice()
  let accessorReads = 0
  Object.defineProperty(bindings, '0', {
    configurable: true,
    enumerable: true,
    get() {
      accessorReads += 1
      return accessor.generation_provenance.reference_consensus.bindings[0]
    },
  })
  accessor.generation_provenance.reference_consensus.bindings = bindings
  assert.equal(normalizeBeginnerDesignProfile(accessor), null)
  assert.equal(accessorReads, 0)

  const hostileScalar = cloneRichProfile()
  let coercionReads = 0
  hostileScalar.generation_provenance.reference_consensus.bindings[0]!.kind = {
    toString() {
      coercionReads += 1
      return 'image'
    },
  } as never
  assert.equal(normalizeBeginnerDesignProfile(hostileScalar), null)
  assert.equal(coercionReads, 0)

  const unknownMutations: Array<(candidate: RichProfile) => void> = [
    (candidate) => {
      Object.assign(candidate.generation_provenance, { unexpected: true })
    },
    (candidate) => {
      Object.assign(
        candidate.generation_provenance.semantic_landmark_provenance,
        { unexpected: true },
      )
    },
    (candidate) => {
      Object.assign(
        candidate.generation_provenance.reference_consensus.bindings[0]!,
        { unexpected: true },
      )
    },
    (candidate) => {
      Object.assign(
        candidate.generation_provenance.reference_consensus.summary,
        { unexpected: true },
      )
    },
    (candidate) => {
      Object.assign(
        candidate.generation_provenance.generic_tree,
        { unexpected: true },
      )
    },
    (candidate) => {
      Object.assign(
        candidate.generation_provenance.generic_tree.instruction_proposal,
        { unexpected: true },
      )
    },
    (candidate) => {
      Object.assign(
        candidate.generation_provenance.generic_tree.instruction_proposal
          .steps[0]!,
        { unexpected: true },
      )
    },
    (candidate) => {
      Object.assign(candidate.reference_consensus_v1, { unexpected: true })
    },
  ]
  for (const mutate of unknownMutations) {
    const candidate = cloneRichProfile()
    mutate(candidate)
    assert.equal(normalizeBeginnerDesignProfile(candidate), null)
  }

  const explicitUndefined = cloneRichProfile()
  Object.assign(
    explicitUndefined.generation_provenance.generic_tree,
    { target_category: undefined },
  )
  assert.equal(normalizeBeginnerDesignProfile(explicitUndefined), null)

  const undefinedConsensusExclusion = cloneRichProfile()
  Object.assign(
    undefinedConsensusExclusion.reference_consensus_v1,
    { excluded_asset_id: undefined },
  )
  assert.equal(
    normalizeBeginnerDesignProfile(undefinedConsensusExclusion),
    null,
  )
})

test('generic tree text, integers, ordering, and consensus summaries are bounded', () => {
  const invalidMutations: Array<(candidate: RichProfile) => void> = [
    (candidate) => {
      candidate.generation_provenance.confidence_reasons[0] = 'あ'.repeat(22)
    },
    (candidate) => {
      candidate.generation_provenance.source_asset_fingerprint =
        'x'.repeat(129)
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .normalized_length_ratios[0] = 4_294_967_296
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps[0]!.tree_depth = 256
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps[0]!.canonical_crease_id =
          'あ'.repeat(22)
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps[0]!.target_branch = 'あ'.repeat(33)
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps[0]!.caution = 'あ'.repeat(86)
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps[0]!.caution = '\ud800'
    },
    (candidate) => {
      candidate.generation_provenance.generic_tree
        .instruction_proposal.steps.reverse()
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus.bindings[1]!
        .asset_id = IMAGE_ASSET_ID
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus.summary
        .source_count = 3
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus_summary
        .agreement_score = 81
    },
    (candidate) => {
      candidate.generation_provenance.reference_consensus.excluded_asset_id =
        '33333333-3333-4333-8333-333333333333'
    },
  ]
  for (const mutate of invalidMutations) {
    const candidate = cloneRichProfile()
    mutate(candidate)
    assert.equal(normalizeBeginnerDesignProfile(candidate), null)
  }

  const utf8Ordered = cloneRichProfile()
  const utf8Steps =
    utf8Ordered.generation_provenance.generic_tree.instruction_proposal.steps
  utf8Steps[0]!.tree_depth = 0
  utf8Steps[0]!.canonical_crease_id = '\ue000'
  utf8Steps[1]!.tree_depth = 0
  utf8Steps[1]!.canonical_crease_id = '\u{10000}'
  assert.ok(normalizeBeginnerDesignProfile(utf8Ordered))
})

test('general target aggregates admit nine through fourteen without widening records', () => {
  for (const count of [9, 10, 11, 12, 13, 14] as const) {
    const normalized = normalizeBeginnerDesignProfile(
      aggregateGeneralProfile(count),
    )
    assert.ok(normalized)
    assert.equal(
      normalized.generation_constraints.target_parts.reduce(
        (sum, part) => sum + part.count,
        0,
      ),
      count,
    )
    assert.equal(
      normalized.generation_constraints.protrusions?.reduce(
        (sum, protrusion) => sum + protrusion.count,
        0,
      ),
      count,
    )
    assert.deepEqual(normalizeBeginnerDesignProfile(normalized), normalized)
  }

  const oversizedPartRecord = aggregateGeneralProfile(9)
  oversizedPartRecord.generation_constraints.target_parts = [
    { kind: 'fin', count: 9 },
  ]
  assert.equal(
    normalizeBeginnerDesignProfile(oversizedPartRecord),
    null,
  )

  const oversizedProtrusionRecord = aggregateGeneralProfile(9)
  oversizedProtrusionRecord.generation_constraints.protrusions = [
    target(1, 9, [1_000, 0, 0], 'none'),
  ]
  assert.equal(
    normalizeBeginnerDesignProfile(oversizedProtrusionRecord),
    null,
  )
})
