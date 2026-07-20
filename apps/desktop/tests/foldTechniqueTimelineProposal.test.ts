import assert from 'node:assert/strict'
import test from 'node:test'

import {
  admitFoldTechniqueDocumentV1,
  createInitialFoldTechniqueDocumentV1,
  type FoldTechniqueFileDocumentV1,
} from '../src/lib/foldTechniqueEditor.ts'
import {
  createFoldTechniqueTimelineProposalV1,
} from '../src/lib/foldTechniqueTimelineProposal.ts'

test('builds one deterministic inert proposal without losing source declarations', () => {
  const document = richDocument()
  const first = createFoldTechniqueTimelineProposalV1(document, 0, 'ja', 3)
  const second = createFoldTechniqueTimelineProposalV1(document, 0, 'ja', 3)

  assert.equal(first.ok, true)
  assert.deepEqual(first, second)
  if (!first.ok) return
  assert.equal(first.techniqueName, '条件付き中割り')
  assert.equal(first.operationCount, 2)
  assert.equal(first.unsupportedOperationCount, 1)
  assert.deepEqual(
    first.proposal.steps.map((step) => step.source_kind),
    ['technique', 'parameter', 'precondition', 'operation', 'operation'],
  )
  assert.deepEqual(
    first.proposal.steps
      .filter((step) => step.source_kind === 'operation')
      .map((step) => step.source_id),
    ['align', 'reverse'],
  )
  assert.ok(first.proposal.steps.every((step) => step.duration_ms === 1_500))
  assert.ok(first.proposal.steps.every((step) =>
    !Object.hasOwn(step, 'pose')
    && !Object.hasOwn(step, 'hinge_angles')))

  const technique = document.techniques[0]!
  assert.deepEqual(
    sourceJson(first.proposal.steps, 'technique', technique.id),
    {
      schema: 'origami2_named_technique_timeline_source_v1',
      package_id: document.package_id,
      metadata: document.metadata,
      technique: {
        id: technique.id,
        version: technique.version,
        names: technique.names,
        descriptions: technique.descriptions,
      },
    },
  )
  for (const parameter of technique.parameters) {
    assert.deepEqual(
      sourceJson(first.proposal.steps, 'parameter', parameter.id),
      parameter,
    )
  }
  for (const precondition of technique.preconditions) {
    assert.deepEqual(
      sourceJson(first.proposal.steps, 'precondition', precondition.id),
      precondition,
    )
  }
  for (const operation of technique.operations) {
    assert.deepEqual(
      sourceJson(first.proposal.steps, 'operation', operation.id),
      operation,
    )
  }
  const unsupported = first.proposal.steps.find(
    (step) => step.source_id === 'reverse',
  )
  assert.match(unsupported?.caution ?? '', /自動実行しません/u)
})

test('splits long canonical source text on code-point boundaries and fails closed on capacity', () => {
  const document = richDocument('折'.repeat(2_048), 'fold'.repeat(512))
  const preview = createFoldTechniqueTimelineProposalV1(document, 0, 'en', 0)
  assert.equal(preview.ok, true)
  if (!preview.ok) return

  const techniqueChunks = preview.proposal.steps.filter(
    (step) => step.source_kind === 'technique',
  )
  assert.ok(techniqueChunks.length > 1)
  assert.deepEqual(
    techniqueChunks.map((step) => step.chunk_index),
    Array.from({ length: techniqueChunks.length }, (_, index) => index + 1),
  )
  assert.ok(techniqueChunks.every((step) =>
    step.chunk_count === techniqueChunks.length
    && [...step.description].length <= 4_000))
  assert.match(
    techniqueChunks.map((step) => step.description).join(''),
    /source-json-v1:/u,
  )

  const full = createFoldTechniqueTimelineProposalV1(document, 0, 'ja', 512)
  assert.deepEqual(full, {
    ok: false,
    error: 'timeline_capacity',
    requiredSteps: preview.proposal.steps.length,
    availableSteps: 0,
  })
})

test('rejects an invalid selection without constructing a partial proposal', () => {
  const result = createFoldTechniqueTimelineProposalV1(
    createInitialFoldTechniqueDocumentV1(),
    9,
    'ja',
    0,
  )
  assert.deepEqual(result, {
    ok: false,
    error: 'invalid_selection',
    requiredSteps: 0,
    availableSteps: 512,
  })
})

function richDocument(
  japaneseDescription = '設定と前提条件を確認して折ります。',
  englishDescription = 'Check parameters and preconditions before folding.',
): FoldTechniqueFileDocumentV1 {
  const initial = createInitialFoldTechniqueDocumentV1()
  const candidate = {
    ...structuredClone(initial),
    techniques: [{
      id: 'artist.conditional-reverse',
      version: 7,
      names: [
        { locale: 'ja', text: '条件付き中割り' },
        { locale: 'en', text: 'Conditional reverse fold' },
      ],
      descriptions: [
        { locale: 'ja', text: japaneseDescription },
        { locale: 'en', text: englishDescription },
      ],
      parameters: [{
        id: 'target-angle',
        names: [
          { locale: 'ja', text: '目標角度' },
          { locale: 'en', text: 'Target angle' },
        ],
        descriptions: [
          { locale: 'ja', text: '折る角度です。' },
          { locale: 'en', text: 'Angle to fold.' },
        ],
        parameter_type: {
          type: 'angle_microdegrees',
          minimum: 0,
          maximum: 180_000_000,
          default: 90_000_000,
        },
      }],
      preconditions: [{
        id: 'angle-positive',
        condition: {
          kind: 'parameter_comparison',
          parameter_id: 'target-angle',
          comparison: 'greater_than',
          value: {
            type: 'angle_microdegrees',
            value: 0,
          },
        },
      }],
      operations: [{
        id: 'align',
        names: [
          { locale: 'ja', text: '辺を合わせる' },
          { locale: 'en', text: 'Align edges' },
        ],
        action: {
          kind: 'instruction_cue',
          instructions: [
            { locale: 'ja', text: '辺を正確に合わせます。' },
            { locale: 'en', text: 'Align the edges precisely.' },
          ],
        },
        parameter_bindings: [{
          role: 'angle',
          parameter_id: 'target-angle',
        }],
        precondition_ids: ['angle-positive'],
        required_capabilities: [
          'human_interpretation_v1',
          'instruction_timeline_v1',
        ],
        execution_support: { status: 'declarative_only' },
      }, {
        id: 'reverse',
        names: [
          { locale: 'ja', text: '中割りにする' },
          { locale: 'en', text: 'Reverse inside' },
        ],
        action: { kind: 'inside_reverse_fold' },
        parameter_bindings: [],
        precondition_ids: ['angle-positive'],
        required_capabilities: ['inside_reverse_fold_motion_v1'],
        execution_support: {
          status: 'unsupported_physical_operation',
          operation: 'inside_reverse_fold_motion_v1',
        },
      }],
    }],
  }
  const admitted = admitFoldTechniqueDocumentV1(candidate)
  assert.ok(admitted)
  return admitted
}

function joinSource(
  steps: readonly Readonly<{
    source_kind: string
    source_id: string
    description: string
  }>[],
  sourceKind: string,
  sourceId: string,
) {
  return steps
    .filter((step) =>
      step.source_kind === sourceKind && step.source_id === sourceId)
    .map((step) => step.description)
    .join('')
}

function sourceJson(
  steps: readonly Readonly<{
    source_kind: string
    source_id: string
    description: string
  }>[],
  sourceKind: string,
  sourceId: string,
) {
  const source = joinSource(steps, sourceKind, sourceId)
  const marker = 'source-json-v1:\n'
  const markerIndex = source.indexOf(marker)
  assert.ok(markerIndex >= 0)
  return JSON.parse(source.slice(markerIndex + marker.length)) as unknown
}
