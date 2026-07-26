import type {
  FoldImportWarningCategory,
} from './foldImportPresentationText.ts'

type StaticFoldImportWarningCategory = Exclude<
  FoldImportWarningCategory,
  'ignored_metadata'
>

export type FoldImportNativeWarningClassification =
  | Readonly<{
    category: StaticFoldImportWarningCategory
    ignoredMetadata: null
  }>
  | Readonly<{
    category: 'ignored_metadata'
    ignoredMetadata: string
  }>

type NativeWarningInput = Readonly<{
  value: string
  category: StaticFoldImportWarningCategory
}>

const FOLD_IMPORT_NATIVE_WARNING_INPUTS = Object.freeze([
  Object.freeze({
    value:
      'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。',
    category: 'missing_spec',
  }),
  Object.freeze({
    value:
      '辺の割当情報（edges_assignment）がないため、折り線種を確認・指定してください。',
    category: 'missing_assignments',
  }),
  Object.freeze({
    value:
      '外周を一意に確定できないため、取り込む用紙外周を選択してください。',
    category: 'boundary_selection',
  }),
  Object.freeze({
    value:
      '実寸へ換算できる単位情報がないため、1単位あたりのmm値を指定してください。',
    category: 'unit_needs_scale',
  }),
  Object.freeze({
    value:
      'FOLD内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。',
    category: 'invalid_title',
  }),
  Object.freeze({
    value:
      'F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。',
    category: 'flat_crease',
  }),
  Object.freeze({
    value:
      'U（未割当）は山折り・谷折り・補助線・除外のいずれかを選ぶ必要があります。',
    category: 'unassigned',
  }),
  Object.freeze({
    value:
      'J（面の結合）は同じ意味の線種がないため、補助線または除外へ変換します。',
    category: 'face_join',
  }),
]) satisfies readonly NativeWarningInput[]

const FOLD_IMPORT_IGNORED_METADATA_INPUT =
  /^取り込まないFOLD情報: ([^。\r\n]{1,500})。$/u

const FOLD_IMPORT_IGNORED_METADATA_LABELS = Object.freeze([
  '複数フレーム',
  '作成ソフト情報',
  '作者情報',
  '説明',
  'ファイル分類',
  'フレーム分類',
  'フレーム属性',
  'フレーム名',
  'フレーム継承',
  '面情報（辺から再計算）',
  '重なり順',
  '折り角度',
  '辺長メタデータ',
  'フレーム変換',
])

export function classifyFoldImportNativeWarning(
  warning: unknown,
): FoldImportNativeWarningClassification | null {
  if (typeof warning !== 'string') return null

  for (const input of FOLD_IMPORT_NATIVE_WARNING_INPUTS) {
    if (warning === input.value) {
      return Object.freeze({
        category: input.category,
        ignoredMetadata: null,
      })
    }
  }

  const ignored = FOLD_IMPORT_IGNORED_METADATA_INPUT.exec(warning)
  if (ignored === null) return null
  const metadata = ignored[1]
  if (metadata === undefined) return null
  const labels = metadata.split('、')
  if (
    labels.length === 0
    || !labels.every((label) => (
      FOLD_IMPORT_IGNORED_METADATA_LABELS.includes(label)
      || isBoundedUnknownFoldMetadataCount(label)
    ))
  ) {
    return null
  }
  return Object.freeze({
    category: 'ignored_metadata',
    ignoredMetadata: metadata,
  })
}

function isBoundedUnknownFoldMetadataCount(value: string) {
  const count = /^その他の拡張フィールド([0-9]{1,20})件$/u.exec(value)
  return count !== null
    && Number.isSafeInteger(Number(count[1]))
    && Number(count[1]) > 0
}
