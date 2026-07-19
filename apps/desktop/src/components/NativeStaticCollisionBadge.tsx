import { useEffect, useRef, useState } from 'react'

import {
  presentNativeStaticCollision,
  selectBoundNativeStaticCollisionView,
  type NativeStaticCollisionViewState,
} from '../lib/nativeStaticCollisionView'
import {
  foldPreviewAppliedPoseKey,
  type FoldPreviewAppliedPoseSnapshot,
} from '../lib/foldPreviewAppliedPose'

export type NativeStaticCollisionBadgeProps = Readonly<{
  state: NativeStaticCollisionViewState
  onRetry?: () => void
}>

export function NativeStaticCollisionBadge({
  state,
  onRetry,
}: NativeStaticCollisionBadgeProps) {
  const presentation = presentNativeStaticCollision(state)
  const previousStateKindRef = useRef(state.kind)
  const [retryInProgress, setRetryInProgress] = useState(false)
  useEffect(() => {
    const previousKind = previousStateKindRef.current
    previousStateKindRef.current = state.kind
    if (
      retryInProgress
      && (
        state.kind === 'ready'
        || (state.kind === 'failed' && previousKind !== 'failed')
      )
    ) {
      setRetryInProgress(false)
    }
  }, [retryInProgress, state.kind])
  const terminalBlocking = presentation.requiresSafetyReview
    && (state.kind === 'failed' || state.kind === 'ready')
  const retryVisible = onRetry !== undefined
    && (state.kind === 'failed' || retryInProgress)
  const requestRetry = () => {
    if (retryInProgress || onRetry === undefined) return
    setRetryInProgress(true)
    try {
      onRetry()
    } catch {
      setRetryInProgress(false)
    }
  }
  return (
    <span
      className={[
        'fold-preview-native-collision',
        presentation.badgeClass,
        retryVisible ? 'has-retry' : '',
      ].filter(Boolean).join(' ')}
      title={presentation.accessibleText}
      data-native-collision-status={presentation.dataStatus}
      data-collision-risk={
        presentation.requiresSafetyReview ? 'blocking' : 'informational'
      }
      role={terminalBlocking ? 'alert' : 'status'}
      aria-live={terminalBlocking ? 'assertive' : 'polite'}
      aria-atomic="true"
      aria-label={`native厳密衝突判定。${presentation.accessibleText}`}
    >
      <span className="fold-preview-native-collision-text">
        {presentation.badgeText}
      </span>
      {retryVisible ? (
        <button
          type="button"
          className="fold-preview-native-collision-retry"
          aria-label={
            retryInProgress
              ? '厳密衝突判定を再試行中'
              : '厳密衝突判定を再試行'
          }
          disabled={retryInProgress}
          onClick={requestRetry}
        >
          {retryInProgress ? '再判定中' : '再試行'}
        </button>
      ) : null}
    </span>
  )
}

export type PoseBoundNativeStaticCollisionBadgeProps = Readonly<{
  state: NativeStaticCollisionViewState
  observedPose: FoldPreviewAppliedPoseSnapshot | null
  renderedPose: FoldPreviewAppliedPoseSnapshot | null
  onRetry?: () => void
}>

/**
 * Gates native evidence against the pose painted by FoldPreview in this same
 * render. The parent's pose observation is published after paint, so using it
 * alone would leave one frame in which an old green certificate is visible.
 */
export function PoseBoundNativeStaticCollisionBadge({
  state,
  observedPose,
  renderedPose,
  onRetry,
}: PoseBoundNativeStaticCollisionBadgeProps) {
  const renderedPoseKey = foldPreviewAppliedPoseKey(renderedPose)
  const gatedState = selectBoundNativeStaticCollisionView(
    renderedPose?.state === 'running',
    renderedPoseKey,
    {
      requestKey: foldPreviewAppliedPoseKey(observedPose),
      view: state,
    },
  )
  return <NativeStaticCollisionBadge state={gatedState} onRetry={onRetry} />
}
