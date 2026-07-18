import assert from 'node:assert/strict'
import test from 'node:test'

import {
  classifyFoldPreviewTopologyContact,
  FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
  type FoldPreviewIntersectionEvidence,
  type FoldPreviewTopologyContactDecision,
  type FoldPreviewTopologyRelation,
} from '../src/lib/foldPreviewTopologyContactPolicy.ts'

const topologies: readonly FoldPreviewTopologyRelation[] = [
  'no_shared_feature',
  'shared_vertex',
  'shared_hinge_edge',
  'same_face',
]
const evidenceKinds: readonly FoldPreviewIntersectionEvidence[] = [
  'separated',
  'point_contact',
  'boundary_line_contact',
  'shared_feature_contact',
  'shared_feature_thickness_overlap',
  'shared_feature_flat_stack',
  'coplanar_area_overlap',
  'transversal_crossing',
  'positive_volume_overlap',
  'indeterminate',
]

const expected: Readonly<Record<
  FoldPreviewTopologyRelation,
  readonly FoldPreviewTopologyContactDecision[]
>> = {
  no_shared_feature: [
    'separated',
    'touching',
    'touching',
    'indeterminate',
    'indeterminate',
    'indeterminate',
    'penetrating',
    'penetrating',
    'penetrating',
    'indeterminate',
  ],
  shared_vertex: [
    'indeterminate',
    'touching',
    'touching',
    'allowed_shared_vertex_contact',
    'allowed_shared_vertex_contact',
    'indeterminate',
    'penetrating',
    'penetrating',
    'penetrating',
    'indeterminate',
  ],
  shared_hinge_edge: [
    'indeterminate',
    'indeterminate',
    'indeterminate',
    'requires_hinge_model',
    'requires_hinge_model',
    'requires_hinge_model',
    'penetrating',
    'penetrating',
    'penetrating',
    'indeterminate',
  ],
  same_face: Array<FoldPreviewTopologyContactDecision>(10)
    .fill('ignored_self'),
}

test('topology contact policy version and complete 4 by 10 table stay fixed', () => {
  assert.equal(
    FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
    'topology_contact_policy_v1',
  )
  assert.equal(topologies.length * evidenceKinds.length, 40)
  for (const topology of topologies) {
    assert.deepEqual(
      evidenceKinds.map((evidence) =>
        classifyFoldPreviewTopologyContact(topology, evidence)),
      expected[topology],
      topology,
    )
  }
})

test('shared identity never exempts area, transversal, or volume penetration', () => {
  for (const topology of ['shared_vertex', 'shared_hinge_edge'] as const) {
    for (const evidence of [
      'coplanar_area_overlap',
      'transversal_crossing',
      'positive_volume_overlap',
    ] as const) {
      assert.equal(
        classifyFoldPreviewTopologyContact(topology, evidence),
        'penetrating',
        `${topology}:${evidence}`,
      )
    }
  }
})

test('only feature-bound evidence can enter a topology allowance', () => {
  const allowed = new Set<FoldPreviewTopologyContactDecision>([
    'allowed_shared_vertex_contact',
    'requires_hinge_model',
  ])
  for (const topology of topologies) {
    for (const evidence of evidenceKinds) {
      const decision = classifyFoldPreviewTopologyContact(topology, evidence)
      if (!allowed.has(decision)) continue
      assert.ok(
        evidence === 'shared_feature_contact'
        || evidence === 'shared_feature_thickness_overlap'
        || (
          topology === 'shared_hinge_edge'
          && evidence === 'shared_feature_flat_stack'
        ),
        `${topology}:${evidence}`,
      )
    }
  }
})

test('impossible shared-feature evidence cells fail closed', () => {
  assert.equal(
    classifyFoldPreviewTopologyContact('shared_vertex', 'separated'),
    'indeterminate',
  )
  for (const evidence of [
    'separated',
    'point_contact',
    'boundary_line_contact',
  ] as const) {
    assert.equal(
      classifyFoldPreviewTopologyContact('shared_hinge_edge', evidence),
      'indeterminate',
      evidence,
    )
  }
})
