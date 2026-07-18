import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  summarizeFoldPreviewCollision,
} from '../src/lib/foldPreviewCollisionPresentation.ts'
import {
  makeFoldPreviewCanonicalAxisRotation,
} from '../src/lib/foldPreviewCanonicalRotation.ts'
import type {
  FoldPreviewHingeContactConstraint,
} from '../src/lib/foldPreviewHingeCollision.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewTreeKinematics,
} from '../src/lib/foldPreviewKinematics.ts'
import {
  isFoldPreviewExclusiveAllowedSharedVertexContact,
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewFullScanNonAdjacentWitnessJob,
  type FoldPreviewNarrowPhaseAnalysisJob,
  type FoldPreviewNarrowPhaseAnalyzer,
  type FoldPreviewNarrowPhaseInteraction,
  type FoldPreviewNarrowPhaseResult,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const THICKNESSES = [0, 0.1, 1, 3] as const
const TINY_ANGLE_DEGREES = 1e-6

const cornerMountainValley = createCornerMountainValleyFixture()
const midpointMountainMountain = createMidpointMountainMountainFixture()

test('certified corner mountain-valley grid never promotes its shared apex to penetration', () => {
  const analyzer = prepareFixture(cornerMountainValley)
  const poses = [
    { label: 'flat', left: 0, right: 0 },
    { label: 'tiny-left', left: TINY_ANGLE_DEGREES, right: 0 },
    { label: 'tiny-right', left: 0, right: TINY_ANGLE_DEGREES },
    { label: 'left-only-10', left: 10, right: 0 },
    { label: 'right-only-10', left: 0, right: 10 },
    { label: 'both-45', left: 45, right: 45 },
    { label: 'both-90', left: 90, right: 90 },
    { label: 'both-91', left: 91, right: 91 },
    { label: 'both-135', left: 135, right: 135 },
    { label: 'both-179', left: 179, right: 179 },
    { label: 'both-180', left: 180, right: 180 },
  ] as const

  for (const thickness of THICKNESSES) {
    for (const angles of poses) {
      const transforms = poseAt(
        cornerMountainValley,
        angles.left,
        angles.right,
      )
      const result = analyzer.analyze(transforms, thickness)
      const diagnostic =
        `${angles.label}:left=${angles.left}:right=${angles.right}`
        + `:${thickness}mm`
      assert.ok(result, diagnostic)
      const outer = requireOuterInteraction(
        result,
        cornerMountainValley,
        diagnostic,
      )

      assert.notEqual(outer.geometryClass, 'penetrating', diagnostic)
      const presentation = summarizeFoldPreviewCollision(result)
      assert.equal(presentation.nonAdjacentPenetrations, 0, diagnostic)
      const layerHoldPermitted =
        thickness > 0
        && angles.left === 180
        && angles.right === 180

      if (outer.geometryClass === 'touching') {
        const topologyAllowed =
          isFoldPreviewExclusiveAllowedSharedVertexContact(outer)
        const expectedTopologyAllowance =
          !(angles.left === 0 && angles.right === 0)
          && !(angles.left === 180 && angles.right === 180)
        assert.equal(
          topologyAllowed,
          expectedTopologyAllowance,
          diagnostic,
        )
        assert.equal(
          presentation.nonAdjacentAllowedSharedVertexContacts,
          topologyAllowed ? 1 : 0,
          diagnostic,
        )
        assert.equal(
          presentation.nonAdjacentContacts,
          topologyAllowed ? 0 : 1,
          diagnostic,
        )
      } else {
        assert.equal(layerHoldPermitted, true, diagnostic)
        assert.equal(outer.geometryClass, 'indeterminate', diagnostic)
        assert.ok(presentation.indeterminateInteractions > 0, diagnostic)
        assert.equal(
          isFoldPreviewExclusiveAllowedSharedVertexContact(outer),
          false,
          diagnostic,
        )
      }
      if (layerHoldPermitted) {
        assert.ok(presentation.indeterminateInteractions > 0, diagnostic)
      } else {
        assert.equal(presentation.indeterminateInteractions, 0, diagnostic)
      }
    }
  }
})

test('certified midpoint mountain-mountain grid blocks every deep crossing pose', () => {
  const analyzer = prepareFixture(midpointMountainMountain)
  const angles = [10, 45, 90, 91, 135, 179, 180] as const
  // The exact mid-surface crossing starts at acos(-1/4), about 104.48°.
  // Owner policy nevertheless fixes 90°/91° as an explicit blocking hold so
  // the approaching mountain/mountain layers cannot inherit the apex-only
  // allowance intended for the geometrically distinct mountain/valley V.
  const requiredBlockingAngles = new Set<number>([90, 91, 135, 179, 180])
  const unsafeCells: string[] = []

  for (const thickness of THICKNESSES) {
    for (const degrees of angles) {
      const transforms = poseAt(
        midpointMountainMountain,
        degrees,
        degrees,
      )
      const result = analyzer.analyze(transforms, thickness)
      const diagnostic = `${degrees}deg:${thickness}mm`
      assert.ok(result, diagnostic)
      const outer = requireOuterInteraction(
        result,
        midpointMountainMountain,
        diagnostic,
      )
      const presentation = summarizeFoldPreviewCollision(result)

      if (requiredBlockingAngles.has(degrees)) {
        const rawBlocking =
          outer.geometryClass === 'penetrating'
          || outer.geometryClass === 'indeterminate'
        const aggregateBlocking =
          presentation.nonAdjacentPenetrations > 0
          || presentation.indeterminateInteractions > 0
        const topologyAllowed =
          isFoldPreviewExclusiveAllowedSharedVertexContact(outer)
        if (!rawBlocking || !aggregateBlocking || topologyAllowed) {
          unsafeCells.push(
            `${diagnostic}:geometry=${outer.geometryClass}`
            + `:penetrations=${presentation.nonAdjacentPenetrations}`
            + `:holds=${presentation.indeterminateInteractions}`
            + `:sharedAllowed=${topologyAllowed}`,
          )
        }
      } else {
        assert.equal(outer.geometryClass, 'touching', diagnostic)
        assert.equal(
          isFoldPreviewExclusiveAllowedSharedVertexContact(outer),
          true,
          diagnostic,
        )
        assert.equal(
          presentation.nonAdjacentAllowedSharedVertexContacts,
          1,
          diagnostic,
        )
        assert.equal(presentation.nonAdjacentPenetrations, 0, diagnostic)
        assert.equal(presentation.indeterminateInteractions, 0, diagnostic)
      }
    }
  }
  assert.deepEqual(unsafeCells, [])
})

test('no-shared-feature representative geometry is stable in both face orders', () => {
  const fixtures: readonly RepresentativeFixture[] = [
    noSharedSeparatedFixture(),
    noSharedPointContactFixture(),
    noSharedLineContactFixture(),
    noSharedCoplanarOverlapFixture(),
    noSharedTransversalCrossingFixture(),
    noSharedPositiveVolumeFixture(),
    noSharedIndeterminateFixture(),
  ]

  for (const fixture of fixtures) {
    assertRepresentativeFixture(fixture)
  }
})

test('shared-vertex evidence is narrow: only the shared feature is allowed', () => {
  const fixtures: readonly RepresentativeFixture[] = [
    sharedVertexOnlyFixture(0),
    sharedVertexOnlyFixture(1),
    sharedVertexLineContactFixture(),
    sharedVertexCoplanarOverlapFixture(0),
    sharedVertexCoplanarOverlapFixture(1),
    sharedVertexTransversalCrossingFixture(),
    sharedVertexIndeterminateFixture(),
  ]

  for (const fixture of fixtures) {
    assertRepresentativeFixture(fixture)
  }
})

test('shared-hinge representatives distinguish boundary, corridor, flat stack, and layer hold', () => {
  const fixtures: readonly RepresentativeFixture[] = [
    sharedHingeFixture('boundary'),
    sharedHingeFixture('corridor'),
    sharedHingeFixture('flat-stack'),
    sharedHingeFixture('layer-hold'),
  ]

  for (const fixture of fixtures) {
    assertRepresentativeFixture(fixture)
  }
})

test('important certified cells agree across sync, resumable, and full scans', () => {
  const cornerTransforms = poseAt(cornerMountainValley, 91, 91)
  const midpointNinetyTransforms = poseAt(
    midpointMountainMountain,
    90,
    90,
  )
  const midpointNinetyOneTransforms = poseAt(
    midpointMountainMountain,
    91,
    91,
  )
  const midpointTransforms = poseAt(midpointMountainMountain, 135, 135)
  const cases: readonly ConsistencyFixture[] = [
    {
      label: 'corner-shared-vertex-allowance',
      analyzer: prepareFixture(cornerMountainValley),
      transforms: cornerTransforms,
      thickness: 1,
      firstFaceId: cornerMountainValley.outerFaceIds[0],
      secondFaceId: cornerMountainValley.outerFaceIds[1],
      expected: 'allowed_shared_vertex_contact',
    },
    {
      label: 'midpoint-90-degree-policy-hold',
      analyzer: prepareFixture(midpointMountainMountain),
      transforms: midpointNinetyTransforms,
      thickness: 0.1,
      firstFaceId: midpointMountainMountain.outerFaceIds[0],
      secondFaceId: midpointMountainMountain.outerFaceIds[1],
      expected: 'indeterminate',
    },
    {
      label: 'midpoint-91-degree-policy-hold-positive-thickness',
      analyzer: prepareFixture(midpointMountainMountain),
      transforms: midpointNinetyOneTransforms,
      thickness: 1,
      firstFaceId: midpointMountainMountain.outerFaceIds[0],
      secondFaceId: midpointMountainMountain.outerFaceIds[1],
      expected: 'indeterminate',
    },
    {
      label: 'midpoint-deep-crossing',
      analyzer: prepareFixture(midpointMountainMountain),
      transforms: midpointTransforms,
      thickness: 1,
      firstFaceId: midpointMountainMountain.outerFaceIds[0],
      secondFaceId: midpointMountainMountain.outerFaceIds[1],
      expected: 'penetrating',
    },
    consistencyFixture(sharedVertexOnlyFixture(1)),
    consistencyFixture(noSharedPointContactFixture(1)),
    consistencyFixture(noSharedPositiveVolumeFixture()),
    consistencyFixture(noSharedPositiveThicknessIndeterminateFixture()),
  ]

  for (const fixture of cases) {
    assertConsistentAnalysisPaths(fixture)
  }
})

type TreeFixture = Readonly<{
  prefix: string
  faces: readonly FoldPreviewCollisionPoseFace[]
  adjacencies: readonly FoldPreviewCollisionAdjacency[]
  constraints: readonly FoldPreviewHingeContactConstraint[]
  tree: FoldPreviewTreeKinematics
  leftEdgeId: string
  rightEdgeId: string
  outerFaceIds: readonly [string, string]
}>

type ExpectedGeometry =
  | 'separated'
  | 'touching'
  | 'allowed_shared_vertex_contact'
  | 'penetrating'
  | 'indeterminate'
  | 'hinge_boundary_contact'
  | 'hinge_corridor_overlap'
  | 'hinge_flat_surface_stack'
  | 'hinge_layer_offset_unmodeled'

type RepresentativeFixture = Readonly<{
  label: string
  faces: readonly FoldPreviewCollisionPoseFace[]
  transforms: ReadonlyMap<string, Matrix4>
  thickness: number
  expected: ExpectedGeometry
  adjacencies?: readonly FoldPreviewCollisionAdjacency[]
  constraints?: readonly FoldPreviewHingeContactConstraint[]
}>

type ConsistencyFixture = Readonly<{
  label: string
  analyzer: FoldPreviewNarrowPhaseAnalyzer
  transforms: ReadonlyMap<string, Matrix4>
  thickness: number
  firstFaceId: string
  secondFaceId: string
  expected:
    | 'touching'
    | 'allowed_shared_vertex_contact'
    | 'penetrating'
    | 'indeterminate'
}>

function createCornerMountainValleyFixture(): TreeFixture {
  const apex = vertex('corner-apex', 400, 400)
  const leftEnd = vertex('corner-left-end', 300, 0)
  const rightEnd = vertex('corner-right-end', 0, 300)
  const leftEdgeId = 'corner-left-hinge'
  const rightEdgeId = 'corner-right-hinge'
  const leftFaceId = 'corner-left'
  const middleFaceId = 'corner-middle'
  const rightFaceId = 'corner-right'
  return {
    prefix: 'corner',
    faces: [
      {
        id: leftFaceId,
        polygon: [
          apex,
          vertex('corner-bottom-right', 400, 0),
          leftEnd,
        ],
      },
      {
        id: middleFaceId,
        polygon: [
          apex,
          leftEnd,
          vertex('corner-bottom-left', 0, 0),
          rightEnd,
        ],
      },
      {
        id: rightFaceId,
        polygon: [
          apex,
          rightEnd,
          vertex('corner-top-left', 0, 400),
        ],
      },
    ],
    adjacencies: [
      {
        edgeId: leftEdgeId,
        firstFaceId: leftFaceId,
        secondFaceId: middleFaceId,
      },
      {
        edgeId: rightEdgeId,
        firstFaceId: middleFaceId,
        secondFaceId: rightFaceId,
      },
    ],
    constraints: [
      {
        edgeId: leftEdgeId,
        leftFaceId,
        rightFaceId: middleFaceId,
        start: apex,
        end: leftEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
      {
        edgeId: rightEdgeId,
        leftFaceId: middleFaceId,
        rightFaceId,
        start: apex,
        end: rightEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
    ],
    tree: {
      kind: 'tree',
      rootFaceId: middleFaceId,
      joints: [
        {
          parentFaceId: middleFaceId,
          childFaceId: leftFaceId,
          childRotationSign: -1,
          hinge: {
            edgeId: leftEdgeId,
            leftFaceId,
            rightFaceId: middleFaceId,
            start: apex,
            end: leftEnd,
            axis: axis(apex, leftEnd),
            assignment: 'mountain',
            rotationSign: 1,
          },
        },
        {
          parentFaceId: middleFaceId,
          childFaceId: rightFaceId,
          childRotationSign: -1,
          hinge: {
            edgeId: rightEdgeId,
            leftFaceId: middleFaceId,
            rightFaceId,
            start: apex,
            end: rightEnd,
            axis: axis(apex, rightEnd),
            assignment: 'valley',
            rotationSign: -1,
          },
        },
      ],
    },
    leftEdgeId,
    rightEdgeId,
    outerFaceIds: [leftFaceId, rightFaceId],
  }
}

function createMidpointMountainMountainFixture(): TreeFixture {
  const apex = vertex('midpoint-apex', 200, 0)
  const leftEnd = vertex('midpoint-left-end', 0, 400)
  const rightEnd = vertex('midpoint-right-end', 400, 400)
  const leftEdgeId = 'midpoint-left-hinge'
  const rightEdgeId = 'midpoint-right-hinge'
  const leftFaceId = 'midpoint-left'
  const middleFaceId = 'midpoint-middle'
  const rightFaceId = 'midpoint-right'
  return {
    prefix: 'midpoint',
    faces: [
      {
        id: leftFaceId,
        polygon: [
          apex,
          vertex('midpoint-bottom-left', 0, 0),
          leftEnd,
        ],
      },
      {
        id: middleFaceId,
        polygon: [apex, leftEnd, rightEnd],
      },
      {
        id: rightFaceId,
        polygon: [
          apex,
          rightEnd,
          vertex('midpoint-bottom-right', 400, 0),
        ],
      },
    ],
    adjacencies: [
      {
        edgeId: leftEdgeId,
        firstFaceId: leftFaceId,
        secondFaceId: middleFaceId,
      },
      {
        edgeId: rightEdgeId,
        firstFaceId: middleFaceId,
        secondFaceId: rightFaceId,
      },
    ],
    constraints: [
      {
        edgeId: leftEdgeId,
        leftFaceId,
        rightFaceId: middleFaceId,
        start: apex,
        end: leftEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
      {
        edgeId: rightEdgeId,
        leftFaceId: middleFaceId,
        rightFaceId,
        start: apex,
        end: rightEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
    ],
    tree: {
      kind: 'tree',
      rootFaceId: middleFaceId,
      joints: [
        {
          parentFaceId: middleFaceId,
          childFaceId: leftFaceId,
          childRotationSign: -1,
          hinge: {
            edgeId: leftEdgeId,
            leftFaceId,
            rightFaceId: middleFaceId,
            start: apex,
            end: leftEnd,
            axis: axis(apex, leftEnd),
            assignment: 'mountain',
            rotationSign: 1,
          },
        },
        {
          parentFaceId: middleFaceId,
          childFaceId: rightFaceId,
          childRotationSign: 1,
          hinge: {
            edgeId: rightEdgeId,
            leftFaceId: middleFaceId,
            rightFaceId,
            start: apex,
            end: rightEnd,
            axis: axis(apex, rightEnd),
            assignment: 'mountain',
            rotationSign: 1,
          },
        },
      ],
    },
    leftEdgeId,
    rightEdgeId,
    outerFaceIds: [leftFaceId, rightFaceId],
  }
}

function prepareFixture(fixture: TreeFixture) {
  const analyzer = prepareFoldPreviewNarrowPhase(
    fixture.faces,
    fixture.adjacencies,
    fixture.constraints,
  )
  assert.ok(analyzer, fixture.prefix)
  return analyzer
}

function poseAt(
  fixture: TreeFixture,
  leftDegrees: number,
  rightDegrees: number,
) {
  const pose = calculateFoldTreePoseWithAngles(fixture.tree, {
    kind: 'per_hinge',
    angles: [
      {
        edgeId: fixture.leftEdgeId,
        angleDegrees: leftDegrees,
      },
      {
        edgeId: fixture.rightEdgeId,
        angleDegrees: rightDegrees,
      },
    ],
  })
  assert.ok(pose, `${fixture.prefix}:${leftDegrees}:${rightDegrees}`)
  return pose.faceTransforms
}

function requireOuterInteraction(
  result: FoldPreviewNarrowPhaseResult,
  fixture: TreeFixture,
  diagnostic: string,
) {
  const interaction = findInteraction(
    result,
    fixture.outerFaceIds[0],
    fixture.outerFaceIds[1],
  )
  assert.ok(interaction, diagnostic)
  assert.equal(interaction.relation, 'non_adjacent', diagnostic)
  return interaction
}

function assertRepresentativeFixture(fixture: RepresentativeFixture) {
  const forward = analyzeRepresentative(fixture, fixture.faces)
  const reverse = analyzeRepresentative(fixture, [...fixture.faces].reverse())
  assert.deepEqual(reverse, forward, `${fixture.label}:face-order`)

  const presentation = summarizeFoldPreviewCollision(forward)
  const interaction = forward.interactions[0]
  switch (fixture.expected) {
    case 'separated':
      assert.deepEqual(forward.interactions, [], fixture.label)
      assert.equal(presentation.narrowInteractions, 0, fixture.label)
      return
    case 'allowed_shared_vertex_contact':
      assert.ok(interaction, fixture.label)
      assert.equal(interaction.geometryClass, 'touching', fixture.label)
      assert.equal(
        isFoldPreviewExclusiveAllowedSharedVertexContact(interaction),
        true,
        fixture.label,
      )
      assert.equal(
        presentation.nonAdjacentAllowedSharedVertexContacts,
        1,
        fixture.label,
      )
      assert.equal(presentation.nonAdjacentContacts, 0, fixture.label)
      assert.equal(presentation.nonAdjacentPenetrations, 0, fixture.label)
      return
    case 'touching':
      assert.ok(interaction, fixture.label)
      assert.equal(interaction.geometryClass, 'touching', fixture.label)
      assert.equal(
        isFoldPreviewExclusiveAllowedSharedVertexContact(interaction),
        false,
        fixture.label,
      )
      assert.equal(presentation.nonAdjacentContacts, 1, fixture.label)
      assert.equal(presentation.nonAdjacentPenetrations, 0, fixture.label)
      return
    case 'penetrating':
      assert.ok(interaction, fixture.label)
      assert.equal(interaction.geometryClass, 'penetrating', fixture.label)
      assert.equal(presentation.nonAdjacentPenetrations, 1, fixture.label)
      return
    case 'indeterminate':
      assert.ok(interaction, fixture.label)
      assert.equal(interaction.geometryClass, 'indeterminate', fixture.label)
      assert.equal(presentation.indeterminateInteractions, 1, fixture.label)
      assert.equal(presentation.nonAdjacentPenetrations, 0, fixture.label)
      return
    case 'hinge_boundary_contact':
      assertHingeDecision(
        fixture.label,
        interaction,
        presentation.hingeModelAllowedContacts,
        'touching',
        'allowed_by_hinge_model',
        'boundary_contact',
      )
      return
    case 'hinge_corridor_overlap':
      assertHingeDecision(
        fixture.label,
        interaction,
        presentation.hingeModelCorridorOverlaps,
        'penetrating',
        'allowed_by_hinge_model',
        'corridor_overlap',
      )
      return
    case 'hinge_flat_surface_stack':
      assertHingeDecision(
        fixture.label,
        interaction,
        presentation.hingeModelFlatSurfaceStacks,
        'penetrating',
        'allowed_by_hinge_model',
        'flat_surface_stack',
      )
      return
    case 'hinge_layer_offset_unmodeled':
      assert.ok(interaction, fixture.label)
      assert.notEqual(interaction.geometryClass, 'touching', fixture.label)
      assert.deepEqual(interaction.hingeDecision, {
        kind: 'indeterminate',
        hingeEdgeIds: ['representative-hinge'],
        reason: 'layer_offset_unmodeled',
      }, fixture.label)
      assert.equal(presentation.hingeLayerOffsetUnmodeled, 1, fixture.label)
      assert.equal(presentation.indeterminateInteractions, 1, fixture.label)
  }
}

function analyzeRepresentative(
  fixture: RepresentativeFixture,
  faces: readonly FoldPreviewCollisionPoseFace[],
) {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    fixture.adjacencies ?? [],
    fixture.constraints ?? [],
  )
  assert.ok(analyzer, fixture.label)
  const result = analyzer.analyze(fixture.transforms, fixture.thickness)
  assert.ok(result, fixture.label)
  return result
}

function assertHingeDecision(
  label: string,
  interaction: FoldPreviewNarrowPhaseInteraction | undefined,
  presentationCount: number,
  geometryClass: 'touching' | 'penetrating',
  decisionKind: 'allowed_by_hinge_model',
  decisionGeometry:
    | 'boundary_contact'
    | 'corridor_overlap'
    | 'flat_surface_stack',
) {
  assert.ok(interaction, label)
  assert.equal(interaction.relation, 'hinge_adjacent', label)
  assert.equal(interaction.geometryClass, geometryClass, label)
  assert.equal(interaction.hingeDecision?.kind, decisionKind, label)
  assert.equal(
    interaction.hingeDecision?.kind === 'allowed_by_hinge_model'
      ? interaction.hingeDecision.geometry
      : null,
    decisionGeometry,
    label,
  )
  assert.equal(presentationCount, 1, label)
}

function assertConsistentAnalysisPaths(fixture: ConsistencyFixture) {
  const synchronous = fixture.analyzer.analyze(
    fixture.transforms,
    fixture.thickness,
  )
  assert.ok(synchronous, fixture.label)
  const resumable = fixture.analyzer.createAnalysisJob(
    fixture.transforms,
    fixture.thickness,
  )
  assert.ok(resumable, fixture.label)
  assert.deepEqual(
    drainAnalysisJob(resumable),
    synchronous,
    `${fixture.label}:sync/resumable`,
  )

  const interaction = findInteraction(
    synchronous,
    fixture.firstFaceId,
    fixture.secondFaceId,
  )
  assert.ok(interaction, fixture.label)
  assertExpectedNonAdjacentInteraction(
    interaction,
    fixture.expected,
    fixture.label,
  )

  const fullScan = fixture.analyzer.collectFullScanNonAdjacentWitnessSet(
    fixture.transforms,
    fixture.thickness,
  )
  assert.ok(fullScan, fixture.label)
  const fullScanJob =
    fixture.analyzer.createFullScanNonAdjacentWitnessSetJob(
      fixture.transforms,
      fixture.thickness,
    )
  assert.ok(fullScanJob, fixture.label)
  assert.deepEqual(
    drainFullScanJob(fullScanJob),
    fullScan,
    `${fixture.label}:full-sync/resumable`,
  )
  assert.ok(fullScan.coverage.authoritativePairScanComplete, fixture.label)

  const coverage = fullScan.coverage
  if (fixture.expected === 'allowed_shared_vertex_contact') {
    assert.ok(coverage.allowedSharedVertexPairCount > 0, fixture.label)
    assert.equal(coverage.touchingPairCount, 0, fixture.label)
    assert.equal(coverage.penetratingPairCount, 0, fixture.label)
    assert.equal(coverage.indeterminatePairCount, 0, fixture.label)
  } else if (fixture.expected === 'touching') {
    assert.ok(coverage.touchingPairCount > 0, fixture.label)
    assert.equal(coverage.penetratingPairCount, 0, fixture.label)
    assert.equal(coverage.indeterminatePairCount, 0, fixture.label)
  } else if (fixture.expected === 'penetrating') {
    assert.ok(coverage.penetratingPairCount > 0, fixture.label)
  } else {
    assert.ok(coverage.indeterminatePairCount > 0, fixture.label)
    assert.equal(fullScan.kind, 'unavailable', fixture.label)
  }
}

function assertExpectedNonAdjacentInteraction(
  interaction: FoldPreviewNarrowPhaseInteraction,
  expected: ConsistencyFixture['expected'],
  label: string,
) {
  assert.equal(interaction.relation, 'non_adjacent', label)
  if (expected === 'allowed_shared_vertex_contact') {
    assert.equal(interaction.geometryClass, 'touching', label)
    assert.equal(
      isFoldPreviewExclusiveAllowedSharedVertexContact(interaction),
      true,
      label,
    )
  } else {
    assert.equal(interaction.geometryClass, expected, label)
  }
}

function consistencyFixture(
  fixture: RepresentativeFixture,
): ConsistencyFixture {
  const analyzer = prepareFoldPreviewNarrowPhase(
    fixture.faces,
    fixture.adjacencies ?? [],
    fixture.constraints ?? [],
  )
  assert.ok(analyzer, fixture.label)
  assert.ok(
    fixture.expected === 'touching'
    || fixture.expected === 'allowed_shared_vertex_contact'
    || fixture.expected === 'penetrating'
    || fixture.expected === 'indeterminate',
    fixture.label,
  )
  const orderedIds = fixture.faces.map((face) => face.id).sort()
  assert.equal(orderedIds.length, 2, fixture.label)
  return {
    label: fixture.label,
    analyzer,
    transforms: fixture.transforms,
    thickness: fixture.thickness,
    firstFaceId: orderedIds[0]!,
    secondFaceId: orderedIds[1]!,
    expected: fixture.expected,
  }
}

function drainAnalysisJob(job: FoldPreviewNarrowPhaseAnalysisJob) {
  let step = job.step(1)
  const limit = job.workBounds.maximumTotalWorkUnits + 8
  for (let index = 0; step.kind === 'pending' && index < limit; index += 1) {
    step = job.step(1)
  }
  if (step.kind !== 'complete') {
    assert.fail(`analysis job ended as ${step.kind}`)
  }
  return step.result
}

function drainFullScanJob(job: FoldPreviewFullScanNonAdjacentWitnessJob) {
  let step = job.step(1)
  const limit = job.workBounds.maximumTotalWorkUnits + 8
  for (let index = 0; step.kind === 'pending' && index < limit; index += 1) {
    step = job.step(1)
  }
  if (step.kind !== 'complete') {
    assert.fail(`full-scan job ended as ${step.kind}`)
  }
  return step.result
}

function findInteraction(
  result: FoldPreviewNarrowPhaseResult,
  firstFaceId: string,
  secondFaceId: string,
) {
  return result.interactions.find((interaction) =>
    interaction.firstFaceId === firstFaceId
    && interaction.secondFaceId === secondFaceId)
}

function noSharedSeparatedFixture(): RepresentativeFixture {
  const faces = pairedTriangles('separated')
  return {
    label: 'no-shared:separated',
    faces,
    transforms: transformsFor(
      faces,
      new Matrix4(),
      new Matrix4().makeTranslation(3, 0, 0),
    ),
    thickness: 1,
    expected: 'separated',
  }
}

function noSharedPointContactFixture(
  thickness = 0,
): RepresentativeFixture {
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'point-a',
      polygon: [
        vertex('point-a-0', 0, 0),
        vertex('point-a-1', 1, 0),
        vertex('point-a-2', 0, 1),
      ],
    },
    {
      id: 'point-b',
      polygon: [
        vertex('point-b-0', 1, 0),
        vertex('point-b-1', 2, 0),
        vertex('point-b-2', 2, -1),
      ],
    },
  ]
  return {
    label: `no-shared:point-contact:${thickness}mm`,
    faces,
    transforms: identityTransforms(faces),
    thickness,
    expected: 'touching',
  }
}

function noSharedLineContactFixture(): RepresentativeFixture {
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'line-a',
      polygon: [
        vertex('line-a-0', 0, 0),
        vertex('line-a-1', 1, 0),
        vertex('line-a-2', 1, 1),
        vertex('line-a-3', 0, 1),
      ],
    },
    {
      id: 'line-b',
      polygon: [
        vertex('line-b-0', 1, 0),
        vertex('line-b-1', 2, 0),
        vertex('line-b-2', 2, 1),
        vertex('line-b-3', 1, 1),
      ],
    },
  ]
  return {
    label: 'no-shared:line-contact',
    faces,
    transforms: identityTransforms(faces),
    thickness: 0,
    expected: 'touching',
  }
}

function noSharedCoplanarOverlapFixture(): RepresentativeFixture {
  const faces = pairedTriangles('coplanar')
  return {
    label: 'no-shared:coplanar-positive-area',
    faces,
    transforms: identityTransforms(faces),
    thickness: 0,
    expected: 'penetrating',
  }
}

function noSharedTransversalCrossingFixture(): RepresentativeFixture {
  const faces = pairedCenteredTriangles('transversal')
  const crossing = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(1, 0, 0),
    Math.PI / 2,
  )
  assert.ok(crossing)
  return {
    label: 'no-shared:transversal-crossing',
    faces,
    transforms: transformsFor(faces, new Matrix4(), crossing),
    thickness: 0,
    expected: 'penetrating',
  }
}

function noSharedPositiveVolumeFixture(): RepresentativeFixture {
  const faces = pairedTriangles('volume')
  return {
    label: 'no-shared:positive-volume',
    faces,
    transforms: identityTransforms(faces),
    thickness: 1,
    expected: 'penetrating',
  }
}

function noSharedIndeterminateFixture(): RepresentativeFixture {
  const faces = pairedTriangles('hold')
  return {
    label: 'no-shared:numerical-hold',
    faces,
    transforms: transformsFor(
      faces,
      new Matrix4(),
      new Matrix4().makeTranslation(0, 1e-15, 0),
    ),
    thickness: 0,
    expected: 'indeterminate',
  }
}

function noSharedPositiveThicknessIndeterminateFixture():
RepresentativeFixture {
  const faces = pairedTriangles('thick-hold')
  return {
    label: 'no-shared:positive-thickness-hold',
    faces,
    transforms: transformsFor(
      faces,
      new Matrix4(),
      new Matrix4().makeTranslation(0, 1 + 1e-14, 0),
    ),
    thickness: 1,
    expected: 'indeterminate',
  }
}

function sharedVertexOnlyFixture(thickness: number): RepresentativeFixture {
  const shared = vertex('shared-only-apex', 0, 0)
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'shared-only-a',
      polygon: [
        shared,
        vertex('shared-only-a-low', -2, -1),
        vertex('shared-only-a-high', -2, 1),
      ],
    },
    {
      id: 'shared-only-b',
      polygon: [
        shared,
        vertex('shared-only-b-low', 2, -1),
        vertex('shared-only-b-high', 2, 1),
      ],
    },
  ]
  return {
    label: `shared-vertex:feature-only:${thickness}mm`,
    faces,
    transforms: transformsFor(
      faces,
      new Matrix4().makeRotationZ(Math.PI / 4),
      new Matrix4(),
    ),
    thickness,
    expected: 'allowed_shared_vertex_contact',
  }
}

function sharedVertexLineContactFixture(): RepresentativeFixture {
  const shared = vertex('shared-line-start', 0, 0)
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'shared-line-a',
      polygon: [
        shared,
        vertex('shared-line-a-end', 2, 0),
        vertex('shared-line-a-high', 0, 1),
      ],
    },
    {
      id: 'shared-line-b',
      polygon: [
        shared,
        vertex('shared-line-b-end', 2, 0),
        vertex('shared-line-b-low', 2, -1),
      ],
    },
  ]
  return {
    label: 'shared-vertex:line-beyond-feature',
    faces,
    transforms: identityTransforms(faces),
    thickness: 0,
    expected: 'touching',
  }
}

function sharedVertexCoplanarOverlapFixture(
  thickness: number,
): RepresentativeFixture {
  const shared = vertex('shared-overlap-apex', 0, 0)
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'shared-overlap-a',
      polygon: [
        shared,
        vertex('shared-overlap-a-x', 2, 0),
        vertex('shared-overlap-a-z', 0, 2),
      ],
    },
    {
      id: 'shared-overlap-b',
      polygon: [
        shared,
        vertex('shared-overlap-b-x', 2, 0),
        vertex('shared-overlap-b-z', 0, 2),
      ],
    },
  ]
  return {
    label: `shared-vertex:coplanar-overlap:${thickness}mm`,
    faces,
    transforms: identityTransforms(faces),
    thickness,
    expected: 'penetrating',
  }
}

function sharedVertexTransversalCrossingFixture():
RepresentativeFixture {
  const shared = vertex('shared-crossing-apex', 0, 0)
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'shared-crossing-a',
      polygon: [
        shared,
        vertex('shared-crossing-a-low', 2, -1),
        vertex('shared-crossing-a-high', 2, 1),
      ],
    },
    {
      id: 'shared-crossing-b',
      polygon: [
        shared,
        vertex('shared-crossing-b-low', 2, -1),
        vertex('shared-crossing-b-high', 0, 1),
      ],
    },
  ]
  const crossing = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(1, 0, 0),
    Math.PI / 2,
  )
  assert.ok(crossing)
  return {
    label: 'shared-vertex:transversal-outside-feature',
    faces,
    transforms: transformsFor(faces, new Matrix4(), crossing),
    thickness: 0,
    expected: 'penetrating',
  }
}

function sharedVertexIndeterminateFixture(): RepresentativeFixture {
  const base = sharedVertexOnlyFixture(0)
  const rotatedAndDisplaced = new Matrix4()
    .makeTranslation(0, 1e-15, 0)
    .multiply(new Matrix4().makeRotationZ(Math.PI / 4))
  return {
    ...base,
    label: 'shared-vertex:numerical-pose-hold',
    transforms: transformsFor(
      base.faces,
      rotatedAndDisplaced,
      new Matrix4(),
    ),
    expected: 'indeterminate',
  }
}

function sharedHingeFixture(
  kind: 'boundary' | 'corridor' | 'flat-stack' | 'layer-hold',
): RepresentativeFixture {
  const start = vertex('representative-hinge-start', 0, 0)
  const end = vertex('representative-hinge-end', 0, 2)
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'hinge-left',
      polygon: [
        start,
        vertex('hinge-left-bottom', -2, 0),
        vertex('hinge-left-top', -2, 2),
        end,
      ],
    },
    {
      id: 'hinge-right',
      polygon: [
        start,
        end,
        vertex('hinge-right-top', 2, 2),
        vertex('hinge-right-bottom', 2, 0),
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'representative-hinge',
    firstFaceId: 'hinge-left',
    secondFaceId: 'hinge-right',
  }]
  const constraints: readonly FoldPreviewHingeContactConstraint[] = [{
    edgeId: 'representative-hinge',
    leftFaceId: 'hinge-left',
    rightFaceId: 'hinge-right',
    start,
    end,
    thicknessRule: 'centered_mid_surface_v1',
  }]
  const degrees = kind === 'boundary' ? 0 : kind === 'corridor' ? 90 : 180
  const thickness = kind === 'boundary' || kind === 'flat-stack' ? 0 : 1
  const rotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(0, 0, 1),
    degrees * Math.PI / 180,
  )
  assert.ok(rotation)
  const expected: ExpectedGeometry =
    kind === 'boundary'
      ? 'hinge_boundary_contact'
      : kind === 'corridor'
        ? 'hinge_corridor_overlap'
        : kind === 'flat-stack'
          ? 'hinge_flat_surface_stack'
          : 'hinge_layer_offset_unmodeled'
  return {
    label: `shared-hinge:${kind}`,
    faces,
    transforms: transformsFor(faces, new Matrix4(), rotation),
    thickness,
    expected,
    adjacencies,
    constraints,
  }
}

function pairedTriangles(prefix: string) {
  return [
    {
      id: `${prefix}-a`,
      polygon: [
        vertex(`${prefix}-a-0`, 0, 0),
        vertex(`${prefix}-a-1`, 1, 0),
        vertex(`${prefix}-a-2`, 0, 1),
      ],
    },
    {
      id: `${prefix}-b`,
      polygon: [
        vertex(`${prefix}-b-0`, 0, 0),
        vertex(`${prefix}-b-1`, 1, 0),
        vertex(`${prefix}-b-2`, 0, 1),
      ],
    },
  ] as const satisfies readonly FoldPreviewCollisionPoseFace[]
}

function pairedCenteredTriangles(prefix: string) {
  return [
    {
      id: `${prefix}-a`,
      polygon: [
        vertex(`${prefix}-a-0`, -1, -1),
        vertex(`${prefix}-a-1`, 1, -1),
        vertex(`${prefix}-a-2`, 0, 1),
      ],
    },
    {
      id: `${prefix}-b`,
      polygon: [
        vertex(`${prefix}-b-0`, -1, -1),
        vertex(`${prefix}-b-1`, 1, -1),
        vertex(`${prefix}-b-2`, 0, 1),
      ],
    },
  ] as const satisfies readonly FoldPreviewCollisionPoseFace[]
}

function identityTransforms(
  faces: readonly FoldPreviewCollisionPoseFace[],
) {
  return new Map(faces.map((face) => [face.id, new Matrix4()]))
}

function transformsFor(
  faces: readonly FoldPreviewCollisionPoseFace[],
  first: Matrix4,
  second: Matrix4,
) {
  assert.equal(faces.length, 2)
  return new Map([
    [faces[0]!.id, first],
    [faces[1]!.id, second],
  ])
}

function vertex(vertexId: string, x: number, z: number) {
  return { vertexId, x, z } as const
}

function axis(
  start: Readonly<{ x: number; z: number }>,
  end: Readonly<{ x: number; z: number }>,
) {
  const length = Math.hypot(end.x - start.x, end.z - start.z)
  return {
    x: (end.x - start.x) / length,
    z: (end.z - start.z) / length,
  }
}
