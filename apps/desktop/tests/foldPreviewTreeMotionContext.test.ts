import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
} from '../src/lib/foldPreviewCollision.ts'
import type { FoldPreviewHingeAngle } from '../src/lib/foldPreviewKinematics.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from '../src/lib/foldPreviewModel.ts'
import {
  classifyFoldPreviewTreeMotionTarget,
  FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION,
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_ID_LENGTH,
  prepareFoldPreviewTreeMotionContext,
  rebaseFoldPreviewTreeMotionContextSelectedAngle,
  replaceFoldPreviewTreeMotionSelectedAngle,
  type FoldPreviewTreeMotionContext,
  type FoldPreviewTreeMotionContextInput,
} from '../src/lib/foldPreviewTreeMotionContext.ts'
import {
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const BASE_ANGLES: readonly FoldPreviewHingeAngle[] = [
  { edgeId: 'hinge-z', angleDegrees: 55 },
  { edgeId: 'hinge-x', angleDegrees: 35 },
]

test('prepares a canonical deeply frozen snapshot detached from mutable inputs', () => {
  const model = treeModel()
  const appliedAngles = BASE_ANGLES.map((angle) => ({ ...angle }))
  const context = prepareFoldPreviewTreeMotionContext({
    model,
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    appliedAngles,
    collisionThickness: 0.1,
    visualThickness: 0.12,
  })

  assert.ok(context)
  assert.equal(context.version, FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION)
  assert.equal(context.tree.rootFaceId, 'root')
  assert.equal(context.selectedAngleDegrees, 55)
  assert.deepEqual(context.appliedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 55 },
  ])
  assert.deepEqual(context.nonSelectedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ])
  assertDeeplyFrozen(context)

  const originalKey = context.contextKey
  const mutableModel = model as unknown as {
    revision: number
    faces: Array<{
      polygon: Array<{ vertexId: string; x: number; z: number }>
    }>
    hinges: Array<{
      start: { vertexId: string; x: number; z: number }
    }>
  }
  mutableModel.revision = 999
  mutableModel.faces[0].polygon[0].x = 999
  mutableModel.hinges[0].start.x = 999
  appliedAngles[0].angleDegrees = 1

  assert.equal(context.model.revision, 1)
  assert.notEqual(context.model.faces[0].polygon[0].x, 999)
  assert.notEqual(context.model.hinges[0].start.x, 999)
  assert.equal(context.selectedAngleDegrees, 55)
  assert.equal(context.contextKey, originalKey)
})

test('context freezing and authority ignore later intrinsic replacement', {
  concurrency: false,
}, () => {
  const originalAdd = WeakSet.prototype.add
  const originalHas = WeakSet.prototype.has
  const originalApply = Reflect.apply
  const originalOwnKeys = Reflect.ownKeys
  const originalFreeze = Object.freeze
  let replacementAddCalled = false
  let replacementHasCalled = false
  let replacementApplyCalled = false
  let replacementOwnKeysCalled = false
  let replacementFreezeCalled = false
  let context: FoldPreviewTreeMotionContext | null = null

  WeakSet.prototype.add = (function replacedAdd() {
    replacementAddCalled = true
    return this
  }) as typeof WeakSet.prototype.add
  WeakSet.prototype.has = (function replacedHas() {
    replacementHasCalled = true
    return true
  }) as typeof WeakSet.prototype.has
  Reflect.apply = (function replacedApply() {
    replacementApplyCalled = true
    throw new Error('replaced Reflect.apply')
  }) as typeof Reflect.apply
  Reflect.ownKeys = (function replacedOwnKeys() {
    replacementOwnKeysCalled = true
    return []
  }) as typeof Reflect.ownKeys
  Object.freeze = ((value: object) => {
    if (
      (value as Record<PropertyKey, unknown>).version
        === FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION
    ) {
      replacementFreezeCalled = true
      return value
    }
    return originalFreeze(value)
  }) as typeof Object.freeze

  try {
    context = prepared()
  } finally {
    Object.freeze = originalFreeze
    Reflect.ownKeys = originalOwnKeys
    Reflect.apply = originalApply
    WeakSet.prototype.add = originalAdd
    WeakSet.prototype.has = originalHas
  }

  assert.ok(context)
  assert.equal(replacementAddCalled, false)
  assert.equal(replacementHasCalled, false)
  assert.equal(replacementApplyCalled, false)
  assert.equal(replacementOwnKeysCalled, false)
  assert.equal(replacementFreezeCalled, false)
  assertDeeplyFrozen(context)
  assert.deepEqual(
    replaceFoldPreviewTreeMotionSelectedAngle(context, 90),
    [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 90 },
    ],
  )
  assert.equal(
    replaceFoldPreviewTreeMotionSelectedAngle(
      structuredClone(context),
      90,
    ),
    null,
  )
})

test('the opaque key ignores order and selected magnitude but binds every other identity input', () => {
  const baseline = prepared()
  assert.ok(baseline)

  const orderChanged = prepared({
    appliedAngles: [...BASE_ANGLES].reverse(),
  })
  const selectedMagnitudeChanged = prepared({
    appliedAngles: [
      { edgeId: 'hinge-z', angleDegrees: 179 },
      { edgeId: 'hinge-x', angleDegrees: 35 },
    ],
  })
  assert.equal(orderChanged?.contextKey, baseline.contextKey)
  assert.equal(selectedMagnitudeChanged?.contextKey, baseline.contextKey)

  const variants = [
    prepared({
      appliedAngles: [
        { edgeId: 'hinge-z', angleDegrees: 55 },
        { edgeId: 'hinge-x', angleDegrees: 36 },
      ],
    }),
    prepared({ fixedFaceId: 'leaf' }),
    prepared({ selectedHingeEdgeId: 'hinge-x' }),
    prepared({ model: treeModel({ revision: 2 }) }),
    prepared({ model: treeModel({ projectId: 'other-project' }) }),
    prepared({ collisionThickness: 0.11 }),
    prepared({ visualThickness: 0.13 }),
  ]
  for (const variant of variants) {
    assert.ok(variant)
    assert.notEqual(variant.contextKey, baseline.contextKey)
  }

  assert.deepEqual(JSON.parse(baseline.contextKey), [
    FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION,
    'project',
    1,
    'fold_graph',
    'tree',
    'root',
    0.1,
    0.12,
    'hinge-z',
    [['hinge-x', 35]],
  ])
})

test('replaces exactly the selected angle in a complete canonical vector', () => {
  const context = prepared()
  assert.ok(context)

  const replaced = replaceFoldPreviewTreeMotionSelectedAngle(context, 123)
  assert.deepEqual(replaced, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 123 },
  ])
  assert.ok(replaced)
  assertDeeplyFrozen(replaced)
  assert.equal(context.selectedAngleDegrees, 55)
  assert.equal(context.appliedAngles[1].angleDegrees, 55)

  for (const invalid of [
    -1,
    181,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.NEGATIVE_INFINITY,
  ]) {
    assert.equal(
      replaceFoldPreviewTreeMotionSelectedAngle(context, invalid),
      null,
    )
  }
  assert.equal(
    replaceFoldPreviewTreeMotionSelectedAngle(
      { ...context },
      90,
    ),
    null,
  )
})

test('rebasing issues a fresh authentic context with the exact prepared model', () => {
  const context = prepared()
  assert.ok(context)

  const rebased = rebaseFoldPreviewTreeMotionContextSelectedAngle(
    context,
    123,
  )
  assert.ok(rebased)
  assert.notStrictEqual(rebased, context)
  assert.strictEqual(rebased.model, context.model)
  assert.strictEqual(rebased.tree, context.tree)
  assert.strictEqual(
    rebased.nonSelectedAngles,
    context.nonSelectedAngles,
  )
  assert.equal(rebased.contextKey, context.contextKey)
  assert.equal(rebased.selectedAngleDegrees, 123)
  assert.deepEqual(rebased.appliedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 123 },
  ])
  assert.notStrictEqual(rebased.appliedAngles, context.appliedAngles)
  assert.deepEqual(
    replaceFoldPreviewTreeMotionSelectedAngle(rebased, 90),
    [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 90 },
    ],
  )
  assert.equal(context.selectedAngleDegrees, 55)
  assert.equal(context.appliedAngles[1]?.angleDegrees, 55)
  assertDeeplyFrozen(rebased)

  const sameAngle = rebaseFoldPreviewTreeMotionContextSelectedAngle(
    context,
    context.selectedAngleDegrees,
  )
  assert.ok(sameAngle)
  assert.notStrictEqual(sameAngle, context)
  assert.strictEqual(sameAngle.model, context.model)
  assert.strictEqual(sameAngle.tree, context.tree)
  assert.strictEqual(
    sameAngle.nonSelectedAngles,
    context.nonSelectedAngles,
  )
  assert.deepEqual(sameAngle.appliedAngles, context.appliedAngles)
  assert.notStrictEqual(sameAngle.appliedAngles, context.appliedAngles)
  assertDeeplyFrozen(sameAngle)

  for (const invalid of [
    structuredClone(context),
    { ...context },
    null,
    undefined,
  ]) {
    assert.equal(
      rebaseFoldPreviewTreeMotionContextSelectedAngle(
        invalid as FoldPreviewTreeMotionContext,
        90,
      ),
      null,
    )
  }
  for (const invalidAngle of [
    -1,
    181,
    Number.NaN,
    Number.POSITIVE_INFINITY,
  ]) {
    assert.equal(
      rebaseFoldPreviewTreeMotionContextSelectedAngle(
        context,
        invalidAngle,
      ),
      null,
    )
  }
  assert.equal(
    rebaseFoldPreviewTreeMotionContextSelectedAngle(
      throwingProxy<FoldPreviewTreeMotionContext>(),
      90,
    ),
    null,
  )
})

test('classifies same, selected-only, non-selected, and multiple changes deterministically', () => {
  const context = prepared()
  assert.ok(context)

  const same = classifyFoldPreviewTreeMotionTarget(context, [
    { edgeId: 'hinge-z', angleDegrees: 55 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ])
  assert.deepEqual(same, {
    kind: 'same',
    targetAngles: [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 55 },
    ],
  })
  assertDeeplyFrozen(same)

  const selectedOnly = classifyFoldPreviewTreeMotionTarget(context, [
    { edgeId: 'hinge-z', angleDegrees: 125 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ])
  assert.deepEqual(selectedOnly, {
    kind: 'selected_only',
    targetSelectedAngleDegrees: 125,
    targetAngles: [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 125 },
    ],
  })
  assertDeeplyFrozen(selectedOnly)

  assert.deepEqual(classifyFoldPreviewTreeMotionTarget(context, [
    { edgeId: 'hinge-x', angleDegrees: 40 },
    { edgeId: 'hinge-z', angleDegrees: 55 },
  ]), {
    kind: 'invalid_or_multiple',
    reason: 'non_selected_change',
    changedHingeEdgeIds: ['hinge-x'],
  })
  assert.deepEqual(classifyFoldPreviewTreeMotionTarget(context, [
    { edgeId: 'hinge-z', angleDegrees: 100 },
    { edgeId: 'hinge-x', angleDegrees: 40 },
  ]), {
    kind: 'invalid_or_multiple',
    reason: 'multiple_changes',
    changedHingeEdgeIds: ['hinge-x', 'hinge-z'],
  })
})

test('complete vectors reject missing, duplicate, unknown, non-finite, and out-of-range entries', () => {
  const context = prepared()
  assert.ok(context)
  const invalidVectors: readonly (readonly FoldPreviewHingeAngle[])[] = [
    [{ edgeId: 'hinge-z', angleDegrees: 55 }],
    [
      { edgeId: 'hinge-z', angleDegrees: 55 },
      { edgeId: 'hinge-z', angleDegrees: 35 },
    ],
    [
      { edgeId: 'hinge-z', angleDegrees: 55 },
      { edgeId: 'unknown', angleDegrees: 35 },
    ],
    [
      { edgeId: 'hinge-z', angleDegrees: Number.NaN },
      { edgeId: 'hinge-x', angleDegrees: 35 },
    ],
    [
      { edgeId: 'hinge-z', angleDegrees: 181 },
      { edgeId: 'hinge-x', angleDegrees: 35 },
    ],
  ]

  for (const vector of invalidVectors) {
    assert.equal(prepareFoldPreviewTreeMotionContext({
      ...baseInput(),
      appliedAngles: vector,
    }), null)
    assert.deepEqual(
      classifyFoldPreviewTreeMotionTarget(context, vector),
      {
        kind: 'invalid_or_multiple',
        reason: 'invalid_target_vector',
        changedHingeEdgeIds: [],
      },
    )
  }
})

test('preparation fails closed for unsupported model, identity, thickness, and tree inconsistencies', () => {
  const staticCycle = {
    ...treeModel(),
    kinematics: {
      kind: 'static_cycle',
      reason: 'cyclic_hinge_graph',
    },
  } as FoldGraphPreviewModel
  const mismatchedModel = treeModel()
  const mutableMismatch = mismatchedModel as unknown as {
    kinematics: {
      kind: 'tree'
      rootFaceId: string
      joints: Array<{
        parentFaceId: string
        childFaceId: string
        childRotationSign: 1 | -1
        hinge: FoldPreviewHingeModel
      }>
    }
  }
  mutableMismatch.kinematics.joints[1].childRotationSign = 1

  const invalidInputs: FoldPreviewTreeMotionContextInput[] = [
    { ...baseInput(), model: staticCycle },
    { ...baseInput(), model: mismatchedModel },
    { ...baseInput(), model: treeModel({ revision: -1 }) },
    {
      ...baseInput(),
      model: treeModel({
        projectId: 'p'.repeat(
          MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_ID_LENGTH + 1,
        ),
      }),
    },
    { ...baseInput(), fixedFaceId: 'unknown' },
    { ...baseInput(), selectedHingeEdgeId: 'unknown' },
    { ...baseInput(), collisionThickness: -1 },
    { ...baseInput(), collisionThickness: Number.NaN },
    { ...baseInput(), collisionThickness: Number.POSITIVE_INFINITY },
    { ...baseInput(), visualThickness: 0 },
    { ...baseInput(), visualThickness: Number.NaN },
    { ...baseInput(), visualThickness: Number.POSITIVE_INFINITY },
  ]

  for (const input of invalidInputs) {
    assert.equal(prepareFoldPreviewTreeMotionContext(input), null)
  }
})

test('throwing proxies are contained by every public boundary', () => {
  const throwingModel = throwingProxy<FoldGraphPreviewModel>()
  const throwingAngles = throwingProxy<readonly FoldPreviewHingeAngle[]>()
  const throwingInput = throwingProxy<FoldPreviewTreeMotionContextInput>()

  assert.doesNotThrow(() => prepareFoldPreviewTreeMotionContext({
    ...baseInput(),
    model: throwingModel,
  }))
  assert.equal(prepareFoldPreviewTreeMotionContext({
    ...baseInput(),
    model: throwingModel,
  }), null)
  assert.equal(prepareFoldPreviewTreeMotionContext({
    ...baseInput(),
    appliedAngles: throwingAngles,
  }), null)
  assert.equal(prepareFoldPreviewTreeMotionContext(throwingInput), null)

  const context = prepared()
  assert.ok(context)
  assert.deepEqual(
    classifyFoldPreviewTreeMotionTarget(context, throwingAngles),
    {
      kind: 'invalid_or_multiple',
      reason: 'invalid_target_vector',
      changedHingeEdgeIds: [],
    },
  )
  assert.equal(
    replaceFoldPreviewTreeMotionSelectedAngle(
      throwingProxy<FoldPreviewTreeMotionContext>(),
      90,
    ),
    null,
  )
})

test('stateful getters are captured once before a context becomes trusted', () => {
  const model = treeModel()
  const reads = {
    inputModel: 0,
    fixedFaceId: 0,
    selectedHingeEdgeId: 0,
    appliedAngles: 0,
    collisionThickness: 0,
    visualThickness: 0,
    projectId: 0,
    revision: 0,
    faceId: 0,
    pointX: 0,
    modelHingeEdgeId: 0,
    jointParentFaceId: 0,
    jointHingeEdgeId: 0,
    angleEdgeId: 0,
    angleDegrees: 0,
  }

  Object.defineProperty(model, 'projectId', {
    enumerable: true,
    get() {
      reads.projectId += 1
      return reads.projectId === 1 ? 'project' : ''
    },
  })
  Object.defineProperty(model, 'revision', {
    enumerable: true,
    get() {
      reads.revision += 1
      return reads.revision === 1 ? 1 : -1
    },
  })

  const firstFace = model.faces[0]
  const firstPoint = firstFace.polygon[0]
  const statefulPoint = {
    vertexId: firstPoint.vertexId,
    get x() {
      reads.pointX += 1
      return reads.pointX === 1 ? firstPoint.x : Number.NaN
    },
    z: firstPoint.z,
  }
  const statefulFace = {
    get id() {
      reads.faceId += 1
      return reads.faceId === 1 ? firstFace.id : 'changed-face'
    },
    polygon: [statefulPoint, ...firstFace.polygon.slice(1)],
  }
  ;(model.faces as FoldPreviewFaceModel[])[0] = statefulFace

  const firstHinge = model.hinges[0]
  const statefulModelHinge = { ...firstHinge }
  Object.defineProperty(statefulModelHinge, 'edgeId', {
    enumerable: true,
    get() {
      reads.modelHingeEdgeId += 1
      return reads.modelHingeEdgeId === 1 ? firstHinge.edgeId : ''
    },
  })
  ;(model.hinges as FoldPreviewHingeModel[])[0] = statefulModelHinge

  assert.equal(model.kinematics.kind, 'tree')
  if (model.kinematics.kind !== 'tree') {
    throw new Error('tree fixture unexpectedly changed kind')
  }
  const firstJoint = model.kinematics.joints[0]
  const statefulJointHinge = { ...firstHinge }
  Object.defineProperty(statefulJointHinge, 'edgeId', {
    enumerable: true,
    get() {
      reads.jointHingeEdgeId += 1
      return reads.jointHingeEdgeId === 1 ? firstHinge.edgeId : ''
    },
  })
  const statefulJoint = {
    ...firstJoint,
    hinge: statefulJointHinge,
  }
  Object.defineProperty(statefulJoint, 'parentFaceId', {
    enumerable: true,
    get() {
      reads.jointParentFaceId += 1
      return reads.jointParentFaceId === 1
        ? firstJoint.parentFaceId
        : 'changed-parent'
    },
  })
  ;(model.kinematics.joints as Array<typeof firstJoint>)[0] = statefulJoint

  const statefulAngles = [
    {
      get edgeId() {
        reads.angleEdgeId += 1
        return reads.angleEdgeId === 1 ? 'hinge-z' : 'unknown-hinge'
      },
      get angleDegrees() {
        reads.angleDegrees += 1
        return reads.angleDegrees === 1 ? 55 : Number.NaN
      },
    },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ]

  const input = {
    get model() {
      reads.inputModel += 1
      return reads.inputModel === 1
        ? model
        : throwingProxy<FoldGraphPreviewModel>()
    },
    get fixedFaceId() {
      reads.fixedFaceId += 1
      return reads.fixedFaceId === 1 ? 'root' : 'unknown-fixed'
    },
    get selectedHingeEdgeId() {
      reads.selectedHingeEdgeId += 1
      return reads.selectedHingeEdgeId === 1 ? 'hinge-z' : 'unknown-hinge'
    },
    get appliedAngles() {
      reads.appliedAngles += 1
      return reads.appliedAngles === 1
        ? statefulAngles
        : throwingProxy<readonly FoldPreviewHingeAngle[]>()
    },
    get collisionThickness() {
      reads.collisionThickness += 1
      return reads.collisionThickness === 1 ? 0.1 : Number.NaN
    },
    get visualThickness() {
      reads.visualThickness += 1
      return reads.visualThickness === 1 ? 0.12 : Number.NaN
    },
  } as FoldPreviewTreeMotionContextInput

  const context = prepareFoldPreviewTreeMotionContext(input)
  assert.ok(context)
  assert.equal(context.model.projectId, 'project')
  assert.equal(context.model.revision, 1)
  assert.equal(context.model.faces[0].id, 'root')
  assert.equal(context.model.faces[0].polygon[0].x, firstPoint.x)
  assert.equal(context.model.hinges[0].edgeId, 'hinge-z')
  assert.equal(context.tree.joints[0].parentFaceId, 'root')
  assert.equal(context.tree.joints[0].hinge.edgeId, 'hinge-z')
  assert.equal(context.fixedFaceId, 'root')
  assert.equal(context.selectedHingeEdgeId, 'hinge-z')
  assert.equal(context.collisionThickness, 0.1)
  assert.equal(context.visualThickness, 0.12)
  assert.deepEqual(
    replaceFoldPreviewTreeMotionSelectedAngle(context, 90),
    [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 90 },
    ],
  )
  for (const count of Object.values(reads)) assert.equal(count, 1)
})

test('oversized proxy arrays fail before any indexed element access', () => {
  const cases: Array<Readonly<{
    maximum: number
    install(model: FoldGraphPreviewModel, array: unknown[]): void
  }>> = [
    {
      maximum: MAX_FOLD_PREVIEW_COLLISION_FACES,
      install(model, array) {
        ;(model as unknown as { faces: unknown[] }).faces = array
      },
    },
    {
      maximum: MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
      install(model, array) {
        ;(model as unknown as { hinges: unknown[] }).hinges = array
      },
    },
    {
      maximum: MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
      install(model, array) {
        assert.equal(model.kinematics.kind, 'tree')
        if (model.kinematics.kind !== 'tree') return
        ;(model.kinematics as unknown as { joints: unknown[] }).joints = array
      },
    },
  ]

  for (const { maximum, install } of cases) {
    const access = { length: 0, index: 0 }
    const oversized = oversizedArrayProxy(maximum + 1, access)
    const model = treeModel()
    install(model, oversized)
    assert.equal(prepareFoldPreviewTreeMotionContext({
      ...baseInput(),
      model,
    }), null)
    assert.deepEqual(access, { length: 1, index: 0 })
  }

  const polygonAccess = { length: 0, index: 0 }
  const oversizedPolygon = oversizedArrayProxy(
    MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES + 1,
    polygonAccess,
  )
  const polygonModel = treeModel()
  const firstFace = polygonModel.faces[0]
  ;(polygonModel.faces as FoldPreviewFaceModel[])[0] = {
    id: firstFace.id,
    polygon: oversizedPolygon as FoldPreviewFaceModel['polygon'],
  }
  assert.equal(prepareFoldPreviewTreeMotionContext({
    ...baseInput(),
    model: polygonModel,
  }), null)
  assert.deepEqual(polygonAccess, { length: 1, index: 0 })
})

function prepared(
  overrides: Partial<FoldPreviewTreeMotionContextInput> = {},
) {
  return prepareFoldPreviewTreeMotionContext({
    ...baseInput(),
    ...overrides,
  })
}

function baseInput(): FoldPreviewTreeMotionContextInput {
  return {
    model: treeModel(),
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    appliedAngles: BASE_ANGLES,
    collisionThickness: 0.1,
    visualThickness: 0.12,
  }
}

function treeModel(
  overrides: Readonly<{
    projectId?: string
    revision?: number
  }> = {},
): FoldGraphPreviewModel {
  const root = face('root', -1, 0)
  const middle = face('middle', 0, 1)
  const leaf = face('leaf', 1, 2)
  const hingeZ: FoldPreviewHingeModel = {
    edgeId: 'hinge-z',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start: { vertexId: 'z-start', x: 0, z: -1 },
    end: { vertexId: 'z-end', x: 0, z: 1 },
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const hingeX: FoldPreviewHingeModel = {
    edgeId: 'hinge-x',
    leftFaceId: 'middle',
    rightFaceId: 'leaf',
    start: { vertexId: 'x-start', x: 0, z: 0 },
    end: { vertexId: 'x-end', x: 1, z: 0 },
    axis: { x: 1, z: 0 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: overrides.projectId ?? 'project',
    revision: overrides.revision ?? 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0.5, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 2, maxZ: 1 },
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
          childRotationSign: -1,
        },
      ],
    },
  }
}

function face(
  id: string,
  minimumX: number,
  maximumX: number,
): FoldPreviewFaceModel {
  return {
    id,
    polygon: [
      { vertexId: `${id}-a`, x: minimumX, z: -1 },
      { vertexId: `${id}-b`, x: maximumX, z: -1 },
      { vertexId: `${id}-c`, x: maximumX, z: 1 },
      { vertexId: `${id}-d`, x: minimumX, z: 1 },
    ],
  }
}

function assertDeeplyFrozen(value: unknown): void {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const key of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[key],
    )
  }
}

function throwingProxy<T>(): T {
  return new Proxy({}, {
    get() {
      throw new Error('unexpected access')
    },
  }) as T
}

function oversizedArrayProxy(
  reportedLength: number,
  access: { length: number; index: number },
) {
  return new Proxy<unknown[]>([], {
    get(target, property, receiver) {
      if (property === 'length') {
        access.length += 1
        return reportedLength
      }
      if (
        typeof property === 'string'
        && /^(?:0|[1-9]\d*)$/u.test(property)
      ) access.index += 1
      return Reflect.get(target, property, receiver)
    },
  })
}
