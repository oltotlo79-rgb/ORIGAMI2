import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import test from 'node:test'

import {
  GEOMETRIC_CONSTRAINT_PANEL_TEXT as TEXT,
} from '../src/lib/geometricConstraintPanelText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

const SIMPLE_KEYS = [
  'title',
  'constraintCount',
  'addHorizontal',
  'addVertical',
  'moveLegend',
  'xAxis',
  'solveXAria',
  'yAxis',
  'solveYAria',
  'preview',
  'changedVertices',
  'iterations',
  'residual',
  'rank',
  'degreesOfFreedom',
  'condition',
  'exactSatisfaction',
  'deterministicReplayableScope',
  'currentRuntimeFallbackScope',
  'detailSeparator',
  'movePreview',
  'apply',
  'cancel',
  'solveError',
  'edgeTransformLegend',
  'edgeDeltaX',
  'edgeDeltaY',
  'edgeRotation',
  'edgeLengthScale',
  'previewEdgeTransform',
  'reevaluateSavedExpressions',
  'references',
  'selectEdgeHint',
  'creationLegend',
  'constraintKind',
  'createKind',
  'selectPrompt',
  'addFormConstraint',
  'creationInvalid',
  'creationHint',
  'allKindsLegend',
  'constraintJson',
  'constraintJsonPlaceholder',
  'addConstraint',
  'jsonInvalid',
  'jsonHint',
  'noConstraints',
  'unknownConstraint',
  'unknownConstraintKind',
  'targetUnavailable',
  'selectTarget',
  'deleteConstraint',
  'delete',
  'constraintListTruncated',
  'analyzing',
  'analysisFailed',
  'directConflictCount',
  'unknownStatus',
  'provenSatisfiable',
  'constructedSatisfiable',
  'noDirectConflict',
  'unanalyzed',
  'directConflictCauses',
  'causingConstraints',
  'additionalDirectConflicts',
  'uncheckedConstraints',
  'analyzeAgain',
  'boundedMusProven',
  'boundedMusConstraintLimit',
  'boundedMusIncomplete',
  'boundedMusCancelled',
  'boundedMusDeadlineReached',
  'semanticMusHeading',
  'semanticMusCertified',
  'semanticMusUnknownWithCore',
  'semanticMusUnknownWithoutCore',
  'semanticMusLegacyUnavailable',
  'semanticMusNoAuthority',
  'invalidIdentifier',
  'idListSeparator',
  'remainingIds',
] as const

const NESTED_KEYS = {
  creationFieldLabels: [
    'targetLine',
    'angleVertex',
    'firstLine',
    'secondLine',
    'targetPoint',
    'referenceLine',
    'firstPoint',
    'secondPoint',
    'symmetryAxis',
    'rotationCenter',
    'sourcePoint',
    'correspondingPoint',
    'bisectorLine',
    'numeratorLine',
    'denominatorLine',
  ],
  constraintKindNames: [
    'fixed_length',
    'fixed_angle',
    'horizontal',
    'vertical',
    'equal_length',
    'parallel',
    'point_on_line',
    'mirror_symmetry',
    'rotational_symmetry',
    'angle_bisector',
    'length_ratio',
  ],
  scalarLabels: [
    'fixed_length',
    'fixed_angle',
    'rotational_symmetry',
    'length_ratio',
  ],
  systemClassifications: [
    'under_constrained',
    'well_constrained',
    'over_constrained',
  ],
  directConflictLabels: [
    'different_fixed_lengths',
    'different_fixed_angles',
    'different_length_ratios',
    'horizontal_and_vertical',
    'equal_length_with_different_fixed_lengths',
    'equal_length_with_non_unit_ratio_and_fixed_length',
    'non_reciprocal_length_ratios_with_fixed_length',
    'length_ratio_with_incompatible_fixed_lengths',
    'non_unit_length_ratio_cycle_with_fixed_length',
    'inconsistent_length_ratio_graph_with_fixed_length',
    'inconsistent_length_ratio_graph_between_fixed_lengths',
    'different_fixed_lengths_in_equal_length_component',
    'perpendicular_orientations_in_parallel_component',
    'non_parallel_fixed_angle_in_parallel_component',
    'parallel_with_fixed_non_parallel_angle',
    'parallel_with_perpendicular_orientations',
    'same_orientation_with_fixed_non_parallel_angle',
    'perpendicular_orientations_with_fixed_non_right_angle',
    'different_rotational_symmetry_angles_with_fixed_radius',
    'non_complementary_inverse_rotational_symmetry_angles_with_fixed_radius',
    'mirror_symmetry_with_point_on_axis_and_fixed_separation',
    'rotational_symmetry_with_collinear_radius',
    'positive_fixed_length_in_bounded_zero_length_closure',
    'zero_length_closure_reaches_nondegenerate_provider',
  ],
  unknownReasonLabels: [
    'work_limit_exceeded',
    'constraint_limit_exceeded',
    'storage_limit_exceeded',
    'cancelled',
    'deadline_reached',
    'solver_required_constraint_kinds',
    'invalid_document_or_geometry',
  ],
  semanticMusUnknownReasonLabels: [
    'direct_oracle_incomplete',
    'deletion_witness_limit_exceeded',
    'deletion_witness_work_limit_exceeded',
    'deletion_witness_unavailable',
    'cancelled',
    'deadline_reached',
  ],
} as const

test('geometric constraint panel catalog is exact, closed, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), [
    ...SIMPLE_KEYS,
    ...Object.keys(NESTED_KEYS),
  ])
  for (const key of SIMPLE_KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
  }
  for (const [group, expectedKeys] of Object.entries(NESTED_KEYS)) {
    const entries = TEXT[
      group as keyof typeof NESTED_KEYS
    ] as Readonly<Record<string, Readonly<Record<string, string>>>>
    assert.deepEqual(Object.keys(entries), expectedKeys, group)
    for (const [key, localized] of Object.entries(entries)) {
      assert.deepEqual(Object.keys(localized), ['ja', 'en'], `${group}.${key}`)
    }
  }
  assert.equal(
    createHash('sha256').update(JSON.stringify(TEXT), 'utf8').digest('hex'),
    'dd54dbd5d49b0a51123d5b6d31d3015880cde1cc825635b29f6172b3ca6de9fa',
  )
  assert.equal(Object.hasOwn(TEXT, 'ja'), false)
  assert.equal(TEXT.title.ja, '幾何制約')
  assert.equal(TEXT.title.en, 'Geometric constraints')
  assert.equal(
    TEXT.constraintJsonPlaceholder.ja,
    '{"kind":"equal_length","first_edge":"UUID","second_edge":"UUID"}',
  )
  assertDeeplyFrozen(TEXT)
})

test('geometric constraint placeholders preserve exact set, order, and output', () => {
  assert.deepEqual(collectPlaceholders(TEXT), {
    constraintCount: { ja: ['count'], en: ['count'] },
    createKind: { ja: ['name'], en: ['name'] },
    deleteConstraint: { ja: ['name'], en: ['name'] },
    constraintListTruncated: {
      ja: ['visible', 'remaining'],
      en: ['visible', 'remaining'],
    },
    directConflictCount: { ja: ['count'], en: ['count'] },
    unknownStatus: { ja: ['reason'], en: ['reason'] },
    provenSatisfiable: {
      ja: ['constraintCount', 'equationCount', 'scope'],
      en: ['constraintCount', 'equationCount', 'scope'],
    },
    constructedSatisfiable: {
      ja: ['constraintCount', 'equationCount', 'scope'],
      en: ['constraintCount', 'equationCount', 'scope'],
    },
    exactSatisfaction: {
      ja: ['constraintCount', 'equationCount', 'scope'],
      en: ['constraintCount', 'equationCount', 'scope'],
    },
    causingConstraints: { ja: ['ids'], en: ['ids'] },
    additionalDirectConflicts: { ja: ['count'], en: ['count'] },
    uncheckedConstraints: { ja: ['ids'], en: ['ids'] },
    boundedMusProven: {
      ja: ['count', 'calls', 'ids'],
      en: ['count', 'calls', 'ids'],
    },
    semanticMusCertified: {
      ja: [
        'count',
        'calls',
        'checks',
        'work',
        'current',
        'axis',
        'constructive',
        'pairConstructive',
        'pairAlgebraic',
        'lengthConstructive',
        'zeroClosure',
        'mirrorResidual',
        'unitParallelFixedAngleResidual',
        'unitTerminalTwoHopParallelAngleResidual',
        'unitTwoHopParallelResidual',
        'scope',
        'ids',
      ],
      en: [
        'count',
        'calls',
        'checks',
        'work',
        'current',
        'axis',
        'constructive',
        'pairConstructive',
        'pairAlgebraic',
        'lengthConstructive',
        'zeroClosure',
        'mirrorResidual',
        'unitParallelFixedAngleResidual',
        'unitTerminalTwoHopParallelAngleResidual',
        'unitTwoHopParallelResidual',
        'scope',
        'ids',
      ],
    },
    semanticMusUnknownWithCore: {
      ja: ['count', 'reason', 'certified', 'checks', 'work', 'ids'],
      en: ['count', 'reason', 'certified', 'checks', 'work', 'ids'],
    },
    semanticMusUnknownWithoutCore: {
      ja: ['reason', 'calls'],
      en: ['reason', 'calls'],
    },
    remainingIds: {
      ja: ['visible', 'remaining'],
      en: ['visible', 'remaining'],
    },
    'directConflictLabels.different_fixed_lengths': {
      ja: ['edge'],
      en: ['edge'],
    },
    'directConflictLabels.different_fixed_angles': {
      ja: ['vertex'],
      en: ['vertex'],
    },
    'directConflictLabels.horizontal_and_vertical': {
      ja: ['edge'],
      en: ['edge'],
    },
  })

  assert.equal(
    formatLocalizedText('ja', TEXT.constraintCount, { count: 11 }),
    '11件',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.createKind, { name: 'Fixed length' }),
    'Create Fixed length',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.deleteConstraint, { name: '長さ固定' }),
    '長さ固定制約を削除',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.constraintListTruncated, {
      visible: 200,
      remaining: 1,
    }),
    'Showing the first 200 constraints. The remaining 1 appear as displayed constraints are deleted.',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.unknownStatus, { reason: '判定保留です' }),
    '判定保留です。安全確認済みとして扱いません。',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.boundedMusProven, {
      count: 3,
      calls: 7,
      ids: 'a, b, c',
    }),
    'Smallest subset proven by the bounded direct-conflict oracle (3 constraints, 7 calls): a, b, c',
  )
  assert.equal(
    formatLocalizedText(
      'ja',
      TEXT.directConflictLabels.different_fixed_angles,
      { vertex: 'abcdef00…abcd' },
    ),
    '同じ角に異なる角度が指定されています（頂点 abcdef00…abcd）',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.remainingIds, {
      visible: 'a, b',
      remaining: 2,
    }),
    'a, b, 2 more',
  )
})

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) assertDeeplyFrozen(child)
}

function collectPlaceholders(
  value: unknown,
  path: readonly string[] = [],
): Record<string, Readonly<{ ja: string[]; en: string[] }>> {
  if (typeof value !== 'object' || value === null) return {}
  if (
    Object.keys(value).length === 2
    && typeof (value as { ja?: unknown }).ja === 'string'
    && typeof (value as { en?: unknown }).en === 'string'
  ) {
    const localized = value as { ja: string; en: string }
    const ja = placeholderNames(localized.ja)
    const en = placeholderNames(localized.en)
    return ja.length === 0 && en.length === 0
      ? {}
      : { [path.join('.')]: { ja, en } }
  }
  return Object.assign(
    {},
    ...Object.entries(value).map(([key, child]) =>
      collectPlaceholders(child, [...path, key])),
  )
}

function placeholderNames(value: string): string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
