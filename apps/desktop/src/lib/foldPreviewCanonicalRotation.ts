import { Matrix4, Vector3 } from 'three'

/**
 * Builds the same right-handed axis rotation as Three.js while keeping the
 * user-visible 0, ±90, and ±180 degree cardinal angles algebraically exact.
 *
 * JavaScript's sin(π) is not zero. Feeding that residue into collision
 * geometry turns an intended coplanar flat fold into a numerically
 * near-parallel pair. Only exact cardinal radians are canonicalized; nearby
 * angles retain their real sine and must never be promoted to a cardinal pose.
 */
export function makeFoldPreviewCanonicalAxisRotation(
  rawAxis: Vector3,
  radians: number,
): Matrix4 | null {
  if (!rawAxis || !Number.isFinite(radians)) return null
  const axis = rawAxis.clone()
  const length = axis.length()
  if (!Number.isFinite(length) || length <= 0) return null
  axis.multiplyScalar(1 / length)
  if (![axis.x, axis.y, axis.z].every(Number.isFinite)) return null

  const endpoint = canonicalEndpointTrig(radians)
  if (!endpoint) {
    const rotation = new Matrix4().makeRotationAxis(axis, radians)
    return rotation.elements.every(Number.isFinite) ? rotation : null
  }

  const { cosine, sine } = endpoint
  const oneMinusCosine = 1 - cosine
  const x = axis.x
  const y = axis.y
  const z = axis.z
  const tx = oneMinusCosine * x
  const ty = oneMinusCosine * y
  const rotation = new Matrix4().set(
    tx * x + cosine,
    tx * y - sine * z,
    tx * z + sine * y,
    0,
    tx * y + sine * z,
    ty * y + cosine,
    ty * z - sine * x,
    0,
    tx * z - sine * y,
    ty * z + sine * x,
    oneMinusCosine * z * z + cosine,
    0,
    0,
    0,
    0,
    1,
  )
  return rotation.elements.every(Number.isFinite) ? rotation : null
}

/**
 * Builds the local scene matrix for geometry already translated by
 * `-pivot`. Keeping the canonical rotation as a Matrix4 avoids converting it
 * through a quaternion and reintroducing residue at exact cardinal angles.
 */
export function makeFoldPreviewCanonicalPivotMatrix(
  rawAxis: Vector3,
  pivot: Readonly<{ x: number; y: number; z: number }>,
  radians: number,
): Matrix4 | null {
  if (
    !pivot
    || ![pivot.x, pivot.y, pivot.z].every(Number.isFinite)
  ) return null
  const rotation = makeFoldPreviewCanonicalAxisRotation(rawAxis, radians)
  if (!rotation) return null
  const matrix = new Matrix4()
    .makeTranslation(pivot.x, pivot.y, pivot.z)
    .multiply(rotation)
  return matrix.elements.every(Number.isFinite) ? matrix : null
}

function canonicalEndpointTrig(radians: number) {
  if (radians === 0) return { cosine: 1, sine: 0 }
  if (radians === Math.PI / 2) return { cosine: 0, sine: 1 }
  if (radians === -Math.PI / 2) return { cosine: 0, sine: -1 }
  if (radians === Math.PI || radians === -Math.PI) {
    return { cosine: -1, sine: 0 }
  }
  return null
}
