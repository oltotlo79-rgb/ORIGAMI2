import { Matrix4, Vector3 } from 'three'
import { resolveSingleFoldAnchor } from './foldPreviewAnchoring.ts'
import type { SingleFoldPreviewModel } from './foldPreviewModel'

export type FoldPreviewSingleFoldPose = Readonly<{
  fixedFaceId: string
  movingFaceId: string
  /** Rest-world coordinates transformed into the posed world. */
  faceTransforms: ReadonlyMap<string, Matrix4>
  /** The fixed world axis used by the moving face throughout this pose. */
  axisStart: Readonly<{ x: number; y: number; z: number }>
  axisEnd: Readonly<{ x: number; y: number; z: number }>
  signedAngleRadians: number
}>

/**
 * Calculates a single-fold pose without reading mutable Three.js scene state.
 *
 * Selecting the other fixed face keeps that face in the common rest frame and
 * reverses only the relative hinge rotation. This is the canonical pose source
 * for pointwise and continuous collision analysis.
 */
export function calculateSingleFoldPose(
  model: SingleFoldPreviewModel,
  fixedFaceId: string,
  angleDegrees: number,
): FoldPreviewSingleFoldPose | null {
  if (
    !model
    || model.kind !== 'single_fold'
    || !Number.isFinite(angleDegrees)
    || angleDegrees < 0
    || angleDegrees > 180
  ) return null
  const anchor = resolveSingleFoldAnchor(model, fixedFaceId)
  if (!anchor || !validHinge(model.hinge)) return null

  const axis = new Vector3(model.hinge.axis.x, 0, model.hinge.axis.z)
  const axisLength = axis.length()
  if (!Number.isFinite(axisLength) || axisLength <= 0) return null
  axis.multiplyScalar(1 / axisLength)
  const signedAngleRadians = angleDegrees
    * anchor.movingRotationSign
    * Math.PI
    / 180
  const movingTransform = new Matrix4()
    .makeTranslation(model.hinge.start.x, 0, model.hinge.start.z)
    .multiply(new Matrix4().makeRotationAxis(axis, signedAngleRadians))
    .multiply(new Matrix4().makeTranslation(
      -model.hinge.start.x,
      0,
      -model.hinge.start.z,
    ))
  if (!movingTransform.elements.every(Number.isFinite)) return null

  const faceTransforms = new Map<string, Matrix4>([
    [anchor.fixedFace.id, new Matrix4()],
    [anchor.movingFace.id, movingTransform],
  ])
  if (faceTransforms.size !== 2) return null
  return {
    fixedFaceId: anchor.fixedFace.id,
    movingFaceId: anchor.movingFace.id,
    faceTransforms,
    axisStart: {
      x: model.hinge.start.x,
      y: 0,
      z: model.hinge.start.z,
    },
    axisEnd: {
      x: model.hinge.end.x,
      y: 0,
      z: model.hinge.end.z,
    },
    signedAngleRadians,
  }
}

function validHinge(hinge: SingleFoldPreviewModel['hinge']) {
  if (
    !hinge
    || (hinge.rotationSign !== 1 && hinge.rotationSign !== -1)
    || ![
      hinge.start.x,
      hinge.start.z,
      hinge.end.x,
      hinge.end.z,
      hinge.axis.x,
      hinge.axis.z,
    ].every(Number.isFinite)
  ) return false
  const segmentX = hinge.end.x - hinge.start.x
  const segmentZ = hinge.end.z - hinge.start.z
  const segmentLength = Math.hypot(segmentX, segmentZ)
  const axisLength = Math.hypot(hinge.axis.x, hinge.axis.z)
  if (!(segmentLength > 0) || !(axisLength > 0)) return false
  const scale = segmentLength * axisLength
  const cross = segmentX * hinge.axis.z - segmentZ * hinge.axis.x
  const dot = segmentX * hinge.axis.x + segmentZ * hinge.axis.z
  return Number.isFinite(scale)
    && Number.isFinite(cross)
    && Number.isFinite(dot)
    && dot > 0
    && Math.abs(cross) <= scale * Number.EPSILON * 16
}
