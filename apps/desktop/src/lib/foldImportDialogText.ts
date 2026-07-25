import type { Locale } from './i18n.ts'

type FoldImportDialogMetadataKey =
  | 'file' | 'specification' | 'unit' | 'geometry' | 'boundary'

type FoldImportDialogTextKey =
  | 'eyebrow' | 'title' | 'close' | 'description' | 'preview'
  | 'previewUnavailable' | 'previewTruncated' | 'unspecified'
  | 'vertexUnit' | 'edgeUnit' | 'edgeUnitOne' | 'lineUnit' | 'lineUnitOne'
  | 'unitPrefix' | 'geometrySeparator' | 'listSeparator'
  | 'millimetresUnit' | 'name' | 'invalidName' | 'scale' | 'missingScale'
  | 'sourceUnit' | 'convertedScale' | 'mappingTitle'
  | 'mappingDescription' | 'boundaryTitle' | 'boundaryDescription'
  | 'boundaryAssigned' | 'boundarySelect' | 'boundaryUnavailable'
  | 'boundaryFixed' | 'assignmentSuffix' | 'select' | 'unresolved'
  | 'warningTitle' | 'acknowledge' | 'cancel' | 'importing' | 'import'

type FoldImportDialogText = Readonly<
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
    vertexUnit: '頂点',
    edgeUnit: '辺',
    edgeUnitOne: '辺',
    unitPrefix: '',
    geometrySeparator: '・',
    listSeparator: '、',
    millimetresUnit: 'mm',
    name: '作品名',
    invalidName: '制御文字を含まない120文字以内の名前が必要です。',
    scale: '1 FOLD単位の長さ',
    missingScale: '単位情報がないため、実寸への換算値を指定してください。',
    sourceUnit: '元の単位',
    convertedScale: 'から換算した値です。必要なら変更できます。',
    mappingTitle: '線種の割当',
    mappingDescription:
      'F・U・JはORIGAMI2に同じ意味の線種がないため、用途を明示的に選んでください。',
    boundaryTitle: '用紙外周',
    boundaryDescription:
      '検証済み候補から、この作品で使う一枚紙の外周を明示してください。候補外のB線は取り込みません。',
    boundaryAssigned: '元のB線が単一の有効な外周を構成しています。',
    boundarySelect: '外周候補を選択してください',
    boundaryUnavailable:
      '安全に使える外周候補がありません。このファイルは取り込めません。',
    lineUnit: '本',
    lineUnitOne: '本',
    boundaryFixed: '用紙境界（固定）',
    assignmentSuffix: 'の割当',
    select: '選択してください',
    unresolved: '未選択',
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
    vertexUnit: 'vertices',
    edgeUnit: 'edges',
    edgeUnitOne: 'edge',
    unitPrefix: ' ',
    geometrySeparator: ' · ',
    listSeparator: ', ',
    millimetresUnit: 'mm',
    name: 'Work name',
    invalidName: 'Enter a name of at most 120 characters without control characters.',
    scale: 'Length of 1 FOLD unit',
    missingScale:
      'No unit metadata is available. Enter a conversion to real-world size.',
    sourceUnit: 'source unit',
    convertedScale: ' conversion. Change it if needed.',
    mappingTitle: 'Line type mapping',
    mappingDescription:
      'F, U, and J have no directly equivalent ORIGAMI2 line type. Explicitly choose how to use them.',
    boundaryTitle: 'Paper boundary',
    boundaryDescription:
      'Explicitly select the validated outline of the single sheet. Source B lines outside the selected candidate are not imported.',
    boundaryAssigned: 'The source B lines form one valid paper boundary.',
    boundarySelect: 'Select a boundary candidate',
    boundaryUnavailable:
      'No boundary candidate can be used safely. This file cannot be imported.',
    lineUnit: 'lines',
    lineUnitOne: 'line',
    boundaryFixed: 'Paper boundary (fixed)',
    assignmentSuffix: ' mapping',
    select: 'Select a mapping',
    unresolved: 'Not selected',
    warningTitle: 'Information that will not be imported',
    acknowledge: 'I have reviewed the above and want to import the crease pattern',
    cancel: 'Cancel',
    importing: 'Importing…',
    import: 'Import',
  }),
})
