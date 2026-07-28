import { useEffect, useMemo, useRef, useState } from 'react'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import { PROOF_PROGRESS_PANEL_TEXT as TEXT } from '../lib/proofProgressPanelText.ts'
import {
  failClosedProofProgressState,
  isSafeCount,
  type ProofFailureViewModel,
  type ProofProgressPanelModel,
  UNPROVEN_HISTORY_STATUS_KEYS,
  type UnprovenHistoryStatusCountsView,
} from '../lib/proofProgressModel.ts'
import './proofProgress.css'

type Props = Readonly<{
  locale: Locale
  model: ProofProgressPanelModel
  disabled?: boolean
  onRequestRevert?(failure: ProofFailureViewModel): void
}>

export function ProofProgressPanel({
  locale,
  model,
  disabled = false,
  onRequestRevert,
}: Props) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) =>
    selectLocalizedText(locale, localized)
  const format = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const safeProgress = normalizePairProgress(
    model.provenPairCount,
    model.totalPairCount,
  )
  const status = model.status === null
    ? null
    : failClosedProofProgressState(model.status)
  const unprovenTrust = status !== null && status !== 'certified'
  const knownHistory = model.unprovenHistory.kind === 'known'
    ? model.unprovenHistory
    : null
  const appliedUnproven = knownHistory
    ? knownHistory.appliedTotal
    : null
  const redoUnproven = knownHistory
    ? knownHistory.unappliedRedoTotal
    : null

  return (
    <section
      className="proof-progress-panel"
      aria-label={text(TEXT.ariaLabel)}
      data-proof-trust={status === 'certified' ? 'proven' : 'unproven'}
    >
      <h3>{text(TEXT.title)}</h3>
      {status !== null && (
        <div
          role={status === 'blocked' ? 'alert' : 'status'}
          aria-live={status === 'blocked' ? 'assertive' : 'polite'}
          data-proof-status={status}
        >
          <span>{text(TEXT.statusLabel)}: {text(TEXT.status[status])}</span>
          <span
            className={status === 'certified'
              ? 'proof-badge proof-badge--proven'
              : 'proof-badge proof-badge--unproven'}
            data-testid={status === 'certified'
              ? 'proven-proof-badge'
              : 'unproven-proof-badge'}
          >
            {text(status === 'certified' ? TEXT.certifiedBadge : TEXT.unprovenBadge)}
          </span>
          <p>
            {safeProgress.total === null
              ? format(TEXT.pairProgressUnknownTotal, {
                  proven: safeProgress.proven,
                })
              : format(TEXT.pairProgress, {
                  proven: safeProgress.proven,
                  total: safeProgress.total,
                })}
          </p>
          {model.postApplyNotice === 'starting' && (
            <p data-testid="post-apply-proof-starting">
              {text(TEXT.postApplyStarting)}
            </p>
          )}
          {model.postApplyNotice === 'unavailable' && (
            <p data-testid="post-apply-proof-unavailable">
              {text(TEXT.postApplyUnavailable)}
            </p>
          )}
          {unprovenTrust && model.speculativeApplyAvailable && (
            <p role="note">{text(TEXT.speculativeApplyWarning)}</p>
          )}
        </div>
      )}
      {model.unprovenHistory.kind === 'unavailable' && (
        <p role="alert" data-testid="unproven-summary-unavailable">
          <span className="proof-badge proof-badge--unproven">
            {text(TEXT.unprovenBadge)}
          </span>{' '}
          {text(TEXT.unprovenSummaryUnavailable)}
        </p>
      )}
      {knownHistory && appliedUnproven !== null && redoUnproven !== null && (
        <div>
          <p>{format(TEXT.unprovenCounts, {
            applied: appliedUnproven,
            redo: redoUnproven,
          })}</p>
          <UnprovenStatusCounts
            locale={locale}
            label={text(TEXT.appliedBreakdown)}
            counts={knownHistory.applied}
          />
          <UnprovenStatusCounts
            locale={locale}
            label={text(TEXT.redoBreakdown)}
            counts={knownHistory.unappliedRedo}
          />
          {appliedUnproven > 0 && (
            <p role="alert" data-testid="applied-unproven-warning">
              <span className="proof-badge proof-badge--unproven">
                {text(TEXT.unprovenBadge)}
              </span>{' '}
              {format(TEXT.appliedUnprovenWarning, { count: appliedUnproven })}
            </p>
          )}
          {redoUnproven > 0 && (
            <p role="status" aria-live="polite" data-testid="redo-unproven-notice">
              {format(TEXT.unappliedRedoNotice, { count: redoUnproven })}
            </p>
          )}
        </div>
      )}
      {model.proofFailure && (
        <ProofFailureRevert
          locale={locale}
          failure={model.proofFailure}
          disabled={disabled}
          onRequestRevert={onRequestRevert}
        />
      )}
    </section>
  )
}

function UnprovenStatusCounts({
  locale,
  label,
  counts,
}: Readonly<{
  locale: Locale
  label: string
  counts: UnprovenHistoryStatusCountsView
}>) {
  return (
    <dl aria-label={label}>
      {UNPROVEN_HISTORY_STATUS_KEYS.map((status) => (
        <div key={status}>
          <dt>{selectLocalizedText(
            locale,
            TEXT.unprovenStatuses[status],
          )}</dt>
          <dd>{counts[status]}</dd>
        </div>
      ))}
    </dl>
  )
}

function ProofFailureRevert({
  locale,
  failure,
  disabled,
  onRequestRevert,
}: Readonly<{
  locale: Locale
  failure: ProofFailureViewModel
  disabled: boolean
  onRequestRevert?: (failure: ProofFailureViewModel) => void
}>) {
  const [confirmed, setConfirmed] = useState(false)
  const [requested, setRequested] = useState(false)
  const callbackLockedRef = useRef(false)
  const failureKey = useMemo(
    () => [
      failure.location,
      failure.reason,
      failure.subsequentEditCount,
      failure.undoStepsToRevert ?? 'none',
    ].join(':'),
    [failure],
  )
  useEffect(() => {
    setConfirmed(false)
    setRequested(false)
    callbackLockedRef.current = false
  }, [failureKey])

  const text = (localized: Parameters<typeof selectLocalizedText>[1]) =>
    selectLocalizedText(locale, localized)
  const format = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const canRequest = failure.undoStepsToRevert !== null && onRequestRevert !== undefined

  return (
    <section
      className="proof-failure-revert"
      aria-label={text(TEXT.proofFailureTitle)}
      role={failure.reason === 'blocked' ? 'alert' : 'status'}
      aria-live={failure.reason === 'blocked' ? 'assertive' : 'polite'}
    >
      <h4>{text(TEXT.proofFailureTitle)}</h4>
      <dl>
        <div>
          <dt>{text(TEXT.failureLocationLabel)}</dt>
          <dd>{text(TEXT.locations[failure.location])}</dd>
        </div>
        <div>
          <dt>{text(TEXT.failureReasonLabel)}</dt>
          <dd>{text(TEXT.reasons[failure.reason])}</dd>
        </div>
      </dl>
      <p>{format(TEXT.subsequentEdits, {
        count: failure.subsequentEditCount,
      })}</p>
      {canRequest ? (
        <>
          <label>
            <input
              type="checkbox"
              checked={confirmed}
              disabled={disabled || requested}
              onChange={(event) => setConfirmed(event.target.checked)}
            />
            {text(TEXT.destructiveConfirmation)}
          </label>
          <button
            className="secondary proof-revert-request"
            type="button"
            disabled={disabled || !confirmed || requested}
            onClick={() => {
              if (!confirmed || requested || callbackLockedRef.current) return
              callbackLockedRef.current = true
              setRequested(true)
              onRequestRevert(failure)
            }}
          >
            {format(TEXT.revertSteps, { steps: failure.undoStepsToRevert! })}
          </button>
          {requested && <p role="status">{text(TEXT.revertRequested)}</p>}
        </>
      ) : (
        <p>{text(TEXT.revertUnavailable)}</p>
      )}
    </section>
  )
}

function normalizePairProgress(
  provenPairCount: unknown,
  totalPairCount: unknown,
): Readonly<{ proven: number; total: number | null }> {
  const proven = isSafeCount(provenPairCount) ? provenPairCount : 0
  if (
    totalPairCount === null
    || !isSafeCount(totalPairCount)
    || totalPairCount < proven
  ) return Object.freeze({ proven: 0, total: null })
  return Object.freeze({ proven, total: totalPairCount })
}
