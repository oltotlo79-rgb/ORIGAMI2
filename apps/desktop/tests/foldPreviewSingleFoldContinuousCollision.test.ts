import assert from 'node:assert/strict'
import test from 'node:test'

import {
  prepareFoldPreviewSingleFoldContinuousCollision,
  type FoldPreviewSingleFoldContinuousAnalyzer,
} from '../src/lib/foldPreviewSingleFoldContinuousCollision.ts'
import type {
  FoldPreviewFaceModel,
  SingleFoldPreviewModel,
} from '../src/lib/foldPreviewModel.ts'

test('a rectangular single fold certifies the complete 0-to-170-degree path', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    rectangleModel(),
    'left',
  )
  assert.ok(analyzer)
  assert.equal(analyzer.trianglePairs, 4)
  const job = analyzer.createJob(0, 170, 0.1)
  assert.ok(job)
  assert.deepEqual(job.step(1), {
    kind: 'clear',
    certifiedSafeThrough: 1,
    stopTime: 1,
    stats: {
      intervalTests: 1,
      pointTests: 2,
      pointCacheHits: 0,
      maximumDepthReached: 0,
    },
  })
})

test('a certified path has no point-policy gap immediately after the flat pose', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    rectangleModel(),
    'left',
  )
  assert.ok(analyzer)
  for (const angle of [1e-12, 1e-8, 0.0005]) {
    const point = analyzer.createJob(angle, angle, 0.1)
    assert.ok(point)
    assert.equal(run(point).kind, 'clear')
  }
})

test('either fixed face and either fold assignment use the same safe path contract', () => {
  const mountain = rectangleModel()
  const valley: SingleFoldPreviewModel = {
    ...mountain,
    hinge: {
      ...mountain.hinge,
      assignment: 'valley',
      rotationSign: -1,
    },
  }
  for (const [model, fixedFaceId] of [
    [mountain, 'right'],
    [valley, 'left'],
    [valley, 'right'],
  ] as const) {
    const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
      model,
      fixedFaceId,
    )
    const job = analyzer?.createJob(15, 150, 0.1)
    assert.ok(analyzer && job)
    assert.equal(run(job).kind, 'clear')
  }
})

test('unsupported remote triangles require and pass strict swept-AABB separation', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    axiallyExtendedModel(),
    'left',
  )
  assert.ok(analyzer)
  assert.ok(analyzer.staticallySupportedTrianglePairs > 0)
  assert.ok(analyzer.staticallySupportedTrianglePairs < analyzer.trianglePairs)
  const job = analyzer.createJob(10, 60, 0.05)
  assert.ok(job)
  const first = job.step(1)
  const result = first.kind === 'pending' ? run(job) : first
  assert.equal(result.kind, 'clear', JSON.stringify(result))
  assert.ok(result.stats.intervalTests >= 1)
})

test('the exact 180-degree singularity is approached but never certified', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    rectangleModel(),
    'left',
  )
  const job = analyzer?.createJob(0, 180, 0.1, {
    maxDepth: 8,
    minTimeSpan: 2 ** -20,
    maxIntervalTests: 100,
  })
  assert.ok(analyzer && job)
  const result = run(job)
  assert.equal(result.kind, 'indeterminate')
  assert.ok(result.kind === 'indeterminate')
  assert.ok(result.certifiedSafeThrough > 0.99)
  assert.ok(result.certifiedSafeThrough < 1)
  assert.equal(result.stopTime, result.certifiedSafeThrough)
  assert.equal(result.unresolvedBracket[1], 1)
  assert.match(result.reason, /unsupported_flat_fold|uncertified_interval/u)
})

test('an analytic interval cannot overrule an indeterminate target policy', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    rectangleModel(),
    'left',
  )
  assert.ok(analyzer)
  for (const [start, target, thickness] of [
    [0, 179.999999, 0.1],
    [0, 90, 1e-14],
  ] as const) {
    const job = analyzer.createJob(start, target, thickness)
    assert.ok(job)
    const result = run(job)
    assert.equal(result.kind, 'indeterminate')
    assert.ok(result.kind === 'indeterminate')
    assert.equal(result.certifiedSafeThrough, 0)
    assert.equal(result.unresolvedBracket[1], 1)
  }
})

test('static support cannot skip a point-policy gap for ultra-thin paper', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    rectangleModel(),
    'left',
  )
  assert.ok(analyzer)
  for (const [thickness, unknownProbeAngle] of [
    [1e-14, 85],
    [1e-13, 20],
  ] as const) {
    const probe = analyzer.createJob(
      unknownProbeAngle,
      unknownProbeAngle,
      thickness,
    )
    assert.ok(probe)
    assert.equal(run(probe).kind, 'indeterminate')

    const path = analyzer.createJob(0, 170, thickness)
    assert.ok(path)
    const result = run(path)
    assert.equal(result.kind, 'indeterminate')
    assert.ok(result.kind === 'indeterminate')
    assert.equal(result.certifiedSafeThrough, 0)
    assert.equal(result.reason, 'hinge_interval_numerical_margin')
    assert.deepEqual(result.unresolvedBracket, [0, 1])
  }
})

test('zero physical thickness remains explicitly indeterminate at the start pose', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    rectangleModel(),
    'left',
  )
  const job = analyzer?.createJob(0, 90, 0)
  assert.ok(analyzer && job)
  const result = job.step(1)
  assert.equal(result.kind, 'indeterminate')
  assert.equal(result.kind === 'indeterminate' && result.certifiedSafeThrough, 0)
  assert.equal(
    result.kind === 'indeterminate' && result.reason,
    'hinge_zero_thickness',
  )
})

test('an endpoint-outside concave overlap is blocking before motion', () => {
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(
    concaveOutsideModel(),
    'left',
  )
  const job = analyzer?.createJob(0, 90, 0.1)
  assert.ok(analyzer && job)
  const result = job.step(1)
  assert.equal(result.kind, 'blocked')
  assert.ok(result.kind === 'blocked')
  assert.equal(result.certifiedSafeThrough, 0)
  assert.deepEqual(result.unsafeBracket, [0, 0])
  assert.equal(result.blocker?.hingeDecisionKind, 'outside_hinge_penetration')
  assert.ok(Object.isFrozen(result.blocker))
})

test('prepared continuous geometry is an immutable model snapshot', () => {
  const model = rectangleModel()
  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(model, 'left')
  assert.ok(analyzer)

  model.faces[0].polygon[1].x = 100
  model.faces[1].polygon[1].z = 100
  model.hinge.start.x = 100
  model.hinge.axis.z = -1

  const job = analyzer.createJob(0, 120, 0.1)
  assert.ok(job)
  assert.equal(run(job).kind, 'clear')
})

test('invalid models, requests, and unbounded triangle work fail closed', () => {
  const model = rectangleModel()
  assert.equal(
    prepareFoldPreviewSingleFoldContinuousCollision(model, 'missing'),
    null,
  )
  assert.equal(prepareFoldPreviewSingleFoldContinuousCollision({
    ...model,
    hinge: {
      ...model.hinge,
      axis: { x: 1, z: 0 },
    },
  }, 'left'), null)

  const analyzer = prepareFoldPreviewSingleFoldContinuousCollision(model, 'left')
  assert.ok(analyzer)
  assert.equal(analyzer.createJob(-1, 90, 0.1), null)
  assert.equal(analyzer.createJob(0, 181, 0.1), null)
  assert.equal(analyzer.createJob(0, 90, Number.NaN), null)
  assert.equal(analyzer.createJob(0, 90, -0.1), null)
  assert.equal(analyzer.createJob(0, 90, 0.1, { maxDepth: -1 }), null)
})

function run(
  job: NonNullable<ReturnType<FoldPreviewSingleFoldContinuousAnalyzer['createJob']>>,
) {
  for (let index = 0; index < 1_000; index += 1) {
    const result = job.step(32)
    if (result.kind !== 'pending') return result
  }
  throw new Error('single-fold continuous job did not terminate')
}

function rectangleModel(): SingleFoldPreviewModel {
  const start = { vertexId: 'start', x: 0, z: 0 }
  const end = { vertexId: 'end', x: 0, z: 1 }
  const left: FoldPreviewFaceModel = {
    id: 'left',
    polygon: [
      start,
      { vertexId: 'left-bottom', x: -1, z: 0 },
      { vertexId: 'left-top', x: -1, z: 1 },
      end,
    ],
  }
  const right: FoldPreviewFaceModel = {
    id: 'right',
    polygon: [
      end,
      { vertexId: 'right-top', x: 1, z: 1 },
      { vertexId: 'right-bottom', x: 1, z: 0 },
      start,
    ],
  }
  return {
    kind: 'single_fold',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: -1, minZ: 0, maxX: 1, maxZ: 1 },
    faces: [left, right],
    fixedFace: left,
    movingFace: right,
    hinge: {
      edgeId: 'hinge',
      leftFaceId: 'left',
      rightFaceId: 'right',
      start,
      end,
      axis: { x: 0, z: 1 },
      assignment: 'mountain',
      rotationSign: 1,
    },
  }
}

function concaveOutsideModel(): SingleFoldPreviewModel {
  const model = rectangleModel()
  const start = model.hinge.start
  const end = model.hinge.end
  const left: FoldPreviewFaceModel = {
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
  }
  const right: FoldPreviewFaceModel = {
    id: 'right',
    polygon: [
      end,
      { vertexId: 'right-notch', x: 0.5, z: 1.5 },
      { vertexId: 'right-lobe-left', x: 0.5, z: 2 },
      { vertexId: 'right-lobe-right', x: 2, z: 2 },
      { vertexId: 'right-bottom', x: 2, z: 0 },
      start,
    ],
  }
  return {
    ...model,
    faces: [left, right],
    fixedFace: left,
    movingFace: right,
    worldBounds: { minX: -1, minZ: 0, maxX: 2, maxZ: 2 },
  }
}

function axiallyExtendedModel(): SingleFoldPreviewModel {
  const model = rectangleModel()
  const start = model.hinge.start
  const end = model.hinge.end
  const left: FoldPreviewFaceModel = {
    id: 'left',
    polygon: [
      start,
      { vertexId: 'left-bottom', x: -1, z: 0 },
      { vertexId: 'left-join-bottom', x: -1, z: 1 },
      { vertexId: 'left-extension-outer-bottom', x: -2, z: 1 },
      { vertexId: 'left-extension-outer-top', x: -2, z: 2 },
      { vertexId: 'left-extension-top', x: -0.5, z: 2 },
      { vertexId: 'left-join-top', x: -0.5, z: 1 },
      end,
    ],
  }
  return {
    ...model,
    faces: [left, model.faces[1]],
    fixedFace: left,
    worldBounds: { minX: -2, minZ: 0, maxX: 1, maxZ: 2 },
  }
}
