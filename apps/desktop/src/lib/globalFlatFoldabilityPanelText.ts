import type { LocalizedText } from './i18n.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const GLOBAL_FLAT_FOLDABILITY_PANEL_TEXT = Object.freeze({
  eyebrow: localized(
    '時間制限つき・3値判定',
    'Time-limited three-way result',
  ),
  title: localized('全体平坦折り判定', 'Global flat-foldability check'),
  timeLimit: localized('時間制限', 'Time limit'),
  seconds: localized('{seconds}秒', '{seconds} seconds'),
  checking: localized('判定中…', 'Checking…'),
  runAgain: localized('再判定', 'Run again'),
  start: localized('判定を開始', 'Start check'),
  cancelRequested: localized('中止（要求済み）', 'Cancel requested'),
  cancel: localized('判定を中止', 'Cancel check'),
  layerUnavailable: localized(
    '認証済みの層順序表示を取得できませんでした。この状態を「重なりなし」と解釈しないでください。',
    'The certified layer-order view is unavailable. Do not interpret this as having no overlaps.',
  ),
  limitationsLabel: localized(
    '判定結果の重要な制約',
    'Important limitations of the result',
  ),
  limitationsTitle: localized(
    '「可」が保証しないこと',
    'What “Possible” does not guarantee',
  ),
  limitationsDetail: localized(
    '理想的な厚さ0の判定です。紙厚や層ずれを含めて折れること、手で折りやすいこと、平坦状態まで安全にたどれる連続した折り経路があることは保証しません。',
    'This check uses an ideal zero-thickness model. It does not guarantee foldability with paper thickness or layer offsets, ease of folding by hand, or a continuous collision-safe path to the flat state.',
  ),
})
