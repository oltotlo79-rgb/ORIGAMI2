import { Matrix4 } from 'three'
import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
} from './foldPreviewCollision.ts'
import { collisionPoseKey } from './foldPreviewCollisionView.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
  type FoldPreviewTreeKinematics,
} from './foldPreviewKinematics.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewHingeModel,
} from './foldPreviewModel.ts'

export type FoldPreviewTreeScenePoseInput = Readonly<{
  tree: FoldPreviewTreeKinematics
  appliedAngles: readonly FoldPreviewHingeAngle[]
  /** Every matrix must first pass lockFoldPreviewTreeSceneMatrixTarget. */
  faceTargets: ReadonlyMap<string, Matrix4>
  /** Every matrix must first pass lockFoldPreviewTreeSceneMatrixTarget. */
  hingeTargets: ReadonlyMap<string, Matrix4>
}>

export type FoldPreviewTreeScenePoseApplication = Readonly<{
  /**
   * Complete edge-ID-sorted vector detached from both the input and scene.
   * It is the only angle vector represented by the copied transforms.
   */
  appliedAngles: readonly FoldPreviewHingeAngle[]
  /** Detached transforms for current-pose collision diagnostics. */
  faceTransforms: ReadonlyMap<string, Matrix4>
  /** Detached transforms matching the matrices copied to hinge targets. */
  hingeTransforms: ReadonlyMap<string, Matrix4>
}>

export type FoldPreviewTreeCollisionModelIdentity = Pick<
  FoldGraphPreviewModel,
  'projectId' | 'revision' | 'kind'
>

type TreeSnapshot = Readonly<{
  tree: FoldPreviewTreeKinematics
  faceIds: readonly string[]
  hingeEdgeIds: readonly string[]
}>

type RawMapEntry = readonly [unknown, unknown]

type RawMapSnapshot = Readonly<{
  map: Map<unknown, unknown>
  entries: readonly RawMapEntry[]
}>

type MatrixTarget = Readonly<{
  id: string
  matrix: Matrix4
  elements: number[]
  previousElements: readonly number[]
}>

type TargetMapSnapshot = Readonly<{
  raw: RawMapSnapshot
  targets: readonly MatrixTarget[]
}>

type StagedTransform = Readonly<{
  id: string
  elements: readonly number[]
}>

const MAX_TREE_JOINTS = Math.min(
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES - 1,
)
const MAX_ID_LENGTH = 512
const MATRIX_ELEMENT_COUNT = 16
const mapSizeGetter = Object.getOwnPropertyDescriptor(
  Map.prototype,
  'size',
)?.get
const lockedMatrixTargetElements = new WeakMap<object, number[]>()

/**
 * Gives one internally owned Three.js matrix a stable element-array identity.
 *
 * Three.js mutates the 16 array entries rather than replacing `elements`, so
 * making that property non-writable remains compatible with Matrix4 methods.
 * A fresh plain array also prevents an input Proxy from participating in the
 * later all-target commit.
 */
export function lockFoldPreviewTreeSceneMatrixTarget(
  value: Matrix4,
): boolean {
  try {
    if (
      typeof value !== 'object'
      || value === null
      || Object.getPrototypeOf(value) !== Matrix4.prototype
    ) return false
    const existingElements = lockedMatrixTargetElements.get(value)
    if (existingElements) {
      return validWritableMatrixElements(existingElements)
    }
    const matrix = value as Matrix4
    const rawMatrixBrand = (
      matrix as unknown as Readonly<{ isMatrix4?: unknown }>
    ).isMatrix4
    const elementsDescriptor = Object.getOwnPropertyDescriptor(
      matrix,
      'elements',
    )
    const rawElements = matrix.elements
    const sourceElements = snapshotWritableMatrixElements(rawElements)
    if (
      rawMatrixBrand !== true
      || !elementsDescriptor
      || !('value' in elementsDescriptor)
      || elementsDescriptor.value !== rawElements
      || elementsDescriptor.writable !== true
      || elementsDescriptor.configurable !== true
      || !sourceElements
    ) return false
    const safeElements = [...sourceElements]
    Object.defineProperty(matrix, 'elements', {
      value: safeElements,
      enumerable: elementsDescriptor.enumerable,
      writable: false,
      configurable: false,
    })
    if (!lockedMatrixTargetStillMatches(matrix, safeElements)) return false
    copyElements(safeElements, sourceElements)
    lockedMatrixTargetElements.set(matrix, safeElements)
    return true
  } catch {
    return false
  }
}

/**
 * Calculates and copies one complete fold-tree pose as a fail-closed batch.
 *
 * Every tree field, angle, transform, target ID, and target Matrix4 is staged
 * before the first scene matrix element changes. A rejected call therefore
 * leaves every supplied target at its previous pose.
 */
export function applyFoldPreviewTreeScenePose(
  input: FoldPreviewTreeScenePoseInput,
): FoldPreviewTreeScenePoseApplication | null {
  try {
    if (!isRecord(input)) return null
    const rawTree = input.tree
    const rawAppliedAngles = input.appliedAngles
    const rawFaceTargets = input.faceTargets
    const rawHingeTargets = input.hingeTargets

    const treeSnapshot = snapshotTree(rawTree)
    if (!treeSnapshot) return null
    const appliedAngles = normalizeCompleteAngles(
      rawAppliedAngles,
      treeSnapshot.hingeEdgeIds,
    )
    if (!appliedAngles) return null

    const faceTargets = snapshotTargetMap(
      rawFaceTargets,
      treeSnapshot.faceIds,
      MAX_FOLD_PREVIEW_COLLISION_FACES,
    )
    const hingeTargets = snapshotTargetMap(
      rawHingeTargets,
      treeSnapshot.hingeEdgeIds,
      MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
    )
    if (
      !faceTargets
      || !hingeTargets
      || !matrixTargetsAreUnique(
        faceTargets.targets,
        hingeTargets.targets,
      )
    ) return null

    const pose = calculateFoldTreePoseWithAngles(treeSnapshot.tree, {
      kind: 'per_hinge',
      angles: appliedAngles,
    })
    if (!pose) return null

    const stagedFaceTransforms = stageTransformMap(
      pose.faceTransforms,
      treeSnapshot.faceIds,
      MAX_FOLD_PREVIEW_COLLISION_FACES,
    )
    const stagedHingeTransforms = stageTransformMap(
      pose.hingeTransforms,
      treeSnapshot.hingeEdgeIds,
      MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
    )
    if (!stagedFaceTransforms || !stagedHingeTransforms) return null

    const faceUpdates = joinTargetsAndTransforms(
      faceTargets.targets,
      stagedFaceTransforms,
    )
    const hingeUpdates = joinTargetsAndTransforms(
      hingeTargets.targets,
      stagedHingeTransforms,
    )
    if (!faceUpdates || !hingeUpdates) return null

    const result = Object.freeze({
      appliedAngles: freezeAngles(appliedAngles),
      faceTransforms: createDetachedTransformMap(stagedFaceTransforms),
      hingeTransforms: createDetachedTransformMap(stagedHingeTransforms),
    })

    // Matrix validation can invoke hostile Proxy traps. Confirm that neither
    // target map changed during staging before any matrix element is copied.
    if (
      !rawMapStillMatches(faceTargets.raw)
      || !rawMapStillMatches(hingeTargets.raw)
      || !matrixTargetsStillMatch(faceTargets.targets)
      || !matrixTargetsStillMatch(hingeTargets.targets)
    ) return null

    const updates = [...faceUpdates, ...hingeUpdates]
    try {
      for (const update of updates) {
        copyElements(update.target.elements, update.transform.elements)
      }
    } catch {
      // Genuine Matrix4 element arrays were preflighted as writable. Restore
      // the original pose as a final containment boundary for hostile inputs.
      for (const update of updates) {
        try {
          copyElements(
            update.target.elements,
            update.target.previousElements,
          )
        } catch {
          // JavaScript cannot make adversarial Proxy setters transactional.
          // Normal Three.js Matrix4 targets are fully restored here.
        }
      }
      return null
    }

    return result
  } catch {
    return null
  }
}

/**
 * Creates the collision identity for an already applied complete tree vector.
 *
 * The uniform-angle slot is deliberately fixed at zero. Consequently, only
 * the canonical per-hinge vector can identify the tree pose.
 */
export function createFoldPreviewTreeSceneCollisionPoseKey(
  model: FoldPreviewTreeCollisionModelIdentity,
  fixedFaceId: string,
  thickness: number | null,
  appliedAngles: readonly FoldPreviewHingeAngle[],
): string | null {
  try {
    if (!isRecord(model)) return null
    const rawProjectId = model.projectId
    const rawRevision = model.revision
    const rawKind = model.kind
    if (
      !validId(rawProjectId)
      || !validRevision(rawRevision)
      || rawKind !== 'fold_graph'
      || !validId(fixedFaceId)
      || !validCollisionThickness(thickness)
    ) return null
    const canonicalAngles = normalizeCollisionAngles(appliedAngles)
    if (!canonicalAngles) return null
    const key = collisionPoseKey(
      {
        projectId: rawProjectId,
        revision: rawRevision,
        kind: rawKind,
      },
      fixedFaceId,
      thickness,
      0,
      canonicalAngles,
    )
    return typeof key === 'string' && key.length > 0 ? key : null
  } catch {
    return null
  }
}

function snapshotTree(value: unknown): TreeSnapshot | null {
  if (!isRecord(value)) return null
  const rawKind = value.kind
  const rawRootFaceId = value.rootFaceId
  const rawJoints = value.joints
  if (
    rawKind !== 'tree'
    || !validId(rawRootFaceId)
    || !Array.isArray(rawJoints)
  ) return null
  const jointCount = rawJoints.length
  if (
    !Number.isSafeInteger(jointCount)
    || jointCount < 0
    || jointCount > MAX_TREE_JOINTS
  ) return null

  const rootFaceId = rawRootFaceId
  const reachedFaceIds = new Set<string>([rootFaceId])
  const faceIds = [rootFaceId]
  const hingeEdgeIdSet = new Set<string>()
  const joints: FoldPreviewTreeKinematics['joints'][number][] = []
  for (let jointIndex = 0; jointIndex < jointCount; jointIndex += 1) {
    const rawJoint = rawJoints[jointIndex]
    if (!isRecord(rawJoint)) return null
    const rawParentFaceId = rawJoint.parentFaceId
    const rawChildFaceId = rawJoint.childFaceId
    const rawHinge = rawJoint.hinge
    const rawChildRotationSign = rawJoint.childRotationSign
    if (
      !validId(rawParentFaceId)
      || !validId(rawChildFaceId)
      || !reachedFaceIds.has(rawParentFaceId)
      || reachedFaceIds.has(rawChildFaceId)
      || (
        rawChildRotationSign !== 1
        && rawChildRotationSign !== -1
      )
    ) return null
    const hinge = snapshotHinge(rawHinge)
    if (
      !hinge
      || hingeEdgeIdSet.has(hinge.edgeId)
      || !jointMatchesHinge(
        rawParentFaceId,
        rawChildFaceId,
        rawChildRotationSign,
        hinge,
      )
    ) return null
    reachedFaceIds.add(rawChildFaceId)
    faceIds.push(rawChildFaceId)
    hingeEdgeIdSet.add(hinge.edgeId)
    joints.push({
      parentFaceId: rawParentFaceId,
      childFaceId: rawChildFaceId,
      hinge,
      childRotationSign: rawChildRotationSign,
    })
  }
  if (
    faceIds.length !== jointCount + 1
    || faceIds.length > MAX_FOLD_PREVIEW_COLLISION_FACES
    || hingeEdgeIdSet.size !== jointCount
  ) return null

  return {
    tree: {
      kind: 'tree',
      rootFaceId,
      joints,
    },
    faceIds: Object.freeze([...faceIds]),
    hingeEdgeIds: Object.freeze(
      [...hingeEdgeIdSet].sort(compareText),
    ),
  }
}

function snapshotHinge(value: unknown): FoldPreviewHingeModel | null {
  if (!isRecord(value)) return null
  const rawEdgeId = value.edgeId
  const rawLeftFaceId = value.leftFaceId
  const rawRightFaceId = value.rightFaceId
  const rawStart = value.start
  const rawEnd = value.end
  const rawAxis = value.axis
  const rawAssignment = value.assignment
  const rawRotationSign = value.rotationSign
  const start = snapshotPoint(rawStart)
  const end = snapshotPoint(rawEnd)
  const axis = snapshotAxis(rawAxis)
  if (
    !validId(rawEdgeId)
    || !validId(rawLeftFaceId)
    || !validId(rawRightFaceId)
    || rawLeftFaceId === rawRightFaceId
    || !start
    || !end
    || start.vertexId === end.vertexId
    || !axis
    || (rawAssignment !== 'mountain' && rawAssignment !== 'valley')
    || (rawRotationSign !== 1 && rawRotationSign !== -1)
    || rawRotationSign !== (rawAssignment === 'mountain' ? 1 : -1)
  ) return null
  const hinge: FoldPreviewHingeModel = {
    edgeId: rawEdgeId,
    leftFaceId: rawLeftFaceId,
    rightFaceId: rawRightFaceId,
    start,
    end,
    axis,
    assignment: rawAssignment,
    rotationSign: rawRotationSign,
  }
  return validHingeGeometry(hinge) ? hinge : null
}

function snapshotPoint(
  value: unknown,
): FoldPreviewHingeModel['start'] | null {
  if (!isRecord(value)) return null
  const rawVertexId = value.vertexId
  const rawX = value.x
  const rawZ = value.z
  return validId(rawVertexId)
    && finiteNumber(rawX)
    && finiteNumber(rawZ)
    ? {
        vertexId: rawVertexId,
        x: normalizeZero(rawX),
        z: normalizeZero(rawZ),
      }
    : null
}

function snapshotAxis(
  value: unknown,
): FoldPreviewHingeModel['axis'] | null {
  if (!isRecord(value)) return null
  const rawX = value.x
  const rawZ = value.z
  return finiteNumber(rawX) && finiteNumber(rawZ)
    ? {
        x: normalizeZero(rawX),
        z: normalizeZero(rawZ),
      }
    : null
}

function jointMatchesHinge(
  parentFaceId: string,
  childFaceId: string,
  childRotationSign: 1 | -1,
  hinge: FoldPreviewHingeModel,
) {
  const pairMatches = (
    parentFaceId === hinge.leftFaceId
    && childFaceId === hinge.rightFaceId
  ) || (
    parentFaceId === hinge.rightFaceId
    && childFaceId === hinge.leftFaceId
  )
  return pairMatches
    && childRotationSign === (
      parentFaceId === hinge.leftFaceId
        ? hinge.rotationSign
        : -hinge.rotationSign
    )
}

function validHingeGeometry(hinge: FoldPreviewHingeModel) {
  const deltaX = hinge.end.x - hinge.start.x
  const deltaZ = hinge.end.z - hinge.start.z
  const segmentLength = Math.hypot(deltaX, deltaZ)
  const axisLength = Math.hypot(hinge.axis.x, hinge.axis.z)
  if (!(segmentLength > 0) || !(axisLength > 0)) return false
  const scale = segmentLength * axisLength
  const cross = deltaX * hinge.axis.z - deltaZ * hinge.axis.x
  const dot = deltaX * hinge.axis.x + deltaZ * hinge.axis.z
  return Number.isFinite(scale)
    && Number.isFinite(cross)
    && Number.isFinite(dot)
    && dot > 0
    && Math.abs(cross) <= scale * Number.EPSILON * 16
}

function normalizeCompleteAngles(
  value: unknown,
  expectedEdgeIds: readonly string[],
): readonly FoldPreviewHingeAngle[] | null {
  if (!Array.isArray(value)) return null
  const angleCount = value.length
  if (
    !Number.isSafeInteger(angleCount)
    || angleCount !== expectedEdgeIds.length
    || angleCount > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
  ) return null
  const expected = new Set(expectedEdgeIds)
  if (expected.size !== expectedEdgeIds.length) return null
  const byEdgeId = new Map<string, number>()
  for (let angleIndex = 0; angleIndex < angleCount; angleIndex += 1) {
    const rawAngle = value[angleIndex]
    if (!isRecord(rawAngle)) return null
    const rawEdgeId = rawAngle.edgeId
    const rawAngleDegrees = rawAngle.angleDegrees
    if (
      !validId(rawEdgeId)
      || !expected.has(rawEdgeId)
      || byEdgeId.has(rawEdgeId)
      || !validAngle(rawAngleDegrees)
    ) return null
    byEdgeId.set(rawEdgeId, normalizeZero(rawAngleDegrees))
  }
  const result: FoldPreviewHingeAngle[] = []
  for (const edgeId of expectedEdgeIds) {
    const angleDegrees = byEdgeId.get(edgeId)
    if (!validAngle(angleDegrees)) return null
    result.push({ edgeId, angleDegrees })
  }
  return result
}

function normalizeCollisionAngles(
  value: unknown,
): readonly FoldPreviewHingeAngle[] | null {
  if (!Array.isArray(value)) return null
  const angleCount = value.length
  if (
    !Number.isSafeInteger(angleCount)
    || angleCount < 0
    || angleCount > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
  ) return null
  const result: FoldPreviewHingeAngle[] = []
  const edgeIds = new Set<string>()
  for (let angleIndex = 0; angleIndex < angleCount; angleIndex += 1) {
    const rawAngle = value[angleIndex]
    if (!isRecord(rawAngle)) return null
    const rawEdgeId = rawAngle.edgeId
    const rawAngleDegrees = rawAngle.angleDegrees
    if (
      !validId(rawEdgeId)
      || edgeIds.has(rawEdgeId)
      || !validAngle(rawAngleDegrees)
    ) return null
    edgeIds.add(rawEdgeId)
    result.push({
      edgeId: rawEdgeId,
      angleDegrees: normalizeZero(rawAngleDegrees),
    })
  }
  return result.sort((first, second) =>
    compareText(first.edgeId, second.edgeId))
}

function snapshotTargetMap(
  value: unknown,
  expectedIds: readonly string[],
  maximumCount: number,
): TargetMapSnapshot | null {
  const raw = snapshotRawMap(value, maximumCount)
  if (!raw || raw.entries.length !== expectedIds.length) return null
  const expected = new Set(expectedIds)
  if (expected.size !== expectedIds.length) return null
  const targets: MatrixTarget[] = []
  const found = new Set<string>()
  for (const [rawId, rawMatrix] of raw.entries) {
    if (
      !validId(rawId)
      || !expected.has(rawId)
      || found.has(rawId)
    ) return null
    const matrix = snapshotMatrixTarget(rawMatrix)
    if (!matrix) return null
    found.add(rawId)
    targets.push({
      id: rawId,
      ...matrix,
    })
  }
  return found.size === expected.size
    ? {
        raw,
        targets: Object.freeze(targets),
      }
    : null
}

function snapshotRawMap(
  value: unknown,
  maximumCount: number,
): RawMapSnapshot | null {
  if (
    typeof value !== 'object'
    || value === null
    || typeof mapSizeGetter !== 'function'
  ) return null
  let rawSize: unknown
  let iterator: IterableIterator<[unknown, unknown]>
  try {
    rawSize = mapSizeGetter.call(value)
    iterator = Map.prototype.entries.call(value) as IterableIterator<
      [unknown, unknown]
    >
  } catch {
    return null
  }
  if (
    !Number.isSafeInteger(rawSize)
    || (rawSize as number) < 0
    || (rawSize as number) > maximumCount
  ) return null
  const entries: RawMapEntry[] = []
  for (const entry of iterator) {
    if (
      !Array.isArray(entry)
      || entry.length !== 2
      || entries.length >= maximumCount
    ) return null
    entries.push(Object.freeze([entry[0], entry[1]]))
  }
  return entries.length === rawSize
    ? {
        map: value as Map<unknown, unknown>,
        entries: Object.freeze(entries),
      }
    : null
}

function snapshotMatrixTarget(
  value: unknown,
): Omit<MatrixTarget, 'id'> | null {
  if (
    typeof value !== 'object'
    || value === null
  ) return null
  const matrix = value as Matrix4
  const registeredElements = lockedMatrixTargetElements.get(matrix)
  if (
    !registeredElements
    || !validWritableMatrixElements(registeredElements)
  ) return null
  return {
    matrix,
    elements: registeredElements,
    previousElements: Object.freeze([...registeredElements]),
  }
}

function matrixTargetsAreUnique(
  faceTargets: readonly MatrixTarget[],
  hingeTargets: readonly MatrixTarget[],
) {
  const matrices = new Set<Matrix4>()
  const elementArrays = new Set<number[]>()
  for (const targets of [faceTargets, hingeTargets]) {
    for (const target of targets) {
      if (
        matrices.has(target.matrix)
        || elementArrays.has(target.elements)
      ) return false
      matrices.add(target.matrix)
      elementArrays.add(target.elements)
    }
  }
  return true
}

function matrixTargetsStillMatch(
  targets: readonly MatrixTarget[],
) {
  for (const target of targets) {
    if (
      lockedMatrixTargetElements.get(target.matrix) !== target.elements
      || !validWritableMatrixElements(target.elements)
    ) return false
  }
  return true
}

function lockedMatrixTargetStillMatches(
  matrix: Matrix4,
  elements: number[],
) {
  if (Object.getPrototypeOf(matrix) !== Matrix4.prototype) return false
  const rawMatrixBrand = (
    matrix as unknown as Readonly<{ isMatrix4?: unknown }>
  ).isMatrix4
  const descriptor = Object.getOwnPropertyDescriptor(matrix, 'elements')
  const currentElements = matrix.elements
  return rawMatrixBrand === true
    && !!descriptor
    && 'value' in descriptor
    && descriptor.value === elements
    && descriptor.writable === false
    && descriptor.configurable === false
    && currentElements === elements
    && validWritableMatrixElements(elements)
}

function validWritableMatrixElements(
  value: unknown,
): value is number[] {
  return snapshotWritableMatrixElements(value) !== null
}

function snapshotWritableMatrixElements(
  value: unknown,
): readonly number[] | null {
  if (
    !Array.isArray(value)
    || Object.getPrototypeOf(value) !== Array.prototype
    || value.length !== MATRIX_ELEMENT_COUNT
  ) return null
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, 'length')
  if (!lengthDescriptor || lengthDescriptor.writable !== true) return null
  const elements: number[] = []
  for (
    let elementIndex = 0;
    elementIndex < MATRIX_ELEMENT_COUNT;
    elementIndex += 1
  ) {
    const descriptor = Object.getOwnPropertyDescriptor(
      value,
      String(elementIndex),
    )
    if (
      !descriptor
      || !('value' in descriptor)
      || descriptor.writable !== true
      || !finiteNumber(descriptor.value)
    ) return null
    elements.push(normalizeZero(descriptor.value))
  }
  return Object.freeze(elements)
}

function stageTransformMap(
  value: unknown,
  expectedIds: readonly string[],
  maximumCount: number,
): readonly StagedTransform[] | null {
  const raw = snapshotRawMap(value, maximumCount)
  if (!raw || raw.entries.length !== expectedIds.length) return null
  const expected = new Set(expectedIds)
  if (expected.size !== expectedIds.length) return null
  const found = new Set<string>()
  const transforms: StagedTransform[] = []
  for (const [rawId, rawMatrix] of raw.entries) {
    if (
      !validId(rawId)
      || !expected.has(rawId)
      || found.has(rawId)
    ) return null
    const elements = snapshotFiniteMatrixElements(rawMatrix)
    if (!elements) return null
    found.add(rawId)
    transforms.push({
      id: rawId,
      elements,
    })
  }
  return found.size === expected.size
    ? Object.freeze(transforms)
    : null
}

function snapshotFiniteMatrixElements(
  value: unknown,
): readonly number[] | null {
  if (
    typeof value !== 'object'
    || value === null
    || Object.getPrototypeOf(value) !== Matrix4.prototype
  ) return null
  const matrix = value as Matrix4
  if ((
    matrix as unknown as Readonly<{ isMatrix4?: unknown }>
  ).isMatrix4 !== true) return null
  const rawElements = matrix.elements
  if (
    !Array.isArray(rawElements)
    || rawElements.length !== MATRIX_ELEMENT_COUNT
  ) return null
  const elements: number[] = []
  for (
    let elementIndex = 0;
    elementIndex < MATRIX_ELEMENT_COUNT;
    elementIndex += 1
  ) {
    const rawElement = rawElements[elementIndex]
    if (!finiteNumber(rawElement)) return null
    elements.push(normalizeZero(rawElement))
  }
  return Object.freeze(elements)
}

function joinTargetsAndTransforms(
  targets: readonly MatrixTarget[],
  transforms: readonly StagedTransform[],
) {
  if (targets.length !== transforms.length) return null
  const transformsById = new Map(
    transforms.map((transform) => [transform.id, transform]),
  )
  const updates: Array<Readonly<{
    target: MatrixTarget
    transform: StagedTransform
  }>> = []
  for (const target of targets) {
    const transform = transformsById.get(target.id)
    if (!transform) return null
    updates.push({ target, transform })
  }
  return updates.length === transformsById.size
    ? Object.freeze(updates)
    : null
}

function rawMapStillMatches(snapshot: RawMapSnapshot) {
  const current = snapshotRawMap(
    snapshot.map,
    snapshot.entries.length,
  )
  if (!current || current.entries.length !== snapshot.entries.length) {
    return false
  }
  for (let entryIndex = 0; entryIndex < current.entries.length; entryIndex += 1) {
    const before = snapshot.entries[entryIndex]
    const after = current.entries[entryIndex]
    if (before[0] !== after[0] || before[1] !== after[1]) return false
  }
  return true
}

function createDetachedTransformMap(
  transforms: readonly StagedTransform[],
): ReadonlyMap<string, Matrix4> {
  return new Map(transforms.map((transform) => [
    transform.id,
    new Matrix4().fromArray([...transform.elements]),
  ]))
}

function copyElements(
  target: number[],
  source: readonly number[],
) {
  if (
    target.length !== MATRIX_ELEMENT_COUNT
    || source.length !== MATRIX_ELEMENT_COUNT
  ) throw new Error('invalid Matrix4 element count')
  for (
    let elementIndex = 0;
    elementIndex < MATRIX_ELEMENT_COUNT;
    elementIndex += 1
  ) {
    target[elementIndex] = source[elementIndex]
  }
}

function freezeAngles(
  angles: readonly FoldPreviewHingeAngle[],
): readonly FoldPreviewHingeAngle[] {
  return Object.freeze(angles.map((angle) => Object.freeze({
    edgeId: angle.edgeId,
    angleDegrees: angle.angleDegrees,
  })))
}

function validCollisionThickness(
  value: unknown,
): value is number | null {
  return value === null
    || (
      typeof value === 'number'
      && Number.isFinite(value)
      && value >= 0
    )
}

function validRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_ID_LENGTH
    && value.trim().length > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}
