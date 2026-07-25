import type { LocalizedText } from './i18n.ts'

export type RecognitionContourCopyActionText = Readonly<Record<
  'summary' | 'confirmation' | 'copy',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const RECOGNITION_CONTOUR_COPY_ACTION_TEXT =
  Object.freeze({
    summary: text(
      '編集可能な胴体輪郭 {bodyPointCount} 点・局所輪郭 {localContourCount} 件',
      'Editable body contour: {bodyPointCount} points; local contours: {localContourCount}',
    ),
    confirmation: text(
      '認識候補の輪郭を編集欄へコピーしますか？保存するまでprojectは変更されません。',
      'Copy the proposed contours into the editor? The project stays unchanged until saved.',
    ),
    copy: text(
      '確認して輪郭を編集欄へコピー',
      'Review and copy contours to editor',
    ),
  }) satisfies RecognitionContourCopyActionText
