import type { FoldPreviewTreeKinematics } from './foldPreviewKinematics'
import type {
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
  SingleFoldPreviewModel,
} from './foldPreviewModel'

export type SingleFoldAnchor = Readonly<{
  fixedFace: FoldPreviewFaceModel
  movingFace: FoldPreviewFaceModel
  movingRotationSign: 1 | -1
}>

/**
 * Rebuilds a validated tree from another fixed face. Reversing a joint also
 * reverses its signed relative rotation; hinge geometry remains canonical.
 */
export function rerootFoldPreviewTree(
  tree: FoldPreviewTreeKinematics,
  targetRootFaceId: string,
): FoldPreviewTreeKinematics | null {
  if (
    typeof tree.rootFaceId !== 'string'
    || tree.rootFaceId.length === 0
    || typeof targetRootFaceId !== 'string'
    || targetRootFaceId.length === 0
  ) return null

  type Neighbor = Readonly<{
    faceId: string
    hinge: FoldPreviewTreeKinematics['joints'][number]['hinge']
    childRotationSign: 1 | -1
  }>
  const adjacency = new Map<string, Neighbor[]>([[tree.rootFaceId, []]])
  const reachedInSourceOrder = new Set<string>([tree.rootFaceId])
  const edgeIds = new Set<string>()
  for (const joint of tree.joints) {
    if (
      typeof joint.parentFaceId !== 'string'
      || joint.parentFaceId.length === 0
      || typeof joint.childFaceId !== 'string'
      || joint.childFaceId.length === 0
      || !reachedInSourceOrder.has(joint.parentFaceId)
      || reachedInSourceOrder.has(joint.childFaceId)
      || (joint.childRotationSign !== 1 && joint.childRotationSign !== -1)
      || typeof joint.hinge.edgeId !== 'string'
      || joint.hinge.edgeId.length === 0
      || edgeIds.has(joint.hinge.edgeId)
      || !validHinge(joint.hinge)
    ) return null

    reachedInSourceOrder.add(joint.childFaceId)
    edgeIds.add(joint.hinge.edgeId)
    const parentNeighbors = adjacency.get(joint.parentFaceId)
    if (!parentNeighbors) return null
    const childNeighbors: Neighbor[] = []
    adjacency.set(joint.childFaceId, childNeighbors)
    parentNeighbors.push({
      faceId: joint.childFaceId,
      hinge: joint.hinge,
      childRotationSign: joint.childRotationSign,
    })
    childNeighbors.push({
      faceId: joint.parentFaceId,
      hinge: joint.hinge,
      childRotationSign: joint.childRotationSign === 1 ? -1 : 1,
    })
  }
  if (!reachedInSourceOrder.has(targetRootFaceId)) return null
  if (targetRootFaceId === tree.rootFaceId) return tree

  const joints: FoldPreviewTreeKinematics['joints'][number][] = []
  const visited = new Set<string>([targetRootFaceId])
  const queue = [targetRootFaceId]
  for (let index = 0; index < queue.length; index += 1) {
    const parentFaceId = queue[index]
    for (const neighbor of adjacency.get(parentFaceId) ?? []) {
      if (visited.has(neighbor.faceId)) continue
      visited.add(neighbor.faceId)
      queue.push(neighbor.faceId)
      joints.push({
        parentFaceId,
        childFaceId: neighbor.faceId,
        hinge: neighbor.hinge,
        childRotationSign: neighbor.childRotationSign,
      })
    }
  }

  return visited.size === reachedInSourceOrder.size
    && joints.length === tree.joints.length
    ? { kind: 'tree', rootFaceId: targetRootFaceId, joints }
    : null
}

/** Resolves which of the canonical left/right faces stays fixed. */
export function resolveSingleFoldAnchor(
  model: SingleFoldPreviewModel,
  fixedFaceId: string,
): SingleFoldAnchor | null {
  if (
    typeof fixedFaceId !== 'string'
    || fixedFaceId.length === 0
    || model.faces[0].id === model.faces[1].id
    || model.fixedFace.id !== model.faces[0].id
    || model.movingFace.id !== model.faces[1].id
    || (model.hinge.rotationSign !== 1 && model.hinge.rotationSign !== -1)
  ) return null
  if (fixedFaceId === model.faces[0].id) {
    return {
      fixedFace: model.fixedFace,
      movingFace: model.movingFace,
      movingRotationSign: model.hinge.rotationSign,
    }
  }
  if (fixedFaceId === model.faces[1].id) {
    return {
      fixedFace: model.movingFace,
      movingFace: model.fixedFace,
      movingRotationSign: model.hinge.rotationSign === 1 ? -1 : 1,
    }
  }
  return null
}

function validHinge(hinge: FoldPreviewHingeModel) {
  if (![
    hinge.start.x,
    hinge.start.z,
    hinge.end.x,
    hinge.end.z,
    hinge.axis.x,
    hinge.axis.z,
  ].every(Number.isFinite)) return false
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
