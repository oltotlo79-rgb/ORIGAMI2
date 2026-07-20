import assert from 'node:assert/strict'
import test from 'node:test'
import { normalizeCurrentLayerOrderView } from '../src/lib/currentLayerOrderView.ts'

const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
const project = '018f47a2-4b7a-7cc1-8abc-665544332211'

const value = {
  projectInstanceId: instance,
  projectId: project,
  revision: 3,
  layerOrderGeneration: 4,
  cells: [{
    cellKeySha256: 'a'.repeat(64),
    bottomToTopFaces: [project, instance],
    boundaryWorld: [[0, 0, 0], [1, 0, 0], [0, 0, -1]],
  }],
  readOnly: true,
}

test('current layer viewer admits only bounded read-only generation data', () => {
  assert.ok(normalizeCurrentLayerOrderView(value))
  assert.equal(normalizeCurrentLayerOrderView({ ...value, proof: {} }), null)
  assert.equal(normalizeCurrentLayerOrderView({ ...value, readOnly: false }), null)
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [{ ...value.cells[0], boundaryWorld: [[0, 0, Number.NaN]] }],
  }), null)
})
