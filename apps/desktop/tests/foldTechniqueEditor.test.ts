import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  FOLD_TECHNIQUE_LIMITS_V1,
  admitFoldTechniqueDocumentV1,
  changeFoldTechniqueOperationActionV1,
  createInitialFoldTechniqueDocumentV1,
  createInitialFoldTechniqueOperationV1,
  foldTechniqueDocumentsEqualV1,
  updateFoldTechniqueDocumentDraftV1,
  validateFoldTechniqueDocumentV1,
  type FoldTechniqueActionKindV1,
} from '../src/lib/foldTechniqueEditor.ts'

test('initial editor value is a deeply frozen strict V1 document', () => {
  const document = createInitialFoldTechniqueDocumentV1()
  assert.equal(document.schema, 'origami2_fold_technique_file')
  assert.equal(document.version, 1)
  assert.equal(document.techniques.length, 1)
  assert.equal(document.techniques[0]?.operations.length, 2)
  assert.ok(Object.isFrozen(document))
  assert.ok(Object.isFrozen(document.metadata))
  assert.ok(Object.isFrozen(document.techniques))
  assert.ok(Object.isFrozen(document.techniques[0]?.operations[0]?.action))
  assert.deepEqual(
    document.techniques[0]?.names.map(({ locale }) => locale),
    ['en', 'ja'],
  )
  assert.deepEqual(admitFoldTechniqueDocumentV1(document), document)
})

test('admission rejects unknown fields, accessors, exotic prototypes, and proxies', () => {
  const initial = clone(createInitialFoldTechniqueDocumentV1())
  assert.equal(
    admitFoldTechniqueDocumentV1({ ...initial, execute: 'script' }),
    null,
  )

  let getterCalls = 0
  Object.defineProperty(initial, 'metadata', {
    enumerable: true,
    get() {
      getterCalls += 1
      return {}
    },
  })
  assert.equal(admitFoldTechniqueDocumentV1(initial), null)
  assert.equal(getterCalls, 0)

  const exotic = Object.assign(
    Object.create({ inherited: true }),
    clone(createInitialFoldTechniqueDocumentV1()),
  )
  assert.equal(admitFoldTechniqueDocumentV1(exotic), null)

  const revoked = Proxy.revocable(
    clone(createInitialFoldTechniqueDocumentV1()),
    {},
  )
  revoked.revoke()
  assert.doesNotThrow(() => {
    assert.equal(admitFoldTechniqueDocumentV1(revoked.proxy), null)
  })
})

test('all physical action helpers emit explicit inert support metadata', () => {
  const expected = new Map<FoldTechniqueActionKindV1, readonly string[]>([
    [
      'instruction_cue',
      ['human_interpretation_v1', 'declarative_only'],
    ],
    [
      'straight_line_stacked_fold',
      ['straight_line_stacked_fold_v1', 'declarative_only'],
    ],
    [
      'inside_reverse_fold',
      [
        'inside_reverse_fold_motion_v1',
        'unsupported_physical_operation',
        'inside_reverse_fold_motion_v1',
      ],
    ],
    [
      'outside_reverse_fold',
      [
        'outside_reverse_fold_motion_v1',
        'unsupported_physical_operation',
        'outside_reverse_fold_motion_v1',
      ],
    ],
    [
      'sink_fold',
      [
        'sink_fold_motion_v1',
        'unsupported_physical_operation',
        'sink_fold_motion_v1',
      ],
    ],
    [
      'layer_selective_manipulation',
      [
        'layer_selective_motion_v1',
        'unsupported_physical_operation',
        'layer_selective_motion_v1',
      ],
    ],
  ])
  const original = createInitialFoldTechniqueOperationV1(1)

  for (const [kind, values] of expected) {
    const operation = kind === original.action.kind
      ? original
      : changeFoldTechniqueOperationActionV1(original, kind)
    assert.equal(operation.required_capabilities[0], values[0])
    assert.equal(operation.execution_support.status, values[1])
    if (operation.execution_support.status === 'unsupported_physical_operation') {
      assert.equal(operation.execution_support.operation, values[2])
    }
    assert.doesNotMatch(JSON.stringify(operation), /executable|project_command/u)
    const document = clone(createInitialFoldTechniqueDocumentV1())
    document.techniques[0].operations[0] = clone(operation)
    assert.ok(
      admitFoldTechniqueDocumentV1(document),
      `${kind} must remain valid inert V1 metadata`,
    )
  }
})

test('unsupported physical actions cannot be downgraded or mislabeled', () => {
  const initial = clone(createInitialFoldTechniqueDocumentV1())
  const operation = initial.techniques[0].operations[0]
  operation.action = { kind: 'inside_reverse_fold' }
  operation.required_capabilities = ['inside_reverse_fold_motion_v1']
  operation.execution_support = { status: 'declarative_only' }
  assert.deepEqual(validateFoldTechniqueDocumentV1(initial), {
    ok: false,
    error: 'inconsistent_execution_support',
  })

  operation.execution_support = {
    status: 'unsupported_physical_operation',
    operation: 'sink_fold_motion_v1',
  }
  assert.deepEqual(validateFoldTechniqueDocumentV1(initial), {
    ok: false,
    error: 'inconsistent_execution_support',
  })
})

test('changing an action preserves non-action capabilities canonically', () => {
  const operation = clone(createInitialFoldTechniqueOperationV1(1))
  operation.required_capabilities = [
    'manual_pose_registration_v1',
    'human_interpretation_v1',
    'instruction_timeline_v1',
    'manual_pose_registration_v1',
    'inside_reverse_fold_motion_v1',
  ]
  const changed = changeFoldTechniqueOperationActionV1(
    operation,
    'inside_reverse_fold',
  )

  assert.deepEqual(changed.required_capabilities, [
    'instruction_timeline_v1',
    'manual_pose_registration_v1',
    'inside_reverse_fold_motion_v1',
  ])
  assert.deepEqual(changed.execution_support, {
    status: 'unsupported_physical_operation',
    operation: 'inside_reverse_fold_motion_v1',
  })

  const document = clone(createInitialFoldTechniqueDocumentV1())
  document.techniques[0].operations[0] = clone(changed)
  assert.ok(admitFoldTechniqueDocumentV1(document))
})

test('basic edits canonically preserve V1 parameters, preconditions, bindings, locales, and capabilities', () => {
  const candidate = clone(createInitialFoldTechniqueDocumentV1())
  const technique = candidate.techniques[0]
  technique.names.push({ locale: 'fr', text: 'Technique conservée' })
  technique.parameters = [
    {
      id: 'count',
      names: localized('回数', 'Count'),
      descriptions: localized('繰り返し回数', 'Repeat count'),
      parameter_type: {
        type: 'integer',
        minimum: 0,
        maximum: 10,
        default: 0,
      },
    },
  ]
  technique.preconditions = [
    {
      id: 'count-is-zero',
      condition: {
        kind: 'parameter_comparison',
        parameter_id: 'count',
        comparison: 'equal',
        value: { type: 'integer', value: 0 },
      },
    },
  ]
  const firstOperation = technique.operations[0]
  assert.equal(firstOperation.action.kind, 'instruction_cue')
  if (firstOperation.action.kind !== 'instruction_cue') {
    throw new Error('initial fixture drift')
  }
  firstOperation.names.push({ locale: 'fr', text: 'Préparer' })
  firstOperation.action.instructions.push({
    locale: 'fr',
    text: 'Préparez le papier.',
  })
  firstOperation.parameter_bindings = [
    { role: 'repeat-count', parameter_id: 'count' },
  ]
  firstOperation.precondition_ids = ['count-is-zero']
  firstOperation.required_capabilities = [
    'instruction_timeline_v1',
    'human_interpretation_v1',
  ]
  const admitted = admitFoldTechniqueDocumentV1(candidate)
  assert.ok(admitted)

  const changed = updateFoldTechniqueDocumentDraftV1(admitted, {
    kind: 'technique_name',
    techniqueIndex: 0,
    locale: 'ja',
    value: '保持確認',
  })
  const readmitted = admitFoldTechniqueDocumentV1(changed)
  assert.ok(readmitted)
  const updated = readmitted.techniques[0]
  assert.equal(
    updated?.names.find(({ locale }) => locale === 'fr')?.text,
    'Technique conservée',
  )
  assert.equal(
    JSON.stringify(updated?.parameters),
    JSON.stringify(admitted.techniques[0]?.parameters),
  )
  assert.equal(
    JSON.stringify(updated?.preconditions),
    JSON.stringify(admitted.techniques[0]?.preconditions),
  )
  assert.equal(
    JSON.stringify(updated?.operations[0]?.parameter_bindings),
    JSON.stringify(
      admitted.techniques[0]?.operations[0]?.parameter_bindings,
    ),
  )
  assert.deepEqual(updated?.operations[0]?.required_capabilities, [
    'human_interpretation_v1',
    'instruction_timeline_v1',
  ])
})

test('update helpers are immutable and preserve identity for no changes', () => {
  const initial = createInitialFoldTechniqueDocumentV1()
  assert.equal(
    updateFoldTechniqueDocumentDraftV1(initial, {
      kind: 'package_id',
      value: initial.package_id,
    }),
    initial,
  )
  assert.equal(
    updateFoldTechniqueDocumentDraftV1(initial, {
      kind: 'technique_name',
      techniqueIndex: 0,
      locale: 'ja',
      value: '新しい折り技法',
    }),
    initial,
  )
  assert.equal(
    updateFoldTechniqueDocumentDraftV1(initial, {
      kind: 'remove_operation',
      techniqueIndex: 0,
      operationIndex: 0,
    }),
    initial,
  )

  const inserted = updateFoldTechniqueDocumentDraftV1(initial, {
    kind: 'insert_operation',
    techniqueIndex: 0,
    operationIndex: 2,
    operation: createInitialFoldTechniqueOperationV1(3),
  })
  assert.notEqual(inserted, initial)
  assert.ok(Object.isFrozen(inserted))
  assert.equal(initial.techniques[0]?.operations.length, 2)
  assert.equal(inserted.techniques[0]?.operations.length, 3)

  const moved = updateFoldTechniqueDocumentDraftV1(inserted, {
    kind: 'move_operation',
    techniqueIndex: 0,
    operationIndex: 2,
    direction: -1,
  })
  assert.deepEqual(
    moved.techniques[0]?.operations.map(({ id }) => id),
    ['step-1', 'step-3', 'step-2'],
  )
  assert.ok(admitFoldTechniqueDocumentV1(moved))
  assert.equal(foldTechniqueDocumentsEqualV1(initial, initial), true)
  assert.equal(foldTechniqueDocumentsEqualV1(initial, moved), false)
})

test('canonical admission sorts sets but preserves operation and choice order', () => {
  const candidate = clone(createInitialFoldTechniqueDocumentV1())
  candidate.metadata.authors = ['Zulu', 'Alpha']
  candidate.techniques[0].names.reverse()
  candidate.techniques[0].operations.reverse()
  candidate.techniques[0].operations[0].required_capabilities = [
    'instruction_timeline_v1',
    'human_interpretation_v1',
  ]
  const admitted = admitFoldTechniqueDocumentV1(candidate)
  assert.ok(admitted)
  assert.deepEqual(admitted.metadata.authors, ['Alpha', 'Zulu'])
  assert.deepEqual(
    admitted.techniques[0]?.names.map(({ locale }) => locale),
    ['en', 'ja'],
  )
  assert.deepEqual(
    admitted.techniques[0]?.operations.map(({ id }) => id),
    ['step-2', 'step-1'],
  )
  assert.deepEqual(
    admitted.techniques[0]?.operations[0]?.required_capabilities,
    ['human_interpretation_v1', 'instruction_timeline_v1'],
  )
})

test('character, identifier, collection, and encoded-byte ceilings are fixed', () => {
  const identifier = clone(createInitialFoldTechniqueDocumentV1())
  identifier.package_id = '../execute'
  assert.equal(validateFoldTechniqueDocumentV1(identifier).ok, false)

  const exactText = clone(createInitialFoldTechniqueDocumentV1())
  exactText.techniques[0].names[0].text = '🟦'.repeat(120)
  assert.ok(admitFoldTechniqueDocumentV1(exactText))
  exactText.techniques[0].names[0].text = '🟦'.repeat(121)
  assert.equal(admitFoldTechniqueDocumentV1(exactText), null)

  const operations = clone(createInitialFoldTechniqueDocumentV1())
  operations.techniques[0].operations = Array.from(
    { length: FOLD_TECHNIQUE_LIMITS_V1.operations + 1 },
    (_, index) => clone(createInitialFoldTechniqueOperationV1(index + 1)),
  )
  assert.deepEqual(validateFoldTechniqueDocumentV1(operations), {
    ok: false,
    error: 'resource_limit',
  })

  const oversized = clone(createInitialFoldTechniqueDocumentV1())
  const template = oversized.techniques[0]
  oversized.techniques = Array.from(
    { length: FOLD_TECHNIQUE_LIMITS_V1.techniques },
    (_, index) => ({
      ...clone(template),
      id: `user.large.technique-${index}`,
      descriptions: localized(
        '🟦'.repeat(FOLD_TECHNIQUE_LIMITS_V1.descriptionChars),
        '🟩'.repeat(FOLD_TECHNIQUE_LIMITS_V1.descriptionChars),
      ),
    }),
  )
  assert.deepEqual(validateFoldTechniqueDocumentV1(oversized), {
    ok: false,
    error: 'encoded_size_limit',
  })
})

test('editor foundation has no execution, project mutation, or resource-fetch path', () => {
  const modelSource = readFileSync(
    new URL('../src/lib/foldTechniqueEditor.ts', import.meta.url),
    'utf8',
  )
  const dialogSource = readFileSync(
    new URL('../src/components/FoldTechniqueEditorDialog.tsx', import.meta.url),
    'utf8',
  )
  const implementation = `${modelSource}\n${dialogSource}`
  assert.doesNotMatch(
    implementation,
    /\b(?:eval|fetch|invoke|open|writeFile|readFile|Function)\s*\(/u,
  )
  assert.doesNotMatch(implementation, /@tauri-apps|coreClient|project_command/u)
  assert.match(implementation, /unsupported_physical_operation/u)
  assert.match(implementation, /inert plain text; never fetched/u)
})

type Mutable<Value> =
  Value extends readonly (infer Item)[]
    ? Mutable<Item>[]
    : Value extends object
      ? { -readonly [Key in keyof Value]: Mutable<Value[Key]> }
      : Value

function clone<Value>(value: Value): Mutable<Value> {
  return JSON.parse(JSON.stringify(value)) as Mutable<Value>
}

function localized(ja: string, en: string) {
  return [
    { locale: 'ja', text: ja },
    { locale: 'en', text: en },
  ]
}
