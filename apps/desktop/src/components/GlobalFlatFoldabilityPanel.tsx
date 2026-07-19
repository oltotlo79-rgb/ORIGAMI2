import {
  useEffect,
  useId,
  useRef,
} from 'react'

import {
  GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS,
  normalizeGlobalFlatFoldabilityTimePreset,
  type GlobalFlatFoldabilityTimePreset,
} from '../lib/globalFlatFoldability.ts'
import {
  createGlobalFlatFoldabilityPresentation,
  type GlobalFlatFoldabilityPresentationKind,
} from '../lib/globalFlatFoldabilityPresentation.ts'
import {
  localeStore,
  useLocale,
  type Locale,
  type LocaleStore,
} from '../lib/i18n.ts'

export type GlobalFlatFoldabilityPanelProps = Readonly<{
  job: unknown
  timeLimitSeconds: GlobalFlatFoldabilityTimePreset
  startDisabled?: boolean
  onTimeLimitChange: (seconds: GlobalFlatFoldabilityTimePreset) => void
  onStart: (seconds: GlobalFlatFoldabilityTimePreset) => void
  onCancel: () => void
  localeStore?: LocaleStore
}>

export function GlobalFlatFoldabilityPanel({
  job,
  timeLimitSeconds,
  startDisabled = false,
  onTimeLimitChange,
  onStart,
  onCancel,
  localeStore: localeStore_ = localeStore,
}: GlobalFlatFoldabilityPanelProps) {
  const locale = useLocale(localeStore_)
  const titleId = useId()
  const cautionId = useId()
  const resultLabelId = useId()
  const presentation = createGlobalFlatFoldabilityPresentation(job, locale)
  const selectedTimeLimit = normalizeGlobalFlatFoldabilityTimePreset(
    timeLimitSeconds,
  )
  const startButtonRef = useRef<HTMLButtonElement>(null)
  const cancelButtonRef = useRef<HTMLButtonElement>(null)
  const previousKindRef = useRef(presentation.kind)
  const hasTerminalResult = !presentation.active
    && presentation.kind !== 'idle'

  useEffect(() => {
    const previousKind = previousKindRef.current
    const wasActive = isActiveKind(previousKind)
    if (
      !wasActive
      && presentation.active
      && document.activeElement === startButtonRef.current
    ) {
      cancelButtonRef.current?.focus({ preventScroll: true })
    }
    previousKindRef.current = presentation.kind
  }, [presentation.active, presentation.kind])

  return (
    <section
      className="global-flat-foldability-panel"
      aria-labelledby={titleId}
      aria-describedby={cautionId}
    >
      <header className="global-flat-foldability-header">
        <div>
          <span className="global-flat-foldability-eyebrow">
            {localized(
              locale,
              '時間制限つき・3値判定',
              'Time-limited three-way result',
            )}
          </span>
          <h3 id={titleId}>
            {localized(
              locale,
              '全体平坦折り判定',
              'Global flat-foldability check',
            )}
          </h3>
        </div>
      </header>

      <div className="global-flat-foldability-controls">
        <label>
          <span>{localized(locale, '時間制限', 'Time limit')}</span>
          <select
            value={selectedTimeLimit}
            disabled={presentation.active}
            onChange={(event) => {
              const next = Number(event.currentTarget.value)
              const normalized = normalizeGlobalFlatFoldabilityTimePreset(next)
              if (next === normalized) onTimeLimitChange(normalized)
            }}
          >
            {GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS.map((seconds) => (
              <option key={seconds} value={seconds}>
                {localized(
                  locale,
                  `${seconds}秒`,
                  `${seconds} seconds`,
                )}
              </option>
            ))}
          </select>
        </label>
        <button
          ref={startButtonRef}
          type="button"
          className="global-flat-foldability-start"
          disabled={presentation.active || startDisabled}
          onClick={() => onStart(selectedTimeLimit)}
        >
          {presentation.active
            ? localized(locale, '判定中…', 'Checking…')
            : hasTerminalResult
              ? localized(locale, '再判定', 'Run again')
              : localized(locale, '判定を開始', 'Start check')}
        </button>
      </div>

      <div
        className={`global-flat-foldability-status is-${presentation.kind}`}
        role="group"
        aria-labelledby={resultLabelId}
        aria-busy={presentation.active}
        data-result-kind={presentation.kind}
      >
        <div className="global-flat-foldability-status-heading">
          <span
            className="global-flat-foldability-status-icon"
            aria-hidden="true"
          >
            {presentation.icon}
          </span>
          <strong id={resultLabelId}>{presentation.label}</strong>
        </div>
        <p>{presentation.detail}</p>

        {presentation.active && (
          <div className="global-flat-foldability-running">
            <p className="global-flat-foldability-phase">
              <strong>{presentation.phaseText}</strong>
              <span>{presentation.workText}</span>
            </p>
            <button
              ref={cancelButtonRef}
              type="button"
              className="global-flat-foldability-cancel"
              onClick={onCancel}
            >
              {presentation.cancelRequested
                ? localized(
                  locale,
                  '中止（要求済み）',
                  'Cancel requested',
                )
                : localized(locale, '判定を中止', 'Cancel check')}
            </button>
          </div>
        )}
      </div>

      <dl className="global-flat-foldability-summary">
        {presentation.summaryEntries.map((entry) => (
          <div key={entry.label}>
            <dt>{entry.label}</dt>
            <dd>{entry.value}</dd>
          </div>
        ))}
      </dl>

      {presentation.resultEntries.length > 0 && (
        <dl className="global-flat-foldability-result-details">
          {presentation.resultEntries.map((entry) => (
            <div key={entry.label}>
              <dt>{entry.label}</dt>
              <dd>{entry.value}</dd>
            </div>
          ))}
        </dl>
      )}

      <aside
        id={cautionId}
        className="global-flat-foldability-caution"
        aria-label={localized(
          locale,
          '判定結果の重要な制約',
          'Important limitations of the result',
        )}
      >
        <strong>
          {localized(
            locale,
            '「可」が保証しないこと',
            'What “Possible” does not guarantee',
          )}
        </strong>
        <p>
          {localized(
            locale,
            '理想的な厚さ0の判定です。紙厚や層ずれを含めて折れること、手で折りやすいこと、平坦状態まで安全にたどれる連続した折り経路があることは保証しません。',
            'This check uses an ideal zero-thickness model. It does not guarantee foldability with paper thickness or layer offsets, ease of folding by hand, or a continuous collision-safe path to the flat state.',
          )}
        </p>
      </aside>

      <p
        className="visually-hidden"
        role="status"
        aria-live="polite"
        aria-atomic="true"
        aria-relevant="text"
      >
        {presentation.liveText}
      </p>
    </section>
  )
}

function isActiveKind(kind: GlobalFlatFoldabilityPresentationKind) {
  return kind === 'queued' || kind === 'running'
}

function localized(locale: Locale, ja: string, en: string): string {
  return locale === 'en' ? en : ja
}
