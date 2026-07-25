import { formatMessage, type Locale } from './i18n.ts'

type FoldImportDialogMetadataKey =
  | 'file' | 'specification' | 'unit' | 'geometry' | 'boundary'

type FoldImportDialogTextKey =
  | 'eyebrow' | 'title' | 'close' | 'closeGlyph' | 'description' | 'preview'
  | 'previewUnavailable' | 'previewTruncated' | 'unspecified'
  | 'geometryValue' | 'boundaryEdgeCount' | 'boundaryEdgeCountOne'
  | 'lineCount' | 'lineCountOne' | 'assignmentCountSeparator'
  | 'listSeparator' | 'millimetresUnit' | 'name' | 'invalidName'
  | 'scale' | 'missingScale' | 'sourceUnit' | 'convertedScaleValue'
  | 'mappingTitle'
  | 'mappingDescription' | 'boundaryTitle' | 'boundaryDescription'
  | 'boundaryAssignedValue' | 'boundarySelect' | 'boundaryUnavailable'
  | 'boundaryFixed' | 'assignmentLabel' | 'select' | 'unresolvedValue'
  | 'warningTitle' | 'acknowledge' | 'cancel' | 'importing' | 'import'

export type FoldImportDialogText = Readonly<
  Record<FoldImportDialogTextKey, string>
  & {
    metadata: Readonly<Record<FoldImportDialogMetadataKey, string>>
  }
>

export const FOLD_IMPORT_DIALOG_TEXT: Readonly<
  Record<Locale, FoldImportDialogText>
> = Object.freeze({
  ja: Object.freeze({
    eyebrow: 'FOLD 1.0–1.2 取込',
    title: '線種と縮尺を確認',
    close: '閉じる',
    closeGlyph: '×',
    description:
      '元のFOLDファイルは変更しません。確認後、編集可能な未保存プロジェクトとして取り込みます。',
    preview: '取り込む展開図のプレビュー',
    previewUnavailable: 'プレビューを表示できません。',
    previewTruncated: '表示用に一部の線だけを描画しています。',
    metadata: Object.freeze({
      file: 'ファイル',
      specification: '仕様',
      unit: '単位',
      geometry: '形状',
      boundary: '境界',
    }),
    unspecified: '記載なし',
    geometryValue: '{vertices}頂点・{edges}辺',
    boundaryEdgeCount: '{count}辺',
    boundaryEdgeCountOne: '{count}辺',
    lineCount: '{count}本',
    lineCountOne: '{count}本',
    assignmentCountSeparator: ' ',
    listSeparator: '、',
    millimetresUnit: 'mm',
    name: '作品名',
    invalidName: '制御文字を含まない120文字以内の名前が必要です。',
    scale: '1 FOLD単位の長さ',
    missingScale: '単位情報がないため、実寸への換算値を指定してください。',
    sourceUnit: '元の単位',
    convertedScaleValue: '{sourceUnit}から換算した値です。必要なら変更できます。',
    mappingTitle: '線種の割当',
    mappingDescription:
      'F・U・JはORIGAMI2に同じ意味の線種がないため、用途を明示的に選んでください。',
    boundaryTitle: '用紙外周',
    boundaryDescription:
      '検証済み候補から、この作品で使う一枚紙の外周を明示してください。候補外のB線は取り込みません。',
    boundaryAssignedValue:
      '元のB線が単一の有効な外周を構成しています。 {candidate}',
    boundarySelect: '外周候補を選択してください',
    boundaryUnavailable:
      '安全に使える外周候補がありません。このファイルは取り込めません。',
    boundaryFixed: '用紙境界（固定）',
    assignmentLabel: '{assignment}の割当',
    select: '選択してください',
    unresolvedValue: '未選択: {assignments}',
    warningTitle: '取り込まれない情報',
    acknowledge: '上記を確認し、展開図として取り込む',
    cancel: 'キャンセル',
    importing: '取込中…',
    import: '取り込む',
  }),
  en: Object.freeze({
    eyebrow: 'Import FOLD 1.0–1.2',
    title: 'Review line types and scale',
    close: 'Close',
    closeGlyph: '×',
    description:
      'The source FOLD file is not modified. After review, it is imported as an editable unsaved project.',
    preview: 'Preview of the crease pattern to import',
    previewUnavailable: 'The preview is unavailable.',
    previewTruncated: 'Only a subset of lines is drawn in this preview.',
    metadata: Object.freeze({
      file: 'File',
      specification: 'Specification',
      unit: 'Unit',
      geometry: 'Geometry',
      boundary: 'Boundary',
    }),
    unspecified: 'Not specified',
    geometryValue: '{vertices} vertices · {edges} edges',
    boundaryEdgeCount: '{count} edges',
    boundaryEdgeCountOne: '{count} edge',
    lineCount: '{count} lines',
    lineCountOne: '{count} line',
    assignmentCountSeparator: ' ',
    listSeparator: ', ',
    millimetresUnit: 'mm',
    name: 'Work name',
    invalidName: 'Enter a name of at most 120 characters without control characters.',
    scale: 'Length of 1 FOLD unit',
    missingScale:
      'No unit metadata is available. Enter a conversion to real-world size.',
    sourceUnit: 'source unit',
    convertedScaleValue: '{sourceUnit} conversion. Change it if needed.',
    mappingTitle: 'Line type mapping',
    mappingDescription:
      'F, U, and J have no directly equivalent ORIGAMI2 line type. Explicitly choose how to use them.',
    boundaryTitle: 'Paper boundary',
    boundaryDescription:
      'Explicitly select the validated outline of the single sheet. Source B lines outside the selected candidate are not imported.',
    boundaryAssignedValue:
      'The source B lines form one valid paper boundary. {candidate}',
    boundarySelect: 'Select a boundary candidate',
    boundaryUnavailable:
      'No boundary candidate can be used safely. This file cannot be imported.',
    boundaryFixed: 'Paper boundary (fixed)',
    assignmentLabel: '{assignment} mapping',
    select: 'Select a mapping',
    unresolvedValue: 'Not selected: {assignments}',
    warningTitle: 'Information that will not be imported',
    acknowledge: 'I have reviewed the above and want to import the crease pattern',
    cancel: 'Cancel',
    importing: 'Importing…',
    import: 'Import',
  }),
})

const FOLD_IMPORT_NUMBER_LOCALES = Object.freeze({
  ja: 'ja-JP',
  en: 'en-US',
}) satisfies Readonly<Record<Locale, string>>

function formatFoldImportInteger(value: number, locale: Locale) {
  return value.toLocaleString(FOLD_IMPORT_NUMBER_LOCALES[locale])
}

export function formatFoldImportGeometry(
  vertexCount: number,
  edgeCount: number,
  locale: Locale,
) {
  return formatMessage(FOLD_IMPORT_DIALOG_TEXT[locale].geometryValue, {
    vertices: formatFoldImportInteger(vertexCount, locale),
    edges: formatFoldImportInteger(edgeCount, locale),
  })
}

export function formatFoldImportBoundaryEdgeCount(
  count: number,
  locale: Locale,
) {
  const copy = FOLD_IMPORT_DIALOG_TEXT[locale]
  return formatMessage(
    count === 1 ? copy.boundaryEdgeCountOne : copy.boundaryEdgeCount,
    { count: formatFoldImportInteger(count, locale) },
  )
}

export function formatFoldImportLineCount(
  count: number,
  locale: Locale,
) {
  const copy = FOLD_IMPORT_DIALOG_TEXT[locale]
  return formatMessage(
    count === 1 ? copy.lineCountOne : copy.lineCount,
    { count: formatFoldImportInteger(count, locale) },
  )
}

export function formatFoldImportConvertedScale(
  sourceUnit: string | null,
  locale: Locale,
) {
  const copy = FOLD_IMPORT_DIALOG_TEXT[locale]
  return formatMessage(copy.convertedScaleValue, {
    sourceUnit: sourceUnit ?? copy.sourceUnit,
  })
}

export function formatFoldImportBoundaryAssigned(
  candidate: string,
  locale: Locale,
) {
  return formatMessage(FOLD_IMPORT_DIALOG_TEXT[locale].boundaryAssignedValue, {
    candidate,
  })
}

export function formatFoldImportAssignmentLabel(
  assignment: string,
  locale: Locale,
) {
  return formatMessage(FOLD_IMPORT_DIALOG_TEXT[locale].assignmentLabel, {
    assignment,
  })
}

export function formatFoldImportUnresolvedAssignments(
  assignments: readonly string[],
  locale: Locale,
) {
  const copy = FOLD_IMPORT_DIALOG_TEXT[locale]
  return formatMessage(copy.unresolvedValue, {
    assignments: assignments.join(copy.listSeparator),
  })
}
