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
  evaluatePositiveMillimetreExpression,
  MAX_NUMERIC_EXPRESSION_SOURCE_BYTES,
  numericExpressionNativeErrorCategory,
  type AdoptedMillimetreExpression,
  type NumericExpressionNativeTransport,
} from '../lib/numericExpressionNative'

type EvaluationState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'evaluating' }>
  | Readonly<{ kind: 'accepted'; result: AdoptedMillimetreExpression }>
  | Readonly<{ kind: 'rejected'; message: string }>

export type NumericExpressionInputProps = Readonly<{
  id: string
  name: string
  defaultSource: string
  disabled?: boolean
  ariaLabel: string
  transport?: NumericExpressionNativeTransport
}>

export function NumericExpressionInput({
  id,
  name,
  defaultSource,
  disabled = false,
  ariaLabel,
  transport,
}: NumericExpressionInputProps) {
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
      setEvaluation({ kind: 'rejected', message: '式を入力してください。' })
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
        message: numericExpressionInputErrorMessage(error),
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
        {evaluation.kind === 'idle' && '式を入力できます（例: 200 * sqrt(2)）'}
        {evaluation.kind === 'evaluating' && '式を評価しています…'}
        {evaluation.kind === 'rejected' && evaluation.message}
        {evaluation.kind === 'accepted' && (
          <>
            <span>
              {showEvaluation
                ? adoptedValueLabel(evaluation.result)
                : `式: ${evaluation.result.source}`}
            </span>
            <button
              type="button"
              className="numeric-expression-display-toggle"
              disabled={disabled}
              aria-pressed={!showEvaluation}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => setShowEvaluation((current) => !current)}
            >
              {showEvaluation ? '式を表示' : '評価値を表示'}
            </button>
          </>
        )}
      </span>
    </span>
  )
}

function adoptedValueLabel(result: AdoptedMillimetreExpression) {
  const prefix = result.evaluation.exact ? '評価値' : '保証区間から採用'
  return `${prefix}: ${result.value.toPrecision(15)} mm`
}

function numericExpressionInputErrorMessage(error: unknown) {
  const category = numericExpressionNativeErrorCategory(error)
  if (category) {
    switch (category) {
      case 'invalid_request':
        return '式が空か、入力上限を超えています。'
      case 'invalid_expression':
        return '式を解釈できません。演算子や括弧を確認してください。'
      case 'resource_limit':
        return '式が複雑すぎるため評価を中止しました。'
      case 'result_out_of_range':
        return '正のmm値として安全に採用できる精度ではありません。'
      case 'native_unavailable':
        return '式の評価はデスクトップ版で利用できます。'
      case 'invalid_response':
      case 'stale_response':
      case 'internal_failure':
        return '式の評価結果を採用できませんでした。'
    }
  }
  return '式の評価結果を採用できませんでした。'
}
