import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createProjectLayerCanvasView,
  placementTouchesLockedLayer,
} from '../src/lib/projectLayerCanvasView.ts'
import {
  DEFAULT_PROJECT_LAYER_ID,
  type ProjectLayerDocumentV1,
} from '../src/lib/projectLayers.ts'

const VISIBLE_LAYER_ID = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
const HIDDEN_LAYER_ID = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
const TRANSPARENT_LAYER_ID = 'cccccccc-cccc-4ccc-8ccc-cccccccccccc'
const LOCKED_LAYER_ID = 'dddddddd-dddd-4ddd-8ddd-dddddddddddd'

const layers: ProjectLayerDocumentV1['layers'] = [
  {
    id: DEFAULT_PROJECT_LAYER_ID,
    name: 'Crease Pattern',
    content_kind: 'crease_pattern',
    visible: true,
    locked: false,
    opacity: 1,
  },
  {
    id: VISIBLE_LAYER_ID,
    name: 'Visible',
    content_kind: 'crease_pattern',
    visible: true,
    locked: false,
    opacity: 0.4,
  },
  {
    id: HIDDEN_LAYER_ID,
    name: 'Hidden',
    content_kind: 'crease_pattern',
    visible: false,
    locked: false,
    opacity: 1,
  },
  {
    id: TRANSPARENT_LAYER_ID,
    name: 'Transparent',
    content_kind: 'crease_pattern',
    visible: true,
    locked: false,
    opacity: 0,
  },
  {
    id: LOCKED_LAYER_ID,
    name: 'Locked',
    content_kind: 'crease_pattern',
    visible: true,
    locked: true,
    opacity: 0.75,
  },
]

const vertices = [
  { id: 'shared', position: { x: 0, y: 0 } },
  { id: 'visible-only', position: { x: 10, y: 0 } },
  { id: 'hidden-only', position: { x: 0, y: 10 } },
  { id: 'transparent-a', position: { x: 20, y: 0 } },
  { id: 'transparent-b', position: { x: 20, y: 10 } },
  { id: 'locked-only', position: { x: 10, y: 10 } },
  { id: 'isolated', position: { x: 30, y: 30 } },
]

const edges = [
  {
    id: 'visible-edge',
    start: 'shared',
    end: 'visible-only',
    kind: 'mountain',
  },
  {
    id: 'hidden-edge',
    start: 'shared',
    end: 'hidden-only',
    kind: 'valley',
  },
  {
    id: 'transparent-edge',
    start: 'transparent-a',
    end: 'transparent-b',
    kind: 'auxiliary',
  },
  {
    id: 'locked-edge',
    start: 'visible-only',
    end: 'locked-only',
    kind: 'cut',
  },
]

function documentWith(
  edgeAssignments: ProjectLayerDocumentV1['edge_assignments'],
  layerRecords = layers,
): ProjectLayerDocumentV1 {
  return {
    schema_version: 1,
    layers: layerRecords,
    edge_assignments: edgeAssignments,
  }
}

const document = documentWith([
  { edge: 'visible-edge', layer: VISIBLE_LAYER_ID },
  { edge: 'hidden-edge', layer: HIDDEN_LAYER_ID },
  { edge: 'transparent-edge', layer: TRANSPARENT_LAYER_ID },
  { edge: 'locked-edge', layer: LOCKED_LAYER_ID },
])

test('hidden and zero-opacity layer geometry is excluded from all canvas interaction sources', () => {
  const view = createProjectLayerCanvasView(document, { vertices, edges })

  assert.deepEqual(
    view.lines.map(({ id }) => id),
    ['visible-edge', 'locked-edge'],
    'the single line source used by drawing, hit-testing, snapping, and intersections must exclude hidden geometry',
  )
  assert.deepEqual(
    view.vertices.map(({ id }) => id),
    ['shared', 'visible-only', 'locked-only', 'isolated'],
  )
  assert.ok(
    view.vertices.some(({ id }) => id === 'shared'),
    'a vertex shared by hidden and visible edges remains visible',
  )
  assert.ok(
    !view.vertices.some(({ id }) => id === 'hidden-only'),
    'a vertex incident only to hidden edges is hidden',
  )
})

test('visible layer order and opacity reach canvas draw records', () => {
  const view = createProjectLayerCanvasView(document, { vertices, edges })

  assert.deepEqual(
    view.lines.map(({ id, layerOrder, opacity }) => ({
      id,
      layerOrder,
      opacity,
    })),
    [
      { id: 'visible-edge', layerOrder: 1, opacity: 0.4 },
      { id: 'locked-edge', layerOrder: 4, opacity: 0.75 },
    ],
  )
})

test('locked visible geometry remains selectable but every placement mutation touching it is blocked', () => {
  const view = createProjectLayerCanvasView(document, { vertices, edges })

  assert.ok(
    view.lines.some(({ id, locked }) => id === 'locked-edge' && locked),
    'locked visible lines remain in the canvas source for selection and reference',
  )
  assert.ok(view.lockedVertexIds.has('visible-only'))
  assert.ok(view.lockedVertexIds.has('locked-only'))
  assert.equal(placementTouchesLockedLayer({
    operation: 'split-edge',
    edgeId: 'locked-edge',
    fraction: 0.5,
  }, view), true)
  assert.equal(placementTouchesLockedLayer({
    operation: 'connect-intersection',
    firstEdgeId: 'visible-edge',
    secondEdgeId: 'locked-edge',
  }, view), true)
  assert.equal(placementTouchesLockedLayer({
    operation: 'connect-t-junction',
    firstEdgeId: 'locked-edge',
    secondEdgeId: 'visible-edge',
    junctionVertexId: 'visible-only',
  }, view), true)
  assert.equal(placementTouchesLockedLayer({
    operation: 'connect-intersection-cluster',
    targets: [
      { edgeId: 'visible-edge', relation: 'interior' },
      { edgeId: 'locked-edge', relation: 'endpoint' },
    ],
  }, view), true)
  assert.equal(placementTouchesLockedLayer({
    operation: 'split-edge',
    edgeId: 'visible-edge',
    fraction: 0.5,
  }, view), false)
})

test('default-layer lock blocks additions and unknown mutation targets fail closed', () => {
  const lockedDefaultLayers = layers.map((layer) =>
    layer.id === DEFAULT_PROJECT_LAYER_ID
      ? { ...layer, locked: true }
      : layer)
  const view = createProjectLayerCanvasView(
    documentWith(document.edge_assignments, lockedDefaultLayers),
    { vertices, edges },
  )

  assert.equal(placementTouchesLockedLayer({
    operation: 'add',
    x: 5,
    y: 5,
  }, view), true)
  assert.equal(placementTouchesLockedLayer({
    operation: 'split-edge',
    edgeId: 'unknown-edge',
    fraction: 0.5,
  }, view), true)
  assert.ok(view.lockedVertexIds.has('isolated'))
})
