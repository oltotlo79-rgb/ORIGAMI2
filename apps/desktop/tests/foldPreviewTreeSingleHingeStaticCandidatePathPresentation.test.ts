import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  createFoldPreviewTreeSingleHingeStaticCandidatePathJob,
  type FoldPreviewTreeSingleHingeStaticCandidatePathJob,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCandidatePath.ts'
import {
  createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation,
  FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCandidatePathPresentation.ts'
import {
  deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCorrectionCandidates.ts'
import {
  prepareFoldPreviewTreeSingleHingeContinuousCollision,
  type FoldPreviewTreeSingleHingeContinuousAnalyzer,
} from '../src/lib/foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  prepareFoldPreviewTreeMotionContext,
  type FoldPreviewTreeMotionContext,
} from '../src/lib/foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from '../src/lib/foldPreviewTreeScenePose.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from '../src/lib/foldPreviewModel.ts'

const COLLISION_THICKNESS = 0.02
const CLEARANCE = 0.005
const MAXIMUM_TRANSLATION = 0.01
const MAXIMUM_ANGLE_DELTA_DEGREES = 30

test('an exact bound certificate becomes a detached Japanese display DTO', () => {
  const fixture = certifiedFixture()
  const presentation =
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      fixture.certificate,
    )
  assert.ok(presentation)

  assert.equal(
    presentation.version,
    FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION,
  )
  assert.equal(
    presentation.kind,
    'certified_static_candidate_path_presentation',
  )
  assert.deepEqual(presentation.identity, {
    projectId: 'stationary-branch-project',
    revision: 1,
    selectedHingeEdgeId: 'selected',
  })
  assert.deepEqual(presentation.candidate, {
    rank: fixture.certificate.selectedCandidate.rank,
  })
  assert.deepEqual(presentation.angles, {
    sourceDegrees:
      fixture.certificate.path.sourceSelectedAngleDegrees,
    targetDegrees:
      fixture.certificate.path.targetSelectedAngleDegrees,
    deltaDegrees:
      fixture.certificate.path.targetSelectedAngleDegrees
      - fixture.certificate.path.sourceSelectedAngleDegrees,
    absoluteDeltaDegrees:
      fixture.certificate.path.targetSelectedAngleDegrees
      - fixture.certificate.path.sourceSelectedAngleDegrees,
    direction: 'increasing',
  })
  assert.deepEqual(
    presentation.continuous.stats,
    fixture.certificate.path.stats,
  )
  assert.deepEqual(
    presentation.continuous.aggregateStats,
    fixture.certificate.aggregateStats,
  )
  assert.equal(
    presentation.continuous.precedingAttemptCount,
    fixture.certificate.precedingAttempts.length,
  )
  assert.deepEqual(
    presentation.staticInteractionSummary,
    fixture.certificate.staticAnalysis,
  )
  assert.deepEqual(presentation.workBounds, fixture.certificate.workBounds)
  assert.deepEqual(presentation.safety, {
    analysisOnly: true,
    staticCandidateRevalidated: true,
    continuousCandidatePathCertified: true,
    runtimeRequestBound: false,
    activeRequestLeaseBound: false,
    startScenePoseMatched: false,
    sceneApplied: false,
    autoApplicable: false,
  })
  assert.match(presentation.badgeText, /解析上の補正候補1/u)
  assert.match(presentation.badgeText, /静的／連続経路確認済み/u)
  assert.match(presentation.badgeText, /現在姿勢未照合/u)
  assert.match(presentation.accessibleText, /静的衝突検査/u)
  assert.match(presentation.accessibleText, /連続経路検査/u)
  assert.match(presentation.limitation, /現在も有効であることは保証されません/u)
  assert.match(presentation.limitation, /この表示から.+適用できません/u)

  assert.notStrictEqual(
    presentation.continuous.stats,
    fixture.certificate.path.stats,
  )
  assert.notStrictEqual(
    presentation.continuous.aggregateStats,
    fixture.certificate.aggregateStats,
  )
  assert.notStrictEqual(
    presentation.staticInteractionSummary,
    fixture.certificate.staticAnalysis,
  )
  assert.notStrictEqual(
    presentation.workBounds,
    fixture.certificate.workBounds,
  )
  assertDeeplyFrozen(presentation)
})

test('the display DTO omits face, pose, vector, scene, and runtime authority', () => {
  const fixture = certifiedFixture()
  const presentation =
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      fixture.certificate,
    )
  assert.ok(presentation)

  const keys = collectKeys(presentation)
  for (const forbiddenKey of [
    'fixedFaceId',
    'firstFaceId',
    'secondFaceId',
    'sourcePartition',
    'contextKey',
    'poseRequestKey',
    'sourcePoseRequestKey',
    'targetPoseRequestKey',
    'blockingPoseRequestKey',
    'sourceAngles',
    'targetAngles',
    'appliedAngles',
    'sourceSeedRank',
    'source',
    'applicationToken',
    'sceneCommand',
    'runtimeState',
  ]) {
    assert.ok(
      !keys.has(forbiddenKey),
      `presentation leaked ${forbiddenKey}`,
    )
  }
  assert.ok(![...keys].some((key) => /faceid$/iu.test(key)))
  const serialized = JSON.stringify(presentation)
  assert.doesNotMatch(serialized, /"(?:root|moving|obstacle)"/u)

  const source = readFileSync(
    new URL(
      '../src/lib/foldPreviewTreeSingleHingeStaticCandidatePathPresentation.ts',
      import.meta.url,
    ),
    'utf8',
  )
  assert.doesNotMatch(
    source,
    /from\s+['"][^'"]*(?:scene|runtime|three)[^'"]*['"]/iu,
  )
})

test('clone, equivalent context, wrappers, and non-certificates fail closed', () => {
  const fixture = certifiedFixture()
  const equivalentContext = prepareContext()
  const clonedCertificate = structuredClone(fixture.certificate)

  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      clonedCertificate,
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      equivalentContext,
      fixture.certificate,
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      { ...fixture.certificate },
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      { certificate: fixture.certificate },
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      fixture.staticCandidates,
    ),
    null,
  )
  for (const value of [null, undefined, false, 0, '', Symbol('certificate')]) {
    assert.equal(
      createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
        fixture.context,
        value,
      ),
      null,
    )
  }
})

test('hostile and revoked objects fail closed without property access', () => {
  const fixture = certifiedFixture()
  let propertyReads = 0
  const hostile = new Proxy({}, {
    get() {
      propertyReads += 1
      throw new Error('unexpected property read')
    },
    getOwnPropertyDescriptor() {
      propertyReads += 1
      throw new Error('unexpected property descriptor read')
    },
    ownKeys() {
      propertyReads += 1
      throw new Error('unexpected ownKeys read')
    },
  })
  const revokedValue = Proxy.revocable({}, {})
  revokedValue.revoke()
  const revokedContext = Proxy.revocable(fixture.context, {})
  revokedContext.revoke()

  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      hostile,
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      hostile as FoldPreviewTreeMotionContext,
      fixture.certificate,
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      revokedValue.proxy,
    ),
    null,
  )
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      revokedContext.proxy,
      fixture.certificate,
    ),
    null,
  )
  assert.equal(propertyReads, 0)
})

test('repeated projections are deterministic, detached, and deeply frozen', () => {
  const fixture = certifiedFixture()
  const first =
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      fixture.certificate,
    )
  const second =
    createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
      fixture.context,
      fixture.certificate,
    )
  assert.ok(first && second)
  assert.deepEqual(second, first)
  assert.notStrictEqual(second, first)
  assert.notStrictEqual(second.identity, first.identity)
  assert.notStrictEqual(second.continuous, first.continuous)
  assert.notStrictEqual(
    second.staticInteractionSummary,
    first.staticInteractionSummary,
  )
  assertDeeplyFrozen(first)
  assertDeeplyFrozen(second)
})

function certifiedFixture() {
  const context = prepareContext()
  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    context.model,
    context.fixedFaceId,
    context.collisionThickness,
    context.appliedAngles,
  )
  assert.ok(sourcePoseRequestKey)
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    context.model,
    context.fixedFaceId,
    context.selectedHingeEdgeId,
  )
  assert.ok(analyzer)
  const blockingJob = analyzer.createJob(
    context.appliedAngles,
    120,
    context.collisionThickness,
    {
      maxDepth: 18,
      minTimeSpan: 2 ** -22,
      maxIntervalTests: 10_000,
      requestIdentity: {
        contextKey: context.contextKey,
        sourcePoseRequestKey,
        generation: 7,
        requestSequence: 11,
      },
    },
  )
  assert.ok(blockingJob)
  const blockingResult = runContinuous(blockingJob)
  assert.equal(blockingResult.kind, 'blocked', JSON.stringify(blockingResult))
  const binding = blockingResult.kind === 'blocked'
    ? blockingResult.blocker?.blockingSample?.terminalFullScanBinding
    : null
  assert.ok(binding)
  const staticCandidates =
    deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
      context,
      binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(staticCandidates)
  const pathJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      context,
      staticCandidates,
    )
  assert.ok(pathJob)
  const pathResult = runPath(pathJob)
  assert.equal(pathResult.kind, 'certified')
  if (pathResult.kind !== 'certified') {
    throw new Error('expected a certified path')
  }
  return {
    context,
    staticCandidates,
    certificate: pathResult.certificate,
  }
}

function prepareContext(): FoldPreviewTreeMotionContext {
  const context = prepareFoldPreviewTreeMotionContext({
    model: stationaryBranchCollisionModel(),
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    appliedAngles: [
      { edgeId: 'selected', angleDegrees: 0 },
      { edgeId: 'frozen', angleDegrees: 90 },
    ],
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(context)
  return context
}

function runContinuous(
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

function runPath(job: FoldPreviewTreeSingleHingeStaticCandidatePathJob) {
  for (let index = 0; index < 1_000; index += 1) {
    const result = job.step(32)
    if (result.kind !== 'pending') return result
  }
  throw new Error('static candidate path job did not terminate')
}

function collectKeys(value: unknown, keys = new Set<string>()) {
  if (typeof value !== 'object' || value === null) return keys
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key === 'string') keys.add(key)
    collectKeys((value as Record<PropertyKey, unknown>)[key], keys)
  }
  return keys
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
