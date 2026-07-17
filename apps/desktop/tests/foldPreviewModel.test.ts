import assert from 'node:assert/strict'
import test from 'node:test'
import { Vector3 } from 'three'

import { calculateFoldTreePose } from '../src/lib/foldPreviewKinematics.ts'
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
  center: id(0x109),
  west: id(0x401),
  east: id(0x402),
  whole: id(0x403),
  south: id(0x404),
  north: id(0x405),
  ab: id(0x201),
  bc: id(0x202),
  cd: id(0x203),
  de: id(0x204),
  ef: id(0x205),
  fa: id(0x206),
  fold: id(0x301),
  auxiliary: id(0x302),
  malformedAuxiliary: id(0x303),
  foldAg: id(0x304),
  foldDg: id(0x305),
  foldCg: id(0x306),
  foldFg: id(0x307),
  extraBoundaryOne: id(0x207),
  extraBoundaryTwo: id(0x208),
  parallelFoldOne: id(0x308),
  parallelFoldTwo: id(0x309),
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

function foldGraphFixtures(): [ProjectSnapshot, ProjectTopologyResponse] {
  const project = projectFixture()
  project.name = 'four-face fold graph'
  project.crease_pattern.vertices.push({
    id: ids.center,
    position: { x: 2, y: 2 },
  })
  project.crease_pattern.edges.pop()
  project.crease_pattern.edges.push(
    { id: ids.foldAg, start: ids.center, end: ids.a, kind: 'mountain' },
    { id: ids.foldDg, start: ids.center, end: ids.d, kind: 'valley' },
    { id: ids.foldCg, start: ids.center, end: ids.c, kind: 'mountain' },
    { id: ids.foldFg, start: ids.center, end: ids.f, kind: 'valley' },
  )

  const face = (
    faceId: string,
    keyByte: number,
    halfEdges: TopologyFace['outer']['half_edges'],
  ): TopologyFace => ({
    id: faceId,
    key: [keyByte, ...Array<number>(31).fill(0)],
    outer: { signed_double_area: 8, half_edges: halfEdges },
    area: 4,
  })
  const west = face(ids.west, 1, [
    { edge: ids.fa, origin: ids.f, destination: ids.a },
    { edge: ids.foldAg, origin: ids.a, destination: ids.center },
    { edge: ids.foldFg, origin: ids.center, destination: ids.f },
  ])
  const south = face(ids.south, 2, [
    { edge: ids.ab, origin: ids.a, destination: ids.b },
    { edge: ids.bc, origin: ids.b, destination: ids.c },
    { edge: ids.foldCg, origin: ids.c, destination: ids.center },
    { edge: ids.foldAg, origin: ids.center, destination: ids.a },
  ])
  const east = face(ids.east, 3, [
    { edge: ids.cd, origin: ids.c, destination: ids.d },
    { edge: ids.foldDg, origin: ids.d, destination: ids.center },
    { edge: ids.foldCg, origin: ids.center, destination: ids.c },
  ])
  const north = face(ids.north, 4, [
    { edge: ids.de, origin: ids.d, destination: ids.e },
    { edge: ids.ef, origin: ids.e, destination: ids.f },
    { edge: ids.foldFg, origin: ids.f, destination: ids.center },
    { edge: ids.foldDg, origin: ids.center, destination: ids.d },
  ])

  return [project, {
    project_id: ids.project,
    revision: 7,
    simulation_ready: true,
    snapshot: {
      source_revision: 7,
      faces: [west, south, east, north],
      edge_incidence: [
        [ids.fa, { kind: 'boundary', material: ids.west }],
        [ids.ef, { kind: 'boundary', material: ids.north }],
        [ids.de, { kind: 'boundary', material: ids.north }],
        [ids.cd, { kind: 'boundary', material: ids.east }],
        [ids.bc, { kind: 'boundary', material: ids.south }],
        [ids.ab, { kind: 'boundary', material: ids.south }],
        [ids.foldDg, {
          kind: 'hinge',
          left: ids.east,
          right: ids.north,
          assignment: 'valley',
        }],
        [ids.foldFg, {
          kind: 'hinge',
          left: ids.north,
          right: ids.west,
          assignment: 'valley',
        }],
        [ids.foldAg, {
          kind: 'hinge',
          left: ids.west,
          right: ids.south,
          assignment: 'mountain',
        }],
        [ids.foldCg, {
          kind: 'hinge',
          left: ids.south,
          right: ids.east,
          assignment: 'mountain',
        }],
      ],
      hinge_adjacency: [
        {
          edge: ids.foldAg,
          first: ids.west,
          second: ids.south,
          assignment: 'mountain',
        },
        {
          edge: ids.foldFg,
          first: ids.west,
          second: ids.north,
          assignment: 'valley',
        },
        {
          edge: ids.foldCg,
          first: ids.south,
          second: ids.east,
          assignment: 'mountain',
        },
        {
          edge: ids.foldDg,
          first: ids.east,
          second: ids.north,
          assignment: 'valley',
        },
      ],
    },
    issues: [],
  }]
}

function foldTreeFixtures(): [ProjectSnapshot, ProjectTopologyResponse] {
  const vertexIds = [
    ids.a,
    ids.b,
    ids.c,
    ids.d,
    ids.e,
    ids.f,
    ids.auxiliaryVertex,
    ids.missingVertex,
  ]
  const boundaryIds = [
    ids.ab,
    ids.bc,
    ids.cd,
    ids.de,
    ids.ef,
    ids.fa,
    ids.extraBoundaryOne,
    ids.extraBoundaryTwo,
  ]
  const positions = [
    { x: 0, y: 0 },
    { x: 1, y: 0 },
    { x: 3, y: 0 },
    { x: 4, y: 0 },
    { x: 4, y: 4 },
    { x: 3, y: 4 },
    { x: 1, y: 4 },
    { x: 0, y: 4 },
  ]
  const project: ProjectSnapshot = {
    ...projectFixture(),
    name: 'three-face fold tree',
    crease_pattern: {
      vertices: vertexIds.map((vertexId, index) => ({
        id: vertexId,
        position: positions[index],
      })),
      edges: [
        ...boundaryIds.map((edgeId, index) => ({
          id: edgeId,
          start: vertexIds[index],
          end: vertexIds[(index + 1) % vertexIds.length],
          kind: 'boundary' as const,
        })),
        {
          id: ids.parallelFoldOne,
          start: ids.auxiliaryVertex,
          end: ids.b,
          kind: 'mountain',
        },
        {
          id: ids.parallelFoldTwo,
          start: ids.f,
          end: ids.c,
          kind: 'valley',
        },
      ],
    },
    paper: {
      ...projectFixture().paper,
      boundary_vertices: vertexIds,
    },
  }

  const face = (
    faceId: string,
    keyByte: number,
    signedDoubleArea: number,
    halfEdges: TopologyFace['outer']['half_edges'],
  ): TopologyFace => ({
    id: faceId,
    key: [keyByte, ...Array<number>(31).fill(0)],
    outer: { signed_double_area: signedDoubleArea, half_edges: halfEdges },
    area: signedDoubleArea / 2,
  })
  const west = face(ids.west, 1, 8, [
    { edge: ids.ab, origin: ids.a, destination: ids.b },
    { edge: ids.parallelFoldOne, origin: ids.b, destination: ids.auxiliaryVertex },
    {
      edge: ids.extraBoundaryOne,
      origin: ids.auxiliaryVertex,
      destination: ids.missingVertex,
    },
    { edge: ids.extraBoundaryTwo, origin: ids.missingVertex, destination: ids.a },
  ])
  const middle = face(ids.south, 2, 16, [
    { edge: ids.bc, origin: ids.b, destination: ids.c },
    { edge: ids.parallelFoldTwo, origin: ids.c, destination: ids.f },
    { edge: ids.fa, origin: ids.f, destination: ids.auxiliaryVertex },
    { edge: ids.parallelFoldOne, origin: ids.auxiliaryVertex, destination: ids.b },
  ])
  const east = face(ids.east, 3, 8, [
    { edge: ids.cd, origin: ids.c, destination: ids.d },
    { edge: ids.de, origin: ids.d, destination: ids.e },
    { edge: ids.ef, origin: ids.e, destination: ids.f },
    { edge: ids.parallelFoldTwo, origin: ids.f, destination: ids.c },
  ])
  const materialByBoundary = [
    ids.west,
    ids.south,
    ids.east,
    ids.east,
    ids.east,
    ids.south,
    ids.west,
    ids.west,
  ]
  const topology: ProjectTopologyResponse = {
    project_id: ids.project,
    revision: 7,
    simulation_ready: true,
    snapshot: {
      source_revision: 7,
      faces: [west, middle, east],
      edge_incidence: [
        ...boundaryIds.map((edgeId, index) => [
          edgeId,
          { kind: 'boundary' as const, material: materialByBoundary[index] },
        ] as const),
        [ids.parallelFoldOne, {
          kind: 'hinge',
          left: ids.west,
          right: ids.south,
          assignment: 'mountain',
        }],
        [ids.parallelFoldTwo, {
          kind: 'hinge',
          left: ids.south,
          right: ids.east,
          assignment: 'valley',
        }],
      ],
      hinge_adjacency: [
        {
          edge: ids.parallelFoldOne,
          first: ids.west,
          second: ids.south,
          assignment: 'mountain',
        },
        {
          edge: ids.parallelFoldTwo,
          first: ids.south,
          second: ids.east,
          assignment: 'valley',
        },
      ],
    },
    issues: [],
  }
  return [project, topology]
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
    leftFaceId: ids.west,
    rightFaceId: ids.east,
    start: { vertexId: ids.b, x: 0, z: 2.2 },
    end: { vertexId: ids.e, x: 0, z: -2.2 },
    axis: { x: 0, z: -1 },
    assignment: 'mountain',
    rotationSign: 1,
  })
})

test('a cellular fold graph preserves every face and validated hinge in canonical order', () => {
  const [project, topology] = foldGraphFixtures()

  const model = buildFoldPreviewModel(project, topology)

  assert.ok(model?.kind === 'fold_graph')
  assert.deepEqual(
    model.faces.map((face) => face.id),
    [ids.west, ids.south, ids.east, ids.north],
  )
  assert.deepEqual(
    model.hinges.map((hinge) => hinge.edgeId),
    [ids.foldAg, ids.foldFg, ids.foldCg, ids.foldDg],
  )
  assert.deepEqual(model.kinematics, {
    kind: 'static_cycle',
    reason: 'cyclic_hinge_graph',
  })
  assert.deepEqual(
    model.hinges.map((hinge) => [
      hinge.start.vertexId,
      hinge.end.vertexId,
      hinge.assignment,
      hinge.rotationSign,
    ]),
    [
      [ids.a, ids.center, 'mountain', 1],
      [ids.f, ids.center, 'valley', -1],
      [ids.c, ids.center, 'mountain', 1],
      [ids.d, ids.center, 'valley', -1],
    ],
  )
  for (const hinge of model.hinges) {
    assert.ok(Math.abs(Math.hypot(hinge.axis.x, hinge.axis.z) - 1) < Number.EPSILON * 2)
  }

  const expected = clone(model)
  project.crease_pattern.vertices.reverse()
  project.crease_pattern.edges.reverse()
  project.paper.boundary_vertices.push(...project.paper.boundary_vertices.splice(0, 3))
  topology.snapshot?.edge_incidence.reverse()
  assert.deepEqual(buildFoldPreviewModel(project, topology), expected)
})

test('an acyclic fold graph exposes a deterministic parent-before-child motion tree', () => {
  const [project, topology] = foldTreeFixtures()

  const model = buildFoldPreviewModel(project, topology)

  assert.ok(model?.kind === 'fold_graph')
  assert.deepEqual(model.kinematics, {
    kind: 'tree',
    rootFaceId: ids.west,
    joints: [
      {
        parentFaceId: ids.west,
        childFaceId: ids.south,
        hinge: model.hinges[0],
        childRotationSign: 1,
      },
      {
        parentFaceId: ids.south,
        childFaceId: ids.east,
        hinge: model.hinges[1],
        childRotationSign: -1,
      },
    ],
  })
})

test('tree traversal reverses relative hinge rotation when the canonical root is on the right', () => {
  const [project, topology] = foldTreeFixtures()
  assert.ok(topology.snapshot)
  const [west, middle, east] = topology.snapshot.faces
  west.key[0] = 3
  middle.key[0] = 2
  east.key[0] = 1
  topology.snapshot.faces = [east, middle, west]
  topology.snapshot.hinge_adjacency = [
    {
      edge: ids.parallelFoldTwo,
      first: ids.east,
      second: ids.south,
      assignment: 'valley',
    },
    {
      edge: ids.parallelFoldOne,
      first: ids.south,
      second: ids.west,
      assignment: 'mountain',
    },
  ]

  const model = buildFoldPreviewModel(project, topology)

  assert.ok(model?.kind === 'fold_graph' && model.kinematics.kind === 'tree')
  assert.equal(model.kinematics.rootFaceId, ids.east)
  assert.deepEqual(
    model.kinematics.joints.map((joint) => [
      joint.parentFaceId,
      joint.childFaceId,
      joint.hinge.edgeId,
      joint.childRotationSign,
    ]),
    [
      [ids.east, ids.south, ids.parallelFoldTwo, 1],
      [ids.south, ids.west, ids.parallelFoldOne, -1],
    ],
  )
})

test('validated tree metadata feeds a pose whose parent and child hinge axes stay coincident', () => {
  const [project, topology] = foldTreeFixtures()
  const model = buildFoldPreviewModel(project, topology)
  assert.ok(model?.kind === 'fold_graph' && model.kinematics.kind === 'tree')

  const pose = calculateFoldTreePose(model.kinematics, 63)

  assert.ok(pose)
  assert.equal(pose.faceTransforms.size, model.faces.length)
  for (const joint of model.kinematics.joints) {
    const parent = pose.faceTransforms.get(joint.parentFaceId)
    const child = pose.faceTransforms.get(joint.childFaceId)
    assert.ok(parent && child)
    for (const endpoint of [joint.hinge.start, joint.hinge.end]) {
      const rest = new Vector3(endpoint.x, 0, endpoint.z)
      const fromParent = rest.clone().applyMatrix4(parent)
      const fromChild = rest.clone().applyMatrix4(child)
      assert.ok(fromParent.distanceTo(fromChild) < 1e-12)
    }
  }
})

test('every fold-graph hinge must match incidence, adjacency, source, and oriented boundaries', () => {
  const corruptions: Array<[
    string,
    (project: ProjectSnapshot, topology: ProjectTopologyResponse) => void,
  ]> = [
    ['missing adjacency', (_project, topology) => {
      topology.snapshot?.hinge_adjacency.pop()
    }],
    ['duplicate adjacency', (_project, topology) => {
      const adjacency = topology.snapshot?.hinge_adjacency[0]
      if (adjacency) topology.snapshot?.hinge_adjacency.push(clone(adjacency))
    }],
    ['non-canonical adjacency order', (_project, topology) => {
      topology.snapshot?.hinge_adjacency.reverse()
    }],
    ['non-canonical face order', (_project, topology) => {
      topology.snapshot?.faces.reverse()
    }],
    ['reversed adjacency pair', (_project, topology) => {
      const adjacency = topology.snapshot?.hinge_adjacency[2]
      if (adjacency) [adjacency.first, adjacency.second] = [adjacency.second, adjacency.first]
    }],
    ['wrong adjacency assignment', (_project, topology) => {
      const adjacency = topology.snapshot?.hinge_adjacency[1]
      if (adjacency) adjacency.assignment = 'mountain'
    }],
    ['wrong adjacency faces', (_project, topology) => {
      const adjacency = topology.snapshot?.hinge_adjacency[1]
      if (adjacency) adjacency.second = ids.south
    }],
    ['swapped incidence sides', (_project, topology) => {
      const incidence = topology.snapshot?.edge_incidence
        .find(([edge]) => edge === ids.foldDg)?.[1]
      if (incidence?.kind === 'hinge') {
        [incidence.left, incidence.right] = [incidence.right, incidence.left]
      }
    }],
    ['wrong incidence assignment', (_project, topology) => {
      const incidence = topology.snapshot?.edge_incidence
        .find(([edge]) => edge === ids.foldCg)?.[1]
      if (incidence?.kind === 'hinge') incidence.assignment = 'valley'
    }],
    ['wrong source assignment', (project) => {
      const source = project.crease_pattern.edges.find((edge) => edge.id === ids.foldAg)
      if (source) source.kind = 'valley'
    }],
    ['wrong source endpoints', (project) => {
      const source = project.crease_pattern.edges.find((edge) => edge.id === ids.foldFg)
      if (source) source.end = ids.e
    }],
  ]

  for (const [label, corrupt] of corruptions) {
    const [project, topology] = foldGraphFixtures()
    corrupt(project, topology)
    assert.doesNotThrow(() => buildFoldPreviewModel(project, topology), label)
    assert.equal(buildFoldPreviewModel(project, topology), null, label)
  }
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
