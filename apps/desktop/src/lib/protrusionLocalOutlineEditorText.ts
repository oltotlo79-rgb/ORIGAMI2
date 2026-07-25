import type { LocalizedText } from './i18n.ts'

export type ProtrusionLocalOutlineEditorText = Readonly<Record<
  | 'legend'
  | 'outlinePoints'
  | 'outlinePointsAria'
  | 'applyOutline'
  | 'clearOutline'
  | 'invalidOutline',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const PROTRUSION_LOCAL_OUTLINE_EDITOR_TEXT =
  Object.freeze({
    legend: text(
      '局所輪郭（任意）',
      'Local outline (optional)',
    ),
    outlinePoints: text(
      '局所輪郭点（X, Y mm）',
      'Local outline points (X, Y mm)',
    ),
    outlinePointsAria: text(
      '局所輪郭点 binding {bindingId}',
      'Local outline points binding {bindingId}',
    ),
    applyOutline: text(
      '局所輪郭を反映',
      'Apply local outline',
    ),
    clearOutline: text(
      '局所輪郭を解除',
      'Clear local outline',
    ),
    invalidOutline: text(
      '3〜8点の有界な輪郭を入力してください。左右対称bindingでは鏡像点が必要です。',
      'Enter 3 to 8 bounded points. Bilateral bindings require mirrored points.',
    ),
  }) satisfies ProtrusionLocalOutlineEditorText
