import assert from 'node:assert/strict'
import test from 'node:test'
import {
  MAX_CURRENT_LAYER_ORDER_VIEW_CELLS,
  MAX_CURRENT_LAYER_ORDER_VIEW_FACES,
  MAX_CURRENT_LAYER_ORDER_VIEW_RENDER_VERTEX_INSTANCES_PER_CELL,
  normalizeCurrentLayerOrderView,
} from '../src/lib/currentLayerOrderView.ts'

const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
const project = '018f47a2-4b7a-7cc1-8abc-665544332211'

test('current layer viewer admits a detached deeply frozen native response', () => {
  const value = validValue()
  value.cells[0]!.boundaryWorld[0]![2] = -0
  const parsed = normalizeCurrentLayerOrderView(value)
  assert.ok(parsed)
  assert.equal(Object.isFrozen(parsed), true)
  assert.equal(Object.isFrozen(parsed.cells), true)
  assert.equal(Object.isFrozen(parsed.cells[0]), true)
  assert.equal(Object.isFrozen(parsed.cells[0]!.bottomToTopFaces), true)
  assert.equal(Object.isFrozen(parsed.cells[0]!.boundaryWorld), true)
  assert.equal(Object.isFrozen(parsed.cells[0]!.boundaryWorld[0]), true)
  assert.equal(Object.is(parsed.cells[0]!.boundaryWorld[0]![2], -0), false)

  value.cells[0]!.bottomToTopFaces[0] = instance
  value.cells[0]!.boundaryWorld[0]![0] = 99
  assert.equal(parsed.cells[0]!.bottomToTopFaces[0], project)
  assert.equal(parsed.cells[0]!.boundaryWorld[0]![0], 0)
})

test('current layer viewer rejects malformed records and arrays fail closed', () => {
  const value = validValue()
  assert.equal(normalizeCurrentLayerOrderView({ ...value, proof: {} }), null)
  assert.equal(normalizeCurrentLayerOrderView({ ...value, readOnly: false }), null)
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [{
      ...value.cells[0],
      boundaryWorld: [[0, 0, Number.NaN]],
    }],
  }), null)

  const sparseCells: unknown[] = []
  sparseCells.length = 1
  assert.equal(normalizeCurrentLayerOrderView({ ...value, cells: sparseCells }), null)

  const cellsWithExtraProperty = [...value.cells] as typeof value.cells & {
    extra?: boolean
  }
  cellsWithExtraProperty.extra = true
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: cellsWithExtraProperty,
  }), null)
})

test('current layer viewer never invokes accessors and rejects throwing proxies', () => {
  const accessor = validValue() as Record<string, unknown>
  let getterReads = 0
  Object.defineProperty(accessor, 'revision', {
    enumerable: true,
    get() {
      getterReads += 1
      return 3
    },
  })
  assert.equal(normalizeCurrentLayerOrderView(accessor), null)
  assert.equal(getterReads, 0)

  let reflectionTraps = 0
  const proxy = new Proxy(validValue(), {
    ownKeys() {
      reflectionTraps += 1
      throw new Error('hostile reflection trap')
    },
  })
  assert.equal(normalizeCurrentLayerOrderView(proxy), null)
  assert.equal(reflectionTraps, 1)
})

test('current layer viewer rejects duplicate identities and native cap overflow', () => {
  const value = validValue()
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [value.cells[0], { ...value.cells[0] }],
  }), null)
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [{
      ...value.cells[0],
      bottomToTopFaces: [project, project],
    }],
  }), null)

  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: Array.from(
      { length: MAX_CURRENT_LAYER_ORDER_VIEW_CELLS + 1 },
      (_, index) => validCell(index),
    ),
  }), null)
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [{
      ...value.cells[0],
      bottomToTopFaces: Array.from(
        { length: MAX_CURRENT_LAYER_ORDER_VIEW_FACES + 1 },
        (_, index) => faceId(index),
      ),
    }],
  }), null)
})

test('current layer viewer enforces total vertex and four-MiB response budgets', () => {
  const value = validValue()
  const maximumBoundary = Array.from(
    { length: 4_096 },
    (_, index) => [index, 0, index] as [number, number, number],
  )
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [
      validCell(0, [project], maximumBoundary),
      validCell(1, [project], maximumBoundary),
      validCell(2, [project], maximumBoundary),
      validCell(3, [project], maximumBoundary),
      validCell(4),
    ],
  }), null)

  const maximumFaces = Array.from(
    { length: MAX_CURRENT_LAYER_ORDER_VIEW_FACES },
    (_, index) => faceId(index),
  )
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: Array.from(
      { length: 16 },
      (_, index) => validCell(index, maximumFaces),
    ),
  }), null)
})

test('current layer viewer bounds the per-cell SVG render product', () => {
  const value = validValue()
  const boundary = Array.from(
    { length: 4_096 },
    (_, index) => [index, 0, index] as [number, number, number],
  )
  const exactFaces = Array.from(
    {
      length: MAX_CURRENT_LAYER_ORDER_VIEW_RENDER_VERTEX_INSTANCES_PER_CELL
        / boundary.length,
    },
    (_, index) => faceId(index),
  )
  assert.ok(normalizeCurrentLayerOrderView({
    ...value,
    cells: [validCell(0, exactFaces, boundary)],
  }))
  assert.equal(normalizeCurrentLayerOrderView({
    ...value,
    cells: [validCell(0, [...exactFaces, faceId(exactFaces.length)], boundary)],
  }), null)
})

function validValue() {
  return {
    projectInstanceId: instance,
    projectId: project,
    revision: 3,
    layerOrderGeneration: 4,
    cells: [validCell(0, [project, instance])],
    readOnly: true,
  }
}

function validCell(
  index: number,
  faces: string[] = [project],
  boundary: [number, number, number][] = [
    [0, 0, 0],
    [1, 0, 0],
    [0, 0, -1],
  ],
) {
  return {
    cellKeySha256: index.toString(16).padStart(64, '0'),
    bottomToTopFaces: [...faces],
    boundaryWorld: boundary.map((point) => [...point] as [number, number, number]),
  }
}

function faceId(index: number) {
  return `00000000-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
}
