import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type { FoldPreviewCollisionAdjacency } from '../src/lib/foldPreviewCollision.ts'
import {
  prepareFoldPreviewHingeContactPolicy,
  type FoldPreviewHingeContactConstraint,
  type FoldPreviewHingePolicyFace,
} from '../src/lib/foldPreviewHingeCollision.ts'
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

test('hinge decisions are invariant under one shared non-trivial rigid world transform', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const world = new Matrix4()
    .makeRotationAxis(new Vector3(0.3, 0.7, 0.2).normalize(), 0.731)
    .setPosition(2.3, -1.7, 0.4)
  for (const degrees of [0, 60, 90]) {
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
  }
})

test('zero thickness and a flat-fold singularity stay explicitly indeterminate', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const zeroThickness = analyzer.analyze(identityPose(), 0)
  assert.ok(zeroThickness)
  assert.deepEqual(zeroThickness.interactions[0]?.hingeDecision, {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'zero_thickness',
  })

  const flatFold = analyzer.analyze(new Map([
    ['left', new Matrix4()],
    ['right', new Matrix4().makeRotationZ(Math.PI)],
  ]), 0.1)
  assert.ok(flatFold)
  assert.deepEqual(flatFold.interactions[0]?.hingeDecision, {
    kind: 'indeterminate',
    hingeEdgeIds: ['hinge'],
    reason: 'unsupported_flat_fold',
  })
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

test('hinge candidates scan every triangle pair after an early penetration', () => {
  const rectangleFaces = rectangleHingeFaces()
  const analyzer = prepareFoldPreviewNarrowPhase(
    rectangleFaces,
    adjacency,
    [constraint],
  )
  assert.ok(analyzer)
  const result = analyzer.analyze(foldedPose(60), 0.1)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
  assert.equal(result.trianglePairTests, 4)
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
  return new Matrix4().makeRotationZ(degrees * Math.PI / 180)
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
