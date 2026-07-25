import type { LocalizedText } from './i18n.ts'

export type GenericBodyOutlineEditorText = Readonly<Record<
  | 'legend'
  | 'outlineMode'
  | 'outlineModeAria'
  | 'symmetricOption'
  | 'generalOption'
  | 'outlinePoints'
  | 'outlinePointsAria'
  | 'applyOutline'
  | 'clearOutline'
  | 'invalidSymmetricOutline'
  | 'invalidGeneralOutline',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const GENERIC_BODY_OUTLINE_EDITOR_TEXT =
  Object.freeze({
    legend: text(
      '左右対称の胴体輪郭',
      'Symmetric body outline',
    ),
    outlineMode: text(
      '輪郭モード',
      'Outline mode',
    ),
    outlineModeAria: text(
      '胴体輪郭モード',
      'Body outline mode',
    ),
    symmetricOption: text(
      '左右対称',
      'Left-right symmetric',
    ),
    generalOption: text(
      '非対称一般',
      'General asymmetric',
    ),
    outlinePoints: text(
      '輪郭点（1行に X, Y mm）',
      'Outline points (X, Y mm per line)',
    ),
    outlinePointsAria: text(
      '胴体輪郭点',
      'Body outline points',
    ),
    applyOutline: text(
      '輪郭を反映',
      'Apply outline',
    ),
    clearOutline: text(
      '輪郭指定を解除',
      'Clear outline',
    ),
    invalidSymmetricOutline: text(
      '4〜16点の左右対称な有限座標を入力してください。',
      'Enter 4 to 16 finite, left-right symmetric points.',
    ),
    invalidGeneralOutline: text(
      '4〜16点の有限座標を入力してください。',
      'Enter 4 to 16 finite points.',
    ),
  }) satisfies GenericBodyOutlineEditorText
