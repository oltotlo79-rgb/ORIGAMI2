import type { InstructionPose } from './coreClient.ts'
import type { FoldPreviewModel } from './foldPreviewModel.ts'
import { rerootFoldPreviewTree } from './foldPreviewAnchoring.ts'
import { calculateFoldTreePoseWithAngles } from './foldPreviewKinematics.ts'
import { calculateSingleFoldPose } from './foldPreviewSingleFoldKinematics.ts'
import { Matrix4 } from 'three'

export type OnionSkinDirection = 'previous' | 'next'
export type InstructionOnionSkinRequest = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprint: string
  sourceStepId: string
  targetStepId: string
  direction: OnionSkinDirection
  pose: InstructionPose
}>

type Step = Readonly<{
  id: string
  stale: boolean
  declarativeOnly: boolean
  pose: InstructionPose
}>

export function createInstructionOnionSkinRequest(input: Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprint: string
  steps: readonly Step[]
  selectedStepId: string
  direction: OnionSkinDirection
}>): InstructionOnionSkinRequest | null {
  if (input.direction !== 'previous' && input.direction !== 'next') return null
  const sourceIndex = input.steps.findIndex((step) => step.id === input.selectedStepId)
  const targetIndex = sourceIndex + (input.direction === 'previous' ? -1 : 1)
  const source = input.steps[sourceIndex]
  const target = input.steps[targetIndex]
  if (!source || !target || source.stale || target.stale
    || source.declarativeOnly || target.declarativeOnly
    || source.pose.model !== 'absolute_hinge_angles_v1'
    || target.pose.model !== 'absolute_hinge_angles_v1'
    || source.pose.source_model_fingerprint !== input.foldModelFingerprint
    || target.pose.source_model_fingerprint !== input.foldModelFingerprint) return null
  const hingeAngles = target.pose.hinge_angles.map((hinge) => Object.freeze({
    edge: hinge.edge,
    angle_degrees: hinge.angle_degrees,
  }))
  return Object.freeze({
    projectInstanceId: input.projectInstanceId,
    projectId: input.projectId,
    revision: input.revision,
    foldModelFingerprint: input.foldModelFingerprint,
    sourceStepId: source.id,
    targetStepId: target.id,
    direction: input.direction,
    pose: Object.freeze({
      model: 'absolute_hinge_angles_v1' as const,
      source_model_fingerprint: target.pose.source_model_fingerprint,
      fixed_face: target.pose.fixed_face,
      hinge_angles: Object.freeze(hingeAngles),
    }),
  })
}

export function resolveInstructionOnionSkinTransforms(
  request: InstructionOnionSkinRequest | null,
  model: FoldPreviewModel | null | undefined,
  authority: Readonly<{
    projectInstanceId: string | null
    foldModelFingerprint: string | null
  }>,
): ReadonlyMap<string, Matrix4> | null {
  if (!request || !model
    || (request.direction !== 'previous' && request.direction !== 'next')
    || request.projectInstanceId !== authority.projectInstanceId
    || request.foldModelFingerprint !== authority.foldModelFingerprint
    || request.projectId !== model.projectId || request.revision !== model.revision
    || request.sourceStepId === request.targetStepId
    || request.pose.model !== 'absolute_hinge_angles_v1'
    || request.pose.source_model_fingerprint !== request.foldModelFingerprint) return null
  const expectedEdges = model.kind === 'planar' ? []
    : model.kind === 'single_fold' ? [model.hinge.edgeId]
      : model.hinges.map((hinge) => hinge.edgeId)
  const submitted = request.pose.hinge_angles
  const expectedEdgeSet = new Set(expectedEdges)
  if (submitted.length !== expectedEdges.length
    || expectedEdgeSet.size !== expectedEdges.length
    || new Set(submitted.map(({ edge }) => edge)).size !== submitted.length
    || submitted.some(({ edge, angle_degrees }) => !expectedEdgeSet.has(edge)
      || !Number.isFinite(angle_degrees) || angle_degrees < 0 || angle_degrees > 180)) return null
  for (let index = 1; index < submitted.length; index += 1) {
    if (submitted[index - 1]!.edge >= submitted[index]!.edge) return null
  }
  if (model.kind === 'planar') {
    return request.pose.fixed_face === null
      ? new Map([[model.faces[0].id, new Matrix4()]])
      : null
  }
  const fixedFace = request.pose.fixed_face
  if (!fixedFace || !model.faces.some(({ id }) => id === fixedFace)) return null
  if (model.kind === 'single_fold') {
    return calculateSingleFoldPose(
      model, fixedFace, submitted[0]!.angle_degrees,
    )?.faceTransforms ?? null
  }
  if (model.kinematics.kind !== 'tree') return null
  const tree = rerootFoldPreviewTree(model.kinematics, fixedFace)
  if (!tree) return null
  return calculateFoldTreePoseWithAngles(tree, {
    kind: 'per_hinge',
    angles: submitted.map(({ edge, angle_degrees }) => ({
      edgeId: edge, angleDegrees: angle_degrees,
    })),
  })?.faceTransforms ?? null
}
