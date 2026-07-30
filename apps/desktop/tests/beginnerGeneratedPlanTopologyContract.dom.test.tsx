import { describe, expect, it } from 'vitest'

import {
  beginnerGeneratedPlanTopologyMatchesProfileV1,
} from '../src/lib/beginnerGeneratedPlanTopologyContract.ts'
import type {
  BeginnerDesignProfileV1,
  BeginnerGeneratedPlanV1,
} from '../src/lib/coreClient.ts'

interface TopologyFixture {
  profile: BeginnerDesignProfileV1
  plan: BeginnerGeneratedPlanV1
  ids: {
    contourVertices: readonly [string, string, string]
    treeVertices: readonly [string, string, string]
  }
}

function uuid(namespace: number, index: number): string {
  return `${namespace.toString(16).padStart(8, '0')}`
    + `-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
}

function genericTopologyFixture(): TopologyFixture {
  const skeletonSegments = [
    {
      id: 10,
      start: { x_tenths_mm: 0, y_tenths_mm: 0 },
      end: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
      thickness_tenths_mm: 10,
    },
    {
      id: 20,
      start: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
      end: { x_tenths_mm: 1_000, y_tenths_mm: 500 },
      thickness_tenths_mm: 10,
    },
  ]
  const profile: BeginnerDesignProfileV1 = {
    schema_version: 1,
    preset: 'balanced',
    shape_fidelity_weight: 35,
    foldability_weight: 35,
    step_count_weight: 15,
    paper_efficiency_weight: 15,
    generation_constraints: {
      schema_version: 1,
      maximum_steps: 60,
      detail_level: 'standard',
      target_category: 'custom_object',
      custom_object_display_name: 'Topology fixture',
      target_parts: [],
      skeleton_segments: skeletonSegments.map((segment) => ({
        ...segment,
        start: { ...segment.start },
        end: { ...segment.end },
      })),
      protrusions: [{
        id: 7,
        count: 1,
        length_tenths_mm: 100,
        thickness_tenths_mm: 10,
        local_outline_tenths_mm: [
          [0, 0],
          [10, 0],
          [0, 10],
        ],
        position_tenths_mm: [0, 0, 0],
        direction_milli: [1_000, 0, 0],
        symmetry: 'none',
        curvature_degrees: 0,
        joint: 'fixed',
        motion_degrees: [0, 0],
        side: 'either',
        priority: 50,
      }],
      bulge_targets: [],
      target_asset: null,
      allowed_techniques: ['valley_fold'],
    },
  }

  const center = uuid(1, 1)
  const endpoint = uuid(1, 2)
  const contourVertices = [
    uuid(2, 1),
    uuid(2, 2),
    uuid(2, 3),
  ] as const
  const treeVertices = [
    uuid(3, 1),
    uuid(3, 2),
    uuid(3, 3),
  ] as const
  const plan: BeginnerGeneratedPlanV1 = {
    schema_version: 1,
    kind: 'composite_generic_target_base',
    crease_pattern: {
      vertices: [
        { id: center, position: { x: 0, y: 0 } },
        { id: endpoint, position: { x: 1, y: 0 } },
        ...contourVertices.map((id, index) => ({
          id,
          position: { x: index, y: 1 },
        })),
        ...treeVertices.map((id, index) => ({
          id,
          position: { x: index, y: 2 },
        })),
      ],
      edges: [
        {
          id: uuid(4, 1),
          start: center,
          end: endpoint,
          kind: 'valley',
        },
        {
          id: uuid(5, 1),
          start: contourVertices[0],
          end: contourVertices[1],
          kind: 'auxiliary',
        },
        {
          id: uuid(5, 2),
          start: contourVertices[1],
          end: contourVertices[2],
          kind: 'auxiliary',
        },
        {
          id: uuid(5, 3),
          start: contourVertices[2],
          end: contourVertices[0],
          kind: 'auxiliary',
        },
        {
          id: uuid(6, 1),
          start: treeVertices[0],
          end: treeVertices[1],
          kind: 'auxiliary',
        },
        {
          id: uuid(6, 2),
          start: treeVertices[1],
          end: treeVertices[2],
          kind: 'auxiliary',
        },
      ],
    },
    instruction_codes: [
      'bounded_tree_river_axial_v1:4000000,1000000',
      'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
      'bounded_tree_paper_orientation_v1:horizontal',
    ],
    target_parts: [],
    skeleton_segments: skeletonSegments,
    target_asset: null,
  }
  return { profile, plan, ids: { contourVertices, treeVertices } }
}

function foldVariantFixture(): {
  profile: BeginnerDesignProfileV1
  plan: BeginnerGeneratedPlanV1
} {
  const { profile, plan } = genericTopologyFixture()
  const start = uuid(7, 1)
  const end = uuid(7, 2)
  plan.kind = 'vertical_book_fold'
  plan.instruction_codes = ['book_fold_vertical']
  plan.crease_pattern = {
    vertices: [
      { id: start, position: { x: 0, y: 0 } },
      { id: end, position: { x: 1, y: 0 } },
    ],
    edges: [{
      id: uuid(8, 1),
      start,
      end,
      kind: 'valley',
    }],
  }
  return { profile, plan }
}

describe('beginner generated-plan topology contract', () => {
  it('accepts the canonical contour cycle followed by a generic tree', () => {
    const { profile, plan } = genericTopologyFixture()

    expect(beginnerGeneratedPlanTopologyMatchesProfileV1(
      plan,
      profile,
      0,
    )).toBe(true)
  })

  it('rejects a generic plan with its tree block missing', () => {
    const { profile, plan } = genericTopologyFixture()
    plan.crease_pattern.vertices.splice(-3)
    plan.crease_pattern.edges.splice(-2)

    expect(beginnerGeneratedPlanTopologyMatchesProfileV1(
      plan,
      profile,
      0,
    )).toBe(false)
  })

  it.each([
    ['tree vertex order', ({ plan }: TopologyFixture) => {
      const first = plan.crease_pattern.vertices.length - 3
      const second = first + 1
      const held = plan.crease_pattern.vertices[first]!
      plan.crease_pattern.vertices[first] =
        plan.crease_pattern.vertices[second]!
      plan.crease_pattern.vertices[second] = held
    }],
    ['tree edge direction', ({ plan }: TopologyFixture) => {
      const edge = plan.crease_pattern.edges.at(-2)!
      const held = edge.start
      edge.start = edge.end
      edge.end = held
    }],
    ['tree connectivity', ({ plan, ids }: TopologyFixture) => {
      const edge = plan.crease_pattern.edges.at(-1)!
      edge.start = ids.treeVertices[0]
      edge.end = ids.treeVertices[1]
    }],
  ] as const)('rejects noncanonical %s', (_label, mutate) => {
    const fixture = genericTopologyFixture()
    mutate(fixture)

    expect(beginnerGeneratedPlanTopologyMatchesProfileV1(
      fixture.plan,
      fixture.profile,
      0,
    )).toBe(false)
  })

  it('rejects a broken contour cycle', () => {
    const { profile, plan, ids } = genericTopologyFixture()
    plan.crease_pattern.edges[1]!.end = ids.contourVertices[2]

    expect(beginnerGeneratedPlanTopologyMatchesProfileV1(
      plan,
      profile,
      0,
    )).toBe(false)
  })

  it('accepts exactly two vertices and one directed base edge for a fold variant', () => {
    const { profile, plan } = foldVariantFixture()

    expect(beginnerGeneratedPlanTopologyMatchesProfileV1(
      plan,
      profile,
      1,
    )).toBe(true)
  })

  it.each([
    ['an extra vertex', (plan: BeginnerGeneratedPlanV1) => {
      plan.crease_pattern.vertices.push({
        id: uuid(7, 3),
        position: { x: 2, y: 0 },
      })
    }],
    ['an extra edge', (plan: BeginnerGeneratedPlanV1) => {
      plan.crease_pattern.edges.push({
        id: uuid(8, 2),
        start: plan.crease_pattern.vertices[0]!.id,
        end: plan.crease_pattern.vertices[1]!.id,
        kind: 'valley',
      })
    }],
    ['a reversed edge', (plan: BeginnerGeneratedPlanV1) => {
      const edge = plan.crease_pattern.edges[0]!
      const held = edge.start
      edge.start = edge.end
      edge.end = held
    }],
  ] as const)('rejects a fold variant with %s', (_label, mutate) => {
    const { profile, plan } = foldVariantFixture()
    mutate(plan)

    expect(beginnerGeneratedPlanTopologyMatchesProfileV1(
      plan,
      profile,
      1,
    )).toBe(false)
  })
})
