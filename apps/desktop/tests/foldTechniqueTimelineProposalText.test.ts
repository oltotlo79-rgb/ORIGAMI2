import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  createInitialFoldTechniqueDocumentV1,
} from '../src/lib/foldTechniqueEditor.ts'
import {
  createFoldTechniqueTimelineProposalV1,
} from '../src/lib/foldTechniqueTimelineProposal.ts'
import {
  FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT as TEXT,
} from '../src/lib/foldTechniqueTimelineProposalText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../src/lib/i18n.ts'

const EXPECTED_KEYS = [
  'techniqueTitle',
  'techniqueAndProvenance',
  'descriptionOnlyProposal',
  'parameterTitle',
  'parameterDefinition',
  'preconditionTitle',
  'preconditionCondition',
  'preconditionCaution',
  'operationTitle',
  'writtenFoldingCue',
  'layerSelectiveInstruction',
  'straightLineStackedFold',
  'insideReverseFold',
  'outsideReverseFold',
  'openSinkFold',
  'closedSinkFold',
  'unsupportedPhysicalOperation',
  'stackedFoldNotExecuted',
  'descriptionOnlyStep',
] as const

const EXPECTED_PLACEHOLDERS = new Map<string, readonly string[]>([
  ['techniqueTitle', ['name']],
  ['parameterTitle', ['name']],
  ['preconditionTitle', ['id']],
  ['operationTitle', ['index', 'name']],
  ['unsupportedPhysicalOperation', ['operation']],
])

test('timeline proposal catalog is exact, closed, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), EXPECTED_KEYS)
  for (const [key, value] of Object.entries(TEXT)) {
    assert.deepEqual(Object.keys(value), ['ja', 'en'])
    assert.equal(typeof value.ja, 'string')
    assert.equal(typeof value.en, 'string')
    assert.deepEqual(
      placeholders(value.ja),
      EXPECTED_PLACEHOLDERS.get(key) ?? [],
      `${key}.ja`,
    )
    assert.deepEqual(
      placeholders(value.en),
      EXPECTED_PLACEHOLDERS.get(key) ?? [],
      `${key}.en`,
    )
  }
  assertDeeplyFrozen(TEXT)
  assert.equal(
    createHash('sha256').update(JSON.stringify(TEXT), 'utf8').digest('hex'),
    '2c285632ab6460048cb2cf91c5b0ebf240c75baf3199ab509912463eb37105ce',
  )

  assert.deepEqual(TEXT.techniqueAndProvenance, {
    ja: '技法・出典情報',
    en: 'Technique and provenance',
  })
  assert.deepEqual(TEXT.unsupportedPhysicalOperation, {
    ja: '未対応の物理操作（{operation}）です。説明テンプレートとしてのみ追加し、自動実行しません。',
    en: 'Unsupported physical operation ({operation}). It is added only as an explanation template and is never auto-executed.',
  })
})

test('timeline proposal catalog preserves formatting and Japanese fallback', () => {
  assert.equal(
    formatLocalizedText(
      'ja',
      TEXT.operationTitle,
      { index: 3, name: '沈め折り' },
    ),
    '操作 3: 沈め折り',
  )
  assert.equal(
    formatLocalizedText(
      'en',
      TEXT.operationTitle,
      { index: 3, name: 'Sink fold' },
    ),
    'Operation 3: Sink fold',
  )
  assert.equal(
    selectLocalizedText('unsupported-locale', TEXT.descriptionOnlyStep),
    TEXT.descriptionOnlyStep.ja,
  )

  const unsupportedLocale = 'fr' as Locale
  const preview = createFoldTechniqueTimelineProposalV1(
    createInitialFoldTechniqueDocumentV1(),
    0,
    unsupportedLocale,
    0,
  )
  assert.equal(preview.ok, true)
  if (!preview.ok) return
  assert.equal(preview.techniqueName, '新しい折り技法')
  assert.equal(preview.proposal.steps[0]?.title, '技法: 新しい折り技法')
  assert.match(
    preview.proposal.steps[0]?.description ?? '',
    /^技法・出典情報\nsource-json-v1:\n/u,
  )
  assert.equal(
    preview.proposal.steps[0]?.caution,
    TEXT.descriptionOnlyProposal.ja,
  )
})

test('timeline proposal delegates fixed copy and locale choice to the catalog', () => {
  const source = readFileSync(
    new URL('../src/lib/foldTechniqueTimelineProposal.ts', import.meta.url),
    'utf8',
  )

  assert.match(source, /FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT as TEXT/u)
  assert.match(source, /\bselectLocalizedText\(/u)
  assert.match(source, /\bformatLocalizedText\(/u)
  assert.match(source, /\bisLocale\(locale\)/u)
  assert.doesNotMatch(source, /\bfunction localized\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
})

function placeholders(value: string): readonly string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}

function assertDeeplyFrozen(value: unknown): void {
  if (value === null || typeof value !== 'object') return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) assertDeeplyFrozen(child)
}
