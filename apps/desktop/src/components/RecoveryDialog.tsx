import { useEffect, useRef, useState } from 'react'

import type {
  RecoveryCandidateAvailable,
  RecoveryCandidateInvalid,
} from '../lib/recoveryClient.ts'
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

const FIXED_ACTION_ERROR =
  '復旧データを処理できませんでした。もう一度お試しください。'

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
}: RecoveryDialogProps) {
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
          <span className="recovery-dialog-eyebrow">起動時の復旧</span>
          <h2 id="recovery-dialog-title">
            {available
              ? '未保存の編集内容を復元しますか？'
              : '復旧データを確認できません'}
          </h2>
        </header>

        <div className="recovery-dialog-body">
          {available ? (
            <>
              <p id={descriptionId}>
                前回の終了前に保存できなかった編集内容が見つかりました。
                復元するか、破棄するかを選んでください。
              </p>
              <dl className="recovery-dialog-metadata">
                <div>
                  <dt>最終更新</dt>
                  <dd>{formatRecoveryTimestamp(candidate.updated_at_unix_ms)}</dd>
                </div>
              </dl>
              <p className="recovery-dialog-caution">
                復元後の作品は未保存の新しい編集状態として開きます。
                元のファイルを自動で上書きすることはありません。
              </p>
            </>
          ) : (
            <p id={descriptionId} className="recovery-dialog-invalid">
              復旧データが破損しているか、このバージョンでは読み取れません。
              再確認するか、安全に破棄してください。
            </p>
          )}

          {errorVisible && (
            <p
              className="recovery-dialog-error"
              role="alert"
              aria-live="assertive"
            >
              {FIXED_ACTION_ERROR}
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
              {pendingAction === 'restore' ? '復元中…' : '復元する'}
            </button>
          )}
          <button
            ref={available ? undefined : retryButtonRef}
            type="button"
            disabled={effectiveBusy}
            onClick={retry}
          >
            {pendingAction === 'retry' ? '確認中…' : '再確認'}
          </button>
          <button
            type="button"
            className="recovery-dialog-discard"
            disabled={effectiveBusy}
            onClick={discard}
          >
            {pendingAction === 'discard' ? '破棄中…' : '破棄する'}
          </button>
        </footer>
      </section>
    </div>
  )
}

function formatRecoveryTimestamp(timestamp: number | null): string {
  if (timestamp === null) return '記録なし'
  try {
    const date = new Date(timestamp)
    if (!Number.isFinite(date.getTime())) return '確認できません'
    return new Intl.DateTimeFormat('ja-JP', {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(date)
  } catch {
    return '確認できません'
  }
}
