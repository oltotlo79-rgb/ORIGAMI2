import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type { FoldPreviewCollisionAdjacency } from '../src/lib/foldPreviewCollision.ts'
import { makeFoldPreviewCanonicalAxisRotation } from '../src/lib/foldPreviewCanonicalRotation.ts'
import {
  prepareFoldPreviewHingeContactPolicy,
  type FoldPreviewHingeContactConstraint,
  type FoldPreviewHingePolicyFace,
  type FoldPreviewStaticHingeTrianglePairSupportRequest,
} from '../src/lib/foldPreviewHingeCollision.ts'
import { triangulateFoldPreviewPolygon } from '../src/lib/foldPreviewGeometry.ts'
import { prepareFoldPreviewNarrowPhase } from '../src/lib/foldPreviewNarrowCollision.ts'

const start = { vertexId: 'start', x: 0, z: 0 } as const
const end = { vertexId: 'end', x: 0, z: 1 } as const
const faces: readonly FoldPreviewHingePolicyFace[] = [
  {
    id: 'left',
    polygon: [
      start,
      { vertexId: 'left-tip', x: -1, z: 0.5 },
      end,
    ],
    triangles: [[0, 1, 2]],
  },
  {
    id: 'right',
    polygon: [
      end,
      { vertexId: 'right-tip', x: 1, z: 0.5 },
      start,
    ],
    triangles: [[0, 1, 2]],
  },
]
const outsideFaces: readonly FoldPreviewHingePolicyFace[] = [
  {
    id: 'left',
    polygon: [
      start,
      { vertexId: 'left-near', x: -1, z: 0.2 },
      { vertexId: 'left-far-a', x: 10, z: 0.3 },
      { vertexId: 'left-far-b', x: 10, z: 0.7 },
      end,
    ],
    triangles: [
      [0, 1, 4],
      [1, 2, 3],
      [1, 3, 4],
    ],
  },
  {
    id: 'right',
    polygon: [
      end,
      { vertexId: 'right-near', x: 1, z: 0.2 },
      { vertexId: 'right-far-a', x: 10, z: 0.3 },
      { vertexId: 'right-far-b', x: 10, z: 0.7 },
      start,
    ],
    triangles: [
      [0, 1, 4],
      [1, 2, 3],
      [1, 3, 4],
    ],
  },
]
const adjacency: readonly FoldPreviewCollisionAdjacency[] = [{
  edgeId: 'hinge',
  firstFaceId: 'left',
  secondFaceId: 'right',
}]
const constraint: FoldPreviewHingeContactConstraint = {
  edgeId: 'hinge',
  leftFaceId: 'left',
  rightFaceId: 'right',
  start,
  end,
  thicknessRule: 'centered_mid_surface_v1',
}

test('flat shared-edge contact is allowed only by the explicit centered hinge model', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const result = analyzer.analyze(identityPose(), 0.1)
  assert.ok(result)
  assert.deepEqual(result.interactions, [{
    firstFaceId: 'left',
    secondFaceId: 'right',
    relation: 'hinge_adjacent',
    hingeEdgeIds: ['hinge'],
    geometryClass: 'touching',
    hingeDecision: {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'boundary_contact',
      thicknessRule: 'centered_mid_surface_v1',
    },
  }])
})

test('ordinary 60 and 90 degree folds allow the analytic centered-slab overlap', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  for (const degrees of [60, 90]) {
    const result = analyzer.analyze(foldedPose(degrees), 0.1)
    assert.ok(result)
    assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
    assert.deepEqual(result.interactions[0]?.hingeDecision, {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'corridor_overlap',
      thicknessRule: 'centered_mid_surface_v1',
    })
  }
})

test('every representable positive micro-fold is distinct from the exact flat pose', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  for (const sign of [-1, 1]) {
    for (const degrees of [1e-12, 1e-10, 1e-8, 1e-6, 0.0005]) {
      const result = analyzer.analyze(foldedPose(sign * degrees), 0.1)
      assert.ok(result)
      assert.deepEqual(result.interactions[0]?.hingeDecision, {
        kind: 'allowed_by_hinge_model',
        hingeEdgeId: 'hinge',
        geometry: 'corridor_overlap',
        thicknessRule: 'centered_mid_surface_v1',
      })
    }
  }
})

test('hinge decisions are invariant under one shared non-trivial rigid world transform', () => {
  const rectangleFaces = rectangleHingeFaces()
  const analyzer = prepareFoldPreviewNarrowPhase(
    rectangleFaces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const world = new Matrix4()
    .makeRotationAxis(new Vector3(0.3, 0.7, 0.2).normalize(), 0.731)
    .setPosition(2.3, -1.7, 0.4)
  for (const degrees of [0, 1, 60, 90, 150, 170]) {
    const base = analyzer.analyze(foldedPose(degrees), 0.1)
    const transformed = analyzer.analyze(new Map([
      ['left', world.clone()],
      ['right', world.clone().multiply(hingeRotation(degrees))],
    ]), 0.1)
    assert.ok(base && transformed)
    assert.equal(
      transformed.interactions[0]?.geometryClass,
      base.interactions[0]?.geometryClass,
    )
    assert.deepEqual(
      transformed.interactions[0]?.hingeDecision,
      base.interactions[0]?.hingeDecision,
    )
    assert.equal(base.trianglePairTests, 4)
    assert.equal(transformed.trianglePairTests, 4)
  }
})

test('zero thickness distinguishes shared-edge contact and a flat surface stack', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const zeroThickness = analyzer.analyze(identityPose(), 0)
  assert.ok(zeroThickness)
  assert.deepEqual(zeroThickness.interactions[0]?.hingeDecision, {
    kind: 'allowed_by_hinge_model',
    hingeEdgeId: 'hinge',
    geometry: 'boundary_contact',
    thicknessRule: 'centered_mid_surface_v1',
  })

  const zeroThicknessFlatFold = analyzer.analyze(foldedPose(180), 0)
  assert.ok(zeroThicknessFlatFold)
  assert.deepEqual(
    zeroThicknessFlatFold.interactions[0]?.hingeDecision,
    {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'flat_surface_stack',
      thicknessRule: 'centered_mid_surface_v1',
    },
  )

  const thickFlatFold = analyzer.analyze(foldedPose(180), 0.1)
  assert.ok(thickFlatFold)
  assert.deepEqual(thickFlatFold.interactions[0]?.hingeDecision, {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'layer_offset_unmodeled',
  })
})

test('an exact anti-parallel pose still requires positive-area overlap evidence', () => {
  const policy = prepareFoldPreviewHingeContactPolicy(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const pose = foldedPose(180)
  const leftTransform = pose.get('left')
  const rightTransform = pose.get('right')
  assert.ok(leftTransform && rightTransform)
  const common = {
    firstFaceId: 'left',
    secondFaceId: 'right',
    hingeEdgeIds: ['hinge'],
    faceTransforms: pose,
    thickness: 0,
    numericalMargin: Number.EPSILON * 256,
    testedTrianglePairs: 1,
  } as const

  for (const geometryClass of ['touching', 'indeterminate'] as const) {
    assert.deepEqual(policy.classify({
      ...common,
      pairs: [{
        firstTriangleIndex: 0,
        secondTriangleIndex: 0,
        firstVertices: prismFor(faces[0], 0, leftTransform, 0),
        secondVertices: prismFor(faces[1], 0, rightTransform, 0),
        geometryClass,
      }],
    }), {
      kind: 'indeterminate',
      hingeEdgeIds: ['hinge'],
      reason: 'numerical_geometry',
    })
  }
})

test('forged prism witnesses cannot obtain a hinge-model decision', () => {
  const policy = prepareFoldPreviewHingeContactPolicy(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const outside = prismAt(10)
  assert.deepEqual(policy.classify({
    firstFaceId: 'left',
    secondFaceId: 'right',
    hingeEdgeIds: ['hinge'],
    faceTransforms: identityPose(),
    thickness: 0.1,
    numericalMargin: Number.EPSILON * 256,
    testedTrianglePairs: 1,
    pairs: [{
      firstTriangleIndex: 0,
      secondTriangleIndex: 0,
      firstVertices: outside,
      secondVertices: outside.map((point) => point.clone()),
      geometryClass: 'penetrating',
    }],
  }), {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'pair_geometry_mismatch',
  })
})

test('flat penetration contradictions, incomplete scans, and multiple hinges fail closed', () => {
  const policy = prepareFoldPreviewHingeContactPolicy(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const common = {
    firstFaceId: 'left',
    secondFaceId: 'right',
    faceTransforms: identityPose(),
    thickness: 0.1,
    numericalMargin: Number.EPSILON * 256,
    testedTrianglePairs: 1,
  } as const
  const leftPrism = prismFor(faces[0], 0, new Matrix4(), 0.1)
  const rightPrism = prismFor(faces[1], 0, new Matrix4(), 0.1)
  assert.deepEqual(policy.classify({
    ...common,
    hingeEdgeIds: ['hinge'],
    pairs: [{
      firstTriangleIndex: 0,
      secondTriangleIndex: 0,
      firstVertices: leftPrism,
      secondVertices: rightPrism,
      geometryClass: 'penetrating',
    }],
  }), {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'flat_pose_penetration',
  })
  assert.deepEqual(policy.classify({
    ...common,
    hingeEdgeIds: ['hinge'],
    testedTrianglePairs: 0,
    pairs: [],
  }), {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'incomplete_pair_scan',
  })
  assert.deepEqual(policy.classify({
    ...common,
    hingeEdgeIds: ['hinge', 'second-hinge'],
    pairs: [],
  }), {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge', 'second-hinge'],
    reason: 'multiple_shared_hinges',
  })
})

test('an authentic non-hinge prism overlap outside the finite corridor is blocking', () => {
  const policy = prepareFoldPreviewHingeContactPolicy(
    outsideFaces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const leftHinge = prismFor(outsideFaces[0], 0, new Matrix4(), 0.1)
  const rightHinge = prismFor(outsideFaces[1], 0, new Matrix4(), 0.1)
  const leftOutside = prismFor(outsideFaces[0], 1, new Matrix4(), 0.1)
  const rightOutside = prismFor(outsideFaces[1], 1, new Matrix4(), 0.1)
  assert.deepEqual(policy.classify({
    firstFaceId: 'left',
    secondFaceId: 'right',
    hingeEdgeIds: ['hinge'],
    faceTransforms: identityPose(),
    thickness: 0.1,
    numericalMargin: Number.EPSILON * 256,
    testedTrianglePairs: 9,
    pairs: [
      {
        firstTriangleIndex: 0,
        secondTriangleIndex: 0,
        firstVertices: leftHinge,
        secondVertices: rightHinge,
        geometryClass: 'touching',
      },
      {
        firstTriangleIndex: 1,
        secondTriangleIndex: 1,
        firstVertices: leftOutside,
        secondVertices: rightOutside,
        geometryClass: 'penetrating',
      },
    ],
  }), {
    kind: 'outside_hinge_penetration',
    hingeEdgeId: 'hinge',
  })
})

test('a concave lobe crossing beyond a hinge endpoint remains blocking', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    concaveOutsideHingeFaces(),
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const result = analyzer.analyze(identityPose(), 0.1)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
  assert.deepEqual(result.interactions[0]?.hingeDecision, {
    kind: 'outside_hinge_penetration',
    hingeEdgeId: 'hinge',
  })
})

test('non-hinge indeterminate pairs and duplicate pair witnesses remain unresolved', () => {
  const policy = prepareFoldPreviewHingeContactPolicy(
    outsideFaces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const leftOutside = prismFor(outsideFaces[0], 1, new Matrix4(), 0.1)
  const rightOutside = prismFor(outsideFaces[1], 1, new Matrix4(), 0.1)
  const pair = {
    firstTriangleIndex: 1,
    secondTriangleIndex: 1,
    firstVertices: leftOutside,
    secondVertices: rightOutside,
    geometryClass: 'indeterminate' as const,
  }
  const common = {
    firstFaceId: 'left',
    secondFaceId: 'right',
    hingeEdgeIds: ['hinge'],
    faceTransforms: identityPose(),
    thickness: 0.1,
    numericalMargin: Number.EPSILON * 256,
    testedTrianglePairs: 9,
  } as const
  assert.deepEqual(policy.classify({
    ...common,
    pairs: [pair],
  }), {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'numerical_geometry',
  })
  const hingePair = {
    firstTriangleIndex: 0,
    secondTriangleIndex: 0,
    firstVertices: prismFor(outsideFaces[0], 0, new Matrix4(), 0.1),
    secondVertices: prismFor(outsideFaces[1], 0, new Matrix4(), 0.1),
    geometryClass: 'touching' as const,
  }
  assert.deepEqual(policy.classify({
    ...common,
    pairs: [hingePair, hingePair],
  }), {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'pair_geometry_mismatch',
  })
})

test('rectangular hinge faces allow every pair at ordinary and near-limit angles', () => {
  const rectangleFaces = rectangleHingeFaces()
  const analyzer = prepareFoldPreviewNarrowPhase(
    rectangleFaces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  for (const degrees of [0, 1, 60, 90, 150, 170]) {
    const result = analyzer.analyze(foldedPose(degrees), 0.1)
    assert.ok(result)
    assert.equal(
      result.interactions[0]?.geometryClass,
      degrees === 0 ? 'touching' : 'penetrating',
    )
    assert.deepEqual(result.interactions[0]?.hingeDecision, {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: degrees === 0 ? 'boundary_contact' : 'corridor_overlap',
      thicknessRule: 'centered_mid_surface_v1',
    })
    assert.equal(result.trianglePairTests, 4)
    assert.equal(result.satTests, 4)
  }
})

test('static finite-hinge support proves all rectangle pairs in either face order', () => {
  const rectangleFaces = triangulatedPolicyFaces(rectangleHingeFaces())
  const policy = prepareFoldPreviewHingeContactPolicy(
    rectangleFaces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  for (let leftIndex = 0; leftIndex < 2; leftIndex += 1) {
    for (let rightIndex = 0; rightIndex < 2; rightIndex += 1) {
      const expected = {
        kind: 'proven_static_hinge_support',
        hingeEdgeId: 'hinge',
        thicknessRule: 'centered_mid_surface_v1',
      } as const
      assert.deepEqual(policy.proveStaticTrianglePairSupport({
        firstFaceId: 'left',
        secondFaceId: 'right',
        hingeEdgeIds: ['hinge'],
        firstTriangleIndex: leftIndex,
        secondTriangleIndex: rightIndex,
      }), expected)
      assert.deepEqual(policy.proveStaticTrianglePairSupport({
        firstFaceId: 'right',
        secondFaceId: 'left',
        hingeEdgeIds: ['hinge'],
        firstTriangleIndex: rightIndex,
        secondTriangleIndex: leftIndex,
      }), expected)
    }
  }
})

test('static finite-hinge support fails closed for malformed pair requests', () => {
  const rectangleFaces = triangulatedPolicyFaces(rectangleHingeFaces())
  const policy = prepareFoldPreviewHingeContactPolicy(
    rectangleFaces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const common = {
    firstFaceId: 'left',
    secondFaceId: 'right',
    hingeEdgeIds: ['hinge'],
    firstTriangleIndex: 0,
    secondTriangleIndex: 0,
  } as const
  const cases: readonly [
    FoldPreviewStaticHingeTrianglePairSupportRequest,
    string,
  ][] = [
    [{ ...common, hingeEdgeIds: [] }, 'missing_constraint'],
    [{
      ...common,
      hingeEdgeIds: ['hinge', 'another-hinge'],
    }, 'multiple_shared_hinges'],
    [{ ...common, hingeEdgeIds: ['missing'] }, 'missing_constraint'],
    [{ ...common, firstFaceId: 'missing' }, 'face_pair_mismatch'],
    [{ ...common, secondFaceId: 'left' }, 'face_pair_mismatch'],
    [{ ...common, firstTriangleIndex: -1 }, 'triangle_index_out_of_range'],
    [{ ...common, firstTriangleIndex: 0.5 }, 'triangle_index_out_of_range'],
    [{ ...common, secondTriangleIndex: 2 }, 'triangle_index_out_of_range'],
  ]
  for (const [request, reason] of cases) {
    const decision = policy.proveStaticTrianglePairSupport(request)
    assert.equal(decision.kind, 'not_proven')
    assert.equal(decision.kind === 'not_proven' && decision.reason, reason)
  }

  assert.deepEqual(policy.proveStaticTrianglePairSupport(
    null as unknown as FoldPreviewStaticHingeTrianglePairSupportRequest,
  ), {
    kind: 'not_proven',
    hingeEdgeIds: [],
    reason: 'malformed_request',
  })
})

test('static finite-hinge support does not prove every endpoint-outside concave pair', () => {
  const concaveFaces = triangulatedPolicyFaces(concaveOutsideHingeFaces())
  const policy = prepareFoldPreviewHingeContactPolicy(
    concaveFaces,
    adjacency,
    [constraint],
  )
  assert.ok(policy)
  const decisions = concaveFaces[0].triangles.flatMap((_, firstTriangleIndex) =>
    concaveFaces[1].triangles.map((__, secondTriangleIndex) =>
      policy.proveStaticTrianglePairSupport({
        firstFaceId: 'left',
        secondFaceId: 'right',
        hingeEdgeIds: ['hinge'],
        firstTriangleIndex,
        secondTriangleIndex,
      })))
  assert.ok(decisions.some((decision) =>
    decision.kind === 'not_proven'
    && (
      decision.reason === 'material_half_slabs_not_proven'
      || decision.reason === 'finite_hinge_segment_not_proven'
    )))
})

test('static finite-hinge support uses immutable prepared proofs', () => {
  const mutableFaces = triangulatedPolicyFaces(rectangleHingeFaces())
    .map((face) => ({
      id: face.id,
      polygon: face.polygon.map((point) => ({ ...point })),
      triangles: face.triangles.map((triangle) => [...triangle]),
    }))
  const mutableConstraint = {
    ...constraint,
    start: { ...constraint.start },
    end: { ...constraint.end },
  }
  const policy = prepareFoldPreviewHingeContactPolicy(
    mutableFaces,
    adjacency,
    [mutableConstraint],
  )
  assert.ok(policy)
  const request = {
    firstFaceId: 'left',
    secondFaceId: 'right',
    hingeEdgeIds: ['hinge'],
    firstTriangleIndex: 1,
    secondTriangleIndex: 1,
  } as const
  const expected = policy.proveStaticTrianglePairSupport(request)
  assert.equal(expected.kind, 'proven_static_hinge_support')

  mutableFaces[0].polygon[0].x = 100
  mutableFaces[0].triangles[1][0] = 99
  mutableFaces[1].polygon[2].z = 100
  mutableFaces[1].triangles.reverse()
  mutableConstraint.edgeId = 'mutated-hinge'
  mutableConstraint.start.x = 100
  mutableConstraint.end.z = 100

  assert.deepEqual(policy.proveStaticTrianglePairSupport(request), expected)
})

test('prepared hinge geometry and constraints are immutable snapshots', () => {
  const mutableFaces = faces.map((face) => ({
    id: face.id,
    polygon: face.polygon.map((point) => ({ ...point })),
  }))
  const mutableAdjacency = adjacency.map((item) => ({ ...item }))
  const mutableConstraint = {
    ...constraint,
    start: { ...constraint.start },
    end: { ...constraint.end },
  }
  const analyzer = prepareFoldPreviewNarrowPhase(
    mutableFaces,
    mutableAdjacency,
    [mutableConstraint],
  )
  assert.ok(analyzer)
  const pose = foldedPose(60)
  const expected = analyzer.analyze(pose, 0.1)
  assert.ok(expected)

  mutableFaces[0].id = 'mutated-left'
  mutableFaces[0].polygon[0].x = 100
  mutableFaces[0].polygon[0].vertexId = 'mutated-start'
  mutableFaces[1].polygon.reverse()
  mutableAdjacency[0].edgeId = 'mutated-hinge'
  mutableConstraint.edgeId = 'mutated-hinge'
  mutableConstraint.start.x = 100
  mutableConstraint.end.vertexId = 'mutated-end'

  assert.deepEqual(analyzer.analyze(pose, 0.1), expected)
})

test('constraint preparation rejects incomplete, forged, and misoriented snapshots', () => {
  assert.equal(prepareFoldPreviewHingeContactPolicy(
    faces,
    adjacency,
    [],
  ), null)
  assert.equal(prepareFoldPreviewHingeContactPolicy(
    faces,
    adjacency,
    [{ ...constraint, rightFaceId: 'missing' }],
  ), null)
  assert.equal(prepareFoldPreviewHingeContactPolicy(
    faces,
    adjacency,
    [{
      ...constraint,
      start: { ...start, vertexId: 'forged' },
    }],
  ), null)
  assert.equal(prepareFoldPreviewHingeContactPolicy(
    [
      faces[0],
      {
        ...faces[1],
        polygon: [...faces[1].polygon].reverse(),
      },
    ],
    adjacency,
    [constraint],
  ), null)
  assert.equal(prepareFoldPreviewHingeContactPolicy(
    [
      faces[0],
      {
        ...faces[1],
        polygon: [
          end,
          { vertexId: 'same-side-tip', x: -1, z: 0.5 },
          start,
        ],
      },
    ],
    adjacency,
    [constraint],
  ), null)
})

function identityPose() {
  return new Map([
    ['left', new Matrix4()],
    ['right', new Matrix4()],
  ])
}

function hingeRotation(degrees: number) {
  const rotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(0, 0, 1),
    degrees * Math.PI / 180,
  )
  assert.ok(rotation)
  return rotation
}

function foldedPose(degrees: number) {
  return new Map([
    ['left', new Matrix4()],
    ['right', hingeRotation(degrees)],
  ])
}

function prismFor(
  face: FoldPreviewHingePolicyFace,
  triangleIndex: number,
  transform: Matrix4,
  thickness: number,
) {
  const triangle = face.triangles[triangleIndex]
  if (!triangle) throw new RangeError('missing test triangle')
  const halfThickness = thickness / 2
  return [
    ...triangle.map((index) => new Vector3(
      face.polygon[index].x,
      halfThickness,
      face.polygon[index].z,
    ).applyMatrix4(transform)),
    ...triangle.map((index) => new Vector3(
      face.polygon[index].x,
      -halfThickness,
      face.polygon[index].z,
    ).applyMatrix4(transform)),
  ]
}

function rectangleHingeFaces() {
  return [
    {
      id: 'left',
      polygon: [
        start,
        { vertexId: 'left-bottom', x: -1, z: 0 },
        { vertexId: 'left-top', x: -1, z: 1 },
        end,
      ],
    },
    {
      id: 'right',
      polygon: [
        end,
        { vertexId: 'right-top', x: 1, z: 1 },
        { vertexId: 'right-bottom', x: 1, z: 0 },
        start,
      ],
    },
  ]
}

function triangulatedPolicyFaces(
  sourceFaces: readonly {
    id: string
    polygon: readonly FoldPreviewHingePolicyFace['polygon'][number][]
  }[],
): FoldPreviewHingePolicyFace[] {
  return sourceFaces.map((face) => ({
    id: face.id,
    polygon: face.polygon,
    triangles: triangulateFoldPreviewPolygon(face.polygon),
  }))
}

function concaveOutsideHingeFaces() {
  return [
    {
      id: 'left',
      polygon: [
        start,
        { vertexId: 'left-bottom', x: -1, z: 0 },
        { vertexId: 'left-outer-top', x: -1, z: 2 },
        { vertexId: 'left-lobe-top', x: 2, z: 2 },
        { vertexId: 'left-lobe-bottom', x: 2, z: 1.5 },
        { vertexId: 'left-notch', x: -0.5, z: 1.5 },
        end,
      ],
    },
    {
      id: 'right',
      polygon: [
        end,
        { vertexId: 'right-notch', x: 0.5, z: 1.5 },
        { vertexId: 'right-lobe-left', x: 0.5, z: 2 },
        { vertexId: 'right-lobe-right', x: 2, z: 2 },
        { vertexId: 'right-bottom', x: 2, z: 0 },
        start,
      ],
    },
  ]
}

function prismAt(x: number) {
  return [
    new Vector3(x, -0.01, 0.2),
    new Vector3(x + 0.01, -0.01, 0.2),
    new Vector3(x, -0.01, 0.8),
    new Vector3(x, 0.01, 0.2),
    new Vector3(x + 0.01, 0.01, 0.2),
    new Vector3(x, 0.01, 0.8),
  ]
}
