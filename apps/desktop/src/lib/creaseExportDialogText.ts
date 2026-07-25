import type { CreasePatternExportFormat } from './creaseExport.ts'
import type { Locale } from './i18n.ts'

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

export type CreaseExportDialogCopy = Readonly<{
  eyebrow: string
  title: string
  close: string
  description: string
  format: string
  formatOption: string
  optionDetails: Readonly<Record<CreasePatternExportFormat, string>>
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
}>

const JA_CREASE_EXPORT_COPY = Object.freeze({
  eyebrow: '展開図の書き出し',
  title: '形式と情報損失を確認',
  close: '閉じる',
  description:
    '現在の編集リビジョンから展開図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
  format: '出力形式',
  formatOption: '{label} — {detail}',
  optionDetails: Object.freeze({
    fold: '他の折り紙ソフトと交換しやすいJSON形式',
    svg: '印刷・作図ソフトで扱いやすい静的な線図',
    pdf: '実寸1:1・四辺10 mm余白の白黒ベクター印刷',
    dxf: 'AutoCAD 2007・mm・5意味レイヤーのCAD交換',
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
}) satisfies CreaseExportDialogCopy

const EN_CREASE_EXPORT_COPY = Object.freeze({
  eyebrow: 'Export crease pattern',
  title: 'Review format and information loss',
  close: 'Close',
  description:
    'Generate a crease pattern from the current edit revision. Exporting does not change the project save state or history.',
  format: 'Export format',
  formatOption: '{label} — {detail}',
  optionDetails: Object.freeze({
    fold: 'JSON for exchanging data with other origami software',
    svg: 'Static line art for printing and drawing software',
    pdf: 'Full-size 1:1 monochrome vector print with 10 mm margins',
    dxf: 'CAD exchange using AutoCAD 2007, mm, and five semantic layers',
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
}) satisfies CreaseExportDialogCopy

export const CREASE_EXPORT_COPY = Object.freeze({
  ja: JA_CREASE_EXPORT_COPY,
  en: EN_CREASE_EXPORT_COPY,
}) satisfies Readonly<Record<Locale, CreaseExportDialogCopy>>

const CREASE_EXPORT_NUMBER_LOCALES = Object.freeze({
  ja: 'ja-JP',
  en: 'en-US',
}) satisfies Readonly<Record<Locale, string>>

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
  return value.toLocaleString(CREASE_EXPORT_NUMBER_LOCALES[locale])
}

export function resolveCreaseExportFormatSummary(
  locale: Locale,
  format: CreasePatternExportFormat,
  nativeSummary: string,
) {
  return FORMAT_SUMMARY_RESOLVERS[locale](format, nativeSummary)
}
