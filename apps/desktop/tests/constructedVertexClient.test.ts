import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createConstructedVertexTransport,
} from '../src/lib/constructedVertexClient.ts'
import {
  classifyVertexPlacementAuthorityV1,
  CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
  CONSTRUCTED_VERTEX_MODEL_ID_V1,
  CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1,
  type ConstructedVertexPlacement,
  type NativeVertexConstructionV1,
} from '../src/lib/vertexPlacement.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel.ts'

const INSTANCE_ID = '00000000-0000-4000-8000-000000000001'
const PROJECT_ID = '00000000-0000-4000-8000-000000000002'
const NIL_ID = '00000000-0000-0000-0000-000000000000'

function anglePlacement(): ConstructedVertexPlacement {
  return {
    operation: 'add',
    x: 3,
    y: 4,
    constructedVertexAuthority: CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
    nativeConstruction: {
      schemaVersion: CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1,
      constructionModelId: CONSTRUCTED_VERTEX_MODEL_ID_V1,
      transcendentalModelId: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
      source: {
        kind: 'angle',
        anchorId: '00000000-0000-4000-8000-000000000003',
        rawX: 5,
        rawY: 6,
        angleDegrees: 45,
        angleSide: 'counterclockwise',
        referenceKind: 'edge',
        referenceEdgeId: '00000000-0000-4000-8000-000000000004',
      },
    },
  }
}

function circleLinePlacement(): ConstructedVertexPlacement {
  return {
    operation: 'add',
    x: 3,
    y: 4,
    constructedVertexAuthority: CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
    nativeConstruction: {
      schemaVersion: CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1,
      constructionModelId: CONSTRUCTED_VERTEX_MODEL_ID_V1,
      transcendentalModelId: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
      source: {
        kind: 'circle-line',
        centerVertexId: '00000000-0000-4000-8000-000000000006',
        radius: 5,
        edgeId: '00000000-0000-4000-8000-000000000007',
        rootSide: 0,
      },
    },
  }
}

function circleCirclePlacement(): ConstructedVertexPlacement {
  return {
    operation: 'add',
    x: 3,
    y: 4,
    constructedVertexAuthority: CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
    nativeConstruction: {
      schemaVersion: CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1,
      constructionModelId: CONSTRUCTED_VERTEX_MODEL_ID_V1,
      transcendentalModelId: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
      source: {
        kind: 'circle-circle',
        firstCenterVertexId: '00000000-0000-4000-8000-000000000008',
        firstRadius: 5,
        secondCenterVertexId: '00000000-0000-4000-8000-000000000009',
        secondRadius: 5,
        intersectionSide: 0,
      },
    },
  }
}

test('constructed vertex transport sends only OCC, models, source provenance, and expected operation', async () => {
  const calls: unknown[] = []
  const snapshot = { revision: 8 }
  const transport = createConstructedVertexTransport((command, arguments_) => {
    calls.push([command, arguments_])
    return snapshot
  })
  assert.equal(
    await transport.place(INSTANCE_ID, PROJECT_ID, 7, anglePlacement()),
    snapshot,
  )
  assert.deepEqual(calls, [[
    'place_constructed_vertex_v1',
    {
      request: {
        schemaVersion: 1,
        constructionModelId: CONSTRUCTED_VERTEX_MODEL_ID_V1,
        transcendentalModelId: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
        expectedProjectInstanceId: INSTANCE_ID,
        expectedProjectId: PROJECT_ID,
        expectedRevision: 7,
        expectedPlacement: { kind: 'add' },
        construction: {
          kind: 'angle',
          anchorId: '00000000-0000-4000-8000-000000000003',
          rawX: 5,
          rawY: 6,
          angleDegrees: 45,
          angleSide: 'counterclockwise',
          referenceKind: 'edge',
          referenceEdgeId: '00000000-0000-4000-8000-000000000004',
        },
      },
    },
  ]])
  const request = (
    calls[0] as readonly [string, { request: Record<string, unknown> }]
  )[1].request
  assert.equal(Object.hasOwn(request, 'x'), false)
  assert.equal(Object.hasOwn(request, 'y'), false)
  assert.equal(Object.hasOwn(request, 'fraction'), false)
})

test('split expectation carries only the current edge identity, never the preview fraction', async () => {
  const calls: unknown[] = []
  const addPlacement = anglePlacement()
  const placement: ConstructedVertexPlacement = {
    operation: 'split-edge',
    edgeId: '00000000-0000-4000-8000-000000000005',
    fraction: 0.25,
    constructedVertexAuthority: CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
    nativeConstruction: addPlacement.nativeConstruction,
  }
  const transport = createConstructedVertexTransport((command, arguments_) => {
    calls.push([command, arguments_])
    return {}
  })
  await transport.place(INSTANCE_ID, PROJECT_ID, 11, placement)
  const request = (
    calls[0] as readonly [string, { request: {
      expectedPlacement: Record<string, unknown>
    } }]
  )[1].request
  assert.deepEqual(request.expectedPlacement, {
    kind: 'split-edge',
    edgeId: '00000000-0000-4000-8000-000000000005',
  })
  assert.equal(Object.hasOwn(request.expectedPlacement, 'fraction'), false)
})

test('constructed move sends angle sources without preview coordinates or fallback authority', async () => {
  const calls: unknown[] = []
  const transport = createConstructedVertexTransport((command, arguments_) => {
    calls.push([command, arguments_])
    return { revision: 13 }
  })
  const construction = anglePlacement().nativeConstruction
  await transport.move(
    INSTANCE_ID,
    PROJECT_ID,
    12,
    construction.source.kind === 'angle'
      ? construction.source.anchorId
      : assert.fail('angle source'),
    construction,
  )
  assert.deepEqual(calls, [[
    'move_constructed_vertex_v1',
    {
      request: {
        schemaVersion: 1,
        constructionModelId: CONSTRUCTED_VERTEX_MODEL_ID_V1,
        transcendentalModelId: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
        expectedProjectInstanceId: INSTANCE_ID,
        expectedProjectId: PROJECT_ID,
        expectedRevision: 12,
        vertexId: '00000000-0000-4000-8000-000000000003',
        construction: construction.source,
      },
    },
  ]])
  const request = (
    calls[0] as readonly [string, { request: Record<string, unknown> }]
  )[1].request
  assert.equal(Object.hasOwn(request, 'x'), false)
  assert.equal(Object.hasOwn(request, 'y'), false)
})

test('invalid OCC bindings are rejected before native invocation', async () => {
  let calls = 0
  const transport = createConstructedVertexTransport(() => {
    calls += 1
    return {}
  })
  for (const [instanceId, projectId, revision] of [
    ['', PROJECT_ID, 0],
    [INSTANCE_ID, '', 0],
    [NIL_ID, PROJECT_ID, 0],
    [INSTANCE_ID, NIL_ID, 0],
    ['not-a-uuid', PROJECT_ID, 0],
    [INSTANCE_ID, 'AAAAAAAA-0000-4000-8000-000000000002', 0],
    [INSTANCE_ID, PROJECT_ID, -1],
    [INSTANCE_ID, PROJECT_ID, -0],
    [INSTANCE_ID, PROJECT_ID, 0.5],
    [INSTANCE_ID, PROJECT_ID, Number.MAX_SAFE_INTEGER + 1],
  ] as const) {
    await assert.rejects(
      transport.place(instanceId, projectId, revision, anglePlacement()),
      /invalid_constructed_vertex_occ_binding/u,
    )
  }
  await assert.rejects(
    transport.move(
      INSTANCE_ID,
      PROJECT_ID,
      0,
      '00000000-0000-4000-8000-000000000099',
      anglePlacement().nativeConstruction,
    ),
    /invalid_constructed_vertex_occ_binding/u,
  )
  assert.equal(calls, 0)
})

test('constructed authority survives enumerable spread and structured clone', async () => {
  const original = anglePlacement()
  const spread = { ...original }
  const cloned = structuredClone(original)
  assert.ok(Object.keys(original).includes('constructedVertexAuthority'))
  assert.ok(Object.keys(original).includes('nativeConstruction'))
  assert.equal(
    classifyVertexPlacementAuthorityV1(spread).kind,
    'native',
  )
  assert.equal(
    classifyVertexPlacementAuthorityV1(cloned).kind,
    'native',
  )
  assert.equal(
    classifyVertexPlacementAuthorityV1({
      operation: 'add',
      x: 3,
      y: 4,
    }).kind,
    'legacy',
  )

  const calls: unknown[] = []
  const transport = createConstructedVertexTransport((command) => {
    calls.push(command)
    return {}
  })
  await transport.place(INSTANCE_ID, PROJECT_ID, 0, spread)
  await transport.place(INSTANCE_ID, PROJECT_ID, 1, cloned)
  assert.deepEqual(calls, [
    'place_constructed_vertex_v1',
    'place_constructed_vertex_v1',
  ])
})

test('transport invokes native with a detached validated construction snapshot', async () => {
  const placement = anglePlacement()
  let request: Record<string, unknown> | undefined
  const transport = createConstructedVertexTransport((_command, arguments_) => {
    const source = placement.nativeConstruction.source as {
      rawX: number
    }
    source.rawX = 999
    request = (
      arguments_ as { request: Record<string, unknown> }
    ).request
    return {}
  })
  await transport.place(INSTANCE_ID, PROJECT_ID, 0, placement)
  const construction = request?.construction as {
    rawX: number
  }
  assert.equal(construction.rawX, 5)
})

test('invalid or collapsed constructed authority fails closed before native invocation', async () => {
  let calls = 0
  const transport = createConstructedVertexTransport(() => {
    calls += 1
    return {}
  })
  const invalidCases: unknown[] = []

  const invalidMarker = structuredClone(anglePlacement()) as Record<string, unknown>
  invalidMarker.constructedVertexAuthority = 'forged'
  invalidCases.push(invalidMarker)

  const missingMarker = structuredClone(anglePlacement()) as Record<string, unknown>
  delete missingMarker.constructedVertexAuthority
  invalidCases.push(missingMarker)

  const missingConstruction = structuredClone(anglePlacement()) as Record<string, unknown>
  delete missingConstruction.nativeConstruction
  invalidCases.push(missingConstruction)

  const inheritedAuthority = Object.assign(
    Object.create({
      constructedVertexAuthority: CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
      nativeConstruction: anglePlacement().nativeConstruction,
    }) as Record<string, unknown>,
    { operation: 'add', x: 3, y: 4 },
  )
  invalidCases.push(inheritedAuthority)

  const revokedAuthority = Proxy.revocable({}, {})
  revokedAuthority.revoke()
  invalidCases.push(revokedAuthority.proxy)

  const invalidModel = structuredClone(anglePlacement()) as {
    nativeConstruction: Record<string, unknown>
  }
  invalidModel.nativeConstruction.constructionModelId = 'forged'
  invalidCases.push(invalidModel)

  const invalidSource = structuredClone(anglePlacement()) as {
    nativeConstruction: { source: Record<string, unknown> }
  }
  invalidSource.nativeConstruction.source.kind = 'forged'
  invalidCases.push(invalidSource)

  for (const invalid of invalidCases) {
    assert.equal(
      classifyVertexPlacementAuthorityV1(invalid).kind,
      'invalid-native',
    )
    await assert.rejects(
      transport.place(
        INSTANCE_ID,
        PROJECT_ID,
        0,
        invalid as ConstructedVertexPlacement,
      ),
      /invalid_constructed_vertex_occ_binding/u,
    )
  }

  const invalidNativeIds: unknown[] = []
  const invalidAngleAnchor = structuredClone(anglePlacement()) as {
    nativeConstruction: { source: { anchorId: string } }
  }
  invalidAngleAnchor.nativeConstruction.source.anchorId = NIL_ID
  invalidNativeIds.push(invalidAngleAnchor)

  const invalidAngleReference = structuredClone(anglePlacement()) as {
    nativeConstruction: { source: { referenceEdgeId: string } }
  }
  invalidAngleReference.nativeConstruction.source.referenceEdgeId = 'not-a-uuid'
  invalidNativeIds.push(invalidAngleReference)

  const invalidCircleLineCenter = structuredClone(circleLinePlacement()) as {
    nativeConstruction: { source: { centerVertexId: string } }
  }
  invalidCircleLineCenter.nativeConstruction.source.centerVertexId = NIL_ID
  invalidNativeIds.push(invalidCircleLineCenter)

  const invalidCircleLineEdge = structuredClone(circleLinePlacement()) as {
    nativeConstruction: { source: { edgeId: string } }
  }
  invalidCircleLineEdge.nativeConstruction.source.edgeId = 'not-a-uuid'
  invalidNativeIds.push(invalidCircleLineEdge)

  const invalidFirstCircle = structuredClone(circleCirclePlacement()) as {
    nativeConstruction: { source: { firstCenterVertexId: string } }
  }
  invalidFirstCircle.nativeConstruction.source.firstCenterVertexId = NIL_ID
  invalidNativeIds.push(invalidFirstCircle)

  const invalidSecondCircle = structuredClone(circleCirclePlacement()) as {
    nativeConstruction: { source: { secondCenterVertexId: string } }
  }
  invalidSecondCircle.nativeConstruction.source.secondCenterVertexId = 'not-a-uuid'
  invalidNativeIds.push(invalidSecondCircle)

  const base = anglePlacement()
  invalidNativeIds.push({
    operation: 'split-edge',
    edgeId: NIL_ID,
    fraction: 0.25,
    constructedVertexAuthority: CONSTRUCTED_VERTEX_AUTHORITY_MARKER_V1,
    nativeConstruction: base.nativeConstruction,
  })

  for (const invalid of invalidNativeIds) {
    assert.notEqual(
      classifyVertexPlacementAuthorityV1(invalid).kind,
      'legacy',
    )
    await assert.rejects(
      transport.place(
        INSTANCE_ID,
        PROJECT_ID,
        0,
        invalid as ConstructedVertexPlacement,
      ),
      /invalid_constructed_vertex_occ_binding/u,
    )
  }
  const invalidMoveConstruction = structuredClone(
    anglePlacement().nativeConstruction,
  ) as {
    constructionModelId: string
  }
  invalidMoveConstruction.constructionModelId = 'forged'
  await assert.rejects(
    transport.move(
      INSTANCE_ID,
      PROJECT_ID,
      0,
      '00000000-0000-4000-8000-000000000003',
      invalidMoveConstruction as unknown as NativeVertexConstructionV1,
    ),
    /invalid_constructed_vertex_occ_binding/u,
  )
  await assert.rejects(
    transport.move(
      INSTANCE_ID,
      PROJECT_ID,
      0,
      NIL_ID,
      anglePlacement().nativeConstruction,
    ),
    /invalid_constructed_vertex_occ_binding/u,
  )
  assert.equal(calls, 0)
})
