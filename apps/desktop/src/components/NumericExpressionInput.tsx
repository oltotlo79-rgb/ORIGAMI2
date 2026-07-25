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
import {
  NUMERIC_EXPRESSION_ERROR_TEXT,
  NUMERIC_EXPRESSION_TEXT,
} from '../lib/numericExpressionInputText.ts'

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
