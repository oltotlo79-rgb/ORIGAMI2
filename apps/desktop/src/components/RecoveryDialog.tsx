import { useEffect, useRef, useState } from 'react'

import type {
  RecoveryCandidateAvailable,
  RecoveryCandidateInvalid,
} from '../lib/recoveryClient.ts'
import {
  localeStore,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
import './RecoveryDialog.css'

export type RecoveryDialogProps = Readonly<{
  candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid
  busy: boolean
  error: boolean
  onRestore: (candidate: RecoveryCandidateAvailable) => void | Promise<void>
  onDiscard: (
    candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid,
  ) => void | Promise<void>
  onRetry: () => void | Promise<void>
  localeStore?: LocaleStore
}>

type RecoveryAction = 'restore' | 'discard' | 'retry'

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

/**
 * A mandatory startup decision. The parent owns background `inert` state and
 * removes this component only after restore/discard succeeds.
 */
export function RecoveryDialog({
  candidate,
  busy,
  error,
  onRestore,
  onDiscard,
  onRetry,
  localeStore: localeStore_ = localeStore,
}: RecoveryDialogProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const [pendingAction, setPendingAction] = useState<RecoveryAction | null>(null)
  const [localError, setLocalError] = useState(false)
  const dialogRef = useRef<HTMLElement>(null)
  const restoreButtonRef = useRef<HTMLButtonElement>(null)
  const retryButtonRef = useRef<HTMLButtonElement>(null)
  const submittingRef = useRef(false)
  const busyRef = useRef(busy)
  const mountedRef = useRef(true)
  const operationSequenceRef = useRef(0)
  const candidateStatusRef = useRef(candidate.status)
  busyRef.current = busy
  candidateStatusRef.current = candidate.status

  const effectiveBusy = busy || pendingAction !== null
  const errorVisible = error || localError

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      operationSequenceRef.current += 1
    }
  }, [])

  useEffect(() => {
    operationSequenceRef.current += 1
    submittingRef.current = false
    setPendingAction(null)
    setLocalError(false)
  }, [candidate.recovery_id])

  useEffect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null

    const preferredFocus = () => {
      const preferred = candidateStatusRef.current === 'available'
        ? restoreButtonRef.current
        : retryButtonRef.current
      if (preferred && !preferred.disabled) {
        preferred.focus()
      } else {
        dialogRef.current?.focus()
      }
    }

    const focusableElements = () => {
      const dialog = dialogRef.current
      if (!dialog) return []
      return Array.from(
        dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
      ).filter((element) => !element.closest('[inert]'))
    }

    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      if (
        dialog
        && event.target instanceof Node
        && !dialog.contains(event.target)
      ) {
        preferredFocus()
      }
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        event.stopImmediatePropagation()
        return
      }
      if (event.key !== 'Tab') return

      const dialog = dialogRef.current
      if (!dialog) return
      const focusable = focusableElements()
      if (focusable.length === 0) {
        event.preventDefault()
        dialog.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      if (event.shiftKey && (active === first || !dialog.contains(active))) {
        event.preventDefault()
        last.focus()
      } else if (
        !event.shiftKey
        && (active === last || !dialog.contains(active))
      ) {
        event.preventDefault()
        first.focus()
      }
    }

    const frame = requestAnimationFrame(preferredFocus)
    document.addEventListener('focusin', handleFocusIn, true)
    document.addEventListener('keydown', handleKeyDown, true)
    return () => {
      cancelAnimationFrame(frame)
      document.removeEventListener('focusin', handleFocusIn, true)
      document.removeEventListener('keydown', handleKeyDown, true)
      if (previouslyFocused?.isConnected) previouslyFocused.focus()
    }
  }, [])

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const preferred = candidate.status === 'available'
        ? restoreButtonRef.current
        : retryButtonRef.current
      if (preferred && !preferred.disabled) preferred.focus()
    })
    return () => cancelAnimationFrame(frame)
  }, [candidate.status])

  const runAction = async (
    action: RecoveryAction,
    callback: () => void | Promise<void>,
  ) => {
    if (submittingRef.current || busyRef.current) return
    submittingRef.current = true
    const operation = ++operationSequenceRef.current
    setPendingAction(action)
    setLocalError(false)
    try {
      await callback()
    } catch {
      if (mountedRef.current && operation === operationSequenceRef.current) {
        setLocalError(true)
      }
    } finally {
      if (mountedRef.current && operation === operationSequenceRef.current) {
        submittingRef.current = false
        setPendingAction(null)
      }
    }
  }

  const restore = () => {
    if (candidate.status !== 'available') return
    void runAction('restore', () => onRestore(candidate))
  }

  const discard = () => {
    void runAction('discard', () => onDiscard(candidate))
  }

  const retry = () => {
    void runAction('retry', onRetry)
  }

  const available = candidate.status === 'available'
  const descriptionId = available
    ? 'recovery-dialog-available-description'
    : 'recovery-dialog-invalid-description'

  return (
    <div
      className="dialog-backdrop recovery-dialog-backdrop"
      data-testid="recovery-dialog-backdrop"
    >
      <section
        ref={dialogRef}
        className="recovery-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="recovery-dialog-title"
        aria-describedby={descriptionId}
        aria-busy={effectiveBusy}
        tabIndex={-1}
      >
        <header className="recovery-dialog-header">
          <span className="recovery-dialog-eyebrow">
            {text(RECOVERY_DIALOG_TEXT.eyebrow)}
          </span>
          <h2 id="recovery-dialog-title">
            {available
              ? text(RECOVERY_DIALOG_TEXT.availableTitle)
              : text(RECOVERY_DIALOG_TEXT.invalidTitle)}
          </h2>
        </header>

        <div className="recovery-dialog-body">
          {available ? (
            <>
              <p id={descriptionId}>
                {text(RECOVERY_DIALOG_TEXT.availableDescription)}
              </p>
              <dl className="recovery-dialog-metadata">
                <div>
                  <dt>{text(RECOVERY_DIALOG_TEXT.lastUpdated)}</dt>
                  <dd>
                    {formatRecoveryTimestamp(
                      candidate.updated_at_unix_ms,
                      locale,
                    )}
                  </dd>
                </div>
              </dl>
              <p className="recovery-dialog-caution">
                {text(RECOVERY_DIALOG_TEXT.caution)}
              </p>
            </>
          ) : (
            <p id={descriptionId} className="recovery-dialog-invalid">
              {text(RECOVERY_DIALOG_TEXT.invalidDescription)}
            </p>
          )}

          {errorVisible && (
            <p
              className="recovery-dialog-error"
              role="alert"
              aria-live="assertive"
            >
              {text(RECOVERY_DIALOG_TEXT.actionError)}
            </p>
          )}
        </div>

        <footer className="recovery-dialog-actions">
          {available && (
            <button
              ref={restoreButtonRef}
              type="button"
              className="recovery-dialog-primary"
              disabled={effectiveBusy}
              onClick={restore}
            >
              {pendingAction === 'restore'
                ? text(RECOVERY_DIALOG_TEXT.restoring)
                : text(RECOVERY_DIALOG_TEXT.restore)}
            </button>
          )}
          <button
            ref={available ? undefined : retryButtonRef}
            type="button"
            disabled={effectiveBusy}
            onClick={retry}
          >
            {pendingAction === 'retry'
              ? text(RECOVERY_DIALOG_TEXT.checking)
              : text(RECOVERY_DIALOG_TEXT.retry)}
          </button>
          <button
            type="button"
            className="recovery-dialog-discard"
            disabled={effectiveBusy}
            onClick={discard}
          >
            {pendingAction === 'discard'
              ? text(RECOVERY_DIALOG_TEXT.discarding)
              : text(RECOVERY_DIALOG_TEXT.discard)}
          </button>
        </footer>
      </section>
    </div>
  )
}

function formatRecoveryTimestamp(
  timestamp: number | null,
  locale: Locale,
): string {
  if (timestamp === null) {
    return selectLocalizedText(locale, RECOVERY_DIALOG_TEXT.noTimestamp)
  }
  try {
    const date = new Date(timestamp)
    if (!Number.isFinite(date.getTime())) {
      return selectLocalizedText(locale, RECOVERY_DIALOG_TEXT.unavailable)
    }
    return new Intl.DateTimeFormat(locale === 'ja' ? 'ja-JP' : 'en-US', {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(date)
  } catch {
    return selectLocalizedText(locale, RECOVERY_DIALOG_TEXT.unavailable)
  }
}

const RECOVERY_DIALOG_TEXT = Object.freeze({
  eyebrow: Object.freeze({ ja: '起動時の復旧', en: 'Startup recovery' }),
  availableTitle: Object.freeze({
    ja: '未保存の編集内容を復元しますか？',
    en: 'Restore unsaved edits?',
  }),
  invalidTitle: Object.freeze({
    ja: '復旧データを確認できません',
    en: 'Recovery data could not be verified',
  }),
  availableDescription: Object.freeze({
    ja: '前回の終了前に保存できなかった編集内容が見つかりました。復元するか、破棄するかを選んでください。',
    en: 'Edits that could not be saved before the previous session ended were found. Choose whether to restore or discard them.',
  }),
  lastUpdated: Object.freeze({ ja: '最終更新', en: 'Last updated' }),
  caution: Object.freeze({
    ja: '復元後の作品は未保存の新しい編集状態として開きます。元のファイルを自動で上書きすることはありません。',
    en: 'The restored work opens as a new unsaved editing state. The original file is never overwritten automatically.',
  }),
  invalidDescription: Object.freeze({
    ja: '復旧データが破損しているか、このバージョンでは読み取れません。再確認するか、安全に破棄してください。',
    en: 'The recovery data is damaged or cannot be read by this version. Check again or discard it safely.',
  }),
  actionError: Object.freeze({
    ja: '復旧データを処理できませんでした。もう一度お試しください。',
    en: 'The recovery data could not be processed. Try again.',
  }),
  restoring: Object.freeze({ ja: '復元中…', en: 'Restoring…' }),
  restore: Object.freeze({ ja: '復元する', en: 'Restore' }),
  checking: Object.freeze({ ja: '確認中…', en: 'Checking…' }),
  retry: Object.freeze({ ja: '再確認', en: 'Check again' }),
  discarding: Object.freeze({ ja: '破棄中…', en: 'Discarding…' }),
  discard: Object.freeze({ ja: '破棄する', en: 'Discard' }),
  noTimestamp: Object.freeze({ ja: '記録なし', en: 'No record' }),
  unavailable: Object.freeze({
    ja: '確認できません',
    en: 'Unavailable',
  }),
})
