import {
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

const APP_CONFIRMATIONS = Object.freeze({
  quitDiscard: {
    ja: '未保存の変更があります。変更を破棄して終了しますか？\nキャンセルすると編集画面に戻ります。',
    en: 'There are unsaved changes. Discard them and quit?\nChoose Cancel to return to the editor.',
  },
  newProject: {
    ja: '未保存の変更があります。保存せずに新しいプロジェクトを作成しますか？',
    en: 'There are unsaved changes. Create a new project without saving?',
  },
  openProject: {
    ja: '未保存の変更があります。保存せずに別のプロジェクトを開きますか？',
    en: 'There are unsaved changes. Open another project without saving?',
  },
  replaceWithFold: {
    ja: '未保存の変更があります。保存せずにFOLD展開図へ置き換えますか？',
    en: 'There are unsaved changes. Replace them with the FOLD crease pattern?',
  },
  replaceWithSvg: {
    ja: '未保存の変更があります。保存せずにSVG展開図へ置き換えますか？',
    en: 'There are unsaved changes. Replace them with the SVG crease pattern?',
  },
  replaceFoldTechnique: {
    ja: '未保存の折り技法があります。保存せずに別の折り技法へ置き換えますか？',
    en: 'There are unsaved fold-technique changes. Replace them without saving?',
  },
  discardFoldTechniqueDraft: {
    ja: '保存していない折り技法の編集内容があります。破棄して閉じますか？',
    en: 'There are unsaved fold-technique edits. Discard them and close the editor?',
  },
} satisfies Readonly<Record<string, LocalizedText>>)

export type AppConfirmation = keyof typeof APP_CONFIRMATIONS

export const APP_ERROR_CODES = Object.freeze([
  'unexpected_failure',
  'window_close_status_invalid',
  'topology_analysis_failed',
  'native_edit_failed',
  'validation_failed',
  'file_operation_failed',
  'fold_read_failed',
  'fold_cleanup_failed',
  'fold_import_failed',
  'svg_read_failed',
  'svg_cleanup_failed',
  'svg_boundary_validation_failed',
  'svg_import_failed',
  'crease_export_prepare_failed',
  'crease_export_cleanup_failed',
  'crease_export_save_failed',
  'benchmark_failed',
] as const)

export type AppErrorCode = (typeof APP_ERROR_CODES)[number]

const APP_ERROR_MESSAGES = Object.freeze({
  unexpected_failure: Object.freeze({
    ja: '処理を完了できませんでした。もう一度お試しください。',
    en: 'The operation could not be completed. Please try again.',
  }),
  window_close_status_invalid: Object.freeze({
    ja: '終了処理の状態を確認できませんでした。アプリを開いたまま、もう一度お試しください。',
    en: 'The quit status could not be verified. Keep the app open and try again.',
  }),
  topology_analysis_failed: Object.freeze({
    ja: '3D解析エラー: 解析を完了できませんでした。安全確認済みとして扱わないでください。',
    en: '3D analysis error: Analysis could not be completed. Do not treat this result as safety-verified.',
  }),
  native_edit_failed: Object.freeze({
    ja: 'コアエラー: 編集操作を完了できませんでした。内容を確認して再試行してください。',
    en: 'Core error: The edit could not be completed. Review the project and try again.',
  }),
  validation_failed: Object.freeze({
    ja: '検証エラー: 検証を完了できませんでした。安全確認済みとして扱わないでください。',
    en: 'Validation error: Validation could not be completed. Do not treat the project as safety-verified.',
  }),
  file_operation_failed: Object.freeze({
    ja: 'ファイル操作を完了できませんでした。対象と保存先を確認して再試行してください。',
    en: 'The file operation could not be completed. Check the file and destination, then try again.',
  }),
  fold_read_failed: Object.freeze({
    ja: 'FOLDファイルを読み込めませんでした。ファイルの内容を確認して再試行してください。',
    en: 'The FOLD file could not be read. Check its contents and try again.',
  }),
  fold_cleanup_failed: Object.freeze({
    ja: 'FOLD取込の取消処理を完了できませんでした。もう一度お試しください。',
    en: 'The FOLD import cancellation could not be completed. Please try again.',
  }),
  fold_import_failed: Object.freeze({
    ja: 'FOLD取込を完了できませんでした。ファイルの内容と設定を確認して再試行してください。',
    en: 'The FOLD import could not be completed. Check the file and settings, then try again.',
  }),
  svg_read_failed: Object.freeze({
    ja: 'SVGファイルを読み込めませんでした。ファイルの内容を確認して再試行してください。',
    en: 'The SVG file could not be read. Check its contents and try again.',
  }),
  svg_cleanup_failed: Object.freeze({
    ja: 'SVG取込の取消処理を完了できませんでした。もう一度お試しください。',
    en: 'The SVG import cancellation could not be completed. Please try again.',
  }),
  svg_boundary_validation_failed: Object.freeze({
    ja: 'SVG外周を検証できませんでした。外周と取込設定を確認して再試行してください。',
    en: 'The SVG boundary could not be validated. Check the boundary and import settings, then try again.',
  }),
  svg_import_failed: Object.freeze({
    ja: 'SVG取込を完了できませんでした。ファイルの内容と設定を確認して再試行してください。',
    en: 'The SVG import could not be completed. Check the file and settings, then try again.',
  }),
  crease_export_prepare_failed: Object.freeze({
    ja: '展開図の書き出しデータを準備できませんでした。編集内容を確認して再試行してください。',
    en: 'The crease-pattern export data could not be prepared. Review the project and try again.',
  }),
  crease_export_cleanup_failed: Object.freeze({
    ja: '展開図書き出しの取消処理を完了できませんでした。もう一度お試しください。',
    en: 'The crease-pattern export cancellation could not be completed. Please try again.',
  }),
  crease_export_save_failed: Object.freeze({
    ja: '展開図を書き出せませんでした。保存先を確認して再試行してください。',
    en: 'The crease pattern could not be exported. Check the destination and try again.',
  }),
  benchmark_failed: Object.freeze({
    ja: 'ベンチマーク失敗: 性能テストを完了できませんでした。',
    en: 'Benchmark failed: The performance test could not be completed.',
  }),
} satisfies Readonly<Record<AppErrorCode, LocalizedText>>)

export function appConfirmationText(
  locale: Locale,
  confirmation: AppConfirmation,
) {
  return selectLocalizedText(locale, APP_CONFIRMATIONS[confirmation])
}

/**
 * Returns only reviewed, fixed UI text for an application error code.
 * A forged runtime code fails closed to the generic message and is never
 * reflected into UI text.
 */
export function appErrorLocalizedText(
  code: AppErrorCode,
): LocalizedText {
  return Object.prototype.hasOwnProperty.call(APP_ERROR_MESSAGES, code)
    ? APP_ERROR_MESSAGES[code]
    : APP_ERROR_MESSAGES.unexpected_failure
}
