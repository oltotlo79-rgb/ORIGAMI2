import assert from 'node:assert/strict'
import test from 'node:test'

import { STATIC_MESH_EXPORT_COPY } from '../src/lib/staticMeshExportDialogText.ts'

test('static mesh export dialog catalog is locale-complete and deeply frozen', () => {
  assert.deepEqual(Object.keys(STATIC_MESH_EXPORT_COPY), ['ja', 'en'])
  assertLocaleShape(STATIC_MESH_EXPORT_COPY.ja, STATIC_MESH_EXPORT_COPY.en)
  assertDeeplyFrozen(STATIC_MESH_EXPORT_COPY)
})

test('static mesh export catalog preserves reviewed geometry limitations', () => {
  assert.equal(STATIC_MESH_EXPORT_COPY.ja.eyebrow, '現在姿勢の3D書き出し')
  assert.equal(
    STATIC_MESH_EXPORT_COPY.en.generating,
    ' current pose is being validated and generated…',
  )
  assert.equal(
    STATIC_MESH_EXPORT_COPY.ja.midSurface,
    '重要: 出力は紙の「中央面」だけです。紙厚を持つソリッド、閉じた多様体、3Dプリント可能な模型ではありません。',
  )
  assert.equal(
    STATIC_MESH_EXPORT_COPY.en.faceSolids,
    'Important: exactly coplanar adjacent faces are welded. A strictly two-triangle, one-hinge pose is also joined only when the native exact thickness corridor issues and revalidates a boundary capability. Other hinge solids remain separate; general unions and 3D printability are not guaranteed.',
  )
  assert.deepEqual(STATIC_MESH_EXPORT_COPY.en.printabilityStatus, {
    manifold_verified: 'Manifold verified within the bounded checks',
    not_verified: 'Manifold not verified',
    not_applicable: 'Not applicable (positive-thickness STL/GLB only)',
  })
})

function assertLocaleShape(
  left: Readonly<Record<string, unknown>>,
  right: Readonly<Record<string, unknown>>,
) {
  assert.deepEqual(Object.keys(left), Object.keys(right))
  for (const key of Object.keys(left)) {
    const leftValue = left[key]
    const rightValue = right[key]
    assert.equal(typeof leftValue, typeof rightValue, key)
    if (
      typeof leftValue === 'object'
      && leftValue !== null
      && typeof rightValue === 'object'
      && rightValue !== null
    ) {
      assertLocaleShape(
        leftValue as Readonly<Record<string, unknown>>,
        rightValue as Readonly<Record<string, unknown>>,
      )
    }
  }
}

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) {
    assertDeeplyFrozen(child)
  }
}
