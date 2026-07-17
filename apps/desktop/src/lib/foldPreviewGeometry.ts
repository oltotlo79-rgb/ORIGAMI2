import {
  BufferGeometry,
  Float32BufferAttribute,
  ShapeUtils,
  Vector2,
} from 'three'

export type FoldPreviewGeometryPoint = Readonly<{
  x: number
  z: number
}>

export type FoldPreviewTriangleIndices = readonly [number, number, number]

export const FOLD_PREVIEW_FRONT_MATERIAL_INDEX = 0
export const FOLD_PREVIEW_BACK_MATERIAL_INDEX = 1
export const FOLD_PREVIEW_SIDE_MATERIAL_INDEX = 2

const POSITION_COMPONENTS = 3
const TRIANGLE_COMPONENTS = 3 * POSITION_COMPONENTS

/**
 * Builds a closed, flat-shaded paper prism in the world XZ plane.
 *
 * The returned geometry is non-indexed so the two paper surfaces and every
 * boundary wall can carry independent normals. The prism is centred on Y=0;
 * material group 0 is the +Y paper front, group 1 is the -Y back, and group 2
 * contains the thickness walls.
 */
export function createFoldPreviewFaceGeometry(
  polygon: readonly FoldPreviewGeometryPoint[],
  visualThickness: number,
): BufferGeometry {
  validateThickness(visualThickness)
  const { signedDoubleArea, triangles } = validatedTriangulation(polygon)

  const halfThickness = visualThickness * 0.5
  if (!Number.isFinite(halfThickness) || halfThickness <= 0) {
    throw new RangeError('fold preview thickness is not representable')
  }

  const positions: number[] = []
  const normals: number[] = []

  const frontStart = vertexCount(positions)
  for (const triangle of triangles) {
    appendCapTriangle(positions, normals, polygon, triangle, halfThickness, true)
  }
  const frontCount = vertexCount(positions) - frontStart

  const backStart = vertexCount(positions)
  for (const triangle of triangles) {
    appendCapTriangle(positions, normals, polygon, triangle, -halfThickness, false)
  }
  const backCount = vertexCount(positions) - backStart

  const sideStart = vertexCount(positions)
  const polygonWinding = Math.sign(signedDoubleArea)
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    appendSide(
      positions,
      normals,
      current,
      next,
      halfThickness,
      polygonWinding,
    )
  }
  const sideCount = vertexCount(positions) - sideStart

  const positionAttribute = new Float32BufferAttribute(positions, POSITION_COMPONENTS)
  const normalAttribute = new Float32BufferAttribute(normals, POSITION_COMPONENTS)
  validateFloatAttributes(positionAttribute.array, normalAttribute.array)

  const geometry = new BufferGeometry()
  geometry.setAttribute('position', positionAttribute)
  geometry.setAttribute('normal', normalAttribute)
  geometry.addGroup(frontStart, frontCount, FOLD_PREVIEW_FRONT_MATERIAL_INDEX)
  geometry.addGroup(backStart, backCount, FOLD_PREVIEW_BACK_MATERIAL_INDEX)
  geometry.addGroup(sideStart, sideCount, FOLD_PREVIEW_SIDE_MATERIAL_INDEX)
  geometry.computeBoundingBox()
  return geometry
}

/**
 * Returns the same validated double-precision cap triangulation used by the
 * renderer, without exposing its Float32 GPU attributes.
 */
export function triangulateFoldPreviewPolygon(
  polygon: readonly FoldPreviewGeometryPoint[],
): readonly FoldPreviewTriangleIndices[] {
  return validatedTriangulation(polygon).triangles
}

function validatedTriangulation(polygon: readonly FoldPreviewGeometryPoint[]) {
  validatePolygon(polygon)
  const signedDoubleArea = polygonSignedDoubleArea(polygon)
  if (signedDoubleArea === 0) {
    throw new RangeError('fold preview polygon must have non-zero area')
  }
  const contour = polygon.map((point) => new Vector2(point.x, point.z))
  const rawTriangles = ShapeUtils.triangulateShape(contour, [])
  validateTriangulation(polygon, rawTriangles, signedDoubleArea)
  const triangles = rawTriangles.map(([first, second, third]) =>
    [first, second, third] as FoldPreviewTriangleIndices)
  return { signedDoubleArea, triangles }
}

function validatePolygon(polygon: readonly FoldPreviewGeometryPoint[]) {
  if (polygon.length < 3) {
    throw new RangeError('fold preview polygon must contain at least three points')
  }

  const distinctPoints = new Set<string>()
  for (const point of polygon) {
    if (!Number.isFinite(point.x) || !Number.isFinite(point.z)) {
      throw new RangeError('fold preview polygon coordinates must be finite')
    }
    const key = `${normalizeZero(point.x)},${normalizeZero(point.z)}`
    if (distinctPoints.has(key)) {
      throw new RangeError('fold preview polygon must not repeat a point')
    }
    distinctPoints.add(key)
  }

  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    const edgeLength = Math.hypot(next.x - current.x, next.z - current.z)
    if (!Number.isFinite(edgeLength) || edgeLength <= 0) {
      throw new RangeError('fold preview polygon contains a degenerate edge')
    }
  }
}

function validateThickness(visualThickness: number) {
  if (!Number.isFinite(visualThickness) || visualThickness <= 0) {
    throw new RangeError('fold preview thickness must be a positive finite number')
  }
}

function polygonSignedDoubleArea(polygon: readonly FoldPreviewGeometryPoint[]) {
  const origin = polygon[0]
  let sum = 0
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    const currentX = current.x - origin.x
    const currentZ = current.z - origin.z
    const nextX = next.x - origin.x
    const nextZ = next.z - origin.z
    const term = currentX * nextZ - currentZ * nextX
    sum += term
    if (!Number.isFinite(term) || !Number.isFinite(sum)) {
      throw new RangeError('fold preview polygon area is not representable')
    }
  }
  return sum
}

function validateTriangulation(
  polygon: readonly FoldPreviewGeometryPoint[],
  triangles: number[][],
  polygonSignedDoubleArea: number,
) {
  if (triangles.length === 0) {
    throw new RangeError('fold preview polygon triangulation produced no faces')
  }

  let triangulatedDoubleArea = 0
  for (const triangle of triangles) {
    if (
      triangle.length !== 3
      || triangle.some((index) => !Number.isInteger(index) || index < 0 || index >= polygon.length)
      || new Set(triangle).size !== 3
    ) {
      throw new RangeError('fold preview polygon triangulation returned invalid indices')
    }
    const [first, second, third] = triangle
    const triangleArea = triangleSignedDoubleArea(
      polygon[first],
      polygon[second],
      polygon[third],
    )
    if (!Number.isFinite(triangleArea) || triangleArea === 0) {
      throw new RangeError('fold preview polygon triangulation produced a degenerate face')
    }
    triangulatedDoubleArea += Math.abs(triangleArea)
  }

  const expectedDoubleArea = Math.abs(polygonSignedDoubleArea)
  const comparisonScale = Math.max(expectedDoubleArea, triangulatedDoubleArea)
  const tolerance = comparisonScale * Number.EPSILON * Math.max(32, polygon.length * 8)
  if (
    !Number.isFinite(triangulatedDoubleArea)
    || !Number.isFinite(tolerance)
    || Math.abs(triangulatedDoubleArea - expectedDoubleArea) > tolerance
  ) {
    throw new RangeError('fold preview polygon triangulation does not cover the polygon')
  }
}

function triangleSignedDoubleArea(
  first: FoldPreviewGeometryPoint,
  second: FoldPreviewGeometryPoint,
  third: FoldPreviewGeometryPoint,
) {
  const firstX = second.x - first.x
  const firstZ = second.z - first.z
  const secondX = third.x - first.x
  const secondZ = third.z - first.z
  return firstX * secondZ - firstZ * secondX
}

function appendCapTriangle(
  positions: number[],
  normals: number[],
  polygon: readonly FoldPreviewGeometryPoint[],
  triangle: readonly number[],
  y: number,
  front: boolean,
) {
  const [first, second, third] = triangle
  const triangleArea = triangleSignedDoubleArea(
    polygon[first],
    polygon[second],
    polygon[third],
  )
  // A positive XZ triangle points toward -Y in Three's right-handed world.
  // Reverse exactly one cap so its geometric winding matches its flat normal.
  const xzShouldBePositive = !front
  const order = (triangleArea > 0) === xzShouldBePositive
    ? [first, second, third]
    : [first, third, second]
  const normalY = front ? 1 : -1
  for (const index of order) {
    appendVertex(positions, polygon[index].x, y, polygon[index].z)
    appendVertex(normals, 0, normalY, 0)
  }
}

function appendSide(
  positions: number[],
  normals: number[],
  current: FoldPreviewGeometryPoint,
  next: FoldPreviewGeometryPoint,
  halfThickness: number,
  polygonWinding: number,
) {
  const deltaX = next.x - current.x
  const deltaZ = next.z - current.z
  const length = Math.hypot(deltaX, deltaZ)
  if (!Number.isFinite(length) || length <= 0) {
    throw new RangeError('fold preview polygon contains an unrepresentable side')
  }

  // Negative XZ area (the renderer model's +Y-view CCW convention) has its
  // exterior on the left of each directed edge; positive area is the reverse.
  const windingScale = polygonWinding < 0 ? 1 : -1
  const normalX = windingScale * (-deltaZ / length)
  const normalZ = windingScale * (deltaX / length)
  if (!Number.isFinite(normalX) || !Number.isFinite(normalZ)) {
    throw new RangeError('fold preview polygon contains an unrepresentable side normal')
  }

  const topCurrent = [current.x, halfThickness, current.z] as const
  const bottomCurrent = [current.x, -halfThickness, current.z] as const
  const topNext = [next.x, halfThickness, next.z] as const
  const bottomNext = [next.x, -halfThickness, next.z] as const
  const firstTriangle = polygonWinding < 0
    ? [topCurrent, bottomCurrent, bottomNext]
    : [topCurrent, bottomNext, bottomCurrent]
  const secondTriangle = polygonWinding < 0
    ? [topCurrent, bottomNext, topNext]
    : [topCurrent, topNext, bottomNext]
  for (const vertex of [...firstTriangle, ...secondTriangle]) {
    appendVertex(positions, vertex[0], vertex[1], vertex[2])
    appendVertex(normals, normalX, 0, normalZ)
  }
}

function appendVertex(target: number[], x: number, y: number, z: number) {
  target.push(x, y, z)
}

function vertexCount(positions: readonly number[]) {
  return positions.length / POSITION_COMPONENTS
}

function validateFloatAttributes(
  positions: ArrayLike<number>,
  normals: ArrayLike<number>,
) {
  if (
    positions.length === 0
    || positions.length !== normals.length
    || positions.length % TRIANGLE_COMPONENTS !== 0
  ) {
    throw new RangeError('fold preview geometry attributes are structurally invalid')
  }
  for (let index = 0; index < positions.length; index += 1) {
    if (!Number.isFinite(positions[index]) || !Number.isFinite(normals[index])) {
      throw new RangeError('fold preview geometry attributes are not finite')
    }
  }

  for (let offset = 0; offset < positions.length; offset += TRIANGLE_COMPONENTS) {
    const firstX = positions[offset]
    const firstY = positions[offset + 1]
    const firstZ = positions[offset + 2]
    const edgeAX = positions[offset + 3] - firstX
    const edgeAY = positions[offset + 4] - firstY
    const edgeAZ = positions[offset + 5] - firstZ
    const edgeBX = positions[offset + 6] - firstX
    const edgeBY = positions[offset + 7] - firstY
    const edgeBZ = positions[offset + 8] - firstZ
    const crossX = edgeAY * edgeBZ - edgeAZ * edgeBY
    const crossY = edgeAZ * edgeBX - edgeAX * edgeBZ
    const crossZ = edgeAX * edgeBY - edgeAY * edgeBX
    const normalX = normals[offset]
    const normalY = normals[offset + 1]
    const normalZ = normals[offset + 2]
    const alignment = crossX * normalX + crossY * normalY + crossZ * normalZ
    if (!Number.isFinite(alignment) || alignment <= 0) {
      throw new RangeError('fold preview geometry contains a degenerate or reversed triangle')
    }
  }
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}
