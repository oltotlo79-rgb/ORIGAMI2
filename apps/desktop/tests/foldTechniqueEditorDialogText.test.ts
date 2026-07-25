import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FOLD_TECHNIQUE_EDITOR_DIALOG_TEXT as TEXT,
} from '../src/lib/foldTechniqueEditorDialogText.ts'

const EXPECTED_KEYS = [
  'eyebrow',
  'title',
  'close',
  'description',
  'inertTitle',
  'inert',
  'invalidInitial',
  'packageTitle',
  'packageId',
  'authors',
  'author',
  'addAuthor',
  'newAuthor',
  'removeAuthor',
  'source',
  'sourceKinds',
  'citation',
  'license',
  'techniqueTitle',
  'techniquePosition',
  'techniqueSelection',
  'techniqueId',
  'techniqueVersion',
  'nameJa',
  'nameEn',
  'descriptionJa',
  'descriptionEn',
  'preserved',
  'parameters',
  'preconditions',
  'operationsTitle',
  'operationsDescription',
  'addOperation',
  'operation',
  'operationId',
  'operationNameJa',
  'operationNameEn',
  'action',
  'actionLabels',
  'instructionJa',
  'instructionEn',
  'sinkKind',
  'openSink',
  'closedSink',
  'support',
  'declarative',
  'unsupported',
  'moveUp',
  'moveDown',
  'removeOperation',
  'invalid',
  'validation',
  'saveFailed',
  'noChanges',
  'cancel',
  'saving',
  'confirm',
]

const EXPECTED_TEXT = [
  ['title', '説明テンプレートを編集', 'Edit the instruction template'],
  ['close', '閉じる', 'Close'],
  [
    'description',
    '技法名と順序付き手順を、共有可能なV1宣言データとして編集します。この画面から折り操作やプロジェクト変更は実行しません。',
    'Edit the technique name and ordered steps as shareable declarative V1 data. This dialog never performs folds or changes a project.',
  ],
  ['inertTitle', '安全上の重要事項', 'Important safety boundary'],
  [
    'inert',
    '中割り、かぶせ、沈め、層を選ぶ操作は説明metadataとして保存されるだけで、自動実行されません。必要な未対応物理操作も文書内へ明示されます。',
    'Inside reverse, outside reverse, sink, and layer-selective actions are stored only as descriptive metadata and are never executed automatically. Their unsupported physical operation is recorded explicitly.',
  ],
  [
    'invalidInitial',
    '編集元の技法データが厳密なV1契約を満たしていないため、開けません。',
    'The source technique data does not satisfy the strict V1 contract and cannot be opened.',
  ],
  ['packageTitle', '共有パッケージ', 'Shared package'],
  ['packageId', 'パッケージID', 'Package ID'],
  ['authors', '作成者', 'Authors'],
  ['author', '作成者名', 'Author name'],
  ['addAuthor', '作成者を追加', 'Add author'],
  ['newAuthor', 'New author', 'New author'],
  ['removeAuthor', 'この作成者を削除', 'Remove this author'],
  ['source', '出典区分', 'Source provenance'],
  [
    'citation',
    '出典の記述（参照されないplain text）',
    'Citation text (inert plain text; never fetched)',
  ],
  ['license', 'SPDXライセンスID', 'SPDX license ID'],
  ['techniqueTitle', '技法', 'Technique'],
  ['techniquePosition', '編集中の技法', 'Technique being edited'],
  ['techniqueSelection', '編集する技法', 'Technique to edit'],
  ['techniqueId', '技法ID', 'Technique ID'],
  ['techniqueVersion', '技法の改訂番号', 'Technique revision'],
  ['nameJa', '技法名（日本語）', 'Technique name (Japanese)'],
  ['nameEn', '技法名（英語）', 'Technique name (English)'],
  ['descriptionJa', '説明（日本語）', 'Description (Japanese)'],
  ['descriptionEn', '説明（英語）', 'Description (English)'],
  [
    'preserved',
    '既存のparameterとpreconditionは変更せず保持します。この初期UIでは順序付きoperationを編集します。',
    'Existing parameters and preconditions are preserved unchanged. This initial UI edits the ordered operations.',
  ],
  ['parameters', 'parameter', 'parameters'],
  ['preconditions', 'precondition', 'preconditions'],
  ['operationsTitle', '順序付き手順', 'Ordered steps'],
  [
    'operationsDescription',
    '2〜256件。上下の順序は共有ファイルでもそのまま保持されます。',
    '2–256 steps. Their order is preserved in the shared file.',
  ],
  ['addOperation', '説明手順を追加', 'Add instruction step'],
  ['operation', '手順', 'Step'],
  ['operationId', '手順ID', 'Step ID'],
  ['operationNameJa', '手順名（日本語）', 'Step name (Japanese)'],
  ['operationNameEn', '手順名（英語）', 'Step name (English)'],
  ['action', '動作区分', 'Action kind'],
  ['instructionJa', '案内文（日本語）', 'Instruction (Japanese)'],
  ['instructionEn', '案内文（英語）', 'Instruction (English)'],
  ['sinkKind', '沈め方', 'Sink kind'],
  ['openSink', 'オープンシンク', 'Open sink'],
  ['closedSink', 'クローズドシンク', 'Closed sink'],
  ['support', '実行support', 'Execution support'],
  [
    'declarative',
    '宣言のみ。自動実行の許可や物理的な成立証明ではありません。',
    'Declarative only. This is not execution permission or proof of physical validity.',
  ],
  [
    'unsupported',
    '未対応物理操作として保存します。現在のsimulatorは自動実行しません。',
    'Stored as an unsupported physical operation. The current simulator does not execute it.',
  ],
  ['moveUp', '上へ移動', 'Move up'],
  ['moveDown', '下へ移動', 'Move down'],
  ['removeOperation', 'この手順を削除', 'Remove this step'],
  ['invalid', '入力内容を確認してください。', 'Review the entered values.'],
  [
    'saveFailed',
    '技法データを確定できませんでした。もう一度お試しください。',
    'The technique data could not be committed. Try again.',
  ],
  ['noChanges', '変更はありません。', 'No changes.'],
  ['cancel', 'キャンセル', 'Cancel'],
  ['saving', '処理中…', 'Processing…'],
] as const

const EXPECTED_NESTED = {
  eyebrow: [
    ['create', '名前付き折り技法の作成', 'Create named fold technique'],
    ['edit', '名前付き折り技法の編集', 'Edit named fold technique'],
  ],
  sourceKinds: [
    ['user_authored', '利用者が新規作成', 'User authored'],
    ['adapted', '既存資料をもとに改作', 'Adapted from a source'],
    ['published_reference', '公開資料を参照', 'Published reference'],
  ],
  actionLabels: [
    ['instruction_cue', '文章による案内', 'Instruction cue'],
    [
      'straight_line_stacked_fold',
      '一直線の折り重ね',
      'Straight-line stacked fold',
    ],
    ['inside_reverse_fold', '中割り折り', 'Inside reverse fold'],
    ['outside_reverse_fold', 'かぶせ折り', 'Outside reverse fold'],
    ['sink_fold', '沈め折り', 'Sink fold'],
    [
      'layer_selective_manipulation',
      '層を選ぶ操作',
      'Layer-selective manipulation',
    ],
  ],
  validation: [
    [
      'invalid_structure',
      '文書構造に認識できない値があります。',
      'The document contains an unrecognized structure.',
    ],
    [
      'unsupported_schema',
      '対応していないschemaです。',
      'The schema is not supported.',
    ],
    [
      'unsupported_version',
      '対応していないfile versionです。',
      'The file version is not supported.',
    ],
    [
      'resource_limit',
      '件数または構造の固定上限を超えています。',
      'A fixed collection or structure limit was exceeded.',
    ],
    [
      'invalid_field',
      'ID、文字、locale、数値範囲のいずれかが不正です。',
      'An ID, text, locale, or numeric range is invalid.',
    ],
    [
      'duplicate_identifier',
      '同じID、locale、作成者または参照が重複しています。',
      'An ID, locale, author, or reference is duplicated.',
    ],
    [
      'missing_reference',
      'parameterまたはpreconditionへの参照が見つかりません。',
      'A parameter or precondition reference is missing.',
    ],
    [
      'parameter_type_mismatch',
      'parameterの型、範囲または比較が一致しません。',
      'A parameter type, range, or comparison does not match.',
    ],
    [
      'inconsistent_execution_support',
      '動作、必要capability、未対応物理操作metadataが一致しません。',
      'The action, required capability, and physical-support metadata disagree.',
    ],
    [
      'encoded_size_limit',
      '保存後のJSONが1 MiB上限を超えます。',
      'The encoded JSON exceeds the 1 MiB limit.',
    ],
  ],
  confirm: [
    ['create', '技法を作成', 'Create technique'],
    ['edit', '変更を確定', 'Apply changes'],
  ],
} as const

test('fold technique editor dialog catalog preserves exact copy and shape', () => {
  assert.deepEqual(Object.keys(TEXT), ['ja', 'en'])
  assert.deepEqual(Object.keys(TEXT.ja), EXPECTED_KEYS)
  assert.deepEqual(Object.keys(TEXT.en), EXPECTED_KEYS)
  assert.deepEqual(
    EXPECTED_TEXT.map(([key]) => [key, TEXT.ja[key], TEXT.en[key]]),
    EXPECTED_TEXT,
  )

  for (const [group, expected] of Object.entries(EXPECTED_NESTED)) {
    const key = group as keyof typeof EXPECTED_NESTED
    const ja = TEXT.ja[key] as Readonly<Record<string, string>>
    const en = TEXT.en[key] as Readonly<Record<string, string>>
    assert.deepEqual(Object.keys(ja), expected.map(([entry]) => entry))
    assert.deepEqual(Object.keys(en), Object.keys(ja))
    assert.deepEqual(
      expected.map(([entry]) => [
        entry,
        ja[entry],
        en[entry],
      ]),
      expected,
    )
  }
  assertLocaleShape(TEXT.ja, TEXT.en)
})

test('fold technique editor dialog catalog is deeply frozen and placeholder-free', () => {
  assertDeeplyFrozen(TEXT)
  assert.deepEqual(collectPlaceholders(TEXT), [])
})

function assertLocaleShape(
  left: Readonly<Record<string, unknown>>,
  right: Readonly<Record<string, unknown>>,
) {
  assert.deepEqual(Object.keys(left), Object.keys(right))
  for (const key of Object.keys(left)) {
    const leftValue = left[key]
    const rightValue = right[key]
    assert.equal(typeof leftValue, typeof rightValue, key)
    if (
      typeof leftValue === 'object'
      && leftValue !== null
      && typeof rightValue === 'object'
      && rightValue !== null
    ) {
      assertLocaleShape(
        leftValue as Readonly<Record<string, unknown>>,
        rightValue as Readonly<Record<string, unknown>>,
      )
    }
  }
}

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) assertDeeplyFrozen(child)
}

function collectPlaceholders(
  value: unknown,
  path: readonly string[] = [],
): Array<readonly [string, string[]]> {
  if (typeof value === 'string') {
    const placeholders = [
      ...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu),
    ].map((match) => match[1])
    return placeholders.length > 0
      ? [[path.join('.'), placeholders]]
      : []
  }
  if (typeof value !== 'object' || value === null) return []
  return Object.entries(value).flatMap(([key, child]) =>
    collectPlaceholders(child, [...path, key]))
}
