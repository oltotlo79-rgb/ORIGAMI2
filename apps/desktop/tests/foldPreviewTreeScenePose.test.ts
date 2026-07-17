import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import { MAX_FOLD_PREVIEW_COLLISION_FACES } from '../src/lib/foldPreviewCollision.ts'
import type {
  FoldPreviewHingeAngle,
  FoldPreviewTreeKinematics,
} from '../src/lib/foldPreviewKinematics.ts'
import type { FoldPreviewHingeModel } from '../src/lib/foldPreviewModel.ts'
import {
  applyFoldPreviewTreeScenePose,
  createFoldPreviewTreeSceneCollisionPoseKey,
  lockFoldPreviewTreeSceneMatrixTarget,
  type FoldPreviewTreeScenePoseInput,
} from '../src/lib/foldPreviewTreeScenePose.ts'

test('applies one canonical complete pose and returns detached frozen angles', () => {
  const tree = nonCommutingTree()
  const mutableAngles = [
    { edgeId: 'z-axis', angleDegrees: 35 },
    { edgeId: 'x-axis', angleDegrees: 70 },
  ]
  const targets = createTargets(tree)
  const result = applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: mutableAngles,
    ...targets,
  })

  assert.ok(result)
  assert.deepEqual(result.appliedAngles, [
    { edgeId: 'x-axis', angleDegrees: 70 },
    { edgeId: 'z-axis', angleDegrees: 35 },
  ])
  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result.appliedAngles))
  assert.ok(result.appliedAngles.every(Object.isFrozen))
  assertTransformsEqual(targets.faceTargets, result.faceTransforms)
  assertTransformsEqual(targets.hingeTargets, result.hingeTransforms)
  assert.notEqual(
    targets.faceTargets.get('leaf'),
    result.faceTransforms.get('leaf'),
  )

  mutableAngles[0].edgeId = 'mutated'
  mutableAngles[0].angleDegrees = 180
  ;(tree.joints[0].hinge.start as { x: number }).x = 999
  assert.deepEqual(result.appliedAngles, [
    { edgeId: 'x-axis', angleDegrees: 70 },
    { edgeId: 'z-axis', angleDegrees: 35 },
  ])
  const targetLeafBefore = elements(targets.faceTargets.get('leaf')!)
  result.faceTransforms.get('leaf')?.makeScale(9, 9, 9)
  assert.deepEqual(elements(targets.faceTargets.get('leaf')!), targetLeafBefore)
})

test('preserves parent-before-child order for a non-commuting tree', () => {
  const tree = nonCommutingTree()
  const targets = createTargets(tree)
  const result = applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: [
      { edgeId: 'z-axis', angleDegrees: 90 },
      { edgeId: 'x-axis', angleDegrees: 90 },
    ],
    ...targets,
  })

  assert.ok(result)
  const leaf = targets.faceTargets.get('leaf')
  assert.ok(leaf)
  assertPoint(
    new Vector3(2, 0, 1).applyMatrix4(leaf),
    [1, -1, 1],
  )
  assert.ok(
    targets.hingeTargets.get('z-axis')?.equals(
      targets.faceTargets.get('middle')!,
    ),
  )
})

test('incomplete, extra, duplicate, unknown, and invalid angles mutate no target', () => {
  const tree = nonCommutingTree()
  const invalidVectors: readonly (readonly FoldPreviewHingeAngle[])[] = [
    [{ edgeId: 'x-axis', angleDegrees: 20 }],
    [
      { edgeId: 'x-axis', angleDegrees: 20 },
      { edgeId: 'z-axis', angleDegrees: 30 },
      { edgeId: 'extra', angleDegrees: 40 },
    ],
    [
      { edgeId: 'x-axis', angleDegrees: 20 },
      { edgeId: 'x-axis', angleDegrees: 30 },
    ],
    [
      { edgeId: 'x-axis', angleDegrees: 20 },
      { edgeId: 'unknown', angleDegrees: 30 },
    ],
    [
      { edgeId: 'x-axis', angleDegrees: Number.NaN },
      { edgeId: 'z-axis', angleDegrees: 30 },
    ],
    [
      { edgeId: 'x-axis', angleDegrees: -1 },
      { edgeId: 'z-axis', angleDegrees: 30 },
    ],
    [
      { edgeId: 'x-axis', angleDegrees: 20 },
      { edgeId: 'z-axis', angleDegrees: 181 },
    ],
  ]

  for (const appliedAngles of invalidVectors) {
    const targets = createTargets(tree)
    const before = snapshotTargets(targets)
    assert.equal(applyFoldPreviewTreeScenePose({
      tree,
      appliedAngles,
      ...targets,
    }), null)
    assertTargetsUnchanged(targets, before)
  }
})

test('missing and extra target IDs are rejected before any matrix changes', () => {
  const tree = nonCommutingTree()
  const angles = completeAngles()
  const missingFace = createTargets(tree)
  missingFace.faceTargets.delete('leaf')
  const missingFaceBefore = snapshotTargets(missingFace)
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: angles,
    ...missingFace,
  }), null)
  assertTargetsUnchanged(missingFace, missingFaceBefore)

  const extraFace = createTargets(tree)
  extraFace.faceTargets.set('extra-face', new Matrix4())
  const extraFaceBefore = snapshotTargets(extraFace)
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: angles,
    ...extraFace,
  }), null)
  assertTargetsUnchanged(extraFace, extraFaceBefore)

  const missingHinge = createTargets(tree)
  missingHinge.hingeTargets.delete('z-axis')
  const missingHingeBefore = snapshotTargets(missingHinge)
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: angles,
    ...missingHinge,
  }), null)
  assertTargetsUnchanged(missingHinge, missingHingeBefore)

  const extraHinge = createTargets(tree)
  extraHinge.hingeTargets.set('extra-hinge', new Matrix4())
  const extraHingeBefore = snapshotTargets(extraHinge)
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: angles,
    ...extraHinge,
  }), null)
  assertTargetsUnchanged(extraHinge, extraHingeBefore)
})

test('malformed, missing, extra, non-finite, and unwritable matrices fail atomically', () => {
  const tree = nonCommutingTree()
  const cases: Array<(targets: ReturnType<typeof createTargets>) => void> = [
    (targets) => {
      targets.faceTargets.set(
        'leaf',
        { elements: new Array(16).fill(0) } as Matrix4,
      )
    },
    (targets) => {
      const matrix = new Matrix4()
      delete (matrix as unknown as {
        elements?: number[]
      }).elements
      targets.faceTargets.set('leaf', matrix)
    },
    (targets) => {
      const matrix = new Matrix4()
      matrix.elements = new Array(17).fill(0)
      targets.faceTargets.set('leaf', matrix)
    },
    (targets) => {
      targets.faceTargets.get('leaf')!.elements[15] = Number.NaN
    },
    (targets) => {
      Object.freeze(targets.faceTargets.get('leaf')!.elements)
    },
    (targets) => {
      const matrix = new Matrix4()
      matrix.elements = new Array(15).fill(0)
      targets.hingeTargets.set('z-axis', matrix)
    },
  ]

  for (const corrupt of cases) {
    const targets = createTargets(tree)
    corrupt(targets)
    const before = snapshotTargets(targets)
    assert.equal(applyFoldPreviewTreeScenePose({
      tree,
      appliedAngles: completeAngles(),
      ...targets,
    }), null)
    assertTargetsUnchanged(targets, before)
  }
})

test('scene targets are irreversibly registered with fresh stable arrays', () => {
  const sharedElements = new Matrix4().elements
  const first = new Matrix4()
  const second = new Matrix4()
  first.elements = sharedElements
  second.elements = sharedElements

  assert.equal(lockFoldPreviewTreeSceneMatrixTarget(first), true)
  assert.equal(lockFoldPreviewTreeSceneMatrixTarget(second), true)
  assert.equal(lockFoldPreviewTreeSceneMatrixTarget(first), true)
  assert.notStrictEqual(first.elements, sharedElements)
  assert.notStrictEqual(second.elements, sharedElements)
  assert.notStrictEqual(first.elements, second.elements)
  const lockedElements = first.elements
  first.makeTranslation(4, 5, 6)
  assert.strictEqual(first.elements, lockedElements)
  assert.deepEqual(
    new Vector3().applyMatrix4(first).toArray(),
    [4, 5, 6],
  )
  const descriptor = Object.getOwnPropertyDescriptor(first, 'elements')
  assert.equal(descriptor?.writable, false)
  assert.equal(descriptor?.configurable, false)

  const tree = nonCommutingTree()
  const targets = createTargets(tree)
  targets.faceTargets.set('root', new Matrix4())
  const before = snapshotTargets(targets)
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: completeAngles(),
    ...targets,
  }), null)
  assertTargetsUnchanged(targets, before)
})

test('aliased matrix targets are rejected across faces and hinges', () => {
  const tree = nonCommutingTree()
  for (const alias of ['face', 'cross'] as const) {
    const targets = createTargets(tree)
    const root = targets.faceTargets.get('root')!
    if (alias === 'face') {
      targets.faceTargets.set('leaf', root)
    } else {
      targets.hingeTargets.set('z-axis', root)
    }
    const before = snapshotTargets(targets)
    assert.equal(applyFoldPreviewTreeScenePose({
      tree,
      appliedAngles: completeAngles(),
      ...targets,
    }), null)
    assertTargetsUnchanged(targets, before)
  }
})

test('registered target Proxies are never observed during scene commit', () => {
  const tree = nonCommutingTree()
  const targets = createTargets(tree)
  const root = targets.faceTargets.get('root')!
  let armed = false
  let observedGets = 0
  const proxy = new Proxy(new Matrix4(), {
    get(target, property, receiver) {
      if (armed) {
        observedGets += 1
        try {
          Object.defineProperty(root, 'elements', {
            value: new Matrix4().elements,
          })
        } catch {
          // The registered root target cannot have its array replaced.
        }
      }
      return Reflect.get(target, property, receiver)
    },
  })
  assert.equal(lockFoldPreviewTreeSceneMatrixTarget(proxy), true)
  targets.hingeTargets.set('z-axis', proxy)
  armed = true
  const result = applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: completeAngles(),
    ...targets,
  })
  armed = false

  assert.ok(result)
  assert.equal(observedGets, 0)
  assertTransformsEqual(targets.faceTargets, result.faceTransforms)
  assertTransformsEqual(targets.hingeTargets, result.hingeTransforms)
})

test('a malformed last target cannot leave earlier valid targets partially updated', () => {
  const tree = nonCommutingTree()
  const targets = createTargets(tree)
  const firstFace = targets.faceTargets.get('root')!
  const firstHinge = targets.hingeTargets.get('x-axis')!
  const firstFaceBefore = elements(firstFace)
  const firstHingeBefore = elements(firstHinge)
  targets.hingeTargets.get('z-axis')!.elements[7] = Number.POSITIVE_INFINITY

  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: completeAngles(),
    ...targets,
  }), null)
  assert.deepEqual(elements(firstFace), firstFaceBefore)
  assert.deepEqual(elements(firstHinge), firstHingeBefore)
})

test('tree collision keys ignore input order and use a fixed zero uniform slot', () => {
  const model = {
    projectId: 'project',
    revision: 12,
    kind: 'fold_graph',
  } as const
  const reverse = createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    'root',
    0.1,
    [
      { edgeId: 'z-axis', angleDegrees: 80 },
      { edgeId: 'x-axis', angleDegrees: 25 },
    ],
  )
  const forward = createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    'root',
    0.1,
    [
      { edgeId: 'x-axis', angleDegrees: 25 },
      { edgeId: 'z-axis', angleDegrees: 80 },
    ],
  )

  assert.ok(reverse && forward)
  assert.equal(reverse, forward)
  assert.equal(JSON.parse(forward)[5], 0)
  assert.notEqual(
    forward,
    createFoldPreviewTreeSceneCollisionPoseKey(
      model,
      'root',
      0.1,
      [
        { edgeId: 'x-axis', angleDegrees: 26 },
        { edgeId: 'z-axis', angleDegrees: 80 },
      ],
    ),
  )
  assert.notEqual(
    forward,
    createFoldPreviewTreeSceneCollisionPoseKey(
      model,
      'root',
      0.2,
      [
        { edgeId: 'x-axis', angleDegrees: 25 },
        { edgeId: 'z-axis', angleDegrees: 80 },
      ],
    ),
  )
  assert.equal(createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    'root',
    0.1,
    [
      { edgeId: 'x-axis', angleDegrees: 25 },
      { edgeId: 'x-axis', angleDegrees: 80 },
    ],
  ), null)
})

test('throwing Proxy inputs fail closed without mutating genuine targets', () => {
  const tree = nonCommutingTree()
  const targets = createTargets(tree)
  const before = snapshotTargets(targets)
  const throwingInput = new Proxy({
    tree,
    appliedAngles: completeAngles(),
    ...targets,
  }, {
    get() {
      throw new Error('input getter')
    },
  })
  assert.equal(applyFoldPreviewTreeScenePose(
    throwingInput as FoldPreviewTreeScenePoseInput,
  ), null)
  assertTargetsUnchanged(targets, before)

  const angleTargets = createTargets(tree)
  const angleBefore = snapshotTargets(angleTargets)
  const throwingAngles = new Proxy([...completeAngles()], {
    get(target, property, receiver) {
      if (property === '0') throw new Error('angle index')
      return Reflect.get(target, property, receiver)
    },
  })
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: throwingAngles,
    ...angleTargets,
  }), null)
  assertTargetsUnchanged(angleTargets, angleBefore)

  const mapTargets = createTargets(tree)
  const mapBefore = snapshotTargets(mapTargets)
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: completeAngles(),
    faceTargets: new Proxy(mapTargets.faceTargets, {}),
    hingeTargets: mapTargets.hingeTargets,
  }), null)
  assertTargetsUnchanged(mapTargets, mapBefore)

  const matrixTargets = createTargets(tree)
  const matrixBefore = snapshotTargets(matrixTargets)
  const genuineLeaf = matrixTargets.faceTargets.get('leaf')!
  matrixTargets.faceTargets.set('leaf', new Proxy(genuineLeaf, {
    get(target, property, receiver) {
      if (property === 'elements') throw new Error('matrix elements')
      return Reflect.get(target, property, receiver)
    },
  }))
  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: completeAngles(),
    ...matrixTargets,
  }), null)
  assert.deepEqual(elements(genuineLeaf), matrixBefore.faces.get('leaf'))

  const throwingModel = new Proxy({
    projectId: 'project',
    revision: 1,
    kind: 'fold_graph' as const,
  }, {
    get() {
      throw new Error('model getter')
    },
  })
  assert.equal(createFoldPreviewTreeSceneCollisionPoseKey(
    throwingModel,
    'root',
    0.1,
    completeAngles(),
  ), null)
})

test('oversized trees reject from length without reading a joint index', () => {
  let indexed = false
  const oversizedJoints = new Proxy([], {
    get(target, property, receiver) {
      if (property === 'length') return MAX_FOLD_PREVIEW_COLLISION_FACES
      if (typeof property === 'string' && /^(?:0|[1-9]\d*)$/u.test(property)) {
        indexed = true
        throw new Error('oversized index')
      }
      return Reflect.get(target, property, receiver)
    },
  })
  const tree = {
    kind: 'tree',
    rootFaceId: 'root',
    joints: oversizedJoints,
  } as unknown as FoldPreviewTreeKinematics

  assert.equal(applyFoldPreviewTreeScenePose({
    tree,
    appliedAngles: [],
    faceTargets: new Map(),
    hingeTargets: new Map(),
  }), null)
  assert.equal(indexed, false)
})

function nonCommutingTree(): FoldPreviewTreeKinematics {
  const xAxis = hinge({
    edgeId: 'x-axis',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start: [0, 0],
    end: [1, 0],
    assignment: 'mountain',
  })
  const zAxis = hinge({
    edgeId: 'z-axis',
    leftFaceId: 'middle',
    rightFaceId: 'leaf',
    start: [1, 1],
    end: [1, 2],
    assignment: 'mountain',
  })
  return {
    kind: 'tree',
    rootFaceId: 'root',
    joints: [
      {
        parentFaceId: 'root',
        childFaceId: 'middle',
        hinge: xAxis,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'middle',
        childFaceId: 'leaf',
        hinge: zAxis,
        childRotationSign: 1,
      },
    ],
  }
}

function hinge(input: Readonly<{
  edgeId: string
  leftFaceId: string
  rightFaceId: string
  start: readonly [number, number]
  end: readonly [number, number]
  assignment: 'mountain' | 'valley'
}>): FoldPreviewHingeModel {
  const rotationSign = input.assignment === 'mountain' ? 1 : -1
  const deltaX = input.end[0] - input.start[0]
  const deltaZ = input.end[1] - input.start[1]
  const length = Math.hypot(deltaX, deltaZ)
  return {
    edgeId: input.edgeId,
    leftFaceId: input.leftFaceId,
    rightFaceId: input.rightFaceId,
    start: {
      vertexId: `${input.edgeId}-start`,
      x: input.start[0],
      z: input.start[1],
    },
    end: {
      vertexId: `${input.edgeId}-end`,
      x: input.end[0],
      z: input.end[1],
    },
    axis: {
      x: deltaX / length,
      z: deltaZ / length,
    },
    assignment: input.assignment,
    rotationSign,
  }
}

function completeAngles(): readonly FoldPreviewHingeAngle[] {
  return [
    { edgeId: 'x-axis', angleDegrees: 70 },
    { edgeId: 'z-axis', angleDegrees: 35 },
  ]
}

function createTargets(tree: FoldPreviewTreeKinematics) {
  const faceIds = [
    tree.rootFaceId,
    ...tree.joints.map((joint) => joint.childFaceId),
  ]
  const hingeIds = tree.joints.map((joint) => joint.hinge.edgeId)
  const targets = {
    faceTargets: new Map(faceIds.map((faceId, index) => [
      faceId,
      new Matrix4().makeTranslation(index + 1, index + 2, index + 3),
    ])),
    hingeTargets: new Map(hingeIds.map((edgeId, index) => [
      edgeId,
      new Matrix4().makeScale(index + 2, index + 3, index + 4),
    ])),
  }
  for (const matrix of [
    ...targets.faceTargets.values(),
    ...targets.hingeTargets.values(),
  ]) {
    assert.equal(lockFoldPreviewTreeSceneMatrixTarget(matrix), true)
  }
  return targets
}

function snapshotTargets(targets: ReturnType<typeof createTargets>) {
  return {
    faces: new Map([...targets.faceTargets].map(([id, matrix]) => [
      id,
      safeElements(matrix),
    ])),
    hinges: new Map([...targets.hingeTargets].map(([id, matrix]) => [
      id,
      safeElements(matrix),
    ])),
  }
}

function assertTargetsUnchanged(
  targets: ReturnType<typeof createTargets>,
  before: ReturnType<typeof snapshotTargets>,
) {
  assert.deepEqual(
    new Map([...targets.faceTargets].map(([id, matrix]) => [
      id,
      safeElements(matrix),
    ])),
    before.faces,
  )
  assert.deepEqual(
    new Map([...targets.hingeTargets].map(([id, matrix]) => [
      id,
      safeElements(matrix),
    ])),
    before.hinges,
  )
}

function assertTransformsEqual(
  targets: ReadonlyMap<string, Matrix4>,
  transforms: ReadonlyMap<string, Matrix4>,
) {
  assert.equal(targets.size, transforms.size)
  for (const [id, target] of targets) {
    assert.deepEqual(elements(target), elements(transforms.get(id)!))
  }
}

function safeElements(matrix: Matrix4) {
  try {
    return Array.isArray(matrix.elements)
      ? [...matrix.elements]
      : null
  } catch {
    return null
  }
}

function elements(matrix: Matrix4) {
  return [...matrix.elements]
}

function assertPoint(
  point: Vector3,
  expected: readonly [number, number, number],
) {
  const actual = point.toArray()
  for (let index = 0; index < 3; index += 1) {
    assert.ok(
      Math.abs(actual[index] - expected[index]) < 1e-12,
      `${actual} != ${expected}`,
    )
  }
}
