import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  prepareFoldPreviewPhysicalGrab,
  resolveFoldPreviewPhysicalGrabTarget,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabRay,
  type FoldPreviewPhysicalGrabSession,
} from '../src/lib/foldPreviewPhysicalGrab.ts'

test('physical grab uses one stable mapping distinct from vertical parameter drag', () => {
  assert.equal(FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING, 'physical_grab_v2')
})

test('preparation snapshots one off-axis grab at the applied pose', () => {
  const input = prepareInput(30)
  const result = prepareFoldPreviewPhysicalGrab(input)
  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail('expected a ready session')

  assert.equal(result.session.mapping, 'physical_grab_v2')
  assert.equal(result.session.appliedAngleDegrees, 30)
  assert.equal(result.session.orbitRadius, 1)
  assert.deepEqual(result.session.axisUnit, { x: 0, y: 0, z: 1 })
  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result.session))
  assert.ok(Object.isFrozen(result.session.axisOrigin))
  assert.ok(Object.isFrozen(result.session.axisUnit))
  assert.ok(Object.isFrozen(result.session.orbitCenter))
  assert.ok(Object.isFrozen(result.session.restRadialUnit))
  assert.ok(Object.isFrozen(result.session.positiveTangentUnit))

  ;(input.axisStart as { x: number }).x = 100
  ;(input.grabRestWorldPoint as { x: number }).x = 100
  ;(input.grabWorldPoint as { x: number }).x = 100
  assert.deepEqual(result.session.axisOrigin, { x: 0, y: 0, z: 0 })
  assert.equal(result.session.orbitRadius, 1)
})

test('front-facing rays resolve forward and reverse physical motion', () => {
  const session = readySession(30)
  const forward = resolve(session, 30, normalRayThrough(orbitPoint(60)))
  assert.equal(forward.kind, 'unverified_target')
  if (forward.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(forward.angleDegrees, 60)
  assert.ok(Math.abs(forward.rawAngleDegrees - 60) < 0.001)
  assert.equal(forward.endpoint, null)
  assert.ok(forward.missDistance < 1e-6)

  const reverseSession = readySession(90)
  const reverse = resolve(reverseSession, 90, normalRayThrough(orbitPoint(50)))
  assert.equal(reverse.kind, 'unverified_target')
  if (reverse.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(reverse.angleDegrees, 50)
})

test('moving rotation sign changes the orbit without changing the non-negative angle', () => {
  const session = readySession(30, -1)
  const target = resolve(
    session,
    30,
    normalRayThrough(orbitPoint(60, -1)),
  )
  assert.equal(target.kind, 'unverified_target')
  if (target.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(target.angleDegrees, 60)
  assert.ok(target.orbitWorldPoint.y < 0)
})

test('translated non-Z hinge axes use the same physical angle contract', () => {
  const applied = 30
  const grab = xAxisOrbitPoint(applied)
  const prepared = prepareFoldPreviewPhysicalGrab({
    contextKey: 'translated-axis',
    axisStart: { x: 2, y: 3, z: 4 },
    axisEnd: { x: 5, y: 3, z: 4 },
    movingRotationSign: 1,
    appliedAngleDegrees: applied,
    grabRestWorldPoint: xAxisOrbitPoint(0),
    grabWorldPoint: grab,
    startRay: xAxisNormalRayThrough(grab),
    minimumOrbitRadius: 0.01,
  })
  assert.equal(prepared.kind, 'ready')
  if (prepared.kind !== 'ready') assert.fail(`prepare failed: ${prepared.reason}`)
  const targetPoint = xAxisOrbitPoint(60)
  const result = resolveFoldPreviewPhysicalGrabTarget(prepared.session, {
    contextKey: 'translated-axis',
    referenceAngleDegrees: applied,
    ray: xAxisNormalRayThrough(targetPoint),
  })
  assert.equal(result.kind, 'unverified_target')
  if (result.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(result.angleDegrees, 60)
  assert.ok(Math.abs(result.orbitWorldPoint.x - 3) < 1e-9)
  assert.ok(Math.abs(result.orbitWorldPoint.y - 3.5) < 0.002)
})

test('zero and 180 degrees are explicit endpoint targets', () => {
  const zeroSession = readySession(20)
  const zero = resolve(zeroSession, 20, normalRayThrough(orbitPoint(0)))
  assert.equal(zero.kind, 'unverified_target')
  if (zero.kind !== 'unverified_target') assert.fail('expected zero')
  assert.equal(zero.angleDegrees, 0)
  assert.equal(zero.endpoint, 'zero')

  const flatSession = readySession(160)
  const flat = resolve(flatSession, 160, normalRayThrough(orbitPoint(180)))
  assert.equal(flat.kind, 'unverified_target')
  if (flat.kind !== 'unverified_target') assert.fail('expected 180')
  assert.equal(flat.angleDegrees, 180)
  assert.equal(flat.endpoint, 'one_eighty')
})

test('analytic stationary roots preserve non-endpoint targets beside both limits', () => {
  const cases = [
    { reference: 9, target: 0.1 },
    { reference: 45, target: 1 },
    { reference: 135, target: 179 },
    { reference: 171, target: 179.9 },
  ]
  for (const movingRotationSign of [1, -1] as const) {
    for (const fixture of cases) {
      const session = readySession(fixture.reference, movingRotationSign)
      const result = resolve(
        session,
        fixture.reference,
        normalRayThrough(orbitPoint(fixture.target, movingRotationSign)),
      )
      assert.equal(
        result.kind,
        'unverified_target',
        `${movingRotationSign}:${fixture.reference}->${fixture.target}`,
      )
      if (result.kind !== 'unverified_target') assert.fail('expected a target')
      assert.equal(result.angleDegrees, fixture.target)
      assert.equal(result.endpoint, null)
      assert.ok(Math.abs(result.rawAngleDegrees - fixture.target) < 0.001)
    }
  }
})

test('analytic stationary roots recover a minimum hidden inside the first old grid cell', () => {
  const input = prepareInput(30)
  const prepared = prepareFoldPreviewPhysicalGrab({
    ...input,
    startRay: {
      ...input.startRay,
      minimumDistance: 0.001,
      maximumDistance: 20,
    },
  })
  assert.equal(prepared.kind, 'ready')
  if (prepared.kind !== 'ready') assert.fail(`prepare failed: ${prepared.reason}`)

  const origin = {
    x: -4.5814216463,
    y: -3.5608384558,
    z: -5.3256888203,
  }
  const through = {
    x: 1.4632251477,
    y: -0.2167745735,
    z: -1.7527658036,
  }
  const delta = {
    x: through.x - origin.x,
    y: through.y - origin.y,
    z: through.z - origin.z,
  }
  const magnitude = Math.hypot(delta.x, delta.y, delta.z)
  const result = resolveFoldPreviewPhysicalGrabTarget(prepared.session, {
    contextKey: 'context',
    referenceAngleDegrees: 5.00667919870466,
    ray: {
      origin,
      direction: {
        x: delta.x / magnitude,
        y: delta.y / magnitude,
        z: delta.z / magnitude,
      },
      minimumDistance: 0.001,
      maximumDistance: 20,
    },
  })
  assert.equal(result.kind, 'unverified_target')
  if (result.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(result.endpoint, null)
  assert.equal(result.angleDegrees, 0.5)
  assert.ok(Math.abs(result.rawAngleDegrees - 0.496) < 0.01)
})

test('targets quantize to tenths while preserving the raw solver angle', () => {
  const session = readySession(40)
  const ray = normalRayThrough(orbitPoint(52.04))
  const result = resolve(
    session,
    40,
    ray,
  )
  assert.equal(result.kind, 'unverified_target')
  if (result.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(result.angleDegrees, 52)
  assert.ok(Math.abs(result.rawAngleDegrees - 52.04) < 0.001)
  assert.ok(Math.abs(
    result.missDistance - Math.hypot(
      result.orbitWorldPoint.x - ray.origin.x,
      result.orbitWorldPoint.y - ray.origin.y,
    ),
  ) < 1e-12)
})

test('unit pointer rays support the raycaster default infinite maximum distance', () => {
  const input = prepareInput(30)
  const startRay = {
    ...input.startRay,
    maximumDistance: Number.POSITIVE_INFINITY,
  }
  const prepared = prepareFoldPreviewPhysicalGrab({ ...input, startRay })
  assert.equal(prepared.kind, 'ready')
  if (prepared.kind !== 'ready') assert.fail(`prepare failed: ${prepared.reason}`)
  const result = resolveFoldPreviewPhysicalGrabTarget(prepared.session, {
    contextKey: 'context',
    referenceAngleDegrees: 30,
    ray: {
      ...normalRayThrough(orbitPoint(40)),
      maximumDistance: Number.POSITIVE_INFINITY,
    },
  })
  assert.equal(result.kind, 'unverified_target')
  if (result.kind !== 'unverified_target') assert.fail('expected a target')
  assert.equal(result.angleDegrees, 40)
})

test('side-on rays keep the branch clearly nearest the reference angle', () => {
  const session = readySession(40)
  const sideRay = sideRayThroughHeight(Math.sin(degreesToRadians(60)))
  const firstBranch = resolve(session, 40, sideRay)
  assert.equal(firstBranch.kind, 'unverified_target')
  if (firstBranch.kind !== 'unverified_target') assert.fail('expected first branch')
  assert.equal(firstBranch.angleDegrees, 60)
  assert.equal(firstBranch.equivalentCandidateCount, 2)

  const secondBranch = resolve(session, 140, sideRay)
  assert.equal(secondBranch.kind, 'unverified_target')
  if (secondBranch.kind !== 'unverified_target') assert.fail('expected second branch')
  assert.equal(secondBranch.angleDegrees, 120)
  assert.equal(secondBranch.equivalentCandidateCount, 2)
})

test('equidistant side-on branches are rejected instead of guessed', () => {
  const session = readySession(40)
  const result = resolve(
    session,
    90,
    sideRayThroughHeight(Math.sin(degreesToRadians(60))),
  )
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'ambiguous_projection',
  })
})

test('a tangent side-on projection is rejected for insufficient sensitivity', () => {
  const session = readySession(80)
  const result = resolve(session, 80, sideRayThroughHeight(1))
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'unstable_projection',
  })
})

test('one pointer sample cannot jump to a branch over 45 degrees away', () => {
  const session = readySession(30)
  const result = resolve(session, 30, normalRayThrough(orbitPoint(100)))
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'branch_jump',
  })
})

test('context changes reject every otherwise valid target', () => {
  const session = readySession(30)
  const result = resolveFoldPreviewPhysicalGrabTarget(session, {
    contextKey: 'other-context',
    referenceAngleDegrees: 30,
    ray: normalRayThrough(orbitPoint(40)),
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'stale_context',
  })
})

test('points behind the pointer half-ray expose no candidate', () => {
  const session = readySession(30)
  const result = resolve(session, 30, {
    origin: { x: 0, y: 0, z: -5 },
    direction: { x: 0, y: 0, z: -1 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'no_visible_candidate',
  })
})

test('a pointer ray too far from the circular orbit is rejected', () => {
  const session = readySession(30)
  const result = resolve(session, 30, {
    origin: { x: 10, y: 0, z: 5 },
    direction: { x: 0, y: 0, z: -1 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'target_too_far',
  })
})

test('preparation rejects a start ray that misses its supplied grab point', () => {
  const input = prepareInput(30)
  const result = prepareFoldPreviewPhysicalGrab({
    ...input,
    startRay: {
      origin: { x: 3, y: 3, z: 5 },
      direction: { x: 0, y: 0, z: -1 },
      minimumDistance: 0.1,
      maximumDistance: 10,
    },
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'start_ray_miss',
  })
})

test('preparation independently rejects a displayed pose that disagrees with the applied angle', () => {
  const displayedAtSixty = orbitPoint(60)
  const result = prepareFoldPreviewPhysicalGrab({
    ...prepareInput(30),
    grabWorldPoint: displayedAtSixty,
    startRay: normalRayThrough(displayedAtSixty),
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'pose_mismatch',
  })
})

test('preparation rejects coordinate scales that cannot preserve pose-sync tolerance', () => {
  const centerX = 1e15
  const angle = degreesToRadians(150)
  const grabWorldPoint = {
    x: centerX + Math.cos(angle),
    y: Math.sin(angle),
    z: 0,
  }
  const result = prepareFoldPreviewPhysicalGrab({
    contextKey: 'unrepresentable-pose-sync',
    axisStart: { x: centerX, y: 0, z: 0 },
    axisEnd: { x: centerX, y: 0, z: 2 },
    movingRotationSign: 1,
    appliedAngleDegrees: 30,
    grabRestWorldPoint: { x: centerX + 1, y: 0, z: 0 },
    grabWorldPoint,
    startRay: normalRayThrough(grabWorldPoint),
    minimumOrbitRadius: 0.01,
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'numeric',
  })
})

test('preparation rejects a tangent start whose angle cannot be tracked stably', () => {
  const grab = orbitPoint(90)
  const result = prepareFoldPreviewPhysicalGrab({
    ...prepareInput(90),
    grabWorldPoint: grab,
    startRay: {
      origin: { x: 5, y: grab.y, z: 0 },
      direction: { x: -1, y: 0, z: 0 },
      minimumDistance: 0.1,
      maximumDistance: 10,
    },
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'unresolvable_start',
  })
})

test('near-axis grabs and degenerate hinge axes fail closed', () => {
  const nearAxis = prepareFoldPreviewPhysicalGrab({
    ...prepareInput(0),
    grabRestWorldPoint: { x: 0.001, y: 0, z: 0 },
    grabWorldPoint: { x: 0.001, y: 0, z: 0 },
    startRay: normalRayThrough({ x: 0.001, y: 0, z: 0 }),
    minimumOrbitRadius: 0.01,
  })
  assert.deepEqual(nearAxis, {
    kind: 'rejected',
    reason: 'grab_on_axis',
  })

  const degenerate = prepareFoldPreviewPhysicalGrab({
    ...prepareInput(0),
    axisEnd: { x: 0, y: 0, z: 0 },
  })
  assert.deepEqual(degenerate, {
    kind: 'rejected',
    reason: 'degenerate_axis',
  })
})

test('invalid signs, angles, coordinates, radii, and rays are rejected', () => {
  const invalidInputs = [
    { ...prepareInput(0), movingRotationSign: 0 as never },
    { ...prepareInput(0), appliedAngleDegrees: -0.1 },
    { ...prepareInput(0), appliedAngleDegrees: 180.1 },
    {
      ...prepareInput(0),
      grabRestWorldPoint: { x: Number.NaN, y: 0, z: 0 },
    },
    {
      ...prepareInput(0),
      grabWorldPoint: { x: Number.NaN, y: 0, z: 0 },
    },
    { ...prepareInput(0), minimumOrbitRadius: 0 },
    {
      ...prepareInput(0),
      startRay: {
        ...normalRayThrough(orbitPoint(0)),
        direction: { x: 0, y: 0, z: 0 },
      },
    },
    {
      ...prepareInput(0),
      startRay: {
        ...normalRayThrough(orbitPoint(0)),
        direction: { x: 0, y: 0, z: -2 },
      },
    },
  ]
  for (const input of invalidInputs) {
    assert.deepEqual(prepareFoldPreviewPhysicalGrab(input), {
      kind: 'rejected',
      reason: 'invalid_input',
    })
  }
})

test('unrepresentable squared orbit work is rejected before searching', () => {
  const result = prepareFoldPreviewPhysicalGrab({
    contextKey: 'huge',
    axisStart: { x: 0, y: 0, z: 0 },
    axisEnd: { x: 0, y: 0, z: 1 },
    movingRotationSign: 1,
    appliedAngleDegrees: 0,
    grabRestWorldPoint: { x: 1e200, y: 0, z: 0 },
    grabWorldPoint: { x: 1e200, y: 0, z: 0 },
    startRay: {
      origin: { x: 1e200, y: 0, z: 1 },
      direction: { x: 0, y: 0, z: -1 },
      minimumDistance: 0,
      maximumDistance: 2,
    },
    minimumOrbitRadius: 1,
  })
  assert.deepEqual(result, {
    kind: 'rejected',
    reason: 'numeric',
  })
})

test('resolve requires the frozen session contract and the same ray-distance range', () => {
  const session = readySession(30)
  const mutableSession = {
    ...session,
    axisOrigin: { ...session.axisOrigin },
  } as FoldPreviewPhysicalGrabSession
  assert.deepEqual(resolve(
    mutableSession,
    30,
    normalRayThrough(orbitPoint(40)),
  ), {
    kind: 'rejected',
    reason: 'invalid_session',
  })

  assert.deepEqual(resolve(session, 30, {
    ...normalRayThrough(orbitPoint(40)),
    maximumDistance: 11,
  }), {
    kind: 'rejected',
    reason: 'invalid_input',
  })
})

test('malformed resolve samples fail closed', () => {
  const session = readySession(30)
  const invalidSamples = [
    {
      contextKey: 'context',
      referenceAngleDegrees: Number.NaN,
      ray: normalRayThrough(orbitPoint(40)),
    },
    {
      contextKey: 'context',
      referenceAngleDegrees: 30,
      ray: {
        ...normalRayThrough(orbitPoint(40)),
        origin: { x: Number.POSITIVE_INFINITY, y: 0, z: 0 },
      },
    },
    {
      contextKey: '',
      referenceAngleDegrees: 30,
      ray: normalRayThrough(orbitPoint(40)),
    },
  ]
  for (const input of invalidSamples) {
    assert.deepEqual(resolveFoldPreviewPhysicalGrabTarget(session, input), {
      kind: 'rejected',
      reason: 'invalid_input',
    })
  }
})

test('valid results are deeply frozen detached snapshots', () => {
  const session = readySession(30)
  const ray = normalRayThrough(orbitPoint(45))
  const result = resolve(session, 30, ray)
  assert.equal(result.kind, 'unverified_target')
  if (result.kind !== 'unverified_target') assert.fail('expected a target')
  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result.orbitWorldPoint))
  ;(ray.origin as { x: number }).x = 100
  assert.ok(Math.abs(result.orbitWorldPoint.x - Math.SQRT1_2) < 0.002)

  const rejected = resolveFoldPreviewPhysicalGrabTarget(session, {
    contextKey: 'stale',
    referenceAngleDegrees: 30,
    ray: normalRayThrough(orbitPoint(45)),
  })
  assert.ok(Object.isFrozen(rejected))
})

test('the solver is history independent and stays inside its fixed work budget', () => {
  const session = readySession(30)
  const input = {
    contextKey: 'context',
    referenceAngleDegrees: 30,
    ray: normalRayThrough(orbitPoint(55)),
  }
  const first = resolveFoldPreviewPhysicalGrabTarget(session, input)
  const second = resolveFoldPreviewPhysicalGrabTarget(session, input)
  assert.deepEqual(first, second)
  assert.equal(first.kind, 'unverified_target')
  if (first.kind !== 'unverified_target') assert.fail('expected a target')
  assert.ok(first.evaluationCount <= 256)
  assert.ok(first.rootEvaluationCount <= 1_920)
  assert.ok(first.stationaryCandidateCount <= 4)
  assert.ok(first.boundaryCandidateCount <= 4)
})

function readySession(
  appliedAngleDegrees: number,
  movingRotationSign: 1 | -1 = 1,
) {
  const result = prepareFoldPreviewPhysicalGrab(
    prepareInput(appliedAngleDegrees, movingRotationSign),
  )
  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)
  return result.session
}

function prepareInput(
  appliedAngleDegrees: number,
  movingRotationSign: 1 | -1 = 1,
) {
  const grabWorldPoint = orbitPoint(appliedAngleDegrees, movingRotationSign)
  return {
    contextKey: 'context',
    axisStart: { x: 0, y: 0, z: 0 },
    axisEnd: { x: 0, y: 0, z: 2 },
    movingRotationSign,
    appliedAngleDegrees,
    grabRestWorldPoint: orbitPoint(0),
    grabWorldPoint,
    startRay: normalRayThrough(grabWorldPoint),
    minimumOrbitRadius: 0.01,
  }
}

function resolve(
  session: FoldPreviewPhysicalGrabSession,
  referenceAngleDegrees: number,
  ray: FoldPreviewPhysicalGrabRay,
) {
  return resolveFoldPreviewPhysicalGrabTarget(session, {
    contextKey: 'context',
    referenceAngleDegrees,
    ray,
  })
}

function orbitPoint(
  angleDegrees: number,
  movingRotationSign: 1 | -1 = 1,
): FoldPreviewPhysicalGrabPoint {
  const angle = degreesToRadians(angleDegrees) * movingRotationSign
  return {
    x: Math.cos(angle),
    y: Math.sin(angle),
    z: 0,
  }
}

function normalRayThrough(
  point: FoldPreviewPhysicalGrabPoint,
): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: point.x, y: point.y, z: point.z + 5 },
    direction: { x: 0, y: 0, z: -1 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  }
}

function sideRayThroughHeight(height: number): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: 5, y: height, z: 0 },
    direction: { x: -1, y: 0, z: 0 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  }
}

function xAxisOrbitPoint(angleDegrees: number): FoldPreviewPhysicalGrabPoint {
  const angle = degreesToRadians(angleDegrees)
  return {
    x: 3,
    y: 3 + Math.cos(angle),
    z: 4 + Math.sin(angle),
  }
}

function xAxisNormalRayThrough(
  point: FoldPreviewPhysicalGrabPoint,
): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: point.x + 5, y: point.y, z: point.z },
    direction: { x: -1, y: 0, z: 0 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  }
}

function degreesToRadians(value: number) {
  return value * Math.PI / 180
}
