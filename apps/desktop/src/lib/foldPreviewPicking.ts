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
  localPoint: Readonly<{ x: number; y: number; z: number }>
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
 * Returns detached world- and object-local points on the nearest rendered face.
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
  preferredFaceId?: string,
): FoldPreviewFaceSurfaceHit | null {
  if (
    !Number.isFinite(pointer.x)
    || !Number.isFinite(pointer.y)
    || Math.abs(pointer.x) > 1
    || Math.abs(pointer.y) > 1
    || (
      preferredFaceId !== undefined
      && (
        typeof preferredFaceId !== 'string'
        || preferredFaceId.length === 0
      )
    )
  ) return null
  const faceIndex = indexTargets(faces)
  if (!faceIndex) return null
  try {
    raycaster.setFromCamera(pointer, camera)
    const hits = raycaster.intersectObjects([...faceIndex.keys()], false)
    const nearestHit = hits[0]
    if (!validSurfaceIntersection(nearestHit, faceIndex)) return null
    const hit = preferredFaceId === undefined
      ? nearestHit
      : preferredSurfaceIntersection(
          hits,
          nearestHit.distance,
          faceIndex,
          preferredFaceId,
        )
    if (!hit || !validSurfaceIntersection(hit, faceIndex)) return null
    const materialIndex = hit?.face?.materialIndex
    if (
      typeof materialIndex !== 'number'
      || !Number.isSafeInteger(materialIndex)
      || materialIndex < 0
    ) return null
    const faceId = faceIndex.get(hit.object)
    if (!faceId) return null
    const worldDeterminant = hit.object.matrixWorld.determinant()
    if (
      !Number.isFinite(worldDeterminant)
      || Math.abs(worldDeterminant) < Number.EPSILON
    ) return null
    const localPoint = hit.object.worldToLocal(hit.point.clone())
    if (
      !Number.isFinite(localPoint.x)
      || !Number.isFinite(localPoint.y)
      || !Number.isFinite(localPoint.z)
    ) return null
    return Object.freeze({
      faceId,
      worldPoint: Object.freeze({
        x: hit.point.x,
        y: hit.point.y,
        z: hit.point.z,
      }),
      localPoint: Object.freeze({
        x: localPoint.x,
        y: localPoint.y,
        z: localPoint.z,
      }),
      distance: hit.distance,
      materialIndex,
    })
  } catch {
    return null
  }
}

function preferredSurfaceIntersection(
  hits: ReturnType<Raycaster['intersectObjects']>,
  nearestDistance: number,
  faceIndex: ReadonlyMap<Object3D, string>,
  preferredFaceId: string,
) {
  const preferred = hits.find((hit) =>
    faceIndex.get(hit.object) === preferredFaceId)
  if (
    !preferred
    || !Number.isFinite(preferred.distance)
    || preferred.distance < 0
  ) return null
  const distanceScale = Math.max(
    1,
    Math.abs(nearestDistance),
    Math.abs(preferred.distance),
  )
  const coincidentTolerance =
    distanceScale * Number.EPSILON * 1024
  if (
    !Number.isFinite(coincidentTolerance)
    || preferred.distance - nearestDistance > coincidentTolerance
  ) return null
  return preferred
}

function validSurfaceIntersection(
  hit: ReturnType<Raycaster['intersectObjects']>[number] | undefined,
  faceIndex: ReadonlyMap<Object3D, string>,
) {
  return Boolean(
    hit
    && Number.isFinite(hit.distance)
    && hit.distance >= 0
    && Number.isFinite(hit.point.x)
    && Number.isFinite(hit.point.y)
    && Number.isFinite(hit.point.z)
    && faceIndex.has(hit.object),
  )
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
