import test from 'node:test'
import assert from 'node:assert/strict'

import {
  normalizeStaticMeshExportPreviewResponse,
  normalizeStaticMeshExportSaveResponse,
  staticMeshExportWarningMessage,
} from '../src/lib/staticMeshExport.ts'

const UUIDS = {
  export: '11111111-1111-4111-8111-111111111111',
  instance: '22222222-2222-4222-8222-222222222222',
  project: '33333333-3333-4333-8333-333333333333',
} as const

function preview(format: 'obj' | 'stl' | 'glb' = 'obj') {
  const glb = format === 'glb'
  return {
    preview: {
      exportId: UUIDS.export,
      projectInstanceId: UUIDS.instance,
      projectId: UUIDS.project,
      revision: 7,
      sourceFingerprint: 'a'.repeat(64),
      poseGeneration: '42',
      format,
      formatSummary: format === 'obj'
        ? 'Wavefront OBJ・mm・右手系Z-up・静的三角形'
        : format === 'stl'
          ? 'Binary STL・mm・右手系Z-up・静的三角形'
          : 'glTF 2.0 GLB・m・右手系Y-up・静的三角形',
      suggestedFileName: `bird-pose.${format === 'stl' ? 'stl' : format}`,
      byteCount: 1024,
      paperThicknessMm: 0.1,
      faceCount: 3,
      vertexCount: 12,
      triangleCount: 6,
      geometryProfile: 'authenticated_mid_surface_triangle_mesh_v1',
      sourceUnit: 'millimeter',
      encodedUnit: glb ? 'meter' : 'millimeter',
      sourceAxis: 'right-handed X-right Y-forward Z-up',
      encodedAxis: glb
        ? 'glTF 2.0 right-handed -X-right Y-up Z-forward'
        : 'right-handed X-right Y-forward Z-up',
      warnings: [
        'mid_surface_only',
        'no_thickness_solid',
        'no_textures_animation',
        'no_project_semantics',
        ...(format === 'stl'
          ? [
              'stl_triangle_soup_facet_normals',
              'stl_printability_not_guaranteed',
            ]
          : []),
      ],
    },
  }
}

test('strictly admits all native static-mesh preview formats', () => {
  for (const format of ['obj', 'stl', 'glb'] as const) {
    const normalized = normalizeStaticMeshExportPreviewResponse(preview(format))
    assert.ok(normalized)
    assert.equal(normalized.preview.format, format)
    assert.ok(Object.isFrozen(normalized.preview))
    assert.ok(Object.isFrozen(normalized.preview.warnings))
  }
})

test('rejects hostile metadata, unknown fields, and noncanonical bindings', () => {
  const mutations = [
    (value: ReturnType<typeof preview>) => {
      Object.assign(value.preview, { bytes: [1, 2, 3] })
    },
    (value: ReturnType<typeof preview>) => {
      value.preview.poseGeneration = '042'
    },
    (value: ReturnType<typeof preview>) => {
      value.preview.sourceFingerprint = '../private'
    },
    (value: ReturnType<typeof preview>) => {
      value.preview.vertexCount = 100_001
    },
    (value: ReturnType<typeof preview>) => {
      value.preview.encodedAxis = 'left-handed'
    },
    (value: ReturnType<typeof preview>) => {
      value.preview.warnings.reverse()
    },
    (value: ReturnType<typeof preview>) => {
      value.preview.suggestedFileName = 'bird.obj\u0000.exe'
    },
  ]
  for (const mutate of mutations) {
    const value = preview()
    mutate(value)
    assert.equal(normalizeStaticMeshExportPreviewResponse(value), null)
  }
})

test('rejects accessors and proxy failures without invoking a field getter', () => {
  let getterCalls = 0
  const accessor = preview()
  Object.defineProperty(accessor.preview, 'format', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 'obj'
    },
  })
  assert.equal(normalizeStaticMeshExportPreviewResponse(accessor), null)
  assert.equal(getterCalls, 0)

  const revoked = Proxy.revocable(preview().preview, {})
  revoked.revoke()
  assert.doesNotThrow(() => {
    assert.equal(
      normalizeStaticMeshExportPreviewResponse({ preview: revoked.proxy }),
      null,
    )
  })
})

test('save responses expose only a cancellation bit', () => {
  assert.deepEqual(normalizeStaticMeshExportSaveResponse({ canceled: true }), {
    canceled: true,
  })
  assert.equal(
    normalizeStaticMeshExportSaveResponse({
      canceled: false,
      path: 'C:\\private\\bird.obj',
    }),
    null,
  )
})

test('loss messages explicitly disclose mid-surface and STL limitations', () => {
  assert.match(
    staticMeshExportWarningMessage('mid_surface_only', 'ja'),
    /中央面/,
  )
  assert.match(
    staticMeshExportWarningMessage('no_thickness_solid', 'en'),
    /closed manifold/,
  )
  assert.match(
    staticMeshExportWarningMessage('stl_triangle_soup_facet_normals', 'ja'),
    /頂点index.*頂点法線.*triangle soup.*facet normal/,
  )
  assert.match(
    staticMeshExportWarningMessage('stl_triangle_soup_facet_normals', 'en'),
    /vertex indices.*vertex normals.*triangle soup.*facet normal/,
  )
  assert.match(
    staticMeshExportWarningMessage('stl_printability_not_guaranteed', 'ja'),
    /3Dプリント可能性を保証しません/,
  )
})

test('STL warning allowlist requires the exact loss sequence', () => {
  const missingTriangleSoup = preview('stl')
  missingTriangleSoup.preview.warnings.splice(4, 1)
  assert.equal(
    normalizeStaticMeshExportPreviewResponse(missingTriangleSoup),
    null,
  )

  const reordered = preview('stl')
  reordered.preview.warnings.splice(
    4,
    2,
    'stl_printability_not_guaranteed',
    'stl_triangle_soup_facet_normals',
  )
  assert.equal(normalizeStaticMeshExportPreviewResponse(reordered), null)

  const unexpectedForObj = preview('obj')
  unexpectedForObj.preview.warnings.push('stl_triangle_soup_facet_normals')
  assert.equal(normalizeStaticMeshExportPreviewResponse(unexpectedForObj), null)
})
