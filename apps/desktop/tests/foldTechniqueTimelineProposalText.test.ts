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
  'timelineCapacityError',
  'proposalSizeError',
  'proposalBuildError',
  'staleProposalError',
  'appendFailed',
  'appendSucceeded',
] as const

const EXPECTED_PLACEHOLDERS = new Map<string, readonly string[]>([
  ['techniqueTitle', ['name']],
  ['parameterTitle', ['name']],
  ['preconditionTitle', ['id']],
  ['operationTitle', ['index', 'name']],
  ['unsupportedPhysicalOperation', ['operation']],
  ['timelineCapacityError', ['available', 'required']],
  ['appendSucceeded', ['technique']],
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
    '33a65c325fd13b9b2120edc44b41fd0232ce24e2121a7b08e1a81a31bc394ba3',
  )

  assert.deepEqual(TEXT.techniqueAndProvenance, {
    ja: '技法・出典情報',
    en: 'Technique and provenance',
  })
  assert.deepEqual(TEXT.unsupportedPhysicalOperation, {
    ja: '未対応の物理操作（{operation}）です。説明テンプレートとしてのみ追加し、自動実行しません。',
    en: 'Unsupported physical operation ({operation}). It is added only as an explanation template and is never auto-executed.',
  })
  assert.deepEqual(TEXT.timelineCapacityError, {
    ja: '折り手順の上限内に追加できません（必要 {required}、空き {available}）。',
    en: 'The proposal does not fit in the instruction limit (requires {required}, {available} available).',
  })
  assert.deepEqual(TEXT.proposalSizeError, {
    ja: '折り技法の説明案が安全な入力サイズ上限を超えています。',
    en: 'The fold-technique proposal exceeds the safe input-size limit.',
  })
  assert.deepEqual(TEXT.proposalBuildError, {
    ja: '選択中の折り技法から説明案を作成できませんでした。',
    en: 'Could not build a proposal from the selected fold technique.',
  })
  assert.deepEqual(TEXT.staleProposalError, {
    ja: 'プロジェクトまたは選択中の技法が変わりました。案を閉じて作り直してください。',
    en: 'The project or selected technique changed. Close and rebuild the proposal.',
  })
  assert.deepEqual(TEXT.appendFailed, {
    ja: '説明ステップを追加できませんでした。プロジェクトは変更されていません。',
    en: 'Could not append the description steps. The project was not changed.',
  })
  assert.deepEqual(TEXT.appendSucceeded, {
    ja: '「{technique}」から説明専用の折り手順を追加しました。1回のUndoで戻せます。',
    en: 'Added description-only steps from “{technique}”. One Undo removes the complete addition.',
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

test('timeline proposal hook delegates every status message to the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/lib/useFoldTechniqueTimelineProposal.ts',
      import.meta.url,
    ),
    'utf8',
  )

  assert.match(source, /FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT as TEXT/u)
  for (const key of [
    'timelineCapacityError',
    'proposalSizeError',
    'proposalBuildError',
    'staleProposalError',
    'appendFailed',
    'appendSucceeded',
  ]) {
    assert.match(source, new RegExp(`\\bTEXT\\.${key}\\b`, 'u'), key)
  }
  assert.doesNotMatch(source, /\b(?:ja|en)\s*:/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(
    source,
    /The proposal does not fit|Could not build a proposal|Could not append the description steps|One Undo removes/u,
  )
  assert.match(
    source,
    /message\(TEXT\.timelineCapacityError,\s*\{\s*required: proposal\.requiredSteps,\s*available: proposal\.availableSteps,\s*\}\)/u,
  )
  assert.match(
    source,
    /message\(\s*TEXT\.appendSucceeded,\s*\{ technique: pending\.preview\.techniqueName \},\s*\)/u,
  )
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
