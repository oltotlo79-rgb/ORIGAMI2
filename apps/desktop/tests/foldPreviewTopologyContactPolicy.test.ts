import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  classifyFoldPreviewTopologyContact,
  classifyFoldPreviewTopologyContactV2,
  FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
  FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_V2,
  type FoldPreviewIntersectionEvidence,
  type FoldPreviewIntersectionEvidenceV2,
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
const evidenceKindsV2: readonly FoldPreviewIntersectionEvidenceV2[] = [
  'separated',
  'point_contact',
  'boundary_line_contact',
  'boundary_area_contact',
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
const expectedV2: Readonly<Record<
  FoldPreviewTopologyRelation,
  readonly FoldPreviewTopologyContactDecision[]
>> = {
  no_shared_feature: [
    'separated',
    'touching',
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
    'requires_hinge_model',
    'penetrating',
    'penetrating',
    'penetrating',
    'indeterminate',
  ],
  same_face: Array<FoldPreviewTopologyContactDecision>(11)
    .fill('ignored_self'),
}

interface NormativePolicyCorpus {
  policy_id: string
  topology_relations: FoldPreviewTopologyRelation[]
  intersection_evidence: FoldPreviewIntersectionEvidence[]
  decisions: Record<
    FoldPreviewTopologyRelation,
    FoldPreviewTopologyContactDecision[]
  >
}
interface NormativePolicyCorpusV2 {
  policy_id: string
  topology_relations: FoldPreviewTopologyRelation[]
  intersection_evidence: FoldPreviewIntersectionEvidenceV2[]
  decisions: Record<
    FoldPreviewTopologyRelation,
    FoldPreviewTopologyContactDecision[]
  >
}

const normativeCorpus = JSON.parse(readFileSync(
  new URL(
    '../../../docs/collision-contact-policy-v1.json',
    import.meta.url,
  ),
  'utf8',
)) as NormativePolicyCorpus
const normativeCorpusV2 = JSON.parse(readFileSync(
  new URL(
    '../../../docs/collision-contact-policy-v2.json',
    import.meta.url,
  ),
  'utf8',
)) as NormativePolicyCorpusV2

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

test('frontend policy matches the shared native normative corpus', () => {
  assert.equal(
    normativeCorpus.policy_id,
    FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION,
  )
  assert.deepEqual(normativeCorpus.topology_relations, topologies)
  assert.deepEqual(normativeCorpus.intersection_evidence, evidenceKinds)
  assert.deepEqual(normativeCorpus.decisions, expected)
  assert.deepEqual(
    Object.fromEntries(topologies.map((topology) => [
      topology,
      evidenceKinds.map((evidence) =>
        classifyFoldPreviewTopologyContact(topology, evidence)),
    ])),
    normativeCorpus.decisions,
  )
})

test('V2 version and complete 4 by 11 table stay fixed', () => {
  assert.equal(
    FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_V2,
    'topology_contact_policy_v2',
  )
  assert.equal(topologies.length * evidenceKindsV2.length, 44)
  for (const topology of topologies) {
    assert.deepEqual(
      evidenceKindsV2.map((evidence) =>
        classifyFoldPreviewTopologyContactV2(topology, evidence)),
      expectedV2[topology],
      topology,
    )
  }
})

test('frontend V2 policy matches the shared native normative corpus', () => {
  assert.equal(
    normativeCorpusV2.policy_id,
    FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_V2,
  )
  assert.deepEqual(normativeCorpusV2.topology_relations, topologies)
  assert.deepEqual(normativeCorpusV2.intersection_evidence, evidenceKindsV2)
  assert.deepEqual(normativeCorpusV2.decisions, expectedV2)
  assert.deepEqual(
    Object.fromEntries(topologies.map((topology) => [
      topology,
      evidenceKindsV2.map((evidence) =>
        classifyFoldPreviewTopologyContactV2(topology, evidence)),
    ])),
    normativeCorpusV2.decisions,
  )
})

test('V2 embeds every frozen V1 cell without reinterpretation', () => {
  for (const topology of topologies) {
    for (const evidence of evidenceKinds) {
      assert.equal(
        classifyFoldPreviewTopologyContactV2(topology, evidence),
        classifyFoldPreviewTopologyContact(topology, evidence),
        `${topology}:${evidence}`,
      )
    }
  }
})

test('positive-area boundary contact has all four fixed decisions', () => {
  const decisions: Readonly<Record<
    FoldPreviewTopologyRelation,
    FoldPreviewTopologyContactDecision
  >> = {
    no_shared_feature: 'touching',
    shared_vertex: 'touching',
    shared_hinge_edge: 'requires_hinge_model',
    same_face: 'ignored_self',
  }
  for (const topology of topologies) {
    assert.equal(
      classifyFoldPreviewTopologyContactV2(
        topology,
        'boundary_area_contact',
      ),
      decisions[topology],
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
