import { Matrix4, Vector3 } from 'three'
import type { FoldPreviewGraphKinematics } from './foldPreviewModel'

export type FoldPreviewTreeKinematics = Extract<
  FoldPreviewGraphKinematics,
  { kind: 'tree' }
>

export type FoldPreviewTreePose = Readonly<{
  /** Rest-world coordinates transformed into the posed world. */
  faceTransforms: ReadonlyMap<string, Matrix4>
  /** Each hinge follows its parent face; points on the axis also match the child. */
  hingeTransforms: ReadonlyMap<string, Matrix4>
}>

/**
 * Propagates one shared fold magnitude through an acyclic hinge graph.
 *
 * Joints must be parent-before-child. Every local rotation is expressed in
 * the common flat rest frame, then composed after its parent's transform.
 */
export function calculateFoldTreePose(
  kinematics: FoldPreviewTreeKinematics,
  angleDegrees: number,
): FoldPreviewTreePose | null {
  if (!Number.isFinite(angleDegrees) || angleDegrees < 0 || angleDegrees > 180) return null

  const faceTransforms = new Map<string, Matrix4>([
    [kinematics.rootFaceId, new Matrix4()],
  ])
  const hingeTransforms = new Map<string, Matrix4>()
  for (const joint of kinematics.joints) {
    const parent = faceTransforms.get(joint.parentFaceId)
    const hinge = joint.hinge
    if (
      !parent
      || faceTransforms.has(joint.childFaceId)
      || hingeTransforms.has(hinge.edgeId)
      || (joint.childRotationSign !== 1 && joint.childRotationSign !== -1)
      || !finiteHinge(hinge)
    ) return null

    const axis = new Vector3(hinge.axis.x, 0, hinge.axis.z)
    const axisLength = axis.length()
    if (!Number.isFinite(axisLength) || axisLength <= 0) return null
    axis.multiplyScalar(1 / axisLength)
    const radians = angleDegrees * joint.childRotationSign * Math.PI / 180
    const localRotation = new Matrix4()
      .makeTranslation(hinge.start.x, 0, hinge.start.z)
      .multiply(new Matrix4().makeRotationAxis(axis, radians))
      .multiply(new Matrix4().makeTranslation(-hinge.start.x, 0, -hinge.start.z))
    const child = parent.clone().multiply(localRotation)
    if (!child.elements.every(Number.isFinite)) return null

    hingeTransforms.set(hinge.edgeId, parent.clone())
    faceTransforms.set(joint.childFaceId, child)
  }

  return faceTransforms.size === kinematics.joints.length + 1
    ? { faceTransforms, hingeTransforms }
    : null
}

function finiteHinge(hinge: FoldPreviewTreeKinematics['joints'][number]['hinge']) {
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
