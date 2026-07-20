import { invoke } from '@tauri-apps/api/core'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

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

export function normalizeCurrentLayerOrderView(value: unknown): CurrentLayerOrderView | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null
  const root = value as Record<string, unknown>
  if (Object.keys(root).sort().join() !== [
    'cells', 'layerOrderGeneration', 'projectId', 'projectInstanceId', 'readOnly', 'revision',
  ].sort().join()
    || !isCanonicalNonNilUuid(root.projectInstanceId)
    || !isCanonicalNonNilUuid(root.projectId)
    || !Number.isSafeInteger(root.revision) || (root.revision as number) < 0
    || !Number.isSafeInteger(root.layerOrderGeneration)
    || (root.layerOrderGeneration as number) <= 0
    || root.readOnly !== true || !Array.isArray(root.cells) || root.cells.length > 500_000) return null
  for (const value of root.cells) {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return null
    const cell = value as Record<string, unknown>
    if (Object.keys(cell).sort().join() !== ['boundaryWorld', 'bottomToTopFaces', 'cellKeySha256'].sort().join()
      || typeof cell.cellKeySha256 !== 'string' || !/^[0-9a-f]{64}$/.test(cell.cellKeySha256)
      || !Array.isArray(cell.bottomToTopFaces) || cell.bottomToTopFaces.length === 0
      || !cell.bottomToTopFaces.every(isCanonicalNonNilUuid)
      || !Array.isArray(cell.boundaryWorld) || cell.boundaryWorld.length < 3
      || cell.boundaryWorld.length > 4096
      || !cell.boundaryWorld.every((point) => Array.isArray(point) && point.length === 3
        && point.every((coordinate) => typeof coordinate === 'number' && Number.isFinite(coordinate)))) return null
  }
  return root as unknown as CurrentLayerOrderView
}

export async function getCurrentLayerOrderView(authority: {
  projectInstanceId: string
  projectId: string
  revision: number
}) {
  const parsed = normalizeCurrentLayerOrderView(await invoke('get_current_layer_order_view', {
    request: {
      expectedProjectInstanceId: authority.projectInstanceId,
      expectedProjectId: authority.projectId,
      expectedRevision: authority.revision,
    },
  }))
  if (!parsed) throw new Error('invalid current layer-order view')
  return parsed
}
