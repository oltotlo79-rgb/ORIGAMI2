import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  summarizeFoldPreviewCollision,
} from '../src/lib/foldPreviewCollisionPresentation.ts'
import type {
  FoldPreviewHingeContactConstraint,
} from '../src/lib/foldPreviewHingeCollision.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewTreeKinematics,
} from '../src/lib/foldPreviewKinematics.ts'
import {
  findFoldPreviewNarrowPhaseInteractions,
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewFullScanNonAdjacentWitnessJob,
  type FoldPreviewNarrowPhaseInteraction,
  type FoldPreviewNarrowPhaseAnalysisJob,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const thicknesses = [0, 0.1, 1, 3] as const
const sharedVertexAngles = [10, 45, 90, 91, 135, 179] as const
const mountainMountainAngles = [...sharedVertexAngles, 180] as const

const cornerMountainValley = cornerMountainValleyFixture()
const midpointMountainMountain = midpointMountainMountainFixture()

test('corner-start mountain-valley V has a complete 4 by 6 topology grid', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    cornerMountainValley.faces,
    cornerMountainValley.adjacencies,
    cornerMountainValley.constraints,
  )
  assert.ok(analyzer)

  for (const thickness of thicknesses) {
    for (const degrees of sharedVertexAngles) {
      const pose = poseAt(cornerMountainValley, degrees)
      const result = analyzer.analyze(pose.faceTransforms, thickness)
      assert.ok(result)
      const diagnostic = `${thickness} mm at ${degrees} degrees`
      const outer = outerInteraction(result.interactions, 'corner')
      assert.ok(outer, diagnostic)
      assert.equal(outer.relation, 'non_adjacent', diagnostic)
      assert.equal(outer.geometryClass, 'touching', diagnostic)
      assert.equal(outer.topologyContact?.exclusive, true, diagnostic)
      assert.equal(
        outer.topologyContact?.decision,
        'allowed_shared_vertex_contact',
        diagnostic,
      )
      assert.deepEqual(
        outer.topologyContact?.sharedVertexIds,
        ['corner-apex'],
        diagnostic,
      )

      const summary = summarizeFoldPreviewCollision(result)
      assert.equal(summary.nonAdjacentPenetrations, 0, diagnostic)
      assert.equal(summary.nonAdjacentContacts, 0, diagnostic)
      assert.equal(
        summary.nonAdjacentAllowedSharedVertexContacts,
        1,
        diagnostic,
      )
      assert.equal(summary.indeterminateInteractions, 0, diagnostic)
    }
  }
})

test('midpoint mountain-mountain V has a complete 4 by 7 topology grid', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    midpointMountainMountain.faces,
    midpointMountainMountain.adjacencies,
    midpointMountainMountain.constraints,
  )
  assert.ok(analyzer)

  for (const thickness of thicknesses) {
    for (const degrees of mountainMountainAngles) {
      const pose = poseAt(midpointMountainMountain, degrees)
      const result = analyzer.analyze(pose.faceTransforms, thickness)
      assert.ok(result)
      const diagnostic = `${thickness} mm at ${degrees} degrees`
      const outer = outerInteraction(result.interactions, 'midpoint')
      assert.ok(outer, diagnostic)
      assert.equal(outer.relation, 'non_adjacent', diagnostic)
      const allowedSharedVertex = degrees === 10 || degrees === 45
      const blockingIndeterminate = degrees === 90 || degrees === 91
      assert.equal(
        outer.geometryClass,
        allowedSharedVertex
          ? 'touching'
          : blockingIndeterminate
            ? 'indeterminate'
            : 'penetrating',
        diagnostic,
      )
      assert.equal(
        outer.topologyContact?.exclusive ?? false,
        allowedSharedVertex,
        diagnostic,
      )
      assert.equal(
        outer.topologyContact?.decision,
        allowedSharedVertex ? 'allowed_shared_vertex_contact' : undefined,
        diagnostic,
      )
      if (allowedSharedVertex) {
        assert.deepEqual(
          outer.topologyContact?.sharedVertexIds,
          ['midpoint-apex'],
          diagnostic,
        )
      } else {
        assert.equal(outer.topologyContact, undefined, diagnostic)
      }

      const summary = summarizeFoldPreviewCollision(result)
      assert.equal(
        summary.nonAdjacentPenetrations,
        allowedSharedVertex || blockingIndeterminate ? 0 : 1,
        diagnostic,
      )
      assert.equal(summary.nonAdjacentContacts, 0, diagnostic)
      assert.equal(
        summary.nonAdjacentAllowedSharedVertexContacts,
        allowedSharedVertex ? 1 : 0,
        diagnostic,
      )
      assert.equal(
        summary.indeterminateInteractions,
        Number(blockingIndeterminate)
          + Number(degrees === 180 && thickness > 0) * 2,
        diagnostic,
      )
    }
  }
})

test('face input order preserves the topology boundary classifications', () => {
  const representatives = [
    {
      fixture: cornerMountainValley,
      degrees: 91,
    },
    {
      fixture: midpointMountainMountain,
      degrees: 91,
    },
    {
      fixture: midpointMountainMountain,
      degrees: 135,
    },
    {
      fixture: midpointMountainMountain,
      degrees: 180,
    },
  ] as const

  for (const current of representatives) {
    const forward = prepareFoldPreviewNarrowPhase(
      current.fixture.faces,
      current.fixture.adjacencies,
      current.fixture.constraints,
    )
    const reversed = prepareFoldPreviewNarrowPhase(
      [...current.fixture.faces].reverse(),
      current.fixture.adjacencies,
      current.fixture.constraints,
    )
    assert.ok(forward && reversed)
    const pose = poseAt(current.fixture, current.degrees)
    for (const thickness of thicknesses) {
      assert.deepEqual(
        reversed.analyze(pose.faceTransforms, thickness),
        forward.analyze(pose.faceTransforms, thickness),
        `${current.fixture.prefix}:${thickness}:${current.degrees}`,
      )
    }
  }
})

test('shared-vertex capabilities reject cloned or mismatched provenance', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    cornerMountainValley.faces,
    cornerMountainValley.adjacencies,
    cornerMountainValley.constraints,
  )
  assert.ok(analyzer)
  const pose = poseAt(cornerMountainValley, 45)
  const baseline = analyzer.analyze(pose.faceTransforms, 1)
  assert.ok(baseline)
  assert.equal(
    outerInteraction(baseline.interactions, 'corner')?.geometryClass,
    'touching',
  )

  for (const fault of [
    'cloned_certificate',
    'swapped_prisms',
    'different_prism',
    'different_raw_class',
  ] as const) {
    const originalFreeze = Object.freeze
    Object.freeze = ((value: object) => {
      if (!value || typeof value !== 'object') return originalFreeze(value)
      const record = value as Record<string, unknown>
      const isGeometryProvenance = record.first
        && record.second
        && 'thicknessClass' in record
        && !('geometryCertificate' in record)
        && typeof record.first === 'object'
        && record.first !== null
        && 'triangleIndex' in record.first
      const isRuntimeProvenance = record.geometryCertificate
        && record.first
        && record.second
        && 'rawGeometryClass' in record

      if (
        fault === 'swapped_prisms'
        && isGeometryProvenance
      ) {
        return originalFreeze({
          ...record,
          first: record.second,
          second: record.first,
        })
      }
      if (
        fault === 'different_prism'
        && isGeometryProvenance
      ) {
        return originalFreeze({
          ...record,
          first: originalFreeze({
            ...(record.first as Record<string, unknown>),
          }),
        })
      }
      if (
        fault === 'cloned_certificate'
        && isRuntimeProvenance
      ) {
        return originalFreeze({
          ...record,
          geometryCertificate: originalFreeze({
            ...(record.geometryCertificate as Record<string, unknown>),
          }),
        })
      }
      if (
        fault === 'different_raw_class'
        && isRuntimeProvenance
      ) {
        return originalFreeze({
          ...record,
          rawGeometryClass: record.rawGeometryClass === 'touching'
            ? 'penetrating'
            : 'touching',
        })
      }
      return originalFreeze(value)
    }) as typeof Object.freeze
    try {
      const result = analyzer.analyze(pose.faceTransforms, 1)
      assert.ok(result, fault)
      const outer = outerInteraction(result.interactions, 'corner')
      assert.ok(outer, fault)
      assert.equal(outer.geometryClass, 'indeterminate', fault)
      assert.equal(outer.topologyContact, undefined, fault)
      assert.equal(
        summarizeFoldPreviewCollision(result).indeterminateInteractions,
        1,
        fault,
      )
    } finally {
      Object.freeze = originalFreeze
    }
  }
})

test('a disconnected shared vertex is blocking outside the broad phase', () => {
  const shared = { vertexId: 'disconnected-shared', x: 0, z: 0 } as const
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'disconnected-left',
      polygon: [
        shared,
        { vertexId: 'disconnected-left-a', x: -2, z: -1 },
        { vertexId: 'disconnected-left-b', x: -2, z: 1 },
      ],
    },
    {
      id: 'disconnected-right',
      polygon: [
        shared,
        { vertexId: 'disconnected-right-a', x: 2, z: -1 },
        { vertexId: 'disconnected-right-b', x: 2, z: 1 },
      ],
    },
  ]
  const transforms = new Map([
    ['disconnected-left', new Matrix4()],
    ['disconnected-right', new Matrix4().makeTranslation(0, 100, 0)],
  ])

  for (const reverseFaces of [false, true]) {
    const orderedFaces = reverseFaces ? [...faces].reverse() : faces
    const analyzer = prepareFoldPreviewNarrowPhase(orderedFaces, [])
    assert.ok(analyzer)
    for (const thickness of [0, 0.1, 3]) {
      const result = analyzer.analyze(transforms, thickness)
      assert.ok(result)
      assert.equal(result.broadPhaseCandidates, 0)
      assert.equal(result.trianglePairTests, 0)
      assert.equal(result.satTests, 0)
      assert.equal(result.interactions.length, 1)
      const interaction = result.interactions[0]
      assert.deepEqual(interaction, {
        firstFaceId: 'disconnected-left',
        secondFaceId: 'disconnected-right',
        relation: 'non_adjacent',
        hingeEdgeIds: [],
        geometryClass: 'indeterminate',
      })
      assert.equal(
        summarizeFoldPreviewCollision(result).indeterminateInteractions,
        1,
      )

      const job = analyzer.createAnalysisJob(transforms, thickness)
      assert.ok(job)
      assert.deepEqual(drainAnalysisJob(job), result)
      assert.deepEqual(
        findFoldPreviewNarrowPhaseInteractions(
          orderedFaces,
          transforms,
          thickness,
          [],
        ),
        result,
      )
      if (thickness > 0) {
        assert.equal(
          analyzer.collectFullScanNonAdjacentWitnessSet(
            transforms,
            thickness,
          ),
          null,
        )
      }
    }
  }
})

test('a disconnected shared hinge is blocking outside the broad phase', () => {
  const start = {
    vertexId: 'disconnected-hinge-start',
    x: 0,
    z: -1,
  } as const
  const end = {
    vertexId: 'disconnected-hinge-end',
    x: 0,
    z: 1,
  } as const
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'disconnected-hinge-left',
      polygon: [
        start,
        { vertexId: 'disconnected-hinge-left-tip', x: -2, z: 0 },
        end,
      ],
    },
    {
      id: 'disconnected-hinge-right',
      polygon: [
        end,
        { vertexId: 'disconnected-hinge-right-tip', x: 2, z: 0 },
        start,
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'disconnected-hinge-edge',
    firstFaceId: 'disconnected-hinge-left',
    secondFaceId: 'disconnected-hinge-right',
  }]
  const constraints: readonly FoldPreviewHingeContactConstraint[] = [{
    edgeId: 'disconnected-hinge-edge',
    leftFaceId: 'disconnected-hinge-left',
    rightFaceId: 'disconnected-hinge-right',
    start,
    end,
    thicknessRule: 'centered_mid_surface_v1',
  }]
  const transforms = new Map([
    ['disconnected-hinge-left', new Matrix4()],
    [
      'disconnected-hinge-right',
      new Matrix4().makeTranslation(0, 100, 0),
    ],
  ])

  for (const reverseFaces of [false, true]) {
    const analyzer = prepareFoldPreviewNarrowPhase(
      reverseFaces ? [...faces].reverse() : faces,
      adjacencies,
      constraints,
    )
    assert.ok(analyzer)
    for (const thickness of [0, 0.1, 3]) {
      const result = analyzer.analyze(transforms, thickness)
      assert.ok(result)
      assert.equal(result.broadPhaseCandidates, 0)
      assert.equal(result.trianglePairTests, 0)
      assert.equal(result.satTests, 0)
      assert.equal(result.interactions.length, 1)
      assert.deepEqual(result.interactions[0], {
        firstFaceId: 'disconnected-hinge-left',
        secondFaceId: 'disconnected-hinge-right',
        relation: 'hinge_adjacent',
        hingeEdgeIds: ['disconnected-hinge-edge'],
        geometryClass: 'indeterminate',
        hingeDecision: {
          kind: 'indeterminate',
          hingeEdgeIds: ['disconnected-hinge-edge'],
          reason: 'pose_mismatch',
        },
      })
      assert.equal(
        summarizeFoldPreviewCollision(result).indeterminateInteractions,
        1,
      )
      const job = analyzer.createAnalysisJob(transforms, thickness)
      assert.ok(job)
      assert.deepEqual(drainAnalysisJob(job), result)
      if (thickness > 0) {
        assert.equal(
          analyzer.collectFullScanNonAdjacentWitnessSet(
            transforms,
            thickness,
          ),
          null,
        )
      }
    }
  }
})

test('an adjacency without shared vertex IDs is blocking outside the broad phase', () => {
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'untrusted-adjacency-left',
      polygon: [
        { vertexId: 'untrusted-left-start', x: 0, z: -1 },
        { vertexId: 'untrusted-left-tip', x: -2, z: 0 },
        { vertexId: 'untrusted-left-end', x: 0, z: 1 },
      ],
    },
    {
      id: 'untrusted-adjacency-right',
      polygon: [
        { vertexId: 'untrusted-right-end', x: 0, z: 1 },
        { vertexId: 'untrusted-right-tip', x: 2, z: 0 },
        { vertexId: 'untrusted-right-start', x: 0, z: -1 },
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'untrusted-adjacency-edge',
    firstFaceId: 'untrusted-adjacency-left',
    secondFaceId: 'untrusted-adjacency-right',
  }]
  const transforms = new Map([
    ['untrusted-adjacency-left', new Matrix4()],
    [
      'untrusted-adjacency-right',
      new Matrix4().makeTranslation(0, 100, 0),
    ],
  ])

  for (const reverseFaces of [false, true]) {
    const orderedFaces = reverseFaces ? [...faces].reverse() : faces
    const analyzer = prepareFoldPreviewNarrowPhase(orderedFaces, adjacencies)
    assert.ok(analyzer)
    for (const thickness of [0, 0.1, 3]) {
      const result = analyzer.analyze(transforms, thickness)
      assert.ok(result)
      assert.equal(result.broadPhaseCandidates, 0)
      assert.equal(result.trianglePairTests, 0)
      assert.equal(result.satTests, 0)
      assert.deepEqual(result.interactions, [{
        firstFaceId: 'untrusted-adjacency-left',
        secondFaceId: 'untrusted-adjacency-right',
        relation: 'hinge_adjacent',
        hingeEdgeIds: ['untrusted-adjacency-edge'],
        geometryClass: 'indeterminate',
        hingeDecision: {
          kind: 'indeterminate',
          hingeEdgeIds: ['untrusted-adjacency-edge'],
          reason: 'pose_mismatch',
        },
      }])

      const job = analyzer.createAnalysisJob(transforms, thickness)
      assert.ok(job)
      assert.deepEqual(drainAnalysisJob(job), result)
      assert.deepEqual(
        findFoldPreviewNarrowPhaseInteractions(
          orderedFaces,
          transforms,
          thickness,
          adjacencies,
        ),
        result,
      )
      assert.equal(
        analyzer.collectFullScanNonAdjacentWitnessSet(
          transforms,
          thickness,
        ),
        null,
      )
    }
  }
})

test('a separated hinge candidate remains blocking', () => {
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'separated-candidate-left',
      polygon: [
        { vertexId: 'separated-left-a', x: 0, z: 0 },
        { vertexId: 'separated-left-b', x: 2, z: 0 },
        { vertexId: 'separated-left-c', x: 0, z: 2 },
      ],
    },
    {
      id: 'separated-candidate-right',
      polygon: [
        { vertexId: 'separated-right-a', x: 3, z: 3 },
        { vertexId: 'separated-right-b', x: 3, z: 1.5 },
        { vertexId: 'separated-right-c', x: 1.5, z: 3 },
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'separated-candidate-edge',
    firstFaceId: 'separated-candidate-left',
    secondFaceId: 'separated-candidate-right',
  }]
  const transforms = new Map([
    ['separated-candidate-left', new Matrix4()],
    ['separated-candidate-right', new Matrix4()],
  ])

  for (const reverseFaces of [false, true]) {
    const orderedFaces = reverseFaces ? [...faces].reverse() : faces
    const analyzer = prepareFoldPreviewNarrowPhase(orderedFaces, adjacencies)
    assert.ok(analyzer)
    for (const thickness of [0, 0.1, 3]) {
      const result = analyzer.analyze(transforms, thickness)
      assert.ok(result)
      assert.equal(result.broadPhaseCandidates, 1)
      assert.equal(result.broadPhaseHingeAdjacentCandidates, 1)
      assert.equal(result.trianglePairTests, 1)
      assert.equal(result.satTests, 1)
      assert.deepEqual(result.interactions, [{
        firstFaceId: 'separated-candidate-left',
        secondFaceId: 'separated-candidate-right',
        relation: 'hinge_adjacent',
        hingeEdgeIds: ['separated-candidate-edge'],
        geometryClass: 'indeterminate',
        hingeDecision: {
          kind: 'indeterminate',
          hingeEdgeIds: ['separated-candidate-edge'],
          reason: 'missing_constraint',
        },
      }])

      const job = analyzer.createAnalysisJob(transforms, thickness)
      assert.ok(job)
      assert.deepEqual(drainAnalysisJob(job), result)
      assert.deepEqual(
        findFoldPreviewNarrowPhaseInteractions(
          orderedFaces,
          transforms,
          thickness,
          adjacencies,
        ),
        result,
      )
    }
  }
})

test('shared-hinge contact without a hinge constraint fails closed', () => {
  const start = {
    vertexId: 'missing-constraint-start',
    x: 0,
    z: -1,
  } as const
  const end = {
    vertexId: 'missing-constraint-end',
    x: 0,
    z: 1,
  } as const
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'missing-constraint-left',
      polygon: [
        start,
        { vertexId: 'missing-constraint-left-tip', x: -2, z: 0 },
        end,
      ],
    },
    {
      id: 'missing-constraint-right',
      polygon: [
        end,
        { vertexId: 'missing-constraint-right-tip', x: 2, z: 0 },
        start,
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'missing-constraint-edge',
    firstFaceId: 'missing-constraint-left',
    secondFaceId: 'missing-constraint-right',
  }]
  const transforms = new Map([
    ['missing-constraint-left', new Matrix4()],
    ['missing-constraint-right', new Matrix4()],
  ])

  for (const reverseFaces of [false, true]) {
    const orderedFaces = reverseFaces ? [...faces].reverse() : faces
    const analyzer = prepareFoldPreviewNarrowPhase(orderedFaces, adjacencies)
    assert.ok(analyzer)
    for (const thickness of [0, 0.1, 3]) {
      const result = analyzer.analyze(transforms, thickness)
      assert.ok(result)
      assert.equal(result.broadPhaseCandidates, 1)
      assert.equal(result.broadPhaseHingeAdjacentCandidates, 1)
      assert.equal(result.trianglePairTests, 1)
      assert.equal(result.satTests, 1)
      assert.deepEqual(result.interactions, [{
        firstFaceId: 'missing-constraint-left',
        secondFaceId: 'missing-constraint-right',
        relation: 'hinge_adjacent',
        hingeEdgeIds: ['missing-constraint-edge'],
        geometryClass: 'indeterminate',
        hingeDecision: {
          kind: 'indeterminate',
          hingeEdgeIds: ['missing-constraint-edge'],
          reason: 'missing_constraint',
        },
      }])

      const job = analyzer.createAnalysisJob(transforms, thickness)
      assert.ok(job)
      assert.deepEqual(drainAnalysisJob(job), result)
      assert.deepEqual(
        findFoldPreviewNarrowPhaseInteractions(
          orderedFaces,
          transforms,
          thickness,
          adjacencies,
        ),
        result,
      )
    }
  }
})

test('91-degree shared contact agrees across sync, resumable, and full scans', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    cornerMountainValley.faces,
    cornerMountainValley.adjacencies,
    cornerMountainValley.constraints,
  )
  assert.ok(analyzer)
  const pose = poseAt(cornerMountainValley, 91)
  const synchronous = analyzer.analyze(pose.faceTransforms, 1)
  assert.ok(synchronous)
  const analysisJob = analyzer.createAnalysisJob(pose.faceTransforms, 1)
  assert.ok(analysisJob)
  assert.deepEqual(drainAnalysisJob(analysisJob), synchronous)

  const fullScan = analyzer.collectFullScanNonAdjacentWitnessSet(
    pose.faceTransforms,
    1,
  )
  assert.ok(fullScan)
  const fullScanJob = analyzer.createFullScanNonAdjacentWitnessSetJob(
    pose.faceTransforms,
    1,
  )
  assert.ok(fullScanJob)
  assert.deepEqual(drainFullScanJob(fullScanJob), fullScan)
  assert.equal(fullScan.kind, 'complete')
  assert.equal(fullScan.coverage.broadPhaseCandidateCount, 1)
  assert.equal(fullScan.coverage.expectedTrianglePairCount, 1)
  assert.equal(fullScan.coverage.satTests, 1)
  assert.equal(fullScan.coverage.allowedSharedVertexPairCount, 1)
  assert.equal(fullScan.coverage.touchingPairCount, 0)
  assert.equal(fullScan.coverage.penetratingPairCount, 0)
  assert.equal(fullScan.coverage.indeterminatePairCount, 0)
  assert.deepEqual(fullScan.witnessSamples, [])
})

test('90-degree midpoint risk agrees across sync, resumable, and full scans', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    midpointMountainMountain.faces,
    midpointMountainMountain.adjacencies,
    midpointMountainMountain.constraints,
  )
  assert.ok(analyzer)
  const pose = poseAt(midpointMountainMountain, 90)
  const synchronous = analyzer.analyze(pose.faceTransforms, 1)
  assert.ok(synchronous)
  const outer = outerInteraction(synchronous.interactions, 'midpoint')
  assert.ok(outer)
  assert.equal(outer.geometryClass, 'indeterminate')
  assert.equal(outer.topologyContact, undefined)

  const analysisJob = analyzer.createAnalysisJob(pose.faceTransforms, 1)
  assert.ok(analysisJob)
  assert.deepEqual(drainAnalysisJob(analysisJob), synchronous)

  const fullScan = analyzer.collectFullScanNonAdjacentWitnessSet(
    pose.faceTransforms,
    1,
  )
  assert.ok(fullScan)
  const fullScanJob = analyzer.createFullScanNonAdjacentWitnessSetJob(
    pose.faceTransforms,
    1,
  )
  assert.ok(fullScanJob)
  assert.deepEqual(drainFullScanJob(fullScanJob), fullScan)
  assert.equal(fullScan.kind, 'unavailable')
  if (fullScan.kind !== 'unavailable') {
    assert.fail('90-degree midpoint full scan must remain blocking')
  }
  assert.deepEqual(fullScan.reasons, ['indeterminate_pair'])
  assert.equal(fullScan.coverage.broadPhaseCandidateCount, 1)
  assert.equal(fullScan.coverage.expectedTrianglePairCount, 1)
  assert.equal(fullScan.coverage.satTests, 1)
  assert.equal(fullScan.coverage.allowedSharedVertexPairCount, 0)
  assert.equal(fullScan.coverage.touchingPairCount, 0)
  assert.equal(fullScan.coverage.penetratingPairCount, 0)
  assert.equal(fullScan.coverage.indeterminatePairCount, 1)
  assert.deepEqual(fullScan.witnessSamples, [])
})

test('135-degree penetration agrees across sync, resumable, and full scans', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    midpointMountainMountain.faces,
    midpointMountainMountain.adjacencies,
    midpointMountainMountain.constraints,
  )
  assert.ok(analyzer)
  const pose = poseAt(midpointMountainMountain, 135)
  const synchronous = analyzer.analyze(pose.faceTransforms, 1)
  assert.ok(synchronous)
  const analysisJob = analyzer.createAnalysisJob(pose.faceTransforms, 1)
  assert.ok(analysisJob)
  assert.deepEqual(drainAnalysisJob(analysisJob), synchronous)

  const fullScan = analyzer.collectFullScanNonAdjacentWitnessSet(
    pose.faceTransforms,
    1,
  )
  assert.ok(fullScan)
  const fullScanJob = analyzer.createFullScanNonAdjacentWitnessSetJob(
    pose.faceTransforms,
    1,
  )
  assert.ok(fullScanJob)
  assert.deepEqual(drainFullScanJob(fullScanJob), fullScan)
  assert.equal(fullScan.coverage.broadPhaseCandidateCount, 1)
  assert.equal(fullScan.coverage.expectedTrianglePairCount, 1)
  assert.equal(fullScan.coverage.satTests, 1)
  assert.equal(fullScan.coverage.allowedSharedVertexPairCount, 0)
  assert.equal(fullScan.coverage.touchingPairCount, 0)
  assert.equal(fullScan.coverage.penetratingPairCount, 1)
  assert.equal(fullScan.coverage.indeterminatePairCount, 0)
  assert.equal(fullScan.kind, 'complete')
  assert.equal(fullScan.witnessSamples.length, 1)
  assert.equal(fullScan.witnessSamples[0]?.geometryClass, 'penetrating')
})

type TopologyGridFixture = Readonly<{
  prefix: 'corner' | 'midpoint'
  faces: readonly FoldPreviewCollisionPoseFace[]
  adjacencies: readonly FoldPreviewCollisionAdjacency[]
  constraints: readonly FoldPreviewHingeContactConstraint[]
  tree: FoldPreviewTreeKinematics
  leftEdgeId: string
  rightEdgeId: string
}>

function cornerMountainValleyFixture(): TopologyGridFixture {
  const apex = {
    vertexId: 'corner-apex',
    x: 400,
    z: 400,
  } as const
  const leftEnd = {
    vertexId: 'corner-left-end',
    x: 300,
    z: 0,
  } as const
  const rightEnd = {
    vertexId: 'corner-right-end',
    x: 0,
    z: 300,
  } as const
  const leftEdgeId = 'corner-left-hinge'
  const rightEdgeId = 'corner-right-hinge'
  const leftLength = Math.hypot(leftEnd.x - apex.x, leftEnd.z - apex.z)
  const rightLength = Math.hypot(rightEnd.x - apex.x, rightEnd.z - apex.z)
  return {
    prefix: 'corner',
    faces: [
      {
        id: 'corner-left',
        polygon: [
          apex,
          { vertexId: 'corner-bottom-right', x: 400, z: 0 },
          leftEnd,
        ],
      },
      {
        id: 'corner-middle',
        polygon: [
          apex,
          leftEnd,
          { vertexId: 'corner-bottom-left', x: 0, z: 0 },
          rightEnd,
        ],
      },
      {
        id: 'corner-right',
        polygon: [
          apex,
          rightEnd,
          { vertexId: 'corner-top-left', x: 0, z: 400 },
        ],
      },
    ],
    adjacencies: [
      {
        edgeId: leftEdgeId,
        firstFaceId: 'corner-left',
        secondFaceId: 'corner-middle',
      },
      {
        edgeId: rightEdgeId,
        firstFaceId: 'corner-middle',
        secondFaceId: 'corner-right',
      },
    ],
    constraints: [
      {
        edgeId: leftEdgeId,
        leftFaceId: 'corner-left',
        rightFaceId: 'corner-middle',
        start: apex,
        end: leftEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
      {
        edgeId: rightEdgeId,
        leftFaceId: 'corner-middle',
        rightFaceId: 'corner-right',
        start: apex,
        end: rightEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
    ],
    tree: {
      kind: 'tree',
      rootFaceId: 'corner-middle',
      joints: [
        {
          parentFaceId: 'corner-middle',
          childFaceId: 'corner-left',
          childRotationSign: -1,
          hinge: {
            edgeId: leftEdgeId,
            leftFaceId: 'corner-left',
            rightFaceId: 'corner-middle',
            start: apex,
            end: leftEnd,
            axis: {
              x: (leftEnd.x - apex.x) / leftLength,
              z: (leftEnd.z - apex.z) / leftLength,
            },
            assignment: 'mountain',
            rotationSign: 1,
          },
        },
        {
          parentFaceId: 'corner-middle',
          childFaceId: 'corner-right',
          childRotationSign: -1,
          hinge: {
            edgeId: rightEdgeId,
            leftFaceId: 'corner-middle',
            rightFaceId: 'corner-right',
            start: apex,
            end: rightEnd,
            axis: {
              x: (rightEnd.x - apex.x) / rightLength,
              z: (rightEnd.z - apex.z) / rightLength,
            },
            assignment: 'valley',
            rotationSign: -1,
          },
        },
      ],
    },
    leftEdgeId,
    rightEdgeId,
  }
}

function midpointMountainMountainFixture(): TopologyGridFixture {
  const apex = {
    vertexId: 'midpoint-apex',
    x: 200,
    z: 0,
  } as const
  const leftEnd = {
    vertexId: 'midpoint-left-end',
    x: 0,
    z: 400,
  } as const
  const rightEnd = {
    vertexId: 'midpoint-right-end',
    x: 400,
    z: 400,
  } as const
  const leftEdgeId = 'midpoint-left-hinge'
  const rightEdgeId = 'midpoint-right-hinge'
  const leftLength = Math.hypot(leftEnd.x - apex.x, leftEnd.z - apex.z)
  const rightLength = Math.hypot(rightEnd.x - apex.x, rightEnd.z - apex.z)
  return {
    prefix: 'midpoint',
    faces: [
      {
        id: 'midpoint-left',
        polygon: [
          apex,
          { vertexId: 'midpoint-bottom-left', x: 0, z: 0 },
          leftEnd,
        ],
      },
      {
        id: 'midpoint-middle',
        polygon: [apex, leftEnd, rightEnd],
      },
      {
        id: 'midpoint-right',
        polygon: [
          apex,
          rightEnd,
          { vertexId: 'midpoint-bottom-right', x: 400, z: 0 },
        ],
      },
    ],
    adjacencies: [
      {
        edgeId: leftEdgeId,
        firstFaceId: 'midpoint-left',
        secondFaceId: 'midpoint-middle',
      },
      {
        edgeId: rightEdgeId,
        firstFaceId: 'midpoint-middle',
        secondFaceId: 'midpoint-right',
      },
    ],
    constraints: [
      {
        edgeId: leftEdgeId,
        leftFaceId: 'midpoint-left',
        rightFaceId: 'midpoint-middle',
        start: apex,
        end: leftEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
      {
        edgeId: rightEdgeId,
        leftFaceId: 'midpoint-middle',
        rightFaceId: 'midpoint-right',
        start: apex,
        end: rightEnd,
        thicknessRule: 'centered_mid_surface_v1',
      },
    ],
    tree: {
      kind: 'tree',
      rootFaceId: 'midpoint-middle',
      joints: [
        {
          parentFaceId: 'midpoint-middle',
          childFaceId: 'midpoint-left',
          childRotationSign: -1,
          hinge: {
            edgeId: leftEdgeId,
            leftFaceId: 'midpoint-left',
            rightFaceId: 'midpoint-middle',
            start: apex,
            end: leftEnd,
            axis: {
              x: (leftEnd.x - apex.x) / leftLength,
              z: (leftEnd.z - apex.z) / leftLength,
            },
            assignment: 'mountain',
            rotationSign: 1,
          },
        },
        {
          parentFaceId: 'midpoint-middle',
          childFaceId: 'midpoint-right',
          childRotationSign: 1,
          hinge: {
            edgeId: rightEdgeId,
            leftFaceId: 'midpoint-middle',
            rightFaceId: 'midpoint-right',
            start: apex,
            end: rightEnd,
            axis: {
              x: (rightEnd.x - apex.x) / rightLength,
              z: (rightEnd.z - apex.z) / rightLength,
            },
            assignment: 'mountain',
            rotationSign: 1,
          },
        },
      ],
    },
    leftEdgeId,
    rightEdgeId,
  }
}

function poseAt(
  fixture: TopologyGridFixture,
  degrees: number,
) {
  const pose = calculateFoldTreePoseWithAngles(fixture.tree, {
    kind: 'per_hinge',
    angles: [
      {
        edgeId: fixture.leftEdgeId,
        angleDegrees: degrees,
      },
      {
        edgeId: fixture.rightEdgeId,
        angleDegrees: degrees,
      },
    ],
  })
  assert.ok(pose)
  return pose
}

function outerInteraction(
  interactions: readonly FoldPreviewNarrowPhaseInteraction[],
  prefix: 'corner' | 'midpoint',
) {
  return interactions.find((interaction) =>
    interaction.firstFaceId === `${prefix}-left`
    && interaction.secondFaceId === `${prefix}-right`)
}

function drainAnalysisJob(job: FoldPreviewNarrowPhaseAnalysisJob) {
  let step = job.step(1)
  for (let index = 0; step.kind === 'pending' && index < 64; index += 1) {
    step = job.step(1)
  }
  if (step.kind !== 'complete') {
    assert.fail(`analysis job ended as ${step.kind}`)
  }
  return step.result
}

function drainFullScanJob(job: FoldPreviewFullScanNonAdjacentWitnessJob) {
  let step = job.step(1)
  for (let index = 0; step.kind === 'pending' && index < 64; index += 1) {
    step = job.step(1)
  }
  if (step.kind !== 'complete') {
    assert.fail(`full-scan job ended as ${step.kind}`)
  }
  return step.result
}
