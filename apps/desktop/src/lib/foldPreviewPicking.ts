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
