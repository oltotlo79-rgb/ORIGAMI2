import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
  type FocusEvent,
  type KeyboardEvent,
} from 'react'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n'
import {
  evaluatePositiveMillimetreExpression,
  MAX_NUMERIC_EXPRESSION_SOURCE_BYTES,
  numericExpressionNativeErrorCategory,
  type AdoptedMillimetreExpression,
  type NumericExpressionErrorCategory,
  type NumericExpressionNativeTransport,
} from '../lib/numericExpressionNative'

type NumericExpressionRejection =
  | 'empty'
  | 'unknown'
  | NumericExpressionErrorCategory

type EvaluationState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'evaluating' }>
  | Readonly<{ kind: 'accepted'; result: AdoptedMillimetreExpression }>
  | Readonly<{ kind: 'rejected'; reason: NumericExpressionRejection }>

export type NumericExpressionInputProps = Readonly<{
  id: string
  name: string
  defaultSource: string
  disabled?: boolean
  ariaLabel: string
  transport?: NumericExpressionNativeTransport
  localeStore?: LocaleStore
}>

export function NumericExpressionInput({
  id,
  name,
  defaultSource,
  disabled = false,
  ariaLabel,
  transport,
  localeStore: localeStore_ = localeStore,
}: NumericExpressionInputProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const [source, setSource] = useState(defaultSource)
  const [evaluation, setEvaluation] = useState<EvaluationState>({ kind: 'idle' })
  const [showEvaluation, setShowEvaluation] = useState(true)
  const sourceRef = useRef(source)
  const generationRef = useRef(0)
  const composingRef = useRef(false)
  const mountedRef = useRef(true)
  const lastAcceptedRef = useRef<AdoptedMillimetreExpression | null>(null)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      generationRef.current += 1
    }
  }, [])

  const evaluateCurrentSource = useCallback(async () => {
    const candidate = sourceRef.current
    const generation = generationRef.current + 1
    generationRef.current = generation
    if (!candidate.trim()) {
      setEvaluation({ kind: 'rejected', reason: 'empty' })
      return
    }
    setEvaluation({ kind: 'evaluating' })
    try {
      const result = await evaluatePositiveMillimetreExpression(candidate, transport)
      if (
        !mountedRef.current
        || generation !== generationRef.current
        || candidate !== sourceRef.current
      ) return
      lastAcceptedRef.current = result
      setEvaluation({ kind: 'accepted', result })
    } catch (error) {
      if (
        !mountedRef.current
        || generation !== generationRef.current
        || candidate !== sourceRef.current
      ) return
      setEvaluation({
        kind: 'rejected',
        reason: numericExpressionNativeErrorCategory(error) ?? 'unknown',
      })
    }
  }, [transport])

  const changeSource = (event: ChangeEvent<HTMLInputElement>) => {
    const next = event.currentTarget.value
    generationRef.current += 1
    sourceRef.current = next
    setSource(next)
    setEvaluation({ kind: 'idle' })
  }

  const blurInput = (_event: FocusEvent<HTMLInputElement>) => {
    if (!composingRef.current) void evaluateCurrentSource()
  }

  const keyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing || composingRef.current || event.repeat) return
    if (event.key === 'Enter') {
      event.preventDefault()
      void evaluateCurrentSource()
      return
    }
    if (event.key !== 'Escape') return
    event.preventDefault()
    generationRef.current += 1
    const accepted = lastAcceptedRef.current
    const restored = accepted?.source ?? defaultSource
    sourceRef.current = restored
    setSource(restored)
    setEvaluation(accepted
      ? { kind: 'accepted', result: accepted }
      : { kind: 'idle' })
  }

  const statusId = `${id}-numeric-expression-status`
  return (
    <span className="numeric-expression-input">
      <input
        id={id}
        name={name}
        type="text"
        inputMode="text"
        autoComplete="off"
        spellCheck={false}
        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
        value={source}
        required
        disabled={disabled}
        aria-label={ariaLabel}
        aria-describedby={statusId}
        aria-invalid={evaluation.kind === 'rejected'}
        onChange={changeSource}
        onBlur={blurInput}
        onKeyDown={keyDown}
        onCompositionStart={() => {
          composingRef.current = true
        }}
        onCompositionEnd={() => {
          composingRef.current = false
        }}
      />
      <span
        id={statusId}
        className={`numeric-expression-status numeric-expression-${evaluation.kind}`}
        aria-live="polite"
      >
        {evaluation.kind === 'idle' && text(NUMERIC_EXPRESSION_TEXT.idle)}
        {evaluation.kind === 'evaluating'
          && text(NUMERIC_EXPRESSION_TEXT.evaluating)}
        {evaluation.kind === 'rejected'
          && numericExpressionInputErrorMessage(evaluation.reason, locale)}
        {evaluation.kind === 'accepted' && (
          <>
            <span>
              {showEvaluation
                ? adoptedValueLabel(evaluation.result, locale)
                : formatLocalizedText(
                  locale,
                  NUMERIC_EXPRESSION_TEXT.source,
                  { source: evaluation.result.source },
                )}
            </span>
            <button
              type="button"
              className="numeric-expression-display-toggle"
              disabled={disabled}
              aria-pressed={!showEvaluation}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => setShowEvaluation((current) => !current)}
            >
              {showEvaluation
                ? text(NUMERIC_EXPRESSION_TEXT.showSource)
                : text(NUMERIC_EXPRESSION_TEXT.showValue)}
            </button>
          </>
        )}
      </span>
    </span>
  )
}

function adoptedValueLabel(
  result: AdoptedMillimetreExpression,
  locale: Locale,
) {
  const template = result.evaluation.exact
    ? NUMERIC_EXPRESSION_TEXT.exactValue
    : NUMERIC_EXPRESSION_TEXT.guaranteedValue
  return formatLocalizedText(locale, template, {
    value: result.value.toPrecision(15),
  })
}

function numericExpressionInputErrorMessage(
  reason: NumericExpressionRejection,
  locale: Locale,
) {
  return selectLocalizedText(locale, NUMERIC_EXPRESSION_ERROR_TEXT[reason])
}

const NUMERIC_EXPRESSION_TEXT = Object.freeze({
  idle: Object.freeze({
    ja: '式を入力できます（例: 200 * sqrt(2)）',
    en: 'Enter an expression (example: 200 * sqrt(2))',
  }),
  evaluating: Object.freeze({
    ja: '式を評価しています…',
    en: 'Evaluating expression…',
  }),
  source: Object.freeze({ ja: '式: {source}', en: 'Expression: {source}' }),
  showSource: Object.freeze({ ja: '式を表示', en: 'Show expression' }),
  showValue: Object.freeze({ ja: '評価値を表示', en: 'Show value' }),
  exactValue: Object.freeze({
    ja: '評価値: {value} mm',
    en: 'Value: {value} mm',
  }),
  guaranteedValue: Object.freeze({
    ja: '保証区間から採用: {value} mm',
    en: 'Adopted from guaranteed interval: {value} mm',
  }),
})

const FAILED_EVALUATION_TEXT = Object.freeze({
  ja: '式の評価結果を採用できませんでした。',
  en: 'The expression result could not be accepted.',
})

const NUMERIC_EXPRESSION_ERROR_TEXT: Readonly<
  Record<NumericExpressionRejection, LocalizedText>
> = Object.freeze({
  empty: Object.freeze({
    ja: '式を入力してください。',
    en: 'Enter an expression.',
  }),
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
