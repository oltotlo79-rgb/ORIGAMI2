import { invoke } from '@tauri-apps/api/core'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export const MAX_CURRENT_LAYER_ORDER_VIEW_FACES = 2_048
export const MAX_CURRENT_LAYER_ORDER_VIEW_CELLS = 4_096
export const MAX_CURRENT_LAYER_ORDER_VIEW_VERTICES_PER_CELL = 4_096
export const MAX_CURRENT_LAYER_ORDER_VIEW_TOTAL_VERTICES = 16_384
export const MAX_CURRENT_LAYER_ORDER_VIEW_RENDER_VERTEX_INSTANCES_PER_CELL =
  32_768
export const MAX_CURRENT_LAYER_ORDER_VIEW_ESTIMATED_SERIALIZED_BYTES =
  4 * 1024 * 1024

const CURRENT_LAYER_ORDER_VIEW_RESPONSE_BASE_BYTES = 256
const CURRENT_LAYER_ORDER_VIEW_CELL_BASE_BYTES = 192
const CURRENT_LAYER_ORDER_VIEW_FACE_SERIALIZED_BYTES = 128
const CURRENT_LAYER_ORDER_VIEW_VERTEX_SERIALIZED_BYTES = 128

export type LayerOrderViewerCell = Readonly<{
  cellKeySha256: string
  bottomToTopFaces: readonly string[]
  boundaryWorld: readonly (readonly [number, number, number])[]
}>

export type CurrentLayerOrderView = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  layerOrderGeneration: number
  cells: readonly LayerOrderViewerCell[]
  readOnly: true
}>

export function normalizeCurrentLayerOrderView(
  value: unknown,
): CurrentLayerOrderView | null {
  try {
    const root = exactRecord(value, [
      'cells',
      'layerOrderGeneration',
      'projectId',
      'projectInstanceId',
      'readOnly',
      'revision',
    ])
    if (
      !root
      || !isCanonicalNonNilUuid(root.projectInstanceId)
      || !isCanonicalNonNilUuid(root.projectId)
      || !isNonNegativeSafeInteger(root.revision)
      || !isPositiveSafeInteger(root.layerOrderGeneration)
      || root.readOnly !== true
    ) return null

    const rawCells = denseArray(root.cells, MAX_CURRENT_LAYER_ORDER_VIEW_CELLS)
    if (!rawCells) return null

    const cellKeys = new Set<string>()
    const materialFaces = new Set<string>()
    const cells: LayerOrderViewerCell[] = []
    let totalVertices = 0
    let estimatedSerializedBytes =
      CURRENT_LAYER_ORDER_VIEW_RESPONSE_BASE_BYTES

    for (const rawCell of rawCells) {
      const cell = exactRecord(rawCell, [
        'boundaryWorld',
        'bottomToTopFaces',
        'cellKeySha256',
      ])
      if (
        !cell
        || typeof cell.cellKeySha256 !== 'string'
        || !/^[0-9a-f]{64}$/u.test(cell.cellKeySha256)
        || cellKeys.has(cell.cellKeySha256)
      ) return null

      const rawFaces = denseArray(
        cell.bottomToTopFaces,
        MAX_CURRENT_LAYER_ORDER_VIEW_FACES,
      )
      const rawBoundary = denseArray(
        cell.boundaryWorld,
        MAX_CURRENT_LAYER_ORDER_VIEW_VERTICES_PER_CELL,
      )
      if (
        !rawFaces
        || rawFaces.length === 0
        || !rawBoundary
        || rawBoundary.length < 3
        || rawFaces.length * rawBoundary.length
          > MAX_CURRENT_LAYER_ORDER_VIEW_RENDER_VERTEX_INSTANCES_PER_CELL
      ) return null

      totalVertices += rawBoundary.length
      estimatedSerializedBytes +=
        CURRENT_LAYER_ORDER_VIEW_CELL_BASE_BYTES
        + rawFaces.length * CURRENT_LAYER_ORDER_VIEW_FACE_SERIALIZED_BYTES
        + rawBoundary.length * CURRENT_LAYER_ORDER_VIEW_VERTEX_SERIALIZED_BYTES
      if (
        totalVertices > MAX_CURRENT_LAYER_ORDER_VIEW_TOTAL_VERTICES
        || estimatedSerializedBytes
          > MAX_CURRENT_LAYER_ORDER_VIEW_ESTIMATED_SERIALIZED_BYTES
      ) return null

      const faces: string[] = []
      const facesInCell = new Set<string>()
      for (const face of rawFaces) {
        if (
          !isCanonicalNonNilUuid(face)
          || facesInCell.has(face)
        ) return null
        facesInCell.add(face)
        materialFaces.add(face)
        if (materialFaces.size > MAX_CURRENT_LAYER_ORDER_VIEW_FACES) return null
        faces.push(face)
      }

      const boundary: Readonly<[number, number, number]>[] = []
      for (const rawPoint of rawBoundary) {
        const point = denseArray(rawPoint, 3)
        if (!point || point.length !== 3) return null
        const x = finiteCoordinate(point[0])
        const y = finiteCoordinate(point[1])
        const z = finiteCoordinate(point[2])
        if (x === null || y === null || z === null) return null
        boundary.push(Object.freeze([x, y, z] as [number, number, number]))
      }

      cellKeys.add(cell.cellKeySha256)
      cells.push(Object.freeze({
        cellKeySha256: cell.cellKeySha256,
        bottomToTopFaces: Object.freeze(faces),
        boundaryWorld: Object.freeze(boundary),
      }))
    }

    return Object.freeze({
      projectInstanceId: root.projectInstanceId,
      projectId: root.projectId,
      revision: root.revision,
      layerOrderGeneration: root.layerOrderGeneration,
      cells: Object.freeze(cells),
      readOnly: true,
    }) as CurrentLayerOrderView
  } catch {
    return null
  }
}

export async function getCurrentLayerOrderView(authority: {
  projectInstanceId: string
  projectId: string
  revision: number
}) {
  if (
    !isCanonicalNonNilUuid(authority.projectInstanceId)
    || !isCanonicalNonNilUuid(authority.projectId)
    || !isNonNegativeSafeInteger(authority.revision)
  ) throw new Error('invalid current layer-order view authority')
  const parsed = normalizeCurrentLayerOrderView(await invoke('get_current_layer_order_view', {
    request: {
      expectedProjectInstanceId: authority.projectInstanceId,
      expectedProjectId: authority.projectId,
      expectedRevision: authority.revision,
    },
  }))
  if (
    !parsed
    || parsed.projectInstanceId !== authority.projectInstanceId
    || parsed.projectId !== authority.projectId
    || parsed.revision !== authority.revision
  ) throw new Error('invalid current layer-order view')
  return parsed
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0
}

function finiteCoordinate(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  return Object.is(value, -0) ? 0 : value
}

function descriptorSnapshot(value: object): PropertyDescriptorMap | null {
  try {
    return Object.getOwnPropertyDescriptors(value)
  } catch {
    return null
  }
}

function exactRecord(
  value: unknown,
  expectedKeys: readonly string[],
): Record<string, unknown> | null {
  if (typeof value !== 'object' || value === null) return null
  let array: boolean
  try {
    array = Array.isArray(value)
  } catch {
    return null
  }
  if (array) return null
  try {
    if (Object.getPrototypeOf(value) !== Object.prototype) return null
  } catch {
    return null
  }
  const descriptors = descriptorSnapshot(value)
  if (!descriptors) return null
  const keys = Reflect.ownKeys(descriptors)
  if (
    keys.length !== expectedKeys.length
    || keys.some((key) => typeof key === 'symbol')
  ) return null
  const actual = (keys as string[]).slice().sort()
  const expected = [...expectedKeys].sort()
  if (actual.some((key, index) => key !== expected[index])) return null
  const detached = Object.create(null) as Record<string, unknown>
  for (const key of expectedKeys) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !descriptor.enumerable
      || !('value' in descriptor)
    ) return null
    detached[key] = descriptor.value
  }
  return detached
}

function denseArray(value: unknown, maximum: number): unknown[] | null {
  let array: boolean
  try {
    array = Array.isArray(value)
  } catch {
    return null
  }
  if (!array || typeof value !== 'object' || value === null) return null
  try {
    if (Object.getPrototypeOf(value) !== Array.prototype) return null
  } catch {
    return null
  }
  const descriptors = descriptorSnapshot(value)
  if (!descriptors) return null
  const keys = Reflect.ownKeys(descriptors)
  if (keys.some((key) => typeof key === 'symbol')) return null
  const lengthDescriptor = descriptors.length
  if (!lengthDescriptor || !('value' in lengthDescriptor)) return null
  const length = lengthDescriptor.value
  if (
    typeof length !== 'number'
    || !Number.isSafeInteger(length)
    || length < 0
    || length > maximum
    || keys.length !== length + 1
  ) return null
  const detached: unknown[] = []
  for (let index = 0; index < length; index += 1) {
    const descriptor = descriptors[String(index)]
    if (
      !descriptor
      || !descriptor.enumerable
      || !('value' in descriptor)
    ) return null
    detached.push(descriptor.value)
  }
  return detached
}
