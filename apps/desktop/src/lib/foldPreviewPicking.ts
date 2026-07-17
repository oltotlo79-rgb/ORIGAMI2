import {
  Raycaster,
  type Camera,
  type Object3D,
  type Vector2,
} from 'three'

export type FoldPreviewPickObject = Readonly<{
  id: string
  object: Object3D
}>

export type FoldPreviewPickTarget =
  | Readonly<{ kind: 'hinge'; edgeId: string }>
  | Readonly<{ kind: 'face'; faceId: string }>

export type FoldPreviewFaceSurfaceHit = Readonly<{
  faceId: string
  worldPoint: Readonly<{ x: number; y: number; z: number }>
  distance: number
  materialIndex: number
}>

/**
 * Raycasts one pointer sample. Hinges intentionally outrank faces so a crease
 * drawn on the paper remains selectable without relying on sub-pixel depth.
 */
export function pickFoldPreviewTarget(
  raycaster: Raycaster,
  camera: Camera,
  pointer: Vector2,
  hinges: readonly FoldPreviewPickObject[],
  faces: readonly FoldPreviewPickObject[],
  lineThreshold = 0.08,
): FoldPreviewPickTarget | null {
  if (
    !Number.isFinite(pointer.x)
    || !Number.isFinite(pointer.y)
    || Math.abs(pointer.x) > 1
    || Math.abs(pointer.y) > 1
    || !Number.isFinite(lineThreshold)
    || lineThreshold <= 0
  ) return null
  const hingeIndex = indexTargets(hinges)
  const faceIndex = indexTargets(faces)
  if (!hingeIndex || !faceIndex) return null

  raycaster.params.Line = { threshold: lineThreshold }
  raycaster.setFromCamera(pointer, camera)
  const hingeHit = raycaster.intersectObjects([...hingeIndex.keys()], false)[0]
  if (hingeHit) {
    const edgeId = hingeIndex.get(hingeHit.object)
    if (edgeId) return { kind: 'hinge', edgeId }
  }
  const faceHit = raycaster.intersectObjects([...faceIndex.keys()], false)[0]
  if (faceHit) {
    const faceId = faceIndex.get(faceHit.object)
    if (faceId) return { kind: 'face', faceId }
  }
  return null
}

/**
 * Returns a detached world-space point on the nearest rendered face.
 *
 * This is intentionally separate from selection picking: physical grab input
 * needs a material surface point, while adding mutable Three.js intersection
 * records to the stable selection contract would couple unrelated callers.
 */
export function pickFoldPreviewFaceSurface(
  raycaster: Raycaster,
  camera: Camera,
  pointer: Vector2,
  faces: readonly FoldPreviewPickObject[],
): FoldPreviewFaceSurfaceHit | null {
  if (
    !Number.isFinite(pointer.x)
    || !Number.isFinite(pointer.y)
    || Math.abs(pointer.x) > 1
    || Math.abs(pointer.y) > 1
  ) return null
  const faceIndex = indexTargets(faces)
  if (!faceIndex) return null
  try {
    raycaster.setFromCamera(pointer, camera)
    const hit = raycaster.intersectObjects([...faceIndex.keys()], false)[0]
    const materialIndex = hit?.face?.materialIndex
    if (
      !hit
      || !Number.isFinite(hit.distance)
      || hit.distance < 0
      || !Number.isFinite(hit.point.x)
      || !Number.isFinite(hit.point.y)
      || !Number.isFinite(hit.point.z)
      || typeof materialIndex !== 'number'
      || !Number.isSafeInteger(materialIndex)
      || materialIndex < 0
    ) return null
    const faceId = faceIndex.get(hit.object)
    if (!faceId) return null
    return Object.freeze({
      faceId,
      worldPoint: Object.freeze({
        x: hit.point.x,
        y: hit.point.y,
        z: hit.point.z,
      }),
      distance: hit.distance,
      materialIndex,
    })
  } catch {
    return null
  }
}

function indexTargets(targets: readonly FoldPreviewPickObject[]) {
  const byObject = new Map<Object3D, string>()
  const ids = new Set<string>()
  for (const target of targets) {
    if (
      typeof target.id !== 'string'
      || target.id.length === 0
      || ids.has(target.id)
      || byObject.has(target.object)
    ) return null
    ids.add(target.id)
    byObject.set(target.object, target.id)
  }
  return byObject
}
