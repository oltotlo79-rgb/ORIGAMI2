import type { InstructionExportFormat } from './instructionExport.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

type InstructionExportMetadataKey =
  | 'format'
  | 'specification'
  | 'profile'
  | 'projection'
  | 'suggestedName'
  | 'size'
  | 'steps'
  | 'pages'
  | 'cautions'
  | 'revision'

export type InstructionExportCountKind = 'steps' | 'pages' | 'cautions'

type LocalizedCountText = Readonly<{
  one: LocalizedText
  other: LocalizedText
}>

export type InstructionExportDialogText = Readonly<{
  eyebrow: LocalizedText
  title: LocalizedText
  close: LocalizedText
  closeGlyph: LocalizedText
  description: LocalizedText
  format: LocalizedText
  formatOption: LocalizedText
  optionDetails: Readonly<Record<InstructionExportFormat, LocalizedText>>
  progress: LocalizedText
  rebuild: LocalizedText
  retry: LocalizedText
  metadata: Readonly<Record<InstructionExportMetadataKey, LocalizedText>>
  counts: Readonly<Record<InstructionExportCountKind, LocalizedCountText>>
  revisionValue: LocalizedText
  warningTitle: LocalizedText
  acknowledge: LocalizedText
  warningFree: LocalizedText
  stop: LocalizedText
  cancel: LocalizedText
  processing: LocalizedText
  save: LocalizedText
  summaries: Readonly<Record<InstructionExportFormat, LocalizedText>>
  emptyNotice: LocalizedText
  numberLocale: LocalizedText
}>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

const countText = (
  ja: string,
  enOne: string,
  enOther: string,
): LocalizedCountText => Object.freeze({
  one: text(ja, enOne),
  other: text(ja, enOther),
})

export const INSTRUCTION_EXPORT_COPY =
  Object.freeze({
    eyebrow: text('折り図の書き出し', 'Export instructions'),
    title: text('形式と出力内容を確認', 'Review format and output'),
    close: text('閉じる', 'Close'),
    closeGlyph: text('×', '×'),
    description: text(
      '現在の編集リビジョンから折り図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
      'Generate instructions from the current edit revision. Exporting does not change the project save state or history.',
    ),
    format: text('出力形式', 'Export format'),
    formatOption: text(
      '{label} — {detail}',
      '{label} — {detail}',
    ),
    optionDetails: Object.freeze({
      pdf: text(
        '固定アイソメトリック視点の折り図を、複数ページのPDFにまとめます',
        'Combine fixed-isometric diagrams with authored camera and hand/regrip guide details into a multi-page PDF',
      ),
      svg_zip: text(
        '手順ごとのベクターSVG画像を、1つのZIPにまとめます',
        'Package one vector SVG page with camera, fold directions, focus points, and hand positions into a ZIP',
      ),
    }),
    progress: text(
      '{format}: {phase}…',
      '{format}: {phase}…',
    ),
    rebuild: text(
      '現在の編集内容から作り直す',
      'Rebuild from the current edits',
    ),
    retry: text('同じ形式で再試行', 'Retry the same format'),
    metadata: Object.freeze({
      format: text('形式', 'Format'),
      specification: text('出力仕様', 'Specification'),
      profile: text('出力プロファイル', 'Export profile'),
      projection: text('投影プロファイル', 'Projection profile'),
      suggestedName: text('保存名候補', 'Suggested file name'),
      size: text('サイズ', 'Size'),
      steps: text('折り手順', 'Instruction steps'),
      pages: text('ページ', 'Pages'),
      cautions: text('注意事項', 'Notices'),
      revision: text('固定元', 'Source'),
    }),
    counts: Object.freeze({
      steps: countText('{count}手順', '{count} step', '{count} steps'),
      pages: countText('{count}ページ', '{count} page', '{count} pages'),
      cautions: countText('{count}件', '{count} notice', '{count} notices'),
    }),
    revisionValue: text('revision {revision}', 'revision {revision}'),
    warningTitle: text('出力前の確認事項', 'Review before export'),
    acknowledge: text(
      '上記の注意事項を確認しました',
      'I have reviewed the notices above',
    ),
    warningFree: text(
      'この折り図について追加確認が必要な注意事項はありません。',
      'No additional notices require review for these instructions.',
    ),
    stop: text('生成を中止', 'Stop generation'),
    cancel: text('キャンセル', 'Cancel'),
    processing: text('処理中…', 'Processing…'),
    save: text(
      '保存先を選んで書き出す…',
      'Choose destination and export…',
    ),
    summaries: Object.freeze({
      pdf: text(
        'PDF 1.7・A4縦・固定アイソメトリック投影・複数ページ',
        'PDF 1.7 · A4 portrait · fixed isometric projection · multiple pages',
      ),
      svg_zip: text(
        'SVGページ画像・固定アイソメトリック投影・ZIPアーカイブ',
        'SVG page images · fixed isometric projection · ZIP archive',
      ),
    }),
    emptyNotice: text('\u00a0', '\u00a0'),
    numberLocale: text('ja-JP', 'en-US'),
  }) satisfies InstructionExportDialogText

export function formatInstructionExportDialogOption(
  format: InstructionExportFormat,
  label: string,
  locale: Locale,
) {
  return formatLocalizedText(locale, INSTRUCTION_EXPORT_COPY.formatOption, {
    label,
    detail: selectLocalizedText(
      locale,
      INSTRUCTION_EXPORT_COPY.optionDetails[format],
    ),
  })
}

export function formatInstructionExportDialogProgress(
  format: string,
  phase: string,
  locale: Locale,
) {
  return formatLocalizedText(locale, INSTRUCTION_EXPORT_COPY.progress, {
    format,
    phase,
  })
}

export function formatInstructionExportDialogCount(
  count: number,
  kind: InstructionExportCountKind,
  locale: Locale,
) {
  const copy = INSTRUCTION_EXPORT_COPY.counts[kind]
  const template = count === 1 ? copy.one : copy.other
  const numberLocale = selectLocalizedText(
    locale,
    INSTRUCTION_EXPORT_COPY.numberLocale,
  )
  return formatLocalizedText(locale, template, {
    count: count.toLocaleString(numberLocale),
  })
}

export function formatInstructionExportDialogRevision(
  revision: number,
  locale: Locale,
) {
  const numberLocale = selectLocalizedText(
    locale,
    INSTRUCTION_EXPORT_COPY.numberLocale,
  )
  return formatLocalizedText(locale, INSTRUCTION_EXPORT_COPY.revisionValue, {
    revision: revision.toLocaleString(numberLocale),
  })
}

export function instructionExportDialogSummary(
  format: InstructionExportFormat,
  nativeSummary: string,
  locale: Locale,
) {
  const localizedSummary: LocalizedText = Object.freeze({
    ja: nativeSummary,
    en: selectLocalizedText('en', INSTRUCTION_EXPORT_COPY.summaries[format]),
  })
  return selectLocalizedText(locale, localizedSummary)
}
