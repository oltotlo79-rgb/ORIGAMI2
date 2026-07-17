import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_CROSS_TRIANGLE_PAIRS,
  prepareFoldPreviewTreeSingleHingeContinuousCollision,
  type FoldPreviewTreeSingleHingeContinuousAnalyzer,
} from '../src/lib/foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  describeFoldPreviewContinuousMotionDetail,
} from '../src/lib/foldPreviewContinuousMotionDetail.ts'
import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
} from '../src/lib/foldPreviewCollision.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
  FoldPreviewTreeJointModel,
} from '../src/lib/foldPreviewModel.ts'
import {
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
} from '../src/lib/foldPreviewNarrowCollision.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from '../src/lib/foldPreviewTreeScenePose.ts'

const THICKNESS = 0.1

test('one selected hinge moves its complete subtree through a safe path', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'root',
    'hinge-z',
  )
  assert.ok(analyzer)
  assert.equal(analyzer.fixedFaceId, 'root')
  assert.equal(analyzer.parentFaceId, 'root')
  assert.equal(analyzer.childFaceId, 'middle')
  assert.deepEqual(analyzer.stationaryFaceIds, ['root'])
  assert.deepEqual(analyzer.movingFaceIds, ['middle', 'leaf'])
  assert.equal(analyzer.crossTrianglePairs, 8)
  assert.ok(analyzer.staticallySupportedTrianglePairs > 0)

  const job = analyzer.createJob([
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 10 },
  ], 120, THICKNESS)
  assert.ok(job)
  assert.equal(run(job).kind, 'clear')
})

test('full-face point analysis blocks a collision with a stationary branch', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    stationaryBranchCollisionModel(),
    'root',
    'selected',
  )
  assert.ok(analyzer)
  assert.deepEqual(analyzer.stationaryFaceIds, ['root', 'obstacle'])
  assert.deepEqual(analyzer.movingFaceIds, ['moving'])
  assert.equal(analyzer.crossTrianglePairs, 8)
  assert.equal(analyzer.staticallySupportedTrianglePairs, 4)
  const frozenAngles = [
    { edgeId: 'selected', angleDegrees: 0 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ] as const

  const job = analyzer.createJob(frozenAngles, 120, 0.02, {
    maxDepth: 18,
    minTimeSpan: 2 ** -22,
    maxIntervalTests: 10_000,
  })
  assert.ok(job)
  const result = run(job)
  assert.equal(result.kind, 'blocked', JSON.stringify(result))
  assert.ok(result.kind === 'blocked')
  assert.ok(result.certifiedSafeThrough > 0)
  assert.ok(result.certifiedSafeThrough < 1)
  assert.equal(result.blockingSampleTime, result.unsafeBracket[1])
  assert.equal(result.blocker?.relation, 'non_adjacent')
  assert.deepEqual(
    new Set([
      result.blocker?.firstFaceId,
      result.blocker?.secondFaceId,
    ]),
    new Set(['moving', 'obstacle']),
  )
  assert.equal(result.blocker?.geometryClass, 'penetrating')
})

test('a terminal block retains its exact request-bound collision sample', () => {
  const model = stationaryBranchCollisionModel()
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    model,
    'root',
    'selected',
  )
  assert.ok(analyzer)
  const startAngles = [
    { edgeId: 'frozen', angleDegrees: 90 },
    { edgeId: 'selected', angleDegrees: 0 },
  ] as const
  const targetSelectedAngleDegrees = 120
  const thickness = 0.02
  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    'root',
    thickness,
    startAngles,
  )
  assert.ok(sourcePoseRequestKey)
  const expectedRequestIdentity = {
    contextKey: 'stationary-branch-context',
    sourcePoseRequestKey,
    generation: 7,
    requestSequence: 11,
  } as const
  const requestIdentity = { ...expectedRequestIdentity }
  const job = analyzer.createJob(
    startAngles,
    targetSelectedAngleDegrees,
    thickness,
    {
      maxDepth: 18,
      minTimeSpan: 2 ** -22,
      maxIntervalTests: 10_000,
      requestIdentity,
    },
  )
  assert.ok(job)
  requestIdentity.contextKey = 'mutated-after-job-creation'
  requestIdentity.generation = 99

  const result = run(job)
  assert.equal(result.kind, 'blocked', JSON.stringify(result))
  assert.ok(result.kind === 'blocked')
  assert.equal(result.blockingSampleTime, result.unsafeBracket[1])
  const blocker = result.blocker
  const sample = blocker?.blockingSample
  assert.ok(blocker && sample)
  assert.equal(sample.blockingSampleTime, result.blockingSampleTime)
  assert.equal(
    sample.selectedAngleDegrees,
    (
      targetSelectedAngleDegrees
      - startAngles[1].angleDegrees
    ) * result.blockingSampleTime,
  )
  assert.equal(sample.collisionThickness, thickness)
  assert.deepEqual(sample.identity, {
    projectId: 'stationary-branch-project',
    revision: 1,
    revisionBinding: 'project_response_source_equal_v1',
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    request: expectedRequestIdentity,
  })
  assert.notStrictEqual(sample.identity.request, requestIdentity)

  const expectedStart = [
    { edgeId: 'selected', angleDegrees: 0 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ]
  const expectedTarget = [
    { edgeId: 'selected', angleDegrees: targetSelectedAngleDegrees },
    { edgeId: 'frozen', angleDegrees: 90 },
  ]
  const expectedSample = [
    {
      edgeId: 'selected',
      angleDegrees: sample.selectedAngleDegrees,
    },
    { edgeId: 'frozen', angleDegrees: 90 },
  ]
  assert.deepEqual(sample.angleVectors.start, expectedStart)
  assert.deepEqual(sample.angleVectors.target, expectedTarget)
  assert.deepEqual(sample.angleVectors.sample, expectedSample)

  assert.equal(sample.faceTransforms.length, 2)
  assert.deepEqual(
    sample.faceTransforms.map((transform) => transform.faceId),
    [blocker.firstFaceId, blocker.secondFaceId],
  )
  for (const transform of sample.faceTransforms) {
    assert.equal(transform.elements.length, 16)
    assert.ok(transform.elements.every(Number.isFinite))
  }

  const coverage = sample.witnessCoverage
  assert.equal(
    coverage.scope,
    'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
  )
  assert.equal(
    coverage.eligiblePairCount,
    coverage.attemptedPairCount + coverage.omittedByLimitCount,
  )
  assert.equal(
    coverage.attemptedPairCount,
    sample.witnessSamples.length + coverage.unavailablePairCount,
  )
  assert.ok(coverage.attemptedPairCount <= 16)
  assert.equal(typeof coverage.authoritativePairScanComplete, 'boolean')
  for (const witnessSample of sample.witnessSamples) {
    assert.equal(witnessSample.relation, 'non_adjacent')
    assert.ok(Number.isSafeInteger(witnessSample.firstTriangleIndex))
    assert.ok(witnessSample.firstTriangleIndex >= 0)
    assert.ok(Number.isSafeInteger(witnessSample.secondTriangleIndex))
    assert.ok(witnessSample.secondTriangleIndex >= 0)
    assert.equal(
      witnessSample.witness.geometryClass,
      witnessSample.geometryClass,
    )
    assert.equal(
      witnessSample.witness.algorithm,
      'triangle_prism_sat_witness_v1',
    )
    assert.ok(Number.isFinite(witnessSample.witness.numericalMargin))
    assert.ok(witnessSample.witness.numericalMargin >= 0)
    assert.ok(Number.isFinite(witnessSample.witness.escapeDistance))
    assert.ok(witnessSample.witness.escapeDistance >= 0)
    assert.ok(Number.isFinite(witnessSample.witness.toleratedGap))
    assert.ok(witnessSample.witness.toleratedGap >= 0)
    assert.equal(
      witnessSample.witness.localSeparationHint.distance,
      witnessSample.witness.escapeDistance,
    )
    assert.equal(
      witnessSample.witness.localSeparationHint.scope,
      'selected_triangle_prism_pair_only',
    )
    assert.equal(
      witnessSample.witness.localSeparationHint.autoApplicable,
      false,
    )
    assert.equal(
      witnessSample.witness.positionRegion.sourcePose,
      'analyzed_input_pose',
    )
    assert.ok(witnessSample.witness.firstSupport.length > 0)
    assert.ok(witnessSample.witness.secondSupport.length > 0)
    assert.ok(witnessSample.witness.positionRegion.generators.length > 0)
    assertFinitePoint(witnessSample.witness.normal.vector)
    assertFinitePoint(
      witnessSample.witness.localSeparationHint.translation,
    )
    for (const point of witnessSample.witness.firstSupport) {
      assertFinitePoint(point)
    }
    for (const point of witnessSample.witness.secondSupport) {
      assertFinitePoint(point)
    }
    for (const point of witnessSample.witness.positionRegion.generators) {
      assertFinitePoint(point)
    }
  }

  assert.notEqual(sample.primaryWitnessIndex, null)
  const primary = sample.primaryWitnessIndex === null
    ? undefined
    : sample.witnessSamples[sample.primaryWitnessIndex]
  assert.ok(primary)
  assert.equal(primary.firstFaceId, blocker.firstFaceId)
  assert.equal(primary.secondFaceId, blocker.secondFaceId)
  assert.equal(primary.geometryClass, blocker.geometryClass)
  assert.equal(primary.relation, 'non_adjacent')
  assertDeeplyFrozen(sample)

  const repeatedTerminalResult = job.step(1)
  assert.strictEqual(repeatedTerminalResult, result)
  assert.strictEqual(
    repeatedTerminalResult.kind === 'blocked'
      ? repeatedTerminalResult.blocker?.blockingSample
      : null,
    sample,
  )

  const detail = describeFoldPreviewContinuousMotionDetail({
    requested: targetSelectedAngleDegrees,
    applied: (
      startAngles[1].angleDegrees
      + (
        targetSelectedAngleDegrees - startAngles[1].angleDegrees
      ) * result.certifiedSafeThrough
    ),
    start: startAngles[1].angleDegrees,
    status: 'blocked',
    reason: 'motion_blocked',
    result,
  }, model.faces.map((face, index) => ({
    id: face.id,
    number: index + 1,
    label: `面 ${index + 1}`,
  })), {
    projectId: model.projectId,
    revision: model.revision,
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    contextKey: expectedRequestIdentity.contextKey,
    sourcePoseRequestKey,
    generation: expectedRequestIdentity.generation,
    requestSequence: expectedRequestIdentity.requestSequence,
    collisionThickness: thickness,
    startAngles,
    targetSelectedAngleDegrees,
  })
  assert.ok(detail?.blockingEvidence)
  assert.equal(
    detail.blockingEvidence.unsafeAnalysisDegrees,
    sample.selectedAngleDegrees,
  )
  assert.equal(
    detail.blockingEvidence.safety.sampleTransformsAppliedToScene,
    false,
  )
  assert.equal(detail.blockingEvidence.safety.autoApplicable, false)
})

test('request identity mismatches and malformed values reject job creation', () => {
  const model = stationaryBranchCollisionModel()
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    model,
    'root',
    'selected',
  )
  assert.ok(analyzer)
  const startAngles = [
    { edgeId: 'selected', angleDegrees: 0 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ] as const
  const thickness = 0.02
  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    'root',
    thickness,
    startAngles,
  )
  assert.ok(sourcePoseRequestKey)

  assert.equal(analyzer.createJob(startAngles, 120, thickness, {
    requestIdentity: {
      contextKey: 'context',
      sourcePoseRequestKey: `${sourcePoseRequestKey}:mismatch`,
      generation: 0,
      requestSequence: 1,
    },
  }), null)
  for (const requestIdentity of [
    {
      contextKey: '',
      sourcePoseRequestKey,
      generation: 0,
      requestSequence: 1,
    },
    {
      contextKey: 'context',
      sourcePoseRequestKey,
      generation: -1,
      requestSequence: 1,
    },
    {
      contextKey: 'context',
      sourcePoseRequestKey,
      generation: 0,
      requestSequence: 0,
    },
  ]) {
    assert.equal(analyzer.createJob(startAngles, 120, thickness, {
      requestIdentity,
    }), null)
  }
})

test('a requestless pure analyzer still retains a complete blocking snapshot', () => {
  const model = stationaryBranchCollisionModel()
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    model,
    'root',
    'selected',
  )
  assert.ok(analyzer)
  const startAngles = [
    { edgeId: 'selected', angleDegrees: 0 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ] as const
  const job = analyzer.createJob(startAngles, 120, 0.02, {
    maxDepth: 18,
    minTimeSpan: 2 ** -22,
    maxIntervalTests: 10_000,
  })
  assert.ok(job)

  const result = run(job)
  assert.equal(result.kind, 'blocked', JSON.stringify(result))
  const sample = result.kind === 'blocked'
    ? result.blocker?.blockingSample
    : null
  assert.ok(sample)
  assert.equal(sample.identity.request, null)
  assert.deepEqual(sample.identity, {
    projectId: model.projectId,
    revision: model.revision,
    revisionBinding: 'project_response_source_equal_v1',
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    request: null,
  })
  assert.equal(sample.blockingSampleTime, result.blockingSampleTime)
  assert.deepEqual(sample.angleVectors.start, startAngles)
  assert.deepEqual(sample.angleVectors.target, [
    { edgeId: 'selected', angleDegrees: 120 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ])
  assert.equal(sample.faceTransforms.length, 2)
  assert.ok(sample.faceTransforms.every(
    (transform) => transform.elements.length === 16,
  ))
  assert.ok(sample.witnessSamples.length > 0)
  assertDeeplyFrozen(sample)
})

test('blocking samples bind exact start and target endpoint times', () => {
  const model = stationaryBranchCollisionModel()
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    model,
    'root',
    'selected',
  )
  assert.ok(analyzer)
  const safeStart = [
    { edgeId: 'selected', angleDegrees: 0 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ] as const
  const targetEndpointJob = analyzer.createJob(
    safeStart,
    120,
    0.02,
    { maxDepth: 0 },
  )
  assert.ok(targetEndpointJob)
  const targetEndpoint = run(targetEndpointJob)
  assert.equal(targetEndpoint.kind, 'blocked')
  assert.ok(targetEndpoint.kind === 'blocked')
  assert.equal(targetEndpoint.blockingSampleTime, 1)
  assert.equal(
    targetEndpoint.blocker?.blockingSample?.blockingSampleTime,
    1,
  )
  assert.equal(
    targetEndpoint.blocker?.blockingSample?.selectedAngleDegrees,
    120,
  )

  const blockedStart = [
    { edgeId: 'selected', angleDegrees: 120 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ] as const
  const reverseSourcePoseRequestKey =
    createFoldPreviewTreeSceneCollisionPoseKey(
      model,
      'root',
      0.02,
      blockedStart,
    )
  assert.ok(reverseSourcePoseRequestKey)
  const reverseJob = analyzer.createJob(blockedStart, 0, 0.02, {
    requestIdentity: {
      contextKey: 'blocked-start-context',
      sourcePoseRequestKey: reverseSourcePoseRequestKey,
      generation: 3,
      requestSequence: 4,
    },
  })
  assert.ok(reverseJob)
  const reverse = run(reverseJob)
  assert.equal(reverse.kind, 'blocked')
  assert.ok(reverse.kind === 'blocked')
  assert.equal(reverse.blockingSampleTime, 0)
  assert.equal(reverse.blocker?.blockingSample?.blockingSampleTime, 0)
  assert.equal(
    reverse.blocker?.blockingSample?.selectedAngleDegrees,
    120,
  )
  assert.deepEqual(
    reverse.blocker?.blockingSample?.angleVectors.target,
    [
      { edgeId: 'selected', angleDegrees: 0 },
      { edgeId: 'frozen', angleDegrees: 90 },
    ],
  )
  const reverseDetail = describeFoldPreviewContinuousMotionDetail({
    requested: 0,
    applied: 120,
    start: 120,
    status: 'blocked',
    reason: 'motion_blocked',
    result: reverse,
  }, model.faces.map((face, index) => ({
    id: face.id,
    number: index + 1,
    label: `面 ${index + 1}`,
  })), {
    projectId: model.projectId,
    revision: model.revision,
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    contextKey: 'blocked-start-context',
    sourcePoseRequestKey: reverseSourcePoseRequestKey,
    generation: 3,
    requestSequence: 4,
    collisionThickness: 0.02,
    startAngles: blockedStart,
    targetSelectedAngleDegrees: 0,
  })
  assert.ok(reverseDetail?.blockingEvidence)
  assert.equal(reverseDetail.blockingEvidence.unsafeAnalysisDegrees, 120)
  assert.equal(
    reverseDetail.blockingEvidence.safety.sampleTransformsAppliedToScene,
    false,
  )
})

test('rerooting selects the opposite side without changing other hinge angles', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'leaf',
    'hinge-z',
  )
  assert.ok(analyzer)
  assert.equal(analyzer.fixedFaceId, 'leaf')
  assert.equal(analyzer.parentFaceId, 'middle')
  assert.equal(analyzer.childFaceId, 'root')
  assert.deepEqual(analyzer.stationaryFaceIds, ['middle', 'leaf'])
  assert.deepEqual(analyzer.movingFaceIds, ['root'])

  const job = analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 15 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 110, THICKNESS)
  assert.ok(job)
  assert.equal(run(job).kind, 'clear')
})

test('a downstream selection supports non-identity stationary transforms', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'root',
    'hinge-x',
  )
  assert.ok(analyzer)
  assert.deepEqual(analyzer.stationaryFaceIds, ['root', 'middle'])
  assert.deepEqual(analyzer.movingFaceIds, ['leaf'])

  const job = analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 45 },
    { edgeId: 'hinge-x', angleDegrees: 10 },
  ], 120, THICKNESS)
  assert.ok(job)
  assert.equal(run(job).kind, 'clear')
})

test('a non-commuting descendant matches core poses at midpoint and target', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    nonCommutingCornerModel(),
    'root',
    'hinge-z',
  )
  assert.ok(analyzer)
  const job = analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 35 },
    { edgeId: 'hinge-x', angleDegrees: 55 },
  ], 120, THICKNESS)
  // createJob verifies its common world-axis transform against the core tree
  // kinematics at both the representative midpoint and target pose.
  assert.ok(job)
  job.cancel()
})

test('a rerooted valley hinge supports reverse selected-only motion', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'leaf',
    'hinge-x',
  )
  assert.ok(analyzer)
  assert.equal(analyzer.parentFaceId, 'leaf')
  assert.equal(analyzer.childFaceId, 'middle')
  assert.deepEqual(analyzer.stationaryFaceIds, ['leaf'])
  assert.deepEqual(analyzer.movingFaceIds, ['middle', 'root'])

  const job = analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 45 },
    { edgeId: 'hinge-x', angleDegrees: 130 },
  ], 20, THICKNESS)
  assert.ok(job)
  assert.equal(run(job).kind, 'clear')
})

test('the selected hinge never certifies through the exact 180-degree singularity', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'root',
    'hinge-z',
  )
  const job = analyzer?.createJob([
    { edgeId: 'hinge-z', angleDegrees: 0 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 180, THICKNESS, {
    maxDepth: 8,
    maxIntervalTests: 100,
    minTimeSpan: 2 ** -20,
  })
  assert.ok(analyzer && job)
  const result = run(job)
  assert.equal(result.kind, 'indeterminate')
  assert.ok(result.kind === 'indeterminate')
  assert.ok(result.certifiedSafeThrough > 0)
  assert.ok(result.certifiedSafeThrough < 1)
  assert.equal(result.unresolvedBracket[1], 1)
})

test('an exact 180-degree start pose allows no reverse escape', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'root',
    'hinge-z',
  )
  const job = analyzer?.createJob([
    { edgeId: 'hinge-z', angleDegrees: 180 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 90, THICKNESS)
  assert.ok(analyzer && job)
  const result = job.step(1)
  assert.equal(result.kind, 'indeterminate')
  assert.equal(
    result.kind === 'indeterminate' && result.certifiedSafeThrough,
    0,
  )
  assert.deepEqual(
    result.kind === 'indeterminate' ? result.unresolvedBracket : null,
    [0, 0],
  )
  assert.match(
    result.kind === 'indeterminate' ? result.reason : '',
    /unsupported_flat_fold/u,
  )
})

test('per-job point and interval triangle work limits fail closed', () => {
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'root',
    'hinge-z',
  )
  assert.ok(analyzer)
  const angles = [
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ] as const

  const pointLimited = analyzer.createJob(angles, 120, THICKNESS, {
    maxPointTriangleTests: 1,
  })
  assert.ok(pointLimited)
  const pointResult = pointLimited.step(1)
  assert.equal(pointResult.kind, 'indeterminate')
  assert.equal(
    pointResult.kind === 'indeterminate' && pointResult.reason,
    'tree_point_triangle_work_limit',
  )
  assert.equal(
    pointResult.kind === 'indeterminate'
      && pointResult.certifiedSafeThrough,
    0,
  )

  const intervalLimited = analyzer.createJob(angles, 120, THICKNESS, {
    maxIntervalPairVisits: 1,
  })
  assert.ok(intervalLimited)
  const intervalResult = intervalLimited.step(1)
  assert.equal(intervalResult.kind, 'indeterminate')
  assert.equal(
    intervalResult.kind === 'indeterminate' && intervalResult.reason,
    'tree_interval_pair_work_limit',
  )
  assert.deepEqual(
    intervalResult.kind === 'indeterminate'
      ? intervalResult.unresolvedBracket
      : null,
    [0, 1],
  )
})

test('prepared geometry and kinematics are detached from later model mutation', () => {
  const model = treeModel()
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    model,
    'root',
    'hinge-z',
  )
  assert.ok(analyzer)

  ;(model.faces[0].polygon[1] as { x: number }).x = -100
  ;(model.hinges[0].axis as { x: number; z: number }).x = 1
  assert.equal(model.kinematics.kind, 'tree')
  if (model.kinematics.kind === 'tree') {
    ;(model.kinematics.joints[0].hinge.end as { z: number }).z = 100
  }

  const job = analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 120, THICKNESS)
  assert.ok(job)
  assert.equal(run(job).kind, 'clear')
})

test('invalid topology, angle vectors, requests, and work options fail closed', () => {
  const model = treeModel()
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision(
      model,
      'missing',
      'hinge-z',
    ),
    null,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision(
      model,
      'root',
      'missing',
    ),
    null,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...model,
      kinematics: { kind: 'static_cycle', reason: 'cyclic_hinge_graph' },
    }, 'root', 'hinge-z'),
    null,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...model,
      hinges: [
        {
          ...model.hinges[0],
          end: { ...model.hinges[0].end, z: 0.5 },
        },
        model.hinges[1],
      ],
    }, 'root', 'hinge-z'),
    null,
  )

  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    model,
    'root',
    'hinge-z',
  )
  assert.ok(analyzer)
  assert.equal(analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
  ], 90, THICKNESS), null)
  assert.equal(analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-z', angleDegrees: 20 },
  ], 90, THICKNESS), null)
  assert.equal(analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 181, THICKNESS), null)
  assert.equal(analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 90, -1), null)
  assert.equal(analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 90, THICKNESS, {
    maxIntervalPairVisits: 0,
  }), null)
  assert.equal(analyzer.createJob([
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ], 90, THICKNESS, {
    maxPointTriangleTests: Number.POSITIVE_INFINITY,
  }), null)
})

test('a cross-cut triangle product above the hard preparation cap is rejected', () => {
  const triangleCount = 1_001
  const crossTrianglePairs = triangleCount * (triangleCount + 1)
  assert.ok(
    crossTrianglePairs
    > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_CROSS_TRIANGLE_PAIRS,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision(
      pairCapModel(triangleCount),
      'root',
      'selected',
    ),
    null,
  )
})

test('throwing proxies at public preparation and job boundaries fail closed', () => {
  const throwingModel = new Proxy(treeModel(), {
    get() {
      throw new Error('model getter')
    },
  })
  let prepared: unknown
  assert.doesNotThrow(() => {
    prepared = prepareFoldPreviewTreeSingleHingeContinuousCollision(
      throwingModel,
      'root',
      'hinge-z',
    )
  })
  assert.equal(prepared, null)

  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    treeModel(),
    'root',
    'hinge-z',
  )
  assert.ok(analyzer)
  const validAngles = [
    { edgeId: 'hinge-z', angleDegrees: 10 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ] as const
  const throwingAngles = new Proxy([...validAngles], {
    get() {
      throw new Error('angle getter')
    },
  })
  const throwingOptions = new Proxy({}, {
    get() {
      throw new Error('option getter')
    },
  })

  let angleJob: unknown
  assert.doesNotThrow(() => {
    angleJob = analyzer.createJob(throwingAngles, 90, THICKNESS)
  })
  assert.equal(angleJob, null)
  let optionJob: unknown
  assert.doesNotThrow(() => {
    optionJob = analyzer.createJob(
      validAngles,
      90,
      THICKNESS,
      throwingOptions,
    )
  })
  assert.equal(optionJob, null)
})

test('cumulative polygon vertices are capped before any vertex is copied', () => {
  const model = treeModel()
  const rootPolygonLength =
    MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES - 7
  const rootPolygon = guardedSparseArray<
    FoldPreviewFaceModel['polygon'][number]
  >(rootPolygonLength)
  const totalVertices =
    rootPolygonLength
    + model.faces[1].polygon.length
    + model.faces[2].polygon.length
  assert.equal(
    totalVertices,
    MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES + 1,
  )

  let prepared: unknown
  assert.doesNotThrow(() => {
    prepared = prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...model,
      faces: [
        { ...model.faces[0], polygon: rootPolygon.values },
        model.faces[1],
        model.faces[2],
      ],
    }, 'root', 'hinge-z')
  })
  assert.equal(prepared, null)
  assert.equal(rootPolygon.indexReads(), 0)
})

test('face, hinge, and joint counts reject cap plus one without index reads', () => {
  const excessiveFaces = guardedSparseArray<FoldPreviewFaceModel>(
    MAX_FOLD_PREVIEW_COLLISION_FACES + 1,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...treeModel(),
      faces: excessiveFaces.values,
    }, 'root', 'hinge-z'),
    null,
  )
  assert.equal(excessiveFaces.indexReads(), 0)

  const excessiveHinges = guardedSparseArray<FoldPreviewHingeModel>(
    MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES + 1,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...treeModel(),
      hinges: excessiveHinges.values,
    }, 'root', 'hinge-z'),
    null,
  )
  assert.equal(excessiveHinges.indexReads(), 0)

  const jointModel = treeModel()
  assert.equal(jointModel.kinematics.kind, 'tree')
  if (jointModel.kinematics.kind !== 'tree') {
    throw new Error('tree fixture unexpectedly became cyclic')
  }
  const excessiveJoints = guardedSparseArray<FoldPreviewTreeJointModel>(
    MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES + 1,
  )
  assert.equal(
    prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...jointModel,
      kinematics: {
        ...jointModel.kinematics,
        joints: excessiveJoints.values,
      },
    }, 'root', 'hinge-z'),
    null,
  )
  assert.equal(excessiveJoints.indexReads(), 0)
})

test('source collection references and lengths are read exactly once', () => {
  const model = treeModel()
  assert.equal(model.kinematics.kind, 'tree')
  if (model.kinematics.kind !== 'tree') {
    throw new Error('tree fixture unexpectedly became cyclic')
  }

  const polygonTrackers = model.faces.map((face) =>
    singleLengthReadArray(face.polygon))
  const faces = singleLengthReadArray(model.faces.map((face, index) => ({
    ...face,
    polygon: polygonTrackers[index].values,
  })))
  const hinges = singleLengthReadArray(model.hinges)
  const joints = singleLengthReadArray(model.kinematics.joints)
  let jointsReferenceReads = 0
  const kinematics = new Proxy({
    ...model.kinematics,
    joints: joints.values,
  }, {
    get(target, property, receiver) {
      if (property === 'joints') {
        jointsReferenceReads += 1
        if (jointsReferenceReads > 1) {
          throw new Error('joints reference reread')
        }
      }
      return Reflect.get(target, property, receiver)
    },
  })

  let facesReferenceReads = 0
  let hingesReferenceReads = 0
  let kinematicsReferenceReads = 0
  const guardedModel = new Proxy({
    ...model,
    faces: faces.values,
    hinges: hinges.values,
    kinematics,
  }, {
    get(target, property, receiver) {
      if (property === 'faces') {
        facesReferenceReads += 1
        if (facesReferenceReads > 1) {
          throw new Error('faces reference reread')
        }
      } else if (property === 'hinges') {
        hingesReferenceReads += 1
        if (hingesReferenceReads > 1) {
          throw new Error('hinges reference reread')
        }
      } else if (property === 'kinematics') {
        kinematicsReferenceReads += 1
        if (kinematicsReferenceReads > 1) {
          throw new Error('kinematics reference reread')
        }
      }
      return Reflect.get(target, property, receiver)
    },
  })

  assert.ok(prepareFoldPreviewTreeSingleHingeContinuousCollision(
    guardedModel,
    'root',
    'hinge-z',
  ))
  assert.equal(facesReferenceReads, 1)
  assert.equal(hingesReferenceReads, 1)
  assert.equal(kinematicsReferenceReads, 1)
  assert.equal(jointsReferenceReads, 1)
  assert.equal(faces.lengthReads(), 1)
  assert.equal(hinges.lengthReads(), 1)
  assert.equal(joints.lengthReads(), 1)
  assert.deepEqual(
    polygonTrackers.map((tracker) => tracker.lengthReads()),
    [1, 1, 1],
  )
})

test('a getter that fails after shape validation returns null without throwing', () => {
  const model = treeModel()
  const unstablePoint = new Proxy(model.faces[0].polygon[0], {
    get(target, property, receiver) {
      if (property === 'x') throw new Error('point changed during snapshot')
      return Reflect.get(target, property, receiver)
    },
  })
  let prepared: unknown
  assert.doesNotThrow(() => {
    prepared = prepareFoldPreviewTreeSingleHingeContinuousCollision({
      ...model,
      faces: [
        {
          ...model.faces[0],
          polygon: [unstablePoint, ...model.faces[0].polygon.slice(1)],
        },
        model.faces[1],
        model.faces[2],
      ],
    }, 'root', 'hinge-z')
  })
  assert.equal(prepared, null)
})

function run(
  job: NonNullable<
    ReturnType<FoldPreviewTreeSingleHingeContinuousAnalyzer['createJob']>
  >,
) {
  for (let index = 0; index < 1_000; index += 1) {
    const result = job.step(32)
    if (result.kind !== 'pending') return result
  }
  throw new Error('tree single-hinge continuous job did not terminate')
}

function assertFinitePoint(
  point: Readonly<{ x: number; y: number; z: number }>,
) {
  assert.ok([point.x, point.y, point.z].every(Number.isFinite))
}

function assertDeeplyFrozen(value: unknown, seen = new Set<object>()) {
  if (typeof value !== 'object' || value === null || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const key of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
}

function guardedSparseArray<T>(length: number): Readonly<{
  values: readonly T[]
  indexReads: () => number
}> {
  let indexReads = 0
  const values = new Proxy(new Array<T>(length), {
    get(target, property, receiver) {
      if (
        typeof property === 'string'
        && Number.isSafeInteger(Number(property))
        && Number(property) >= 0
      ) {
        indexReads += 1
      }
      return Reflect.get(target, property, receiver)
    },
  })
  return { values, indexReads: () => indexReads }
}

function singleLengthReadArray<T>(values: readonly T[]): Readonly<{
  values: readonly T[]
  lengthReads: () => number
}> {
  let lengthReads = 0
  const guardedValues = new Proxy(values, {
    get(target, property, receiver) {
      if (property === 'length') {
        lengthReads += 1
        if (lengthReads > 1) throw new Error('array length reread')
      }
      return Reflect.get(target, property, receiver)
    },
  })
  return { values: guardedValues, lengthReads: () => lengthReads }
}

function treeModel(): FoldGraphPreviewModel {
  const zStart = { vertexId: 'z-start', x: 0, z: -1 }
  const zEnd = { vertexId: 'z-end', x: 0, z: 1 }
  const xStart = { vertexId: 'x-start', x: 2, z: 1 }
  const xEnd = { vertexId: 'x-end', x: 2, z: -1 }
  const root: FoldPreviewFaceModel = {
    id: 'root',
    polygon: [
      zStart,
      { vertexId: 'root-bottom', x: -1, z: -1 },
      { vertexId: 'root-top', x: -1, z: 1 },
      zEnd,
    ],
  }
  const middle: FoldPreviewFaceModel = {
    id: 'middle',
    polygon: [
      zEnd,
      xStart,
      xEnd,
      zStart,
    ],
  }
  const leaf: FoldPreviewFaceModel = {
    id: 'leaf',
    polygon: [
      xEnd,
      xStart,
      { vertexId: 'leaf-top', x: 3, z: 1 },
      { vertexId: 'leaf-bottom', x: 3, z: -1 },
    ],
  }
  const hingeZ: FoldPreviewHingeModel = {
    edgeId: 'hinge-z',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start: zStart,
    end: zEnd,
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const hingeX: FoldPreviewHingeModel = {
    edgeId: 'hinge-x',
    leftFaceId: 'leaf',
    rightFaceId: 'middle',
    start: xStart,
    end: xEnd,
    axis: { x: 0, z: -1 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 1, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 3, maxZ: 1 },
    faces: [root, middle, leaf],
    hinges: [hingeZ, hingeX],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'middle',
          hinge: hingeZ,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'middle',
          childFaceId: 'leaf',
          hinge: hingeX,
          childRotationSign: 1,
        },
      ],
    },
  }
}

function stationaryBranchCollisionModel(): FoldGraphPreviewModel {
  const movingAxisStart = { vertexId: 'ma', x: 0.25, z: 0 }
  const movingAxisEnd = { vertexId: 'mb', x: 0.25, z: 1 }
  const obstacleAxisStart = { vertexId: 'oa', x: 0, z: 0 }
  const obstacleAxisEnd = { vertexId: 'ob', x: 0, z: 1 }
  const root: FoldPreviewFaceModel = {
    id: 'root',
    polygon: [
      movingAxisStart,
      obstacleAxisStart,
      obstacleAxisEnd,
      movingAxisEnd,
    ],
  }
  const moving: FoldPreviewFaceModel = {
    id: 'moving',
    polygon: [
      movingAxisEnd,
      { vertexId: 'moving-top-right', x: 0.75, z: 1 },
      { vertexId: 'moving-bottom-right', x: 0.75, z: 0 },
      movingAxisStart,
    ],
  }
  const obstacle: FoldPreviewFaceModel = {
    id: 'obstacle',
    polygon: [
      obstacleAxisStart,
      { vertexId: 'obstacle-bottom-left', x: -0.5, z: 0 },
      { vertexId: 'obstacle-top-left', x: -0.5, z: 1 },
      obstacleAxisEnd,
    ],
  }
  const selected: FoldPreviewHingeModel = {
    edgeId: 'selected',
    leftFaceId: 'root',
    rightFaceId: 'moving',
    start: movingAxisStart,
    end: movingAxisEnd,
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const frozen: FoldPreviewHingeModel = {
    edgeId: 'frozen',
    leftFaceId: 'root',
    rightFaceId: 'obstacle',
    start: obstacleAxisStart,
    end: obstacleAxisEnd,
    axis: { x: 0, z: 1 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'stationary-branch-project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0.125, y: 0.5 },
    worldBounds: { minX: -0.5, minZ: 0, maxX: 0.75, maxZ: 1 },
    faces: [root, moving, obstacle],
    hinges: [selected, frozen],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'moving',
          hinge: selected,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'root',
          childFaceId: 'obstacle',
          hinge: frozen,
          childRotationSign: -1,
        },
      ],
    },
  }
}

function nonCommutingCornerModel(): FoldGraphPreviewModel {
  const zStart = { vertexId: 'corner-z-start', x: 0, z: -1 }
  const zEnd = { vertexId: 'corner-z-end', x: 0, z: 1 }
  const xEnd = { vertexId: 'corner-x-end', x: 1, z: 1 }
  const root: FoldPreviewFaceModel = {
    id: 'root',
    polygon: [
      zStart,
      { vertexId: 'corner-root-bottom', x: -1, z: -1 },
      { vertexId: 'corner-root-top', x: -1, z: 1 },
      zEnd,
    ],
  }
  const middle: FoldPreviewFaceModel = {
    id: 'middle',
    polygon: [
      zEnd,
      xEnd,
      { vertexId: 'corner-middle-bottom', x: 1, z: -1 },
      zStart,
    ],
  }
  const leaf: FoldPreviewFaceModel = {
    id: 'leaf',
    polygon: [
      xEnd,
      zEnd,
      { vertexId: 'corner-leaf-left', x: 0, z: 2 },
      { vertexId: 'corner-leaf-right', x: 1, z: 2 },
    ],
  }
  const hingeZ: FoldPreviewHingeModel = {
    edgeId: 'hinge-z',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start: zStart,
    end: zEnd,
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const hingeX: FoldPreviewHingeModel = {
    edgeId: 'hinge-x',
    leftFaceId: 'leaf',
    rightFaceId: 'middle',
    start: zEnd,
    end: xEnd,
    axis: { x: 1, z: 0 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'non-commuting-project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0, y: 0.5 },
    worldBounds: { minX: -1, minZ: -1, maxX: 1, maxZ: 2 },
    faces: [root, middle, leaf],
    hinges: [hingeZ, hingeX],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'middle',
          hinge: hingeZ,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'middle',
          childFaceId: 'leaf',
          hinge: hingeX,
          childRotationSign: 1,
        },
      ],
    },
  }
}

function pairCapModel(interiorPoints: number): FoldGraphPreviewModel {
  const start = { vertexId: 'selected-start', x: 0, z: -1 }
  const end = { vertexId: 'selected-end', x: 0, z: 1 }
  const denominator = interiorPoints + 1
  const leftArc = Array.from({ length: interiorPoints }, (_, offset) => {
    const index = offset + 1
    const theta = -Math.PI / 2 - Math.PI * index / denominator
    return {
      vertexId: `left-${index}`,
      x: Math.cos(theta),
      z: Math.sin(theta),
    }
  })
  const rightArc = Array.from({ length: interiorPoints }, (_, offset) => {
    const index = offset + 1
    const theta = Math.PI / 2 - Math.PI * index / denominator
    return {
      vertexId: `right-${index}`,
      x: Math.cos(theta),
      z: Math.sin(theta),
    }
  })
  const childStart = rightArc[499]
  const childEnd = rightArc[500]
  if (!childStart || !childEnd) throw new Error('pair-cap child edge missing')
  const outward = {
    vertexId: 'leaf-outward',
    x: 1.1 * (childStart.x + childEnd.x) / 2,
    z: 1.1 * (childStart.z + childEnd.z) / 2,
  }
  const root: FoldPreviewFaceModel = {
    id: 'root',
    polygon: [start, ...leftArc, end],
  }
  const middle: FoldPreviewFaceModel = {
    id: 'middle',
    polygon: [end, ...rightArc, start],
  }
  const leaf: FoldPreviewFaceModel = {
    id: 'leaf',
    polygon: [childStart, outward, childEnd],
  }
  const selected: FoldPreviewHingeModel = {
    edgeId: 'selected',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start,
    end,
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const childDeltaX = childEnd.x - childStart.x
  const childDeltaZ = childEnd.z - childStart.z
  const childLength = Math.hypot(childDeltaX, childDeltaZ)
  const child: FoldPreviewHingeModel = {
    edgeId: 'child',
    leftFaceId: 'leaf',
    rightFaceId: 'middle',
    start: childStart,
    end: childEnd,
    axis: {
      x: childDeltaX / childLength,
      z: childDeltaZ / childLength,
    },
    assignment: 'mountain',
    rotationSign: 1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'pair-cap-project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 1.1, maxZ: 1 },
    faces: [root, middle, leaf],
    hinges: [selected, child],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'middle',
          hinge: selected,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'middle',
          childFaceId: 'leaf',
          hinge: child,
          childRotationSign: -1,
        },
      ],
    },
  }
}
