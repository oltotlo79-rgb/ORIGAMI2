import type { LocalizedText } from './i18n.ts'

export type BeginnerShapeCanvasPreviewText = Readonly<Record<
  | 'heading'
  | 'outlineToPreview'
  | 'bodyOption'
  | 'bindingOption'
  | 'canvasAriaLabel'
  | 'help'
  | 'missingOutline',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const BEGINNER_SHAPE_CANVAS_PREVIEW_TEXT =
  Object.freeze({
    heading: text(
      '目標形状2Dプレビュー',
      '2D target-shape preview',
    ),
    outlineToPreview: text(
      '表示する輪郭',
      'Outline to preview',
    ),
    bodyOption: text(
      '胴体',
      'Body',
    ),
    bindingOption: text(
      'binding {bindingId}',
      'Binding {bindingId}',
    ),
    canvasAriaLabel: text(
      '{selectionLabel}の輪郭プレビュー',
      '{selectionLabel} outline preview',
    ),
    help: text(
      'control pointをpointerで移動できます。矢印キーは0.1 mm、Shift+矢印は1 mm移動します。',
      'Move a control point with the pointer. Arrow keys move 0.1 mm; Shift+Arrow moves 1 mm.',
    ),
    missingOutline: text(
      'このbindingには局所輪郭がありません。',
      'This binding has no local outline.',
    ),
  }) satisfies BeginnerShapeCanvasPreviewText
