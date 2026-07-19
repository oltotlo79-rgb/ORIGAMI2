import assert from 'node:assert/strict'
import test from 'node:test'
import {
  foldAssignmentLabel,
  foldImportTargetLabel,
  foldImportTargetOptions,
  foldPreviewBounds,
  initialFoldImportMapping,
  isAllowedFoldImportTarget,
  isValidFoldImportName,
  parseFoldImportScale,
  unresolvedFoldAssignments,
  type FoldImportAssignmentSummary,
} from '../src/lib/foldImport.ts'

const summaries: FoldImportAssignmentSummary[] = [
  { assignment: 'B', count: 4 },
  { assignment: 'M', count: 2 },
  { assignment: 'V', count: 1 },
  { assignment: 'F', count: 3 },
  { assignment: 'U', count: 1 },
  { assignment: 'C', count: 2 },
  { assignment: 'J', count: 1 },
]

test('FOLD import starts only semantically direct assignments with defaults', () => {
  assert.deepEqual(initialFoldImportMapping(summaries), {
    M: 'mountain',
    V: 'valley',
    C: 'cut',
  })
  assert.deepEqual(
    unresolvedFoldAssignments(summaries, initialFoldImportMapping(summaries)),
    ['F', 'U', 'J'],
  )
})

test('FOLD assignment targets preserve direct meanings and constrain lossy choices', () => {
  assert.deepEqual(foldImportTargetOptions('M').map(({ value }) => value), ['mountain'])
  assert.deepEqual(foldImportTargetOptions('V').map(({ value }) => value), ['valley'])
  assert.deepEqual(foldImportTargetOptions('F').map(({ value }) => value), [
    'auxiliary',
    'ignore',
  ])
  assert.deepEqual(foldImportTargetOptions('U').map(({ value }) => value), [
    'mountain',
    'valley',
    'auxiliary',
    'ignore',
  ])
  assert.deepEqual(foldImportTargetOptions('C').map(({ value }) => value), ['cut', 'ignore'])
  assert.deepEqual(foldImportTargetOptions('J').map(({ value }) => value), [
    'auxiliary',
    'ignore',
  ])
  assert.equal(isAllowedFoldImportTarget('F', 'mountain'), false)
  assert.equal(isAllowedFoldImportTarget('U', 'mountain'), true)
  assert.equal(foldAssignmentLabel('B'), 'B · 用紙境界')
  assert.equal(foldAssignmentLabel('B', 'en'), 'B · Paper boundary')
  assert.equal(foldAssignmentLabel('J', 'en'), 'J · Face join')
  assert.equal(foldImportTargetLabel('auxiliary'), '補助線')
  assert.equal(foldImportTargetLabel('auxiliary', 'en'), 'Auxiliary line')
  assert.equal(foldImportTargetLabel('ignore', 'en'), 'Do not import')
})

test('invalid or missing mappings remain unresolved', () => {
  assert.deepEqual(
    unresolvedFoldAssignments(
      [{ assignment: 'F', count: 1 }],
      { F: 'mountain' },
    ),
    ['F'],
  )
  assert.deepEqual(
    unresolvedFoldAssignments(
      [{ assignment: 'F', count: 0 }],
      {},
    ),
    [],
  )
})

test('FOLD scale parser accepts only a bounded positive finite conversion', () => {
  assert.equal(parseFoldImportScale('25.4'), 25.4)
  assert.equal(parseFoldImportScale('0.000001'), 0.000001)
  for (const invalid of ['', '0', '-1', 'Infinity', 'NaN', '1000000001']) {
    assert.equal(parseFoldImportScale(invalid), null)
  }
})

test('FOLD imported project names enforce the native name boundary', () => {
  assert.equal(isValidFoldImportName('  鶴  '), true)
  assert.equal(isValidFoldImportName(''), false)
  assert.equal(isValidFoldImportName(' \n '), false)
  assert.equal(isValidFoldImportName('a'.repeat(120)), true)
  assert.equal(isValidFoldImportName('a'.repeat(121)), false)
})

test('FOLD preview bounds include degenerate spans without producing a zero viewBox', () => {
  assert.equal(foldPreviewBounds([]), null)
  assert.deepEqual(foldPreviewBounds([{ x: 2, y: 3 }, { x: 12, y: 8 }]), {
    minX: 2,
    minY: 3,
    width: 10,
    height: 5,
  })
  assert.deepEqual(foldPreviewBounds([{ x: 2, y: 3 }]), {
    minX: 1.995,
    minY: 2.995,
    width: 0.01,
    height: 0.01,
  })
  assert.equal(foldPreviewBounds([{ x: Number.NaN, y: 0 }]), null)
  assert.equal(
    foldPreviewBounds([
      { x: -Number.MAX_VALUE, y: 0 },
      { x: Number.MAX_VALUE, y: 1 },
    ]),
    null,
  )
})
