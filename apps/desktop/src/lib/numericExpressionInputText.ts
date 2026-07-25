import type { LocalizedText } from './i18n.ts'
import type { NumericExpressionErrorCategory } from './numericExpressionNative.ts'

type NumericExpressionRejection =
  | 'empty'
  | 'unknown'
  | NumericExpressionErrorCategory

export const NUMERIC_EXPRESSION_TEXT: Readonly<Record<
  'idle' | 'evaluating' | 'source' | 'showSource' | 'showValue' | 'exactValue' | 'guaranteedValue',
  LocalizedText
>> = Object.freeze({
  idle: Object.freeze({
    ja: '式を入力できます（例: 200 * sqrt(2)）',
    en: 'Enter an expression (example: 200 * sqrt(2))',
  }),
  evaluating: Object.freeze({ ja: '式を評価しています…', en: 'Evaluating expression…' }),
  source: Object.freeze({ ja: '式: {source}', en: 'Expression: {source}' }),
  showSource: Object.freeze({ ja: '式を表示', en: 'Show expression' }),
  showValue: Object.freeze({ ja: '評価値を表示', en: 'Show value' }),
  exactValue: Object.freeze({ ja: '評価値: {value} mm', en: 'Value: {value} mm' }),
  guaranteedValue: Object.freeze({
    ja: '保証区間から採用: {value} mm',
    en: 'Adopted from guaranteed interval: {value} mm',
  }),
})

export const FAILED_EVALUATION_TEXT: LocalizedText = Object.freeze({
  ja: '式の評価結果を採用できませんでした。',
  en: 'The expression result could not be accepted.',
})

export const NUMERIC_EXPRESSION_ERROR_TEXT: Readonly<
  Record<NumericExpressionRejection, LocalizedText>
> = Object.freeze({
    empty: Object.freeze({ ja: '式を入力してください。', en: 'Enter an expression.' }),
    invalid_request: Object.freeze({
      ja: '式が空か、入力上限を超えています。',
      en: 'The expression is empty or exceeds the input limit.',
    }),
    invalid_expression: Object.freeze({
      ja: '式を解釈できません。演算子や括弧を確認してください。',
      en: 'The expression could not be parsed. Check its operators and parentheses.',
    }),
    resource_limit: Object.freeze({
      ja: '式が複雑すぎるため評価を中止しました。',
      en: 'Evaluation stopped because the expression is too complex.',
    }),
    result_out_of_range: Object.freeze({
      ja: '正のmm値として安全に採用できる精度ではありません。',
      en: 'The result is not precise enough to safely accept as a positive mm value.',
    }),
    native_unavailable: Object.freeze({
      ja: '式の評価はデスクトップ版で利用できます。',
      en: 'Expression evaluation is available in the desktop app.',
    }),
    invalid_response: FAILED_EVALUATION_TEXT,
    stale_response: FAILED_EVALUATION_TEXT,
    internal_failure: FAILED_EVALUATION_TEXT,
    unknown: FAILED_EVALUATION_TEXT,
  })
