import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  beginnerTargetPartRecordCountIsAdmissibleV1,
  beginnerGeneratedPlanInstructionsAreCanonicalV1,
  beginnerGeneratedPlanSizeIsAdmissibleV1,
  beginnerGeneratedPlanTargetPartsAreCompatibleV1,
  beginnerGenericFeatureBindingIdentityIsCanonicalV1,
  MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
  MAX_BEGINNER_GENERIC_PLAN_EDGES_V1,
  MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1,
  MAX_BEGINNER_SPECIALIZED_PLAN_EDGES_V1,
  MAX_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1,
  MAX_BEGINNER_TARGET_PART_RECORDS_V1,
  normalizeBeginnerGenerationConstraints,
} from '../src/lib/coreClient.ts'
import {
  selectBeginnerNonFormProfileFieldsForSubmitV1,
  selectBeginnerTargetPartsForProfileV1,
} from '../src/lib/useBeginnerProfileWorkflow.ts'
import {
  beginnerSemanticPhysicalProfileIsAdmissibleV1,
  resolveBeginnerProtrusionKindsV1,
} from '../src/lib/beginnerProtrusionKinds.ts'

const segment = (
  id: number,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
) => ({
  id,
  start: { x_tenths_mm: startX, y_tenths_mm: startY },
  end: { x_tenths_mm: endX, y_tenths_mm: endY },
  thickness_tenths_mm: 10,
})

const canonicalTree = [
  segment(10, 0, 0, 1_000, 0),
  segment(20, 1_000, 0, 1_000, 500),
]

const canonicalCandidateInstructions = [
  'bounded_tree_river_axial_v1:4000000,1000000',
  'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
]
const radialCornerSupport =
  'bounded_radial_corner_support_v1:added=4:covered=4'
const radialCornerParitySupport =
  'bounded_radial_corner_support_v1:added=5:covered=4'

test('generic candidate and grid instructions use exact context-specific grammar', () => {
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    canonicalTree,
    'candidate',
  ), true)
  for (const orientation of ['horizontal', 'vertical']) {
    assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
      'composite_generic_target_base',
      [
        ...canonicalCandidateInstructions,
        `bounded_tree_paper_orientation_v1:${orientation}`,
      ],
      canonicalTree,
      'grid',
    ), true)
  }
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    [
      canonicalCandidateInstructions[0]!,
      radialCornerParitySupport,
      canonicalCandidateInstructions[1]!,
    ],
    canonicalTree,
    'candidate',
  ), true)
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    [
      canonicalCandidateInstructions[0]!,
      radialCornerSupport,
      canonicalCandidateInstructions[1]!,
      'bounded_tree_paper_orientation_v1:horizontal',
    ],
    canonicalTree,
    'grid',
  ), true)
  const rejected = [
    [...canonicalCandidateInstructions].reverse(),
    [
      'bounded_tree_river_axial_v1:04000000,1000000',
      canonicalCandidateInstructions[1]!,
    ],
    [
      canonicalCandidateInstructions[0]!,
      'bounded_tree_branch_topology_v1:nodes=3:leaves=3:bars=2',
    ],
    [...canonicalCandidateInstructions, 'bounded_tree_paper_orientation_v1:diagonal'],
  ]
  for (const instructions of rejected) {
    assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
      'composite_generic_target_base',
      instructions,
      canonicalTree,
      'candidate',
    ), false)
  }
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    canonicalTree,
    'grid',
  ), false)
  for (const instructions of [
    [
      ...canonicalCandidateInstructions,
      'bounded_tree_paper_orientation_v1:horizontal',
      'bounded_tree_paper_orientation_v1:vertical',
    ],
    [
      canonicalCandidateInstructions[0]!,
      'bounded_tree_paper_orientation_v1:horizontal',
      canonicalCandidateInstructions[1]!,
    ],
    [
      ...canonicalCandidateInstructions,
      'bounded_tree_paper_orientation_v1:Horizontal',
    ],
  ]) {
    assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
      'composite_generic_target_base',
      instructions,
      canonicalTree,
      'grid',
    ), false)
  }
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'symmetric_four_leg_base',
    ['symmetric_wing_base'],
    [],
    'candidate',
  ), false)
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'vertical_book_fold',
    ['book_fold_vertical'],
    [],
    'candidate',
  ), true)
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'symmetric_six_leg_base',
    ['symmetric_six_leg_base', radialCornerSupport],
    [],
    'candidate',
  ), true)
  for (const instructions of [
    [
      'symmetric_six_leg_base',
      'bounded_radial_corner_support_v1:added=5:covered=4',
    ],
    [
      canonicalCandidateInstructions[0]!,
      canonicalCandidateInstructions[1]!,
      radialCornerSupport,
    ],
  ]) {
    assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
      instructions[0] === 'symmetric_four_leg_base'
        ? 'symmetric_four_leg_base'
        : instructions[0] === 'symmetric_six_leg_base'
          ? 'symmetric_six_leg_base'
          : 'composite_generic_target_base',
      instructions,
      instructions[0]?.startsWith('bounded_tree_')
        ? canonicalTree
        : [],
      'candidate',
    ), false)
  }
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'symmetric_four_leg_base',
    ['symmetric_four_leg_base', radialCornerSupport],
    [],
    'candidate',
  ), true)
})

test('generic instruction trees reject duplicate and noncanonical segment IDs', () => {
  const duplicateIds = [
    canonicalTree[0]!,
    { ...canonicalTree[1]!, id: canonicalTree[0]!.id },
  ]
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    duplicateIds,
    'candidate',
  ), false)
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    [...canonicalTree].reverse(),
    'candidate',
  ), false)
  const reverseEndpoint = (value: (typeof canonicalTree)[number]) => ({
    ...value,
    start: value.end,
    end: value.start,
  })
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    [reverseEndpoint(canonicalTree[0]!), canonicalTree[1]!],
    'candidate',
  ), false)
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    canonicalTree.map(reverseEndpoint),
    'candidate',
  ), false)
})

test('generic instruction parsing fails closed before bigint conversion', () => {
  const hostileCoordinates: unknown[] = [
    0.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.NEGATIVE_INFINITY,
    Number.MAX_VALUE,
    1n,
    Symbol('coordinate'),
  ]
  for (const coordinate of hostileCoordinates) {
    const tree = canonicalTree.map((item, index) => index === 0
      ? {
          ...item,
          start: {
            ...item.start,
            x_tenths_mm: coordinate,
          },
        }
      : item)
    assert.doesNotThrow(() => {
      assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
        'composite_generic_target_base',
        canonicalCandidateInstructions,
        tree as never,
        'candidate',
      ), false)
    })
  }

  let getterCalled = false
  const accessorSegment = { ...canonicalTree[0] }
  Object.defineProperty(accessorSegment, 'id', {
    enumerable: true,
    get() {
      getterCalled = true
      return 10
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
      'composite_generic_target_base',
      canonicalCandidateInstructions,
      [accessorSegment, canonicalTree[1]] as never,
      'candidate',
    ), false)
  })
  assert.equal(getterCalled, false)

  const huge: unknown[] = []
  huge.length = 1_000_000
  assert.equal(beginnerGeneratedPlanInstructionsAreCanonicalV1(
    'composite_generic_target_base',
    canonicalCandidateInstructions,
    huge as never,
    'candidate',
  ), false)
})

test('generic plan size admits the legal maximum and closes adjacent overflow', () => {
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'composite_generic_target_base',
    MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1 - 1,
    MAX_BEGINNER_GENERIC_PLAN_EDGES_V1 - 1,
  ), true)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'composite_generic_target_base',
    MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1,
    MAX_BEGINNER_GENERIC_PLAN_EDGES_V1,
  ), true)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'composite_generic_target_base',
    MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1 + 1,
    MAX_BEGINNER_GENERIC_PLAN_EDGES_V1,
  ), false)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'composite_generic_target_base',
    MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1,
    MAX_BEGINNER_GENERIC_PLAN_EDGES_V1 + 1,
  ), false)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'symmetric_four_leg_base',
    163,
    4,
  ), false)
})

test('generic plan cap admits every bounded native append stage together', () => {
  const endpointBase = { vertices: 1 + 32, edges: 32 }
  const bodyOutline = { vertices: 16, edges: 16 }
  const localOutlines = Array.from({ length: 14 }, () => ({
    vertices: 8,
    edges: 8,
  }))
  const radialCornerSupports = { vertices: 5, edges: 5 }
  const treeWitness = { vertices: 17, edges: 16 }
  const fixture = [
    endpointBase,
    bodyOutline,
    ...localOutlines,
    radialCornerSupports,
    treeWitness,
  ]
  const vertices = fixture.reduce(
    (sum, stage) => sum + stage.vertices,
    0,
  )
  const edges = fixture.reduce(
    (sum, stage) => sum + stage.edges,
    0,
  )

  assert.equal(vertices, 183)
  assert.equal(edges, 181)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'composite_generic_target_base',
    vertices,
    edges,
  ), true)
})

test('specialized plan cap includes body and local outlines after its base', () => {
  const symmetricWingBase = { vertices: 5, edges: 4 }
  const bodyOutline = { vertices: 16, edges: 16 }
  const liveWingOutline = { vertices: 8, edges: 8 }
  const vertices = symmetricWingBase.vertices
    + bodyOutline.vertices
    + liveWingOutline.vertices
  const edges = symmetricWingBase.edges
    + bodyOutline.edges
    + liveWingOutline.edges

  assert.equal(vertices, 29)
  assert.equal(edges, 28)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'symmetric_wing_base',
    vertices,
    edges,
  ), true)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'symmetric_four_leg_base',
    MAX_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1,
    MAX_BEGINNER_SPECIALIZED_PLAN_EDGES_V1,
  ), true)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'symmetric_four_leg_base',
    MAX_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1 + 1,
    MAX_BEGINNER_SPECIALIZED_PLAN_EDGES_V1 + 1,
  ), false)
  assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
    'symmetric_four_leg_base',
    5,
    5,
  ), false)
  for (const foldKind of [
    'vertical_book_fold',
    'horizontal_book_fold',
    'diagonal_fold',
  ] as const) {
    assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
      foldKind,
      2,
      1,
    ), true)
    assert.equal(beginnerGeneratedPlanSizeIsAdmissibleV1(
      foldKind,
      3,
      2,
    ), false)
  }
})

test('generic witness separates dense feature ordinals from u16 source IDs', () => {
  const skeleton = [
    segment(0, 0, 0, 10, 0),
    segment(65_535, 10, 0, 20, 0),
  ]
  const bindings = [0, 255, 256, 65_535].map((protrusionId, index) => ({
    protrusion_id: protrusionId,
    generated_feature_id: index + 1,
    endpoint_count: [1, 3, 6, 8][index]!,
    skeleton_segment_id: index % 2 === 0 ? 0 : 65_535,
  }))
  const branches = [
    {
      segment_id: 0,
      parent_segment_id: null,
      parent_endpoint: null,
      child_endpoint: null,
      generated_feature_ids: [1, 3],
    },
    {
      segment_id: 65_535,
      parent_segment_id: 0,
      parent_endpoint: 'end' as const,
      child_endpoint: 'start' as const,
      generated_feature_ids: [2, 4],
    },
  ]
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    bindings,
    branches,
    skeleton,
  ), true)

  const tamper = (
    update: (copy: typeof bindings) => void,
  ) => {
    const copy = bindings.map((binding) => ({ ...binding }))
    update(copy)
    return beginnerGenericFeatureBindingIdentityIsCanonicalV1(
      copy,
      branches,
      skeleton,
    )
  }
  assert.equal(tamper((copy) => { copy[0]!.protrusion_id = -1 }), false)
  assert.equal(tamper((copy) => { copy[3]!.protrusion_id = 65_536 }), false)
  assert.equal(tamper((copy) => { copy[2]!.protrusion_id = 255 }), false)
  assert.equal(tamper((copy) => { copy[2]!.generated_feature_id = 4 }), false)
  assert.equal(tamper((copy) => { copy[1]!.endpoint_count = 0 }), false)
  assert.equal(tamper((copy) => { copy[1]!.endpoint_count = 9 }), false)
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    bindings,
    [
      {
        segment_id: 0,
        parent_segment_id: null,
        parent_endpoint: null,
        child_endpoint: null,
        generated_feature_ids: [1, 3],
      },
      {
        segment_id: 65_535,
        parent_segment_id: 0,
        parent_endpoint: 'end',
        child_endpoint: 'start',
        generated_feature_ids: [2],
      },
    ],
    skeleton,
  ), false)

  const maximumBindings = Array.from(
    { length: MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1 },
    (_, index) => ({
      protrusion_id: index,
      generated_feature_id: index + 1,
      endpoint_count: 1,
      skeleton_segment_id: 0,
    }),
  )
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    maximumBindings,
    [{
      segment_id: 0,
      parent_segment_id: null,
      parent_endpoint: null,
      child_endpoint: null,
      generated_feature_ids: maximumBindings.map(
        (binding) => binding.generated_feature_id,
      ),
    }],
    [skeleton[0]!],
  ), true)
  const overflowBindings = [
    ...maximumBindings,
    {
      protrusion_id: MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
      generated_feature_id: MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1 + 1,
      endpoint_count: 1,
      skeleton_segment_id: 0,
    },
  ]
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    overflowBindings,
    [{
      segment_id: 0,
      parent_segment_id: null,
      parent_endpoint: null,
      child_endpoint: null,
      generated_feature_ids: overflowBindings.map(
        (binding) => binding.generated_feature_id,
      ),
    }],
    [skeleton[0]!],
  ), false)

  const wrongParent = branches.map((branch) => ({ ...branch }))
  wrongParent[1]!.parent_endpoint = 'start'
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    bindings,
    wrongParent,
    skeleton,
  ), false)
  const wrongSegment = branches.map((branch) => ({ ...branch }))
  wrongSegment[1]!.segment_id = 65_536
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    bindings,
    wrongSegment,
    skeleton,
  ), false)
  const overlappingAssignments = branches.map((branch) => ({
    ...branch,
    generated_feature_ids: [...branch.generated_feature_ids],
  }))
  overlappingAssignments[0]!.generated_feature_ids = [1, 2, 3]
  overlappingAssignments[1]!.generated_feature_ids = [2, 4]
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    bindings,
    overlappingAssignments,
    skeleton,
  ), false)

  let getterCalled = false
  const hostileBinding = { ...bindings[0] }
  Object.defineProperty(hostileBinding, 'protrusion_id', {
    enumerable: true,
    get() {
      getterCalled = true
      return 0
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
      [hostileBinding, ...bindings.slice(1)] as never,
      branches,
      skeleton,
    ), false)
  })
  assert.equal(getterCalled, false)
})

test('generated plan kinds require their exact native target signature', () => {
  const base = [
    { kind: 'head', count: 1 },
    { kind: 'torso', count: 1 },
  ] as const
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'center_axis_tail_base',
    [...base, { kind: 'tail', count: 1 }],
  ), true)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'center_axis_tail_base',
    [...base, { kind: 'horn', count: 1 }],
  ), false)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'center_axis_tail_base',
    [],
  ), false)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'composite_generic_target_base',
    [],
  ), true)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'vertical_book_fold',
    [...base, { kind: 'tail', count: 1 }],
  ), true)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'vertical_book_fold',
    base,
  ), false)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'composite_generic_target_base',
    Array.from({ length: 11 }, () => ({ kind: 'tail', count: 1 })),
  ), false)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'composite_generic_target_base',
    [{ kind: 'tail', count: 9 }],
  ), false)
  assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
    'composite_generic_target_base',
    [
      { kind: 'tail', count: 1 },
      { kind: 'tail', count: 1 },
    ],
  ), false)

  let getterCalled = false
  const hostilePart = { kind: 'tail', count: 1 }
  Object.defineProperty(hostilePart, 'count', {
    enumerable: true,
    get() {
      getterCalled = true
      return 1
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(beginnerGeneratedPlanTargetPartsAreCompatibleV1(
      'composite_generic_target_base',
      [hostilePart],
    ), false)
  })
  assert.equal(getterCalled, false)
})

function constraintsWithTargetPartCount(count: number) {
  const kinds = [
    'head', 'torso', 'leg', 'horn', 'ear',
    'wing', 'fin', 'antenna', 'tail',
  ] as const
  return {
    schema_version: 1,
    maximum_steps: 60,
    detail_level: 'simple',
    target_category: 'custom_object',
    target_parts: Array.from({ length: count }, (_, index) => ({
      kind: kinds[index % kinds.length]!,
      count: 1,
    })),
    skeleton_segments: [],
    target_asset: null,
    allowed_techniques: ['valley_fold'],
  } as const
}

test('target-part record limit stays synchronized with the native limit of ten', () => {
  assert.equal(MAX_BEGINNER_TARGET_PART_RECORDS_V1, 10)
  for (const count of [8, 9, 10]) {
    assert.equal(beginnerTargetPartRecordCountIsAdmissibleV1(
      Array.from({ length: count }),
    ), true)
    assert.notEqual(
      normalizeBeginnerGenerationConstraints(
        constraintsWithTargetPartCount(count),
      ),
      null,
    )
  }
  assert.equal(
    beginnerTargetPartRecordCountIsAdmissibleV1(
      Array.from({ length: 11 }),
    ),
    false,
  )
  assert.equal(
    normalizeBeginnerGenerationConstraints(constraintsWithTargetPartCount(11)),
    null,
  )
})

test('profile authoring compacts singleton protrusions by semantic kind', () => {
  const formTargetParts = [
    { kind: 'head' as const, count: 1 },
    { kind: 'torso' as const, count: 1 },
  ]
  const makeProtrusions = (count: number) => Array.from(
    { length: count },
    (_, id) => ({
      id,
      count: 1,
      length_tenths_mm: 100,
      thickness_tenths_mm: 10,
      position_tenths_mm: [id, 0, 0] as [number, number, number],
      direction_milli: [1_000, 0, 0] as [number, number, number],
      symmetry: 'none' as const,
      curvature_degrees: 0,
      joint: 'fixed' as const,
      motion_degrees: [0, 0] as [number, number],
      side: 'either' as const,
      priority: 50,
    }),
  )
  const fiveLegs = makeProtrusions(5)
  for (const count of [2, 3, 5, 6, 7, 8]) {
    const legs = makeProtrusions(count)
    assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
      formTargetParts,
      legs,
      legs.map(() => 'leg'),
      'animal',
    ), [
      ...formTargetParts,
      { kind: 'leg', count },
    ])
  }
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    fiveLegs,
    fiveLegs.map(() => 'leg'),
    'animal',
  ), [
    ...formTargetParts,
    { kind: 'leg', count: 5 },
  ])

  const mixed = makeProtrusions(5)
  const compact = selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    mixed,
    ['leg', 'leg', 'wing', 'tail', 'tail'],
    'animal',
  )
  assert.deepEqual(compact, [
    ...formTargetParts,
    { kind: 'leg', count: 2 },
    { kind: 'wing', count: 1 },
    { kind: 'tail', count: 2 },
  ])
  assert.equal(selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    mixed,
    ['tail', 'leg', 'tail', 'wing', 'leg'],
    'animal',
  ), null)
  assert.equal(
    new Set(compact?.map((part) => part.kind)).size,
    compact?.length,
  )

  const semanticOnlyRecognition = [
    { kind: 'head' as const, count: 1 },
    { kind: 'torso' as const, count: 1 },
    { kind: 'fin' as const, count: 8 },
    { kind: 'antenna' as const, count: 2 },
    { kind: 'tail' as const, count: 1 },
  ]
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    semanticOnlyRecognition,
    [],
    [],
    'insect',
  ), semanticOnlyRecognition)

  const sourceIds = [0, 255, 256, 65_535]
  const physicalSingletons = makeProtrusions(sourceIds.length).map(
    (protrusion, index) => ({ ...protrusion, id: sourceIds[index]! }),
  )
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    physicalSingletons,
    physicalSingletons.map(() => 'leg'),
    'animal',
  ), [
    ...formTargetParts,
    { kind: 'leg', count: 4 },
  ])
  assert.equal(beginnerGenericFeatureBindingIdentityIsCanonicalV1(
    sourceIds.map((protrusionId, index) => ({
      protrusion_id: protrusionId,
      generated_feature_id: index + 1,
      endpoint_count: [1, 3, 6, 8][index]!,
      skeleton_segment_id: canonicalTree[index % 2]!.id,
    })),
    [
      {
        segment_id: 10,
        parent_segment_id: null,
        parent_endpoint: null,
        child_endpoint: null,
        generated_feature_ids: [1, 3],
      },
      {
        segment_id: 20,
        parent_segment_id: 10,
        parent_endpoint: 'end',
        child_endpoint: 'start',
        generated_feature_ids: [2, 4],
      },
    ],
    canonicalTree,
  ), true)
})

test('profile hydration restores one semantic kind per physical protrusion', () => {
  const makeProtrusion = (id: number, count: number) => ({
    id,
    count,
    length_tenths_mm: 100,
    thickness_tenths_mm: 10,
    position_tenths_mm: [id, 0, 0] as [number, number, number],
    direction_milli: [1_000, 0, 0] as [number, number, number],
    symmetry: count === 1 ? 'none' as const : 'radial' as const,
    curvature_degrees: 0,
    joint: 'fixed' as const,
    motion_degrees: [0, 0] as [number, number],
    side: 'either' as const,
    priority: 50,
  })
  const nonCanonicalParts = [
    { kind: 'torso' as const, count: 1 },
    { kind: 'head' as const, count: 1 },
    { kind: 'tail' as const, count: 3 },
    { kind: 'fin' as const, count: 8 },
  ]
  const singletons = Array.from(
    { length: 11 },
    (_, id) => makeProtrusion(id, 1),
  )
  assert.deepEqual(
    resolveBeginnerProtrusionKindsV1(
      nonCanonicalParts,
      singletons,
      { targetCategory: 'animal', allowOrderedGeneric: true },
    ),
    [
      'tail', 'tail', 'tail',
      'fin', 'fin', 'fin', 'fin', 'fin', 'fin', 'fin', 'fin',
    ],
  )
  assert.deepEqual(resolveBeginnerProtrusionKindsV1(
    nonCanonicalParts,
    [makeProtrusion(1, 3), makeProtrusion(2, 8)],
    { targetCategory: 'animal', allowOrderedGeneric: true },
  ), ['tail', 'fin'])
  assert.deepEqual(resolveBeginnerProtrusionKindsV1(
    [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'leg', count: 6 },
    ],
    [
      { ...makeProtrusion(1, 2), symmetry: 'bilateral' as const },
      { ...makeProtrusion(2, 2), symmetry: 'bilateral' as const },
      { ...makeProtrusion(3, 2), symmetry: 'bilateral' as const },
    ],
    { targetCategory: 'insect' },
  ), ['leg', 'leg', 'leg'])
  assert.equal(resolveBeginnerProtrusionKindsV1(
    nonCanonicalParts,
    [makeProtrusion(1, 8), makeProtrusion(2, 3)],
    { targetCategory: 'animal', allowOrderedGeneric: true },
  ), null)
  assert.equal(resolveBeginnerProtrusionKindsV1(
    [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
      { kind: 'tail', count: 1 },
      { kind: 'fin', count: 1 },
    ],
    [makeProtrusion(1, 2)],
    { targetCategory: 'animal', allowOrderedGeneric: true },
  ), null)

  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
    ],
    singletons,
    [
      'tail', 'tail', 'tail',
      'fin', 'fin', 'fin', 'fin', 'fin', 'fin', 'fin', 'fin',
    ],
    'animal',
    nonCanonicalParts,
    singletons,
  ), nonCanonicalParts)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
    ],
    singletons.slice(0, 10),
    [
      'tail', 'tail',
      'fin', 'fin', 'fin', 'fin', 'fin', 'fin', 'fin', 'fin',
    ],
    'animal',
    nonCanonicalParts,
    singletons,
  ), [
    { kind: 'head', count: 1 },
    { kind: 'torso', count: 1 },
    { kind: 'tail', count: 2 },
    { kind: 'fin', count: 8 },
  ])
})

test('profile hydration separates specialized roles, generic order, and direct custom data', () => {
  const target = (
    id: number,
    count: number,
    symmetry: 'none' | 'bilateral' | 'radial',
    direction: [number, number, number],
    priority = 50,
    centerY = 0,
  ) => ({
    id,
    count,
    length_tenths_mm: 100,
    thickness_tenths_mm: 10,
    position_tenths_mm: [0, centerY, 0] as [number, number, number],
    direction_milli: direction,
    symmetry,
    curvature_degrees: 0,
    joint: 'fixed' as const,
    motion_degrees: [0, 0] as [number, number],
    side: 'either' as const,
    priority,
  })

  const tailEarParts = [
    { kind: 'torso' as const, count: 1 },
    { kind: 'ear' as const, count: 2 },
    { kind: 'head' as const, count: 1 },
    { kind: 'tail' as const, count: 1 },
  ]
  const tailEarTargets = [
    target(7, 1, 'none', [1_000, 0, 0]),
    target(2, 2, 'bilateral', [1_000, 0, 0]),
  ]
  assert.deepEqual(resolveBeginnerProtrusionKindsV1(
    tailEarParts,
    tailEarTargets,
    { targetCategory: 'animal' },
  ), ['tail', 'ear'])

  const completeAnimalParts = [
    { kind: 'tail' as const, count: 1 },
    { kind: 'head' as const, count: 1 },
    { kind: 'leg' as const, count: 4 },
    { kind: 'torso' as const, count: 1 },
    { kind: 'ear' as const, count: 2 },
    { kind: 'horn' as const, count: 1 },
  ]
  const completeAnimalTargets = [
    target(40, 4, 'bilateral', [0, 1_000, 0]),
    target(10, 1, 'none', [0, -1_000, 0]),
    target(30, 2, 'bilateral', [1_000, 0, 0]),
    target(20, 1, 'none', [1_000, 0, 0]),
  ]
  assert.deepEqual(resolveBeginnerProtrusionKindsV1(
    completeAnimalParts,
    completeAnimalTargets,
    { targetCategory: 'animal' },
  ), ['leg', 'horn', 'ear', 'tail'])
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    completeAnimalParts,
    completeAnimalTargets,
    completeAnimalTargets.map(() => null),
    'animal',
  ), completeAnimalParts)

  const body = [
    { kind: 'head' as const, count: 1 },
    { kind: 'torso' as const, count: 1 },
  ]
  const animalSpecializedFeatures = [
    [{ kind: 'leg' as const, count: 4 }],
    [{ kind: 'wing' as const, count: 2 }],
    [
      { kind: 'tail' as const, count: 1 },
      { kind: 'fin' as const, count: 2 },
    ],
    [{ kind: 'fin' as const, count: 2 }],
    [{ kind: 'ear' as const, count: 2 }],
    [{ kind: 'horn' as const, count: 2 }],
    [{ kind: 'tail' as const, count: 1 }],
    [{ kind: 'horn' as const, count: 1 }],
    [
      { kind: 'tail' as const, count: 1 },
      { kind: 'ear' as const, count: 2 },
    ],
    [
      { kind: 'horn' as const, count: 1 },
      { kind: 'ear' as const, count: 2 },
    ],
    [
      { kind: 'horn' as const, count: 1 },
      { kind: 'tail' as const, count: 1 },
    ],
    [
      { kind: 'horn' as const, count: 1 },
      { kind: 'tail' as const, count: 1 },
      { kind: 'ear' as const, count: 2 },
    ],
    [
      { kind: 'horn' as const, count: 1 },
      { kind: 'tail' as const, count: 1 },
      { kind: 'ear' as const, count: 2 },
      { kind: 'leg' as const, count: 4 },
    ],
    [
      { kind: 'horn' as const, count: 1 },
      { kind: 'tail' as const, count: 1 },
      { kind: 'ear' as const, count: 2 },
      { kind: 'leg' as const, count: 4 },
      { kind: 'wing' as const, count: 2 },
    ],
  ]
  for (const features of animalSpecializedFeatures) {
    const parts = [...body, ...features]
    assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
      parts,
      [],
      [],
      'animal',
    ), parts)
  }

  const insectSpecializedFeatures = [
    [
      { kind: 'tail' as const, count: 1 },
      { kind: 'wing' as const, count: 2 },
      { kind: 'leg' as const, count: 6 },
    ],
    [{ kind: 'wing' as const, count: 2 }],
    [{ kind: 'wing' as const, count: 4 }],
    [{ kind: 'antenna' as const, count: 2 }],
    [{ kind: 'leg' as const, count: 2 }],
    [{ kind: 'leg' as const, count: 6 }],
    [{ kind: 'antenna' as const, count: 1 }],
    [
      { kind: 'wing' as const, count: 2 },
      { kind: 'antenna' as const, count: 2 },
    ],
    [
      { kind: 'wing' as const, count: 2 },
      { kind: 'antenna' as const, count: 2 },
      { kind: 'leg' as const, count: 6 },
    ],
  ]
  for (const features of insectSpecializedFeatures) {
    const parts = [...body, ...features]
    assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
      parts,
      [],
      [],
      'insect',
    ), parts)
  }

  const completeInsectParts = [
    { kind: 'antenna' as const, count: 2 },
    { kind: 'head' as const, count: 1 },
    { kind: 'leg' as const, count: 6 },
    { kind: 'torso' as const, count: 1 },
    { kind: 'wing' as const, count: 2 },
  ]
  const completeInsectTargets = [
    target(5, 2, 'bilateral', [1_000, 0, 0], 50, 0),
    target(2, 2, 'bilateral', [0, -1_000, 0], 60),
    target(1, 2, 'bilateral', [1_000, 0, 0], 60),
    target(6, 2, 'bilateral', [1_000, 0, 0], 50, 250),
    target(4, 2, 'bilateral', [1_000, 0, 0], 50, -250),
  ]
  assert.deepEqual(resolveBeginnerProtrusionKindsV1(
    completeInsectParts,
    completeInsectTargets,
    { targetCategory: 'insect' },
  ), ['leg', 'antenna', 'wing', 'leg', 'leg'])

  const asymmetricInsectParts = [
    { kind: 'head' as const, count: 1 },
    { kind: 'torso' as const, count: 1 },
    { kind: 'tail' as const, count: 1 },
    { kind: 'wing' as const, count: 2 },
    { kind: 'leg' as const, count: 6 },
  ]
  const asymmetricInsectTargets = Array.from(
    { length: 7 },
    (_, index) => target(
      index + 1,
      1,
      'none',
      [index % 2 === 0 ? 1_000 : -1_000, 0, 0],
      80,
      index - 3,
    ),
  )
  assert.equal(resolveBeginnerProtrusionKindsV1(
    asymmetricInsectParts,
    asymmetricInsectTargets,
    { targetCategory: 'insect' },
  ), null)
  assert.equal(beginnerSemanticPhysicalProfileIsAdmissibleV1(
    asymmetricInsectParts,
    asymmetricInsectTargets,
    'insect',
  ), true)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [...asymmetricInsectParts].reverse(),
    asymmetricInsectTargets,
    asymmetricInsectTargets.map(() => null),
    'insect',
    asymmetricInsectParts,
    asymmetricInsectTargets,
  ), asymmetricInsectParts)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    asymmetricInsectParts,
    asymmetricInsectTargets.slice(0, 6),
    asymmetricInsectTargets.slice(0, 6).map(() => null),
    'insect',
    asymmetricInsectParts,
    asymmetricInsectTargets,
  ), asymmetricInsectParts)

  const referenceAsymmetricInsectTargets = [1, 2, 3].map((id) =>
    target(id, 1, 'none', [1_000, 0, 0]))
  assert.equal(resolveBeginnerProtrusionKindsV1(
    asymmetricInsectParts,
    referenceAsymmetricInsectTargets,
    { targetCategory: 'insect' },
  ), null)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    asymmetricInsectParts,
    referenceAsymmetricInsectTargets,
    referenceAsymmetricInsectTargets.map(() => null),
    'insect',
  ), asymmetricInsectParts)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [...asymmetricInsectParts].reverse(),
    referenceAsymmetricInsectTargets,
    referenceAsymmetricInsectTargets.map(() => null),
    'insect',
    asymmetricInsectParts,
    referenceAsymmetricInsectTargets,
  ), asymmetricInsectParts)

  const asymmetricFishParts = [
    { kind: 'tail' as const, count: 1 },
    { kind: 'head' as const, count: 1 },
    { kind: 'fin' as const, count: 2 },
    { kind: 'torso' as const, count: 1 },
  ]
  const asymmetricFishTargets = [1, 2, 3].map((id) =>
    target(id, 1, 'none', [1_000, 0, 0], 80))
  assert.equal(resolveBeginnerProtrusionKindsV1(
    asymmetricFishParts,
    asymmetricFishTargets,
    { targetCategory: 'animal' },
  ), null)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [...asymmetricFishParts].reverse(),
    asymmetricFishTargets,
    asymmetricFishTargets.map(() => null),
    'animal',
    asymmetricFishParts,
    asymmetricFishTargets,
  ), asymmetricFishParts)
  const referenceAsymmetricFishTargets = [
    target(1, 1, 'none', [1_000, 0, 0]),
  ]
  assert.equal(resolveBeginnerProtrusionKindsV1(
    asymmetricFishParts,
    referenceAsymmetricFishTargets,
    { targetCategory: 'animal' },
  ), null)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    asymmetricFishParts,
    referenceAsymmetricFishTargets,
    [null],
    'animal',
  ), asymmetricFishParts)
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [...asymmetricFishParts].reverse(),
    referenceAsymmetricFishTargets,
    [null],
    'animal',
    asymmetricFishParts,
    referenceAsymmetricFishTargets,
  ), asymmetricFishParts)

  const straddledGenericParts = [
    { kind: 'head' as const, count: 1 },
    { kind: 'torso' as const, count: 1 },
    { kind: 'fin' as const, count: 5 },
    { kind: 'tail' as const, count: 5 },
  ]
  for (const counts of [[8, 2], [4, 6]] as const) {
    const physical = counts.map((count, index) =>
      target(index + 1, count, 'radial', [1_000, 0, 0]))
    assert.equal(resolveBeginnerProtrusionKindsV1(
      straddledGenericParts,
      physical,
      { targetCategory: 'animal', allowOrderedGeneric: true },
    ), null)
    assert.equal(beginnerSemanticPhysicalProfileIsAdmissibleV1(
      straddledGenericParts,
      physical,
      'animal',
    ), true)
    assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
      [...straddledGenericParts].reverse(),
      physical,
      physical.map(() => null),
      'animal',
      straddledGenericParts,
      physical,
    ), straddledGenericParts)
    const edited = physical.map((item, index) => (
      index === 0
        ? { ...item, length_tenths_mm: item.length_tenths_mm + 1 }
        : item
    ))
    assert.equal(selectBeginnerTargetPartsForProfileV1(
      straddledGenericParts,
      edited,
      edited.map(() => null),
      'animal',
      straddledGenericParts,
      physical,
    ), null)
  }

  const customPhysical = [
    target(1, 8, 'radial', [1_000, 0, 0]),
    target(2, 4, 'radial', [1_000, 0, 0]),
  ]
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [],
    customPhysical,
    customPhysical.map(() => null),
    'custom_object',
    [],
    customPhysical,
  ), [])
  const customBodyOnly = [
    { kind: 'torso' as const, count: 1 },
    { kind: 'head' as const, count: 1 },
  ]
  assert.deepEqual(selectBeginnerTargetPartsForProfileV1(
    [...customBodyOnly].reverse(),
    customPhysical,
    customPhysical.map(() => null),
    'custom_object',
    customBodyOnly,
    customPhysical,
  ), customBodyOnly)

  const malformedParts = [
    [
      { kind: 'head' as const, count: 1 },
      { kind: 'torso' as const, count: 1 },
      { kind: 'fin' as const, count: 3 },
      { kind: 'fin' as const, count: 2 },
    ],
    [
      { kind: 'head' as const, count: 2 },
      { kind: 'torso' as const, count: 1 },
      { kind: 'fin' as const, count: 5 },
    ],
    [
      { kind: 'head' as const, count: 1 },
      { kind: 'fin' as const, count: 5 },
    ],
  ]
  for (const parts of malformedParts) {
    assert.equal(resolveBeginnerProtrusionKindsV1(
      parts,
      [target(1, 5, 'radial', [1_000, 0, 0])],
      { targetCategory: 'animal', allowOrderedGeneric: true },
    ), null)
    assert.equal(beginnerSemanticPhysicalProfileIsAdmissibleV1(
      parts,
      [target(1, 5, 'radial', [1_000, 0, 0])],
      'animal',
    ), false)
  }
})

test('profile authoring fails closed on semantic aggregation overflow', () => {
  const formTargetParts = [
    { kind: 'head' as const, count: 1 },
    { kind: 'torso' as const, count: 1 },
  ]
  const makeProtrusions = (count: number) => Array.from(
    { length: count },
    (_, id) => ({
      id,
      count: 1,
      length_tenths_mm: 100,
      thickness_tenths_mm: 10,
      position_tenths_mm: [id, 0, 0] as [number, number, number],
      direction_milli: [1_000, 0, 0] as [number, number, number],
      symmetry: 'none' as const,
      curvature_degrees: 0,
      joint: 'fixed' as const,
      motion_degrees: [0, 0] as [number, number],
      side: 'either' as const,
      priority: 50,
    }),
  )
  const nineLegs = makeProtrusions(9)
  assert.equal(selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    nineLegs,
    nineLegs.map(() => 'leg'),
    'animal',
  ), null)
  const thirtyTwo = makeProtrusions(32)
  assert.equal(selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    thirtyTwo,
    ['leg', 'horn', 'ear', 'wing'].flatMap(
      (kind) => Array(8).fill(kind),
    ) as Array<'leg' | 'horn' | 'ear' | 'wing'>,
    'animal',
  ), null)
  assert.equal(selectBeginnerTargetPartsForProfileV1(
    formTargetParts,
    makeProtrusions(2),
    ['leg'],
    'animal',
  ), null)
  assert.equal(selectBeginnerTargetPartsForProfileV1(
    [
      { kind: 'head', count: 1 },
      { kind: 'head', count: 1 },
      { kind: 'torso', count: 1 },
    ],
    [],
    [],
    'animal',
  ), null)
})

test('profile submit preserves bookkeeping and only live non-form evidence', () => {
  const activeAsset = '11111111-1111-4111-8111-111111111111'
  const peerAsset = '22222222-2222-4222-8222-222222222222'
  const archivedAsset = '33333333-3333-4333-8333-333333333333'
  const constraints = {
    ...constraintsWithTargetPartCount(0),
    target_asset: {
      kind: 'reference_model' as const,
      asset_id: activeAsset,
    },
  }
  const current = {
    beginner_design_profile: {
      generation_constraints: constraints,
      generation_provenance: {
        schema_version: 1,
      },
      reference_surface_landmarks_tenths_mm: [
        [10, 20, 30],
        [40, 50, 60],
      ],
      archived_reference_model_asset_ids: [archivedAsset],
      reference_consensus_v1: {
        schema_version: 1,
        bindings: [
          {
            kind: 'reference_model',
            asset_id: activeAsset,
            sha256: Array(32).fill(1),
            quality: 100,
          },
          {
            kind: 'reference_model',
            asset_id: peerAsset,
            sha256: Array(32).fill(2),
            quality: 100,
          },
        ],
      },
    },
    reference_model_assets: [
      { asset_id: activeAsset, sha256: Array(32).fill(1) },
      { asset_id: peerAsset, sha256: Array(32).fill(2) },
      { asset_id: archivedAsset, sha256: Array(32).fill(3) },
    ],
  } as never

  const unchanged = selectBeginnerNonFormProfileFieldsForSubmitV1(
    current,
    constraints as never,
  )
  assert.deepEqual(unchanged.archived_reference_model_asset_ids, [
    archivedAsset,
  ])
  assert.deepEqual(unchanged.reference_surface_landmarks_tenths_mm, [
    [10, 20, 30],
    [40, 50, 60],
  ])
  assert.notEqual(unchanged.reference_consensus_v1, undefined)
  assert.equal(Object.hasOwn(unchanged, 'generation_provenance'), false)

  const changedConstraints = {
    ...constraints,
    maximum_steps: constraints.maximum_steps + 1,
  }
  const changed = selectBeginnerNonFormProfileFieldsForSubmitV1(
    current,
    changedConstraints as never,
  )
  assert.deepEqual(changed.archived_reference_model_asset_ids, [
    archivedAsset,
  ])
  assert.notEqual(
    changed.reference_surface_landmarks_tenths_mm,
    undefined,
  )
  assert.equal(changed.reference_consensus_v1, undefined)

  const changedAsset = selectBeginnerNonFormProfileFieldsForSubmitV1(
    current,
    { ...constraints, target_asset: null } as never,
  )
  assert.equal(
    changedAsset.reference_surface_landmarks_tenths_mm,
    undefined,
  )
  assert.deepEqual(changedAsset.archived_reference_model_asset_ids, [
    archivedAsset,
  ])
})

test('candidate normalization can require canonical generic source ID order', () => {
  const withSegments = (skeletonSegments: typeof canonicalTree) => ({
    ...constraintsWithTargetPartCount(1),
    skeleton_segments: skeletonSegments,
  })
  assert.notEqual(normalizeBeginnerGenerationConstraints(
    withSegments(canonicalTree),
    { requireCanonicalGenericIds: true },
  ), null)
  assert.equal(normalizeBeginnerGenerationConstraints(
    withSegments([...canonicalTree].reverse()),
    { requireCanonicalGenericIds: true },
  ), null)
  assert.equal(normalizeBeginnerGenerationConstraints(
    withSegments([
      canonicalTree[0]!,
      { ...canonicalTree[1]!, id: canonicalTree[0]!.id },
    ]),
    { requireCanonicalGenericIds: true },
  ), null)
})
