import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_FOLD_PREVIEW_WORLD_SIZE,
  buildFoldPreviewModel,
} from '../src/lib/foldPreviewModel.ts'
import type {
  ProjectSnapshot,
  ProjectTopologyResponse,
  TopologyFace,
} from '../src/lib/coreClient.ts'

const ids = {
  project: id(1),
  a: id(0x101),
  b: id(0x102),
  c: id(0x103),
  d: id(0x104),
  e: id(0x105),
  f: id(0x106),
  auxiliaryVertex: id(0x107),
  missingVertex: id(0x108),
  west: id(0x401),
  east: id(0x402),
  whole: id(0x403),
  ab: id(0x201),
  bc: id(0x202),
  cd: id(0x203),
  de: id(0x204),
  ef: id(0x205),
  fa: id(0x206),
  fold: id(0x301),
  auxiliary: id(0x302),
  malformedAuxiliary: id(0x303),
} as const

const boundaryEdgeIds = [ids.ab, ids.bc, ids.cd, ids.de, ids.ef, ids.fa] as const

function id(suffix: number) {
  return `00000000-0000-0000-0000-${suffix.toString(16).padStart(12, '0')}`
}

function projectFixture(): ProjectSnapshot {
  const vertexIds = [ids.a, ids.b, ids.c, ids.d, ids.e, ids.f]
  const positions = [
    { x: 0, y: 0 },
    { x: 2, y: 0 },
    { x: 4, y: 0 },
    { x: 4, y: 4 },
    { x: 2, y: 4 },
    { x: 0, y: 4 },
  ]
  return {
    project_id: ids.project,
    name: 'single fold',
    current_path: null,
    revision: 7,
    saved_revision: null,
    is_dirty: true,
    crease_pattern: {
      vertices: vertexIds.map((vertexId, index) => ({
        id: vertexId,
        position: positions[index],
      })),
      edges: [
        ...boundaryEdgeIds.map((edgeId, index) => ({
          id: edgeId,
          start: vertexIds[index],
          end: vertexIds[(index + 1) % vertexIds.length],
          kind: 'boundary',
        })),
        // Deliberately opposite to the canonical B -> E topology direction.
        { id: ids.fold, start: ids.e, end: ids.b, kind: 'mountain' },
      ],
    },
    paper: {
      boundary_vertices: vertexIds,
      thickness_mm: 0.1,
      cutting_allowed: false,
      front: { color: { red: 255, green: 255, blue: 255, alpha: 255 }, texture_asset: null },
      back: { color: { red: 248, green: 248, blue: 245, alpha: 255 }, texture_asset: null },
    },
    can_undo: true,
    can_redo: false,
    cutting_allowed: false,
  }
}

function topologyFixture(): ProjectTopologyResponse {
  const west: TopologyFace = {
    id: ids.west,
    key: [1, ...Array<number>(31).fill(0)],
    outer: {
      signed_double_area: 16,
      half_edges: [
        { edge: ids.ab, origin: ids.a, destination: ids.b },
        { edge: ids.fold, origin: ids.b, destination: ids.e },
        { edge: ids.ef, origin: ids.e, destination: ids.f },
        { edge: ids.fa, origin: ids.f, destination: ids.a },
      ],
    },
    area: 8,
  }
  const east: TopologyFace = {
    id: ids.east,
    key: [2, ...Array<number>(31).fill(0)],
    outer: {
      signed_double_area: 16,
      half_edges: [
        { edge: ids.bc, origin: ids.b, destination: ids.c },
        { edge: ids.cd, origin: ids.c, destination: ids.d },
        { edge: ids.de, origin: ids.d, destination: ids.e },
        { edge: ids.fold, origin: ids.e, destination: ids.b },
      ],
    },
    area: 8,
  }
  return {
    project_id: ids.project,
    revision: 7,
    simulation_ready: true,
    snapshot: {
      source_revision: 7,
      faces: [west, east],
      edge_incidence: [
        [ids.ab, { kind: 'boundary', material: ids.west }],
        [ids.bc, { kind: 'boundary', material: ids.east }],
        [ids.cd, { kind: 'boundary', material: ids.east }],
        [ids.de, { kind: 'boundary', material: ids.east }],
        [ids.ef, { kind: 'boundary', material: ids.west }],
        [ids.fa, { kind: 'boundary', material: ids.west }],
        [ids.fold, {
          kind: 'hinge',
          left: ids.west,
          right: ids.east,
          assignment: 'mountain',
        }],
      ],
      hinge_adjacency: [{
        edge: ids.fold,
        first: ids.west,
        second: ids.east,
        assignment: 'mountain',
      }],
    },
    issues: [],
  }
}

function flatFixtures(): [ProjectSnapshot, ProjectTopologyResponse] {
  const project = projectFixture()
  project.crease_pattern.edges.pop()
  const face: TopologyFace = {
    id: ids.whole,
    key: Array<number>(32).fill(3),
    outer: {
      signed_double_area: 32,
      half_edges: [
        { edge: ids.ab, origin: ids.a, destination: ids.b },
        { edge: ids.bc, origin: ids.b, destination: ids.c },
        { edge: ids.cd, origin: ids.c, destination: ids.d },
        { edge: ids.de, origin: ids.d, destination: ids.e },
        { edge: ids.ef, origin: ids.e, destination: ids.f },
        { edge: ids.fa, origin: ids.f, destination: ids.a },
      ],
    },
    area: 16,
  }
  return [project, {
    project_id: ids.project,
    revision: 7,
    simulation_ready: true,
    snapshot: {
      source_revision: 7,
      faces: [face],
      edge_incidence: boundaryEdgeIds.map((edgeId) => [
        edgeId,
        { kind: 'boundary', material: ids.whole },
      ]),
      hinge_adjacency: [],
    },
    issues: [],
  }]
}

function extremeTranslatedFlatFixtures(): [ProjectSnapshot, ProjectTopologyResponse] {
  const [project, topology] = flatFixtures()
  const minX = 1e308
  const maxX = 1.0000000000000002e308
  const width = maxX - minX
  project.paper.boundary_vertices = [ids.a, ids.c, ids.d, ids.f]
  project.crease_pattern.vertices = [
    { id: ids.a, position: { x: minX, y: 0 } },
    { id: ids.c, position: { x: maxX, y: 0 } },
    { id: ids.d, position: { x: maxX, y: 4 } },
    { id: ids.f, position: { x: minX, y: 4 } },
  ]
  project.crease_pattern.edges = [
    { id: ids.ab, start: ids.a, end: ids.c, kind: 'boundary' },
    { id: ids.cd, start: ids.c, end: ids.d, kind: 'boundary' },
    { id: ids.de, start: ids.d, end: ids.f, kind: 'boundary' },
    { id: ids.fa, start: ids.f, end: ids.a, kind: 'boundary' },
  ]
  assert.ok(topology.snapshot)
  topology.snapshot.faces = [{
    id: ids.whole,
    key: Array<number>(32).fill(4),
    outer: {
      signed_double_area: width * 8,
      half_edges: [
        { edge: ids.ab, origin: ids.a, destination: ids.c },
        { edge: ids.cd, origin: ids.c, destination: ids.d },
        { edge: ids.de, origin: ids.d, destination: ids.f },
        { edge: ids.fa, origin: ids.f, destination: ids.a },
      ],
    },
    area: width * 4,
  }]
  topology.snapshot.edge_incidence = [
    [ids.ab, { kind: 'boundary', material: ids.whole }],
    [ids.cd, { kind: 'boundary', material: ids.whole }],
    [ids.de, { kind: 'boundary', material: ids.whole }],
    [ids.fa, { kind: 'boundary', material: ids.whole }],
  ]
  return [project, topology]
}

function clone<T>(value: T): T {
  return structuredClone(value)
}

test('single fold becomes centered world-XZ polygons around the canonical hinge', () => {
  const project = projectFixture()
  const topology = topologyFixture()

  const model = buildFoldPreviewModel(project, topology)

  assert.ok(model)
  assert.equal(model.kind, 'single_fold')
  assert.equal(model.projectId, ids.project)
  assert.equal(model.revision, 7)
  assert.equal(model.worldUnitsPerMillimetre, 1.1)
  assert.equal(MAX_FOLD_PREVIEW_WORLD_SIZE, 4.4)
  assert.deepEqual(model.paperCenter, { x: 2, y: 2 })
  assert.deepEqual(model.worldBounds, { minX: -2.2, minZ: -2.2, maxX: 2.2, maxZ: 2.2 })
  assert.equal(model.fixedFace.id, ids.west)
  assert.equal(model.movingFace.id, ids.east)
  assert.deepEqual(model.fixedFace.polygon, [
    { vertexId: ids.a, x: -2.2, z: 2.2 },
    { vertexId: ids.b, x: 0, z: 2.2 },
    { vertexId: ids.e, x: 0, z: -2.2 },
    { vertexId: ids.f, x: -2.2, z: -2.2 },
  ])
  assert.deepEqual(model.movingFace.polygon, [
    { vertexId: ids.b, x: 0, z: 2.2 },
    { vertexId: ids.c, x: 2.2, z: 2.2 },
    { vertexId: ids.d, x: 2.2, z: -2.2 },
    { vertexId: ids.e, x: 0, z: -2.2 },
  ])
  assert.deepEqual(model.hinge, {
    edgeId: ids.fold,
    start: { vertexId: ids.b, x: 0, z: 2.2 },
    end: { vertexId: ids.e, x: 0, z: -2.2 },
    axis: { x: 0, z: -1 },
    assignment: 'mountain',
    rotationSign: 1,
  })
})

test('face and source record order do not replace geometric left/right semantics', () => {
  const project = projectFixture()
  const topology = topologyFixture()
  const expected = buildFoldPreviewModel(project, topology)
  project.crease_pattern.vertices.reverse()
  project.crease_pattern.edges.reverse()
  project.paper.boundary_vertices.push(...project.paper.boundary_vertices.splice(0, 2))
  topology.snapshot?.faces.reverse()
  topology.snapshot?.edge_incidence.reverse()

  const reordered = buildFoldPreviewModel(project, topology)

  assert.deepEqual(reordered, expected)
})

test('valley assignment keeps geometry but reverses the hinge rotation sign', () => {
  const mountainProject = projectFixture()
  const mountainTopology = topologyFixture()
  const valleyProject = clone(mountainProject)
  const valleyTopology = clone(mountainTopology)
  const sourceFold = valleyProject.crease_pattern.edges.find((edge) => edge.id === ids.fold)
  assert.ok(sourceFold)
  sourceFold.kind = 'valley'
  const hinge = valleyTopology.snapshot?.edge_incidence.find(([edge]) => edge === ids.fold)?.[1]
  assert.ok(hinge && hinge.kind === 'hinge')
  hinge.assignment = 'valley'
  const adjacency = valleyTopology.snapshot?.hinge_adjacency[0]
  assert.ok(adjacency)
  adjacency.assignment = 'valley'

  const mountain = buildFoldPreviewModel(mountainProject, mountainTopology)
  const valley = buildFoldPreviewModel(valleyProject, valleyTopology)

  assert.ok(mountain?.kind === 'single_fold')
  assert.ok(valley?.kind === 'single_fold')
  assert.deepEqual(valley.faces, mountain.faces)
  assert.deepEqual(valley.hinge.start, mountain.hinge.start)
  assert.deepEqual(valley.hinge.end, mountain.hinge.end)
  assert.deepEqual(valley.hinge.axis, mountain.hinge.axis)
  assert.equal(mountain.hinge.rotationSign, 1)
  assert.equal(valley.hinge.rotationSign, -1)
})

test('boundary-only topology produces one fixed planar face', () => {
  const [project, topology] = flatFixtures()

  const model = buildFoldPreviewModel(project, topology)

  assert.ok(model)
  assert.equal(model.kind, 'planar')
  assert.equal(model.faces.length, 1)
  assert.equal(model.fixedFace.id, ids.whole)
  assert.equal(model.movingFace, null)
  assert.equal(model.hinge, null)
  assert.deepEqual(
    model.fixedFace.polygon.map(({ vertexId }) => vertexId),
    [ids.a, ids.b, ids.c, ids.d, ids.e, ids.f],
  )
})

test('minimum-relative scaling centers adjacent floats translated near 1e308', () => {
  const [project, topology] = extremeTranslatedFlatFixtures()

  const model = buildFoldPreviewModel(project, topology)

  assert.ok(model?.kind === 'planar')
  assert.deepEqual(
    model.fixedFace.polygon.map(({ x }) => x),
    [-2.2, 2.2, 2.2, -2.2],
  )
  assert.equal(model.worldBounds.minX, -2.2)
  assert.equal(model.worldBounds.maxX, 2.2)
  assert.equal(model.worldBounds.minZ, -model.worldBounds.maxZ)
  assert.ok(model.fixedFace.polygon.every(({ x, z }) => Number.isFinite(x) && Number.isFinite(z)))
})

test('a well-formed ignored auxiliary edge does not alter the material model', () => {
  const project = projectFixture()
  const topology = topologyFixture()
  const expected = buildFoldPreviewModel(project, topology)
  project.crease_pattern.edges.push({
    id: ids.auxiliary,
    start: ids.a,
    end: ids.d,
    kind: 'auxiliary',
  })
  topology.snapshot?.edge_incidence.push([
    ids.auxiliary,
    { kind: 'auxiliary_ignored' },
  ])

  assert.deepEqual(buildFoldPreviewModel(project, topology), expected)
})

test('malformed auxiliary-only geometry is ignored while global duplicate IDs remain fatal', () => {
  const project = projectFixture()
  const topology = topologyFixture()
  const expected = buildFoldPreviewModel(project, topology)
  project.crease_pattern.vertices.push({
    id: ids.auxiliaryVertex,
    position: { x: Number.NaN, y: Number.POSITIVE_INFINITY },
  })
  project.crease_pattern.edges.push({
    id: ids.auxiliary,
    start: ids.a,
    end: ids.d,
    kind: 'auxiliary',
  }, {
    id: ids.malformedAuxiliary,
    start: ids.auxiliaryVertex,
    end: ids.missingVertex,
    kind: 'auxiliary',
  })
  topology.snapshot?.edge_incidence.push(
    [ids.auxiliary, { kind: 'auxiliary_ignored' }],
    [ids.malformedAuxiliary, { kind: 'auxiliary_ignored' }],
  )

  assert.deepEqual(buildFoldPreviewModel(project, topology), expected)

  const duplicateVertex = clone(project)
  duplicateVertex.crease_pattern.vertices.push({
    id: ids.a,
    position: { x: Number.NaN, y: Number.NaN },
  })
  assert.equal(buildFoldPreviewModel(duplicateVertex, topology), null)

  const duplicateEdge = clone(project)
  duplicateEdge.crease_pattern.edges.push({
    id: ids.fold,
    start: ids.missingVertex,
    end: ids.missingVertex,
    kind: 'auxiliary',
  })
  assert.equal(buildFoldPreviewModel(duplicateEdge, topology), null)
})

test('project, response, and source revisions must all identify the same immutable input', () => {
  const cases: Array<[string, (project: ProjectSnapshot, topology: ProjectTopologyResponse) => void]> = [
    ['project identity', (_project, topology) => { topology.project_id = id(99) }],
    ['response revision', (_project, topology) => { topology.revision += 1 }],
    ['source revision', (_project, topology) => {
      if (topology.snapshot) topology.snapshot.source_revision += 1
    }],
    ['unsafe integer revision', (project) => { project.revision = Number.MAX_SAFE_INTEGER + 1 }],
  ]
  for (const [label, mutate] of cases) {
    const project = projectFixture()
    const topology = topologyFixture()
    mutate(project, topology)
    assert.equal(buildFoldPreviewModel(project, topology), null, label)
  }
})

test('simulation-ready warnings are accepted but blocking, fatal, and malformed issues fail closed', () => {
  const warned = topologyFixture()
  warned.issues.push({
    severity: 'warning',
    kind: { kind: 'internal_boundary_resolution' },
  })
  assert.ok(buildFoldPreviewModel(projectFixture(), warned))

  for (const severity of ['blocks_simulation', 'fatal'] as const) {
    const topology = topologyFixture()
    topology.issues.push({ severity, kind: { kind: 'internal_boundary_resolution' } })
    assert.equal(buildFoldPreviewModel(projectFixture(), topology), null)
  }

  const malformed = topologyFixture()
  ;(malformed.issues as unknown[]).push({ severity: 'unexpected', kind: {} })
  assert.equal(buildFoldPreviewModel(projectFixture(), malformed), null)

  const notReady = topologyFixture()
  notReady.simulation_ready = false
  assert.equal(buildFoldPreviewModel(projectFixture(), notReady), null)
})

test('malformed project and topology records are rejected without throwing', () => {
  const corruptions: Array<[
    string,
    (project: ProjectSnapshot, topology: ProjectTopologyResponse) => void,
  ]> = [
    ['non-finite vertex', (project) => {
      project.crease_pattern.vertices[0].position.x = Number.NaN
    }],
    ['missing edge endpoint', (project) => {
      project.crease_pattern.edges[0].end = id(999)
    }],
    ['duplicate vertex position', (project) => {
      project.crease_pattern.vertices[1].position = { x: 0, y: 0 }
    }],
    ['broken boundary walk', (_project, topology) => {
      const face = topology.snapshot?.faces[0]
      if (face) face.outer.half_edges[0].destination = ids.e
    }],
    ['short face key', (_project, topology) => {
      const face = topology.snapshot?.faces[0]
      if (face) face.key.pop()
    }],
    ['inconsistent reported face area', (_project, topology) => {
      const face = topology.snapshot?.faces[0]
      if (face) face.area += 1
    }],
    ['missing incidence', (_project, topology) => {
      topology.snapshot?.edge_incidence.pop()
    }],
    ['duplicate incidence', (_project, topology) => {
      const entry = topology.snapshot?.edge_incidence[0]
      if (entry) topology.snapshot?.edge_incidence.push(clone(entry))
    }],
    ['null snapshot without diagnostic', (_project, topology) => { topology.snapshot = null }],
  ]
  for (const [label, corrupt] of corruptions) {
    const project = projectFixture()
    const topology = topologyFixture()
    corrupt(project, topology)
    assert.doesNotThrow(() => buildFoldPreviewModel(project, topology), label)
    assert.equal(buildFoldPreviewModel(project, topology), null, label)
  }
})

test('boundary half-edge IDs must resolve the same undirected source endpoints', () => {
  const topology = topologyFixture()
  const west = topology.snapshot?.faces.find((face) => face.id === ids.west)
  assert.ok(west)
  ;[west.outer.half_edges[0].edge, west.outer.half_edges[2].edge] = [
    west.outer.half_edges[2].edge,
    west.outer.half_edges[0].edge,
  ]

  assert.equal(buildFoldPreviewModel(projectFixture(), topology), null)
})

test('all referenced entity IDs must use canonical lowercase hyphenated UUID text', () => {
  const nonCanonicalProjectIds = [
    id(0xabc).toUpperCase(),
    `${ids.project} `,
    'project',
  ]
  for (const projectId of nonCanonicalProjectIds) {
    const project = projectFixture()
    const topology = topologyFixture()
    project.project_id = projectId
    topology.project_id = projectId
    assert.equal(buildFoldPreviewModel(project, topology), null, projectId)
  }

  const arbitraryFace = JSON.parse(
    JSON.stringify(topologyFixture()).replaceAll(ids.west, 'west-face'),
  ) as ProjectTopologyResponse
  assert.equal(buildFoldPreviewModel(projectFixture(), arbitraryFace), null)
})

test('hinge direction and adjacency contracts are checked independently of source direction', () => {
  const validProject = projectFixture()
  const validTopology = topologyFixture()
  assert.ok(buildFoldPreviewModel(validProject, validTopology))

  const swappedSides = topologyFixture()
  const hinge = swappedSides.snapshot?.edge_incidence.find(([edge]) => edge === ids.fold)?.[1]
  assert.ok(hinge && hinge.kind === 'hinge')
  ;[hinge.left, hinge.right] = [hinge.right, hinge.left]
  assert.equal(buildFoldPreviewModel(projectFixture(), swappedSides), null)

  const reverseAdjacency = topologyFixture()
  const adjacency = reverseAdjacency.snapshot?.hinge_adjacency[0]
  assert.ok(adjacency)
  ;[adjacency.first, adjacency.second] = [adjacency.second, adjacency.first]
  assert.equal(buildFoldPreviewModel(projectFixture(), reverseAdjacency), null)

  const wrongSourcePair = projectFixture()
  const sourceFold = wrongSourcePair.crease_pattern.edges.find((edge) => edge.id === ids.fold)
  assert.ok(sourceFold)
  sourceFold.start = ids.a
  assert.equal(buildFoldPreviewModel(wrongSourcePair, topologyFixture()), null)
})
