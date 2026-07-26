import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  createCanvasAnnotations,
  createCanvasFaces,
} from '../src/lib/projectCanvasProjection.ts'
import type {
  ProjectSnapshot,
  ProjectTopologyResponse,
} from '../src/lib/coreClient.ts'

const PROJECT_ID = '10000000-0000-4000-8000-000000000001'
const ANNOTATION_LAYER_ID = '20000000-0000-4000-8000-000000000001'
const HIDDEN_LAYER_ID = '20000000-0000-4000-8000-000000000002'
const VERTEX_IDS = [
  '30000000-0000-4000-8000-000000000001',
  '30000000-0000-4000-8000-000000000002',
  '30000000-0000-4000-8000-000000000003',
] as const
const FACE_ID = '40000000-0000-4000-8000-000000000001'

test('App delegates bound project presentation to the dedicated hook', () => {
  const app = readFileSync(
    new URL('../src/App.tsx', import.meta.url),
    'utf8',
  )
  assert.match(
    app,
    /useProjectCanvasProjection\(nativeSnapshot, topologyResponse\)/u,
  )
  assert.doesNotMatch(app, /const canvasFaces = useMemo/u)
  assert.doesNotMatch(app, /const canvasAnnotations = useMemo/u)
})

test('canvas faces retain exact topology binding, geometry, and saved color', () => {
  const result = createCanvasFaces(project(), topology())

  assert.deepEqual(result, [{
    id: FACE_ID,
    vertexIds: [...VERTEX_IDS],
    edgeIds: ['edge-1', 'edge-2', 'edge-3'],
    polygon: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
      { x: 0, y: 10 },
    ],
    color: 'rgba(17, 34, 51, 0.5019607843137255)',
  }])
  assert.equal(Object.isFrozen(result), true)
  assert.equal(Object.isFrozen(result[0]), true)
  assert.equal(Object.isFrozen(result[0].vertexIds), true)
  assert.equal(Object.isFrozen(result[0].edgeIds), true)
  assert.equal(Object.isFrozen(result[0].polygon), true)
})

test('canvas faces fail closed for stale authority and ambiguous geometry', () => {
  const snapshot = project()
  const staleResponses = [
    topology({ project_id: 'wrong-project' }),
    topology({ revision: 8 }),
    topology({}, 8),
    topology({ snapshot: null }),
  ]
  for (const stale of staleResponses) {
    assert.deepEqual(createCanvasFaces(snapshot, stale), [])
  }

  const duplicateVertex = project()
  duplicateVertex.crease_pattern.vertices.push({
    id: VERTEX_IDS[0],
    position: { x: 99, y: 99 },
  })
  assert.deepEqual(createCanvasFaces(duplicateVertex, topology()), [])

  assert.deepEqual(
    createCanvasFaces(snapshot, topology({}, 7, 2)),
    [],
  )
})

test('annotations keep anchor, layer visibility, opacity, and style filtering', () => {
  const snapshot = project()
  snapshot.annotations = {
    annotations: [
      annotation(
        '50000000-0000-4000-8000-000000000001',
        ANNOTATION_LAYER_ID,
        { kind: 'absolute', position: { x: 4, y: 5 } },
      ),
      annotation(
        '50000000-0000-4000-8000-000000000002',
        ANNOTATION_LAYER_ID,
        {
          kind: 'vertex',
          vertex: VERTEX_IDS[1],
          offset: { x: 2, y: 3 },
        },
      ),
      annotation(
        '50000000-0000-4000-8000-000000000003',
        HIDDEN_LAYER_ID,
        { kind: 'absolute', position: { x: 8, y: 9 } },
      ),
      annotation(
        '50000000-0000-4000-8000-000000000004',
        ANNOTATION_LAYER_ID,
        {
          kind: 'vertex',
          vertex: 'missing-vertex',
          offset: { x: 0, y: 0 },
        },
      ),
    ],
  }

  assert.deepEqual(createCanvasAnnotations(snapshot), [
    {
      id: '50000000-0000-4000-8000-000000000001',
      text: 'Label',
      x: 4,
      y: 5,
      color: 'rgba(10, 20, 30, 0.5019607843137255)',
      opacity: 0.4,
      fontSizeMm: 6,
      bold: true,
      italic: false,
    },
    {
      id: '50000000-0000-4000-8000-000000000002',
      text: 'Label',
      x: 12,
      y: 3,
      color: 'rgba(10, 20, 30, 0.5019607843137255)',
      opacity: 0.4,
      fontSizeMm: 6,
      bold: true,
      italic: false,
    },
  ])
  assert.deepEqual(createCanvasAnnotations(null), [])
})

function project(): ProjectSnapshot {
  return {
    project_id: PROJECT_ID,
    revision: 7,
    crease_pattern: {
      vertices: [
        { id: VERTEX_IDS[0], position: { x: 0, y: 0 } },
        { id: VERTEX_IDS[1], position: { x: 10, y: 0 } },
        { id: VERTEX_IDS[2], position: { x: 0, y: 10 } },
      ],
      edges: [],
    },
    element_metadata: {
      schema_version: 1,
      vertices: [],
      edges: [],
      faces: [{
        face: FACE_ID,
        metadata: {
          color: { red: 17, green: 34, blue: 51, alpha: 128 },
        },
      }],
    },
    project_layers: {
      schema_version: 1,
      layers: [
        {
          id: ANNOTATION_LAYER_ID,
          name: 'Annotations',
          content_kind: 'annotation',
          visible: true,
          locked: false,
          opacity: 0.4,
        },
        {
          id: HIDDEN_LAYER_ID,
          name: 'Hidden',
          content_kind: 'annotation',
          visible: false,
          locked: false,
          opacity: 1,
        },
      ],
      edge_assignments: [],
    },
  } as ProjectSnapshot
}

function topology(
  override: Partial<ProjectTopologyResponse> = {},
  sourceRevision = 7,
  halfEdgeCount = 3,
): ProjectTopologyResponse {
  const halfEdges = VERTEX_IDS.slice(0, halfEdgeCount).map(
    (origin, index) => ({ origin, edge: `edge-${index + 1}` }),
  )
  return {
    project_id: PROJECT_ID,
    revision: 7,
    snapshot: {
      source_revision: sourceRevision,
      faces: [{
        id: FACE_ID,
        outer: { half_edges: halfEdges },
      }],
    },
    ...override,
  } as ProjectTopologyResponse
}

function annotation(
  id: string,
  layer: string,
  anchor: NonNullable<
    ProjectSnapshot['annotations']
  >['annotations'][number]['anchor'],
) {
  return {
    id,
    text: 'Label',
    anchor,
    style: {
      color: { red: 10, green: 20, blue: 30, alpha: 128 },
      font_size_mm: 6,
      bold: true,
      italic: false,
    },
    layer,
  }
}
