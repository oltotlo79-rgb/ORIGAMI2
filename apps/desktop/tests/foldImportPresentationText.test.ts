import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  foldAssignmentLabel,
  foldBoundaryCandidateLabel,
  foldImportPreviewFileName,
  foldImportSuggestedName,
  foldImportTargetLabel,
  foldImportWarningMessage,
  type FoldImportBoundaryCandidate,
  type FoldImportTarget,
} from '../src/lib/foldImport.ts'
import {
  classifyFoldImportNativeWarning,
} from '../src/lib/foldImportNativeWarningInput.ts'
import {
  FOLD_IMPORT_PRESENTATION_TEXT as TEXT,
} from '../src/lib/foldImportPresentationText.ts'
import type { Locale } from '../src/lib/i18n.ts'

const ROOT_KEYS = [
  'assignmentLabels',
  'targetLabels',
  'warningMessages',
  'previewFileNameFallback',
  'suggestedNameFallback',
  'boundaryCandidateLabels',
  'numberLocale',
] as const

const ASSIGNMENT_KEYS = ['B', 'M', 'V', 'F', 'U', 'C', 'J'] as const
const TARGET_KEYS = [
  'mountain',
  'valley',
  'auxiliary',
  'cut',
  'ignore',
] as const
const WARNING_KEYS = [
  'missing_spec',
  'missing_assignments',
  'boundary_selection',
  'unit_needs_scale',
  'ignored_metadata',
  'invalid_title',
  'flat_crease',
  'unassigned',
  'face_join',
  'unknown',
] as const
const BOUNDARY_KEYS = [
  'assigned_boundary',
  'inferred_outer_face',
] as const

test('FOLD import presentation catalog is exact, complete, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), ROOT_KEYS)
  assert.deepEqual(Object.keys(TEXT.assignmentLabels), ASSIGNMENT_KEYS)
  assert.deepEqual(Object.keys(TEXT.targetLabels), TARGET_KEYS)
  assert.deepEqual(Object.keys(TEXT.warningMessages), WARNING_KEYS)
  assert.deepEqual(Object.keys(TEXT.boundaryCandidateLabels), BOUNDARY_KEYS)
  assertDeepFrozen(TEXT)
  assert.equal(assertLocalizedLeaves(TEXT), 27)
  assert.equal(
    createHash('sha256')
      .update(JSON.stringify(TEXT), 'utf8')
      .digest('hex'),
    'ee85f7cc6b730dc060cf51aa0d3d2b93f38fbd03111e4dd0e272d44e691831cc',
  )
})

test('known native warnings preserve every Japanese and English output', () => {
  const cases = [
    [
      'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。',
      'The FOLD specification version is missing, so the file will be interpreted conservatively within the supported range.',
    ],
    [
      '辺の割当情報（edges_assignment）がないため、折り線種を確認・指定してください。',
      'The optional edges_assignment array is missing. Review the paper boundary and explicitly map every remaining unassigned line.',
    ],
    [
      '外周を一意に確定できないため、取り込む用紙外周を選択してください。',
      'The source assignments do not establish one valid paper boundary. Select the intended validated outer-boundary candidate.',
    ],
    [
      '実寸へ換算できる単位情報がないため、1単位あたりのmm値を指定してください。',
      'The file has no unit information that can be converted to physical size. Enter the millimetres per FOLD unit.',
    ],
    [
      'FOLD内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。',
      'The title in the FOLD file does not meet the work-name requirements, so the default name will be used.',
    ],
    [
      'F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。',
      'F (flat crease) has no equivalent line type and must be converted to an auxiliary line or excluded.',
    ],
    [
      'U（未割当）は山折り・谷折り・補助線・除外のいずれかを選ぶ必要があります。',
      'U (unassigned) must be mapped to a mountain fold, valley fold, auxiliary line, or exclusion.',
    ],
    [
      'J（面の結合）は同じ意味の線種がないため、補助線または除外へ変換します。',
      'J (face join) has no equivalent line type and must be converted to an auxiliary line or excluded.',
    ],
    [
      '取り込まないFOLD情報: ファイル分類、その他の拡張フィールド2件。',
      'Some FOLD metadata will not be imported.',
    ],
  ] as const

  for (const [nativeWarning, english] of cases) {
    assert.equal(foldImportWarningMessage(nativeWarning, 'ja'), nativeWarning)
    assert.equal(foldImportWarningMessage(nativeWarning, 'en'), english)
  }
})

test('unknown locale values consistently fail closed to Japanese presentation', () => {
  const unknownLocale = 'fr' as Locale
  const ignoredMetadata =
    '取り込まないFOLD情報: ファイル分類、その他の拡張フィールド2件。'
  assert.equal(foldAssignmentLabel('B', unknownLocale), 'B · 用紙境界')
  assert.equal(foldImportTargetLabel('auxiliary', unknownLocale), '補助線')
  assert.equal(
    foldImportWarningMessage(
      'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。',
      unknownLocale,
    ),
    'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。',
  )
  assert.equal(
    foldImportWarningMessage(ignoredMetadata, unknownLocale),
    ignoredMetadata,
  )
  assert.equal(
    foldImportWarningMessage('private diagnostic', unknownLocale),
    '取り込まれないFOLD情報があります。',
  )
  assert.equal(
    foldImportPreviewFileName('Selected FOLD file', unknownLocale),
    '選択したFOLDファイル',
  )
  assert.equal(
    foldImportSuggestedName('FOLD import', unknownLocale),
    'FOLDインポート',
  )
  assert.equal(
    foldBoundaryCandidateLabel(
      boundaryCandidate('inferred_outer_face', 1_233, 1_234),
      unknownLocale,
    ),
    '検証済み外周候補 1,234（1,234辺）',
  )
  assert.equal(
    foldBoundaryCandidateLabel(
      boundaryCandidate('assigned_boundary', 999, 1_234),
      unknownLocale,
    ),
    '元のB線による外周（1,234辺）',
  )

  const hostileLocale = new Proxy(Object.create(null) as object, {
    get() {
      throw new Error('must not read unknown locale properties')
    },
    getOwnPropertyDescriptor() {
      throw new Error('must not inspect unknown locale properties')
    },
  }) as Locale
  assert.equal(foldAssignmentLabel('B', hostileLocale), 'B · 用紙境界')
  assert.equal(foldImportTargetLabel('auxiliary', hostileLocale), '補助線')
  assert.equal(
    foldImportWarningMessage('private diagnostic', hostileLocale),
    '取り込まれないFOLD情報があります。',
  )
  assert.equal(
    foldImportWarningMessage(ignoredMetadata, hostileLocale),
    ignoredMetadata,
  )
  assert.equal(
    foldImportPreviewFileName('Selected FOLD file', hostileLocale),
    '選択したFOLDファイル',
  )
  assert.equal(
    foldImportSuggestedName('FOLD import', hostileLocale),
    'FOLDインポート',
  )
  assert.equal(
    foldBoundaryCandidateLabel(
      boundaryCandidate('assigned_boundary', 0, 1_234),
      hostileLocale,
    ),
    '元のB線による外周（1,234辺）',
  )
})

test('unknown warning and file data cannot be reflected into presentation output', () => {
  const privatePath = String.raw`C:\Users\alice\秘密\private.fold`
  const malformedMetadata = `取り込まないFOLD情報: ${privatePath}。`
  const hostile = new Proxy(Object.create(null) as object, {
    get() {
      throw new Error('must not read untrusted properties')
    },
    ownKeys() {
      throw new Error('must not enumerate untrusted properties')
    },
  })

  for (const locale of ['ja', 'en'] as const) {
    const expected = locale === 'ja'
      ? '取り込まれないFOLD情報があります。'
      : 'Some FOLD information will not be imported.'
    for (const warning of [
      privatePath,
      malformedMetadata,
      '取り込まないFOLD情報: ファイル分類\nprivate。',
      '取り込まないFOLD情報: ファイル分類\u202e。',
      '取り込まないFOLD情報: ファイル分類\u200d。',
      '取り込まないFOLD情報: ファイル分類似。',
      hostile,
      Symbol(privatePath),
      null,
    ]) {
      const output = foldImportWarningMessage(warning, locale)
      assert.equal(output, expected)
      assert.doesNotMatch(output, /alice|private|秘密/u)
    }
    const fileName = foldImportPreviewFileName(privatePath, locale)
    assert.equal(
      fileName,
      locale === 'ja' ? '選択したFOLDファイル' : 'Selected FOLD file',
    )
    assert.doesNotMatch(fileName, /alice|private|秘密/u)
  }
  assert.equal(classifyFoldImportNativeWarning(malformedMetadata), null)
  assert.equal(
    foldImportTargetLabel('future-target' as FoldImportTarget, 'en'),
    'future-target',
  )
})

test('native ignored-metadata classification preserves its exact bounded allowlist', () => {
  const repeatedKnownMetadata =
    '取り込まないFOLD情報: ファイル分類、ファイル分類。'
  assert.deepEqual(
    classifyFoldImportNativeWarning(repeatedKnownMetadata),
    {
      category: 'ignored_metadata',
      ignoredMetadata: 'ファイル分類、ファイル分類',
    },
  )
  assert.equal(
    foldImportWarningMessage(repeatedKnownMetadata, 'ja'),
    repeatedKnownMetadata,
  )
  assert.equal(
    foldImportWarningMessage(repeatedKnownMetadata, 'en'),
    'Some FOLD metadata will not be imported.',
  )

  const invalidWarnings = [
    `取り込まないFOLD情報: ${'説'.repeat(501)}。`,
    '取り込まないFOLD情報: その他の拡張フィールド0件。',
    '取り込まないFOLD情報: その他の拡張フィールド9007199254740992件。',
    '取り込まないFOLD情報: その他の拡張フィールド123456789012345678901件。',
    '取り込まないFOLD情報: ファイル分類、不明な分類。',
  ]
  for (const warning of invalidWarnings) {
    assert.equal(classifyFoldImportNativeWarning(warning), null)
    assert.equal(
      foldImportWarningMessage(warning, 'ja'),
      '取り込まれないFOLD情報があります。',
    )
    assert.equal(
      foldImportWarningMessage(warning, 'en'),
      'Some FOLD information will not be imported.',
    )
  }
})

test('foldImport delegates all display literals and locale selection to separated modules', async () => {
  const [consumerSource, nativeInputSource, presentationSource] =
    await Promise.all([
      readFile(
        new URL('../src/lib/foldImport.ts', import.meta.url),
        'utf8',
      ),
      readFile(
        new URL('../src/lib/foldImportNativeWarningInput.ts', import.meta.url),
        'utf8',
      ),
      readFile(
        new URL('../src/lib/foldImportPresentationText.ts', import.meta.url),
        'utf8',
      ),
    ])

  assert.match(
    consumerSource,
    /FOLD_IMPORT_PRESENTATION_TEXT as PRESENTATION_TEXT/u,
  )
  assert.match(consumerSource, /classifyFoldImportNativeWarning\(warning\)/u)
  assert.match(consumerSource, /formatFoldImportWarningPresentation\(/u)
  assert.match(
    consumerSource,
    /formatFoldImportBoundaryCandidatePresentation\(/u,
  )
  assert.doesNotMatch(consumerSource, /\blocale\s*(?:===|!==)/u)
  assert.doesNotMatch(consumerSource, /\.toLocaleString\s*\(/u)
  assert.doesNotMatch(consumerSource, /\breturn\s+warning\b|warning\s+as\s+string/u)
  assert.doesNotMatch(consumerSource, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(
    consumerSource,
    /Some FOLD|Selected FOLD|Mountain fold/u,
  )

  assert.match(nativeInputSource, /FOLD_IMPORT_NATIVE_WARNING_INPUTS/u)
  assert.match(
    nativeInputSource,
    /\^取り込まないFOLD情報: \(\[\^。\\r\\n\]\{1,500\}\)。\$/u,
  )
  assert.doesNotMatch(nativeInputSource, /FOLD_IMPORT_PRESENTATION_TEXT/u)
  assert.doesNotMatch(
    nativeInputSource,
    /Some FOLD|Selected FOLD|Mountain fold/u,
  )
  assert.doesNotMatch(
    presentationSource,
    /classifyFoldImportNativeWarning|FOLD_IMPORT_NATIVE_WARNING_INPUTS/u,
  )
  assert.doesNotMatch(presentationSource, /foldImportDialogText/u)
  for (const dialogOnlyKey of [
    'eyebrow',
    'title',
    'close',
    'description',
    'mappingTitle',
    'warningTitle',
    'acknowledge',
    'cancel',
    'importing',
  ]) {
    assert.equal(
      Object.hasOwn(TEXT, dialogOnlyKey),
      false,
      dialogOnlyKey,
    )
  }
})

function boundaryCandidate(
  source: FoldImportBoundaryCandidate['source'],
  id: number,
  edgeCount: number,
): FoldImportBoundaryCandidate {
  return {
    id,
    source,
    edge_indices: Array.from({ length: edgeCount }, (_, index) => index),
  }
}

function assertDeepFrozen(value: unknown, seen = new Set<object>()): void {
  if (!value || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const nested of Object.values(value)) {
    assertDeepFrozen(nested, seen)
  }
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}

function assertLocalizedLeaves(value: unknown): number {
  assert.ok(value && typeof value === 'object')
  const record = value as Record<string, unknown>
  const keys = Object.keys(record)
  if (keys.length === 2 && keys[0] === 'ja' && keys[1] === 'en') {
    assert.equal(typeof record.ja, 'string')
    assert.equal(typeof record.en, 'string')
    assert.notEqual(record.ja, '')
    assert.notEqual(record.en, '')
    assert.deepEqual(
      placeholders(record.ja as string),
      placeholders(record.en as string),
    )
    return 1
  }
  return Object.values(record).reduce(
    (count, nested) => count + assertLocalizedLeaves(nested),
    0,
  )
}
