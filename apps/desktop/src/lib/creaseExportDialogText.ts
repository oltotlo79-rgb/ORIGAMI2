import type {
  CreasePatternExportAssignmentCounts,
  CreasePatternExportFormat,
} from './creaseExport.ts'
import {
  DEFAULT_LOCALE,
  formatMessage,
  isLocale,
  type Locale,
} from './i18n.ts'

type CreaseExportAssignmentKey =
  | 'boundary'
  | 'mountain'
  | 'valley'
  | 'auxiliary'
  | 'cut'

type CreaseExportMetadataKey =
  | 'format'
  | 'specification'
  | 'suggestedName'
  | 'size'
  | 'geometry'
  | 'revision'
  | 'cuts'

type CreaseExportWarningMessageKey =
  | 'paperAppearance'
  | 'editorState'
  | 'poseCamera'
  | 'pdfStructure'
  | 'pdfPrintScale'
  | 'dxfLayers'
  | 'dxfName'
  | 'instructionSteps'
  | 'instructionStepOne'
  | 'cutPermission'
  | 'fallback'

type CreaseExportByteUnitKey = 'byte' | 'kilobyte' | 'megabyte'

export type CreaseExportDialogCopy = Readonly<{
  eyebrow: string
  title: string
  close: string
  description: string
  format: string
  formatOption: string
  formatLabels: Readonly<Record<CreasePatternExportFormat, string>>
  formatOptionLabels: Readonly<Record<CreasePatternExportFormat, string>>
  optionDetails: Readonly<Record<CreasePatternExportFormat, string>>
  numberLocale: string
  unknownSize: string
  byteUnits: Readonly<Record<CreaseExportByteUnitKey, string>>
  generating: string
  rebuild: string
  retry: string
  metadata: Readonly<Record<CreaseExportMetadataKey, string>>
  geometryValue: string
  revisionValue: string
  includes: string
  excludes: string
  lines: string
  lineCount: string
  lineCountOne: string
  assignmentLabels: Readonly<Record<CreaseExportAssignmentKey, string>>
  lossTitle: string
  acknowledge: string
  lossless: string
  cancel: string
  processing: string
  save: string
  formatSummaries: Readonly<Record<CreasePatternExportFormat, string>>
  warningMessages: Readonly<Record<CreaseExportWarningMessageKey, string>>
}>

const JA_CREASE_EXPORT_COPY = Object.freeze({
  eyebrow: '展開図の書き出し',
  title: '形式と情報損失を確認',
  close: '閉じる',
  description:
    '現在の編集リビジョンから展開図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
  format: '出力形式',
  formatOption: '{label} — {detail}',
  formatLabels: Object.freeze({
    fold: 'FOLD 1.2',
    svg: 'SVG',
    pdf: 'PDF 1.7',
    dxf: 'DXF（AutoCAD 2007）',
  }),
  formatOptionLabels: Object.freeze({
    fold: 'FOLD 1.2',
    svg: 'SVG',
    pdf: 'PDF 1.7',
    dxf: 'DXF',
  }),
  optionDetails: Object.freeze({
    fold: '他の折り紙ソフトと交換しやすいJSON形式',
    svg: '印刷・作図ソフトで扱いやすい静的な線図',
    pdf: '実寸1:1・四辺10 mm余白の白黒ベクター印刷',
    dxf: 'AutoCAD 2007・mm・5意味レイヤーのCAD交換',
  }),
  numberLocale: 'ja-JP',
  unknownSize: '不明',
  byteUnits: Object.freeze({
    byte: 'B',
    kilobyte: 'KB',
    megabyte: 'MB',
  }),
  generating: '{format}データを検証・生成しています…',
  rebuild: '現在の編集内容から作り直す',
  retry: '同じ形式で再試行',
  metadata: Object.freeze({
    format: '形式',
    specification: '出力仕様',
    suggestedName: '保存名候補',
    size: 'サイズ',
    geometry: '形状',
    revision: '固定元',
    cuts: '切断線',
  }),
  geometryValue: '{vertices}頂点・{edges}辺',
  revisionValue: 'revision {revision}',
  includes: '含む',
  excludes: '含まない',
  lines: '書き出す線',
  lineCount: '{count}本',
  lineCountOne: '{count}本',
  assignmentLabels: Object.freeze({
    boundary: '外周',
    mountain: '山折り',
    valley: '谷折り',
    auxiliary: '補助線',
    cut: '切断線',
  }),
  lossTitle: 'この形式に含まれない情報',
  acknowledge: '上記の情報が出力に含まれないことを確認しました',
  lossless: '現在の展開図について確認が必要な情報損失はありません。',
  cancel: 'キャンセル',
  processing: '処理中…',
  save: '保存先を選んで書き出す…',
  formatSummaries: Object.freeze({
    fold: 'FOLD 1.2・2D creasePattern・座標単位mm',
    svg: '静的直線SVG・1 SVG unit = 1 mm',
    pdf: '実寸1:1ベクター・図面範囲＋四辺10 mm余白',
    dxf: 'AC1021 text-form・UTF-8・mm・5意味レイヤー',
  }),
  warningMessages: Object.freeze({
    paperAppearance:
      '紙の表裏色・厚み・テクスチャは{format}出力に含まれません。',
    editorState:
      'ORIGAMI2の頂点・辺ID、編集履歴、選択状態は{format}出力に含まれません。',
    poseCamera:
      '現在の3D表示姿勢とカメラ状態は{format}出力に含まれません。',
    pdfStructure:
      'PDFは印刷用の視覚出力で、構造化された線種や座標原点を保持せず、ORIGAMI2へ再取込できません。',
    pdfPrintScale:
      '実寸で印刷するには、PDF viewerの印刷倍率を100%にし「用紙に合わせる」を無効にしてください。',
    dxfLayers:
      '折り線の意味はORIGAMI2独自のDXFレイヤー名で表し、CAD固有の標準意味ではありません。',
    dxfName:
      '作品名はDXFコメントに格納されますが、CADで再保存すると失われる場合があります。',
    instructionSteps:
      '{count}件の折り手順は{format}出力に含まれません。',
    instructionStepOne:
      '{count}件の折り手順は{format}出力に含まれません。',
    cutPermission:
      '切断線を作成できるプロジェクト設定は、切断線がないため{format}出力に含まれません。',
    fallback: '書き出しに含まれないプロジェクト情報があります。',
  }),
}) satisfies CreaseExportDialogCopy

const EN_CREASE_EXPORT_COPY = Object.freeze({
  eyebrow: 'Export crease pattern',
  title: 'Review format and information loss',
  close: 'Close',
  description:
    'Generate a crease pattern from the current edit revision. Exporting does not change the project save state or history.',
  format: 'Export format',
  formatOption: '{label} — {detail}',
  formatLabels: Object.freeze({
    fold: 'FOLD 1.2',
    svg: 'SVG',
    pdf: 'PDF 1.7',
    dxf: 'DXF (AutoCAD 2007)',
  }),
  formatOptionLabels: Object.freeze({
    fold: 'FOLD 1.2',
    svg: 'SVG',
    pdf: 'PDF 1.7',
    dxf: 'DXF',
  }),
  optionDetails: Object.freeze({
    fold: 'JSON for exchanging data with other origami software',
    svg: 'Static line art for printing and drawing software',
    pdf: 'Full-size 1:1 monochrome vector print with 10 mm margins',
    dxf: 'CAD exchange using AutoCAD 2007, mm, and five semantic layers',
  }),
  numberLocale: 'en-US',
  unknownSize: 'Unknown',
  byteUnits: Object.freeze({
    byte: 'B',
    kilobyte: 'KB',
    megabyte: 'MB',
  }),
  generating: '{format} data is being validated and generated…',
  rebuild: 'Rebuild from the current edits',
  retry: 'Retry the same format',
  metadata: Object.freeze({
    format: 'Format',
    specification: 'Specification',
    suggestedName: 'Suggested file name',
    size: 'Size',
    geometry: 'Geometry',
    revision: 'Source',
    cuts: 'Cut lines',
  }),
  geometryValue: '{vertices} vertices · {edges} edges',
  revisionValue: 'revision {revision}',
  includes: 'Included',
  excludes: 'Not included',
  lines: 'Lines to export',
  lineCount: '{count} lines',
  lineCountOne: '{count} line',
  assignmentLabels: Object.freeze({
    boundary: 'Boundary',
    mountain: 'Mountain folds',
    valley: 'Valley folds',
    auxiliary: 'Auxiliary lines',
    cut: 'Cut lines',
  }),
  lossTitle: 'Information not included in this format',
  acknowledge: 'I understand that the information above is not included',
  lossless: 'No information loss requires confirmation for this crease pattern.',
  cancel: 'Cancel',
  processing: 'Processing…',
  save: 'Choose destination and export…',
  formatSummaries: Object.freeze({
    fold: 'FOLD 1.2 · 2D creasePattern · coordinates in mm',
    svg: 'Static line SVG · 1 SVG unit = 1 mm',
    pdf: 'Full-size 1:1 vector · drawing bounds + 10 mm margins',
    dxf: 'AC1021 text form · UTF-8 · mm · 5 semantic layers',
  }),
  warningMessages: Object.freeze({
    paperAppearance:
      'The front and back paper colors, thickness, and texture are not included in the {format} export.',
    editorState:
      'ORIGAMI2 vertex and edge IDs, edit history, and selection state are not included in the {format} export.',
    poseCamera:
      'The current 3D pose and camera state are not included in the {format} export.',
    pdfStructure:
      'PDF is a visual print output. It does not retain structured line types or the coordinate origin and cannot be re-imported into ORIGAMI2.',
    pdfPrintScale:
      'To print at full size, set the PDF viewer scale to 100% and disable “Fit to page.”',
    dxfLayers:
      'Fold meanings use ORIGAMI2-specific DXF layer names and are not standard CAD semantics.',
    dxfName:
      'The work name is stored in a DXF comment but may be lost when the file is resaved by CAD software.',
    instructionSteps:
      '{count} folding steps are not included in the {format} export.',
    instructionStepOne:
      '{count} folding step is not included in the {format} export.',
    cutPermission:
      'No cut line is present, so the project setting that permits cut-line creation is not included in the {format} export.',
    fallback: 'Some project information is not included in this export.',
  }),
}) satisfies CreaseExportDialogCopy

export const CREASE_EXPORT_COPY = Object.freeze({
  ja: JA_CREASE_EXPORT_COPY,
  en: EN_CREASE_EXPORT_COPY,
}) satisfies Readonly<Record<Locale, CreaseExportDialogCopy>>

export const CREASE_PATTERN_EXPORT_FORMATS:
ReadonlyArray<
  Readonly<{
    value: CreasePatternExportFormat
    label: string
    detail: string
  }>
> = Object.freeze([
  Object.freeze({
    value: 'fold',
    label: JA_CREASE_EXPORT_COPY.formatOptionLabels.fold,
    detail: JA_CREASE_EXPORT_COPY.optionDetails.fold,
  }),
  Object.freeze({
    value: 'svg',
    label: JA_CREASE_EXPORT_COPY.formatOptionLabels.svg,
    detail: JA_CREASE_EXPORT_COPY.optionDetails.svg,
  }),
  Object.freeze({
    value: 'pdf',
    label: JA_CREASE_EXPORT_COPY.formatOptionLabels.pdf,
    detail: JA_CREASE_EXPORT_COPY.optionDetails.pdf,
  }),
  Object.freeze({
    value: 'dxf',
    label: JA_CREASE_EXPORT_COPY.formatOptionLabels.dxf,
    detail: JA_CREASE_EXPORT_COPY.optionDetails.dxf,
  }),
])

type FormatSummaryResolver = (
  format: CreasePatternExportFormat,
  nativeSummary: string,
) => string

const preserveNativeFormatSummary: FormatSummaryResolver =
  (_format, nativeSummary) => nativeSummary
const resolveCatalogFormatSummary: FormatSummaryResolver =
  (format) => EN_CREASE_EXPORT_COPY.formatSummaries[format]

const FORMAT_SUMMARY_RESOLVERS = Object.freeze({
  ja: preserveNativeFormatSummary,
  en: resolveCatalogFormatSummary,
}) satisfies Readonly<Record<Locale, FormatSummaryResolver>>

export function formatCreaseExportInteger(value: number, locale: Locale) {
  const copy = copyFor(locale)
  return value.toLocaleString(copy.numberLocale)
}

export function resolveCreaseExportFormatSummary(
  locale: Locale,
  format: CreasePatternExportFormat,
  nativeSummary: string,
) {
  return FORMAT_SUMMARY_RESOLVERS[resolveLocale(locale)](format, nativeSummary)
}

export function creasePatternExportFormatLabel(
  format: CreasePatternExportFormat,
) {
  return JA_CREASE_EXPORT_COPY.formatLabels[format]
}

export function creasePatternExportAssignmentRows(
  counts: CreasePatternExportAssignmentCounts,
) {
  return [
    {
      key: 'boundary',
      label: JA_CREASE_EXPORT_COPY.assignmentLabels.boundary,
      count: counts.boundary,
    },
    {
      key: 'mountain',
      label: JA_CREASE_EXPORT_COPY.assignmentLabels.mountain,
      count: counts.mountain,
    },
    {
      key: 'valley',
      label: JA_CREASE_EXPORT_COPY.assignmentLabels.valley,
      count: counts.valley,
    },
    {
      key: 'auxiliary',
      label: JA_CREASE_EXPORT_COPY.assignmentLabels.auxiliary,
      count: counts.auxiliary,
    },
    {
      key: 'cut',
      label: JA_CREASE_EXPORT_COPY.assignmentLabels.cut,
      count: counts.cut,
    },
  ] as const
}

export function formatCreasePatternExportBytes(
  bytes: number,
  locale: Locale = DEFAULT_LOCALE,
) {
  const resolvedLocale = resolveLocale(locale)
  const copy = CREASE_EXPORT_COPY[resolvedLocale]
  if (!Number.isSafeInteger(bytes) || bytes < 0) return copy.unknownSize
  if (bytes < 1_000) {
    return `${formatCreaseExportInteger(bytes, resolvedLocale)} ${copy.byteUnits.byte}`
  }
  if (bytes < 1_000_000) {
    return `${(bytes / 1_000).toFixed(1)} ${copy.byteUnits.kilobyte}`
  }
  return `${(bytes / 1_000_000).toFixed(1)} ${copy.byteUnits.megabyte}`
}

type CreasePatternExportWarningCategory =
  | Readonly<{ kind: 'paper_appearance' }>
  | Readonly<{ kind: 'editor_state' }>
  | Readonly<{ kind: 'pose_camera' }>
  | Readonly<{ kind: 'pdf_structure' }>
  | Readonly<{ kind: 'pdf_print_scale' }>
  | Readonly<{ kind: 'dxf_layers' }>
  | Readonly<{ kind: 'dxf_name' }>
  | Readonly<{ kind: 'instruction_steps'; count: number }>
  | Readonly<{ kind: 'cut_permission' }>

type WarningMessageResolver = (
  category: CreasePatternExportWarningCategory | null,
  format: CreasePatternExportFormat,
  nativeWarning: unknown,
) => string

const preserveNativeWarningMessage: WarningMessageResolver =
  (category, _format, nativeWarning) =>
    category === null
      ? JA_CREASE_EXPORT_COPY.warningMessages.fallback
      : nativeWarning as string

const resolveEnglishWarningMessage: WarningMessageResolver =
  (category, format) => {
    const messages = EN_CREASE_EXPORT_COPY.warningMessages
    const variables = Object.freeze({
      format: creasePatternExportFormatLabel(format),
    })
    switch (category?.kind) {
      case 'paper_appearance':
        return formatMessage(messages.paperAppearance, variables)
      case 'editor_state':
        return formatMessage(messages.editorState, variables)
      case 'pose_camera':
        return formatMessage(messages.poseCamera, variables)
      case 'pdf_structure':
        return messages.pdfStructure
      case 'pdf_print_scale':
        return messages.pdfPrintScale
      case 'dxf_layers':
        return messages.dxfLayers
      case 'dxf_name':
        return messages.dxfName
      case 'instruction_steps':
        return formatMessage(
          category.count === 1
            ? messages.instructionStepOne
            : messages.instructionSteps,
          {
            ...variables,
            count: formatCreaseExportInteger(category.count, 'en'),
          },
        )
      case 'cut_permission':
        return formatMessage(messages.cutPermission, variables)
      default:
        return messages.fallback
    }
  }

const WARNING_MESSAGE_RESOLVERS = Object.freeze({
  ja: preserveNativeWarningMessage,
  en: resolveEnglishWarningMessage,
}) satisfies Readonly<Record<Locale, WarningMessageResolver>>

export function creasePatternExportWarningMessage(
  warning: unknown,
  format: CreasePatternExportFormat,
  locale: Locale = DEFAULT_LOCALE,
) {
  return WARNING_MESSAGE_RESOLVERS[resolveLocale(locale)](
    classifyCreasePatternExportWarning(warning),
    format,
    warning,
  )
}

const EXPORT_LABEL_PATTERN =
  '(?:FOLD 1\\.2|SVG|PDF 1\\.7|DXF(?:（AutoCAD 2007）)?)'

function classifyCreasePatternExportWarning(
  warning: unknown,
): CreasePatternExportWarningCategory | null {
  if (typeof warning !== 'string') return null
  if (new RegExp(
    `^紙の表裏色・厚み・テクスチャは${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'paper_appearance' }
  }
  if (new RegExp(
    `^ORIGAMI2の頂点・辺ID、編集履歴、選択状態は${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'editor_state' }
  }
  if (new RegExp(
    `^現在の3D表示姿勢とカメラ状態は${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'pose_camera' }
  }
  if (
    warning
    === 'PDFは印刷用の視覚出力で、構造化された線種や座標原点を保持せず、ORIGAMI2へ再取込できません。'
  ) {
    return { kind: 'pdf_structure' }
  }
  if (
    warning
    === '実寸で印刷するには、PDF viewerの印刷倍率を100%にし「用紙に合わせる」を無効にしてください。'
  ) {
    return { kind: 'pdf_print_scale' }
  }
  if (
    warning
    === '折り線の意味はORIGAMI2独自のDXFレイヤー名で表し、CAD固有の標準意味ではありません。'
  ) {
    return { kind: 'dxf_layers' }
  }
  if (
    warning
    === '作品名はDXFコメントに格納されますが、CADで再保存すると失われる場合があります。'
  ) {
    return { kind: 'dxf_name' }
  }
  const steps = new RegExp(
    `^([0-9]{1,20})件の折り手順は${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).exec(warning)
  if (steps) {
    const count = Number(steps[1])
    if (Number.isSafeInteger(count)) {
      return { kind: 'instruction_steps', count }
    }
  }
  if (new RegExp(
    `^切断線を作成できるプロジェクト設定は、切断線がないため${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'cut_permission' }
  }
  return null
}

function copyFor(locale: unknown) {
  return CREASE_EXPORT_COPY[resolveLocale(locale)]
}

function resolveLocale(locale: unknown): Locale {
  return isLocale(locale) ? locale : DEFAULT_LOCALE
}
