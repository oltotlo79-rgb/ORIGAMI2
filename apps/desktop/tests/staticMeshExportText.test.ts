import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  STATIC_MESH_EXPORT_PRESENTATION_TEXT,
  STATIC_MESH_EXPORT_WARNING_TEXT,
} from '../src/lib/staticMeshExportText.ts'

test('static mesh export presentation catalogs are closed and deeply frozen', () => {
  assert.deepEqual(
    Object.keys(STATIC_MESH_EXPORT_PRESENTATION_TEXT),
    ['unknownByteCount', 'numberLocale'],
  )
  assert.deepEqual(
    Object.keys(STATIC_MESH_EXPORT_WARNING_TEXT),
    [
      'mid_surface_only',
      'no_thickness_solid',
      'independent_face_solids',
      'no_textures_animation',
      'no_project_semantics',
      'stl_triangle_soup_facet_normals',
      'stl_printability_not_guaranteed',
    ],
  )
  for (const catalog of [
    STATIC_MESH_EXPORT_PRESENTATION_TEXT,
    STATIC_MESH_EXPORT_WARNING_TEXT,
  ]) {
    assert.equal(Object.isFrozen(catalog), true)
    for (const text of Object.values(catalog)) {
      assert.deepEqual(Object.keys(text), ['ja', 'en'])
      assert.equal(Object.isFrozen(text), true)
    }
  }
})

test('static mesh export delegates locale choice to the presentation catalogs', () => {
  const source = readFileSync(
    new URL('../src/lib/staticMeshExport.ts', import.meta.url),
    'utf8',
  )
  assert.doesNotMatch(source, /locale\s*[!=]==?\s*['"](?:ja|en)['"]/u)
  assert.doesNotMatch(source, /['"](?:ja|en)['"]\s*\?/u)
  assert.match(source, /STATIC_MESH_EXPORT_PRESENTATION_TEXT/u)
  assert.match(source, /STATIC_MESH_EXPORT_WARNING_TEXT/u)
})
