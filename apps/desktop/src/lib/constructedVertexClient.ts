import { invoke } from '@tauri-apps/api/core'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import type { ProjectSnapshot } from './coreClient.ts'
import type {
  ConstructedVertexPlacement,
  NativeVertexConstructionV1,
} from './vertexPlacement.ts'
import {
  isConstructedVertexPlacement,
  isNativeVertexConstructionV1,
} from './vertexPlacement.ts'

export type ConstructedVertexNativeInvoke = (
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) => unknown

export type ConstructedVertexTransport = Readonly<{
  place(
    expectedProjectInstanceId: string,
    expectedProjectId: string,
    expectedRevision: number,
    placement: ConstructedVertexPlacement,
  ): Promise<ProjectSnapshot>
  move(
    expectedProjectInstanceId: string,
    expectedProjectId: string,
    expectedRevision: number,
    vertexId: string,
    construction: NativeVertexConstructionV1,
  ): Promise<ProjectSnapshot>
}>

export function createConstructedVertexTransport(
  nativeInvoke: ConstructedVertexNativeInvoke = invoke,
): ConstructedVertexTransport {
  return Object.freeze({
    async place(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
      placement,
    ) {
      if (
        !isCanonicalNonNilUuid(expectedProjectInstanceId)
        || !isCanonicalNonNilUuid(expectedProjectId)
        || !Number.isSafeInteger(expectedRevision)
        || expectedRevision < 0
        || Object.is(expectedRevision, -0)
      ) throw new Error('invalid_constructed_vertex_occ_binding')
      const placementSnapshot = snapshotConstructedVertexPlacementV1(placement)
      if (!placementSnapshot) {
        throw new Error('invalid_constructed_vertex_occ_binding')
      }
      const expectedPlacement = placementSnapshot.operation === 'add'
        ? { kind: 'add' as const }
        : {
          kind: 'split-edge' as const,
          edgeId: placementSnapshot.edgeId,
        }
      return await nativeInvoke('place_constructed_vertex_v1', {
        request: {
          schemaVersion: placementSnapshot.nativeConstruction.schemaVersion,
          constructionModelId:
            placementSnapshot.nativeConstruction.constructionModelId,
          transcendentalModelId:
            placementSnapshot.nativeConstruction.transcendentalModelId,
          expectedProjectInstanceId,
          expectedProjectId,
          expectedRevision,
          expectedPlacement,
          construction: placementSnapshot.nativeConstruction.source,
        },
      }) as ProjectSnapshot
    },
    async move(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
      vertexId,
      construction,
    ) {
      if (
        !isCanonicalNonNilUuid(expectedProjectInstanceId)
        || !isCanonicalNonNilUuid(expectedProjectId)
        || !isCanonicalNonNilUuid(vertexId)
        || !Number.isSafeInteger(expectedRevision)
        || expectedRevision < 0
        || Object.is(expectedRevision, -0)
      ) throw new Error('invalid_constructed_vertex_occ_binding')
      const constructionSnapshot =
        snapshotAngleMoveConstructionV1(construction, vertexId)
      if (!constructionSnapshot) {
        throw new Error('invalid_constructed_vertex_occ_binding')
      }
      return await nativeInvoke('move_constructed_vertex_v1', {
        request: {
          schemaVersion: constructionSnapshot.schemaVersion,
          constructionModelId: constructionSnapshot.constructionModelId,
          transcendentalModelId: constructionSnapshot.transcendentalModelId,
          expectedProjectInstanceId,
          expectedProjectId,
          expectedRevision,
          vertexId,
          construction: constructionSnapshot.source,
        },
      }) as ProjectSnapshot
    },
  })
}

function snapshotConstructedVertexPlacementV1(
  placement: unknown,
): ConstructedVertexPlacement | null {
  try {
    const snapshot: unknown = structuredClone(placement)
    return isCanonicalConstructedVertexPlacementV1(snapshot)
      ? snapshot
      : null
  } catch {
    return null
  }
}

function snapshotAngleMoveConstructionV1(
  construction: unknown,
  vertexId: string,
): (
  NativeVertexConstructionV1 & Readonly<{
    source: Extract<
      NativeVertexConstructionV1['source'],
      { kind: 'angle' }
    >
  }>
) | null {
  try {
    const snapshot: unknown = structuredClone(construction)
    return isCanonicalAngleMoveConstructionV1(snapshot, vertexId)
      ? snapshot
      : null
  } catch {
    return null
  }
}

function isCanonicalConstructedVertexPlacementV1(
  placement: unknown,
): placement is ConstructedVertexPlacement {
  try {
    return isConstructedVertexPlacement(placement)
      && hasCanonicalConstructedVertexIdsV1(placement.nativeConstruction)
      && (
        placement.operation !== 'split-edge'
        || isCanonicalNonNilUuid(placement.edgeId)
      )
  } catch {
    return false
  }
}

function isCanonicalAngleMoveConstructionV1(
  construction: unknown,
  vertexId: string,
): construction is NativeVertexConstructionV1 & Readonly<{
  source: Extract<
    NativeVertexConstructionV1['source'],
    { kind: 'angle' }
  >
}> {
  try {
    return isNativeVertexConstructionV1(construction)
      && hasCanonicalConstructedVertexIdsV1(construction)
      && construction.source.kind === 'angle'
      && construction.source.anchorId === vertexId
  } catch {
    return false
  }
}

function hasCanonicalConstructedVertexIdsV1(
  construction: NativeVertexConstructionV1,
): boolean {
  const { source } = construction
  if (source.kind === 'angle') {
    return isCanonicalNonNilUuid(source.anchorId)
      && (
        source.referenceKind === 'global-horizontal'
        || isCanonicalNonNilUuid(source.referenceEdgeId)
      )
  }
  if (source.kind === 'circle-line') {
    return isCanonicalNonNilUuid(source.centerVertexId)
      && isCanonicalNonNilUuid(source.edgeId)
  }
  return isCanonicalNonNilUuid(source.firstCenterVertexId)
    && isCanonicalNonNilUuid(source.secondCenterVertexId)
}

const DEFAULT_TRANSPORT = createConstructedVertexTransport()

export function placeConstructedVertexV1(
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  placement: ConstructedVertexPlacement,
) {
  return DEFAULT_TRANSPORT.place(
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    placement,
  )
}

export function moveConstructedVertexV1(
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  vertexId: string,
  construction: NativeVertexConstructionV1,
) {
  return DEFAULT_TRANSPORT.move(
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
    vertexId,
    construction,
  )
}
