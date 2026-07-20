import { useEffect, useRef, useState } from 'react'

import {
  presentNativeStaticCollision,
  presentNativeStaticCollisionPairDiagnostics,
  selectBoundNativeStaticCollisionView,
  type NativeStaticCollisionViewState,
} from '../lib/nativeStaticCollisionView'
import {
  foldPreviewAppliedPoseKey,
  type FoldPreviewAppliedPoseSnapshot,
} from '../lib/foldPreviewAppliedPose'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'

export type NativeStaticCollisionBadgeProps = Readonly<{
  state: NativeStaticCollisionViewState
  onRetry?: () => void
  localeStore?: LocaleStore
}>

export function NativeStaticCollisionBadge({
  state,
  onRetry,
  localeStore: localeStore_ = localeStore,
}: NativeStaticCollisionBadgeProps) {
  const locale = useLocale(localeStore_)
  const presentation = presentNativeStaticCollision(state, locale)
  const pairDetails = state.kind === 'ready'
    ? presentNativeStaticCollisionPairDiagnostics(state.diagnostic, locale)
    : null
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
    <div className="fold-preview-native-collision-group">
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
        aria-label={formatLocalizedText(
          locale,
          NATIVE_COLLISION_BADGE_TEXT.ariaLabel,
          { description: presentation.accessibleText },
        )}
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
                ? selectLocalizedText(
                  locale,
                  NATIVE_COLLISION_BADGE_TEXT.retryingAriaLabel,
                )
                : selectLocalizedText(
                  locale,
                  NATIVE_COLLISION_BADGE_TEXT.retryAriaLabel,
                )
            }
            disabled={retryInProgress}
            onClick={requestRetry}
          >
            {retryInProgress
              ? selectLocalizedText(
                locale,
                NATIVE_COLLISION_BADGE_TEXT.retrying,
              )
              : selectLocalizedText(locale, NATIVE_COLLISION_BADGE_TEXT.retry)}
          </button>
        ) : null}
      </span>
      {pairDetails === null ? null : (
        <section
          className={[
            'fold-preview-native-collision-pairs',
            pairDetails.hasBlockingPair ? 'has-blocking-pair' : '',
          ].filter(Boolean).join(' ')}
          data-native-collision-pair-risk={
            pairDetails.hasBlockingPair ? 'blocking' : 'informational'
          }
          aria-label={locale === 'ja'
            ? '面ペアごとの衝突分類'
            : 'Collision classification for each face pair'}
        >
          <div
            className="fold-preview-native-collision-pair-counts"
            title={pairDetails.accessibleCountsText}
          >
            {pairDetails.countsText}
          </div>
          {pairDetails.omittedText === null ? null : (
            <div
              className="fold-preview-native-collision-pair-omission"
              data-native-collision-omitted-pairs={
                pairDetails.omittedPairCount
              }
            >
              {pairDetails.omittedText}
            </div>
          )}
          {pairDetails.pairs.length === 0 ? null : (
            <ol className="fold-preview-native-collision-pair-list">
              {pairDetails.pairs.map((pair) => (
                <li
                  key={pair.key}
                  className={[
                    'fold-preview-native-collision-pair',
                    pair.rowClass,
                  ].join(' ')}
                  data-native-collision-pair-disposition={pair.disposition}
                  data-collision-risk={pair.risk}
                  title={pair.accessibleText}
                >
                  {pair.text}
                </li>
              ))}
            </ol>
          )}
        </section>
      )}
    </div>
  )
}

export type PoseBoundNativeStaticCollisionBadgeProps = Readonly<{
  state: NativeStaticCollisionViewState
  observedPose: FoldPreviewAppliedPoseSnapshot | null
  renderedPose: FoldPreviewAppliedPoseSnapshot | null
  onRetry?: () => void
  localeStore?: LocaleStore
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
  localeStore: localeStore_ = localeStore,
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
  return (
    <NativeStaticCollisionBadge
      state={gatedState}
      onRetry={onRetry}
      localeStore={localeStore_}
    />
  )
}

const NATIVE_COLLISION_BADGE_TEXT = Object.freeze({
  ariaLabel: Object.freeze({
    ja: 'native厳密衝突判定。{description}',
    en: 'Native exact collision check. {description}',
  }),
  retryingAriaLabel: Object.freeze({
    ja: '厳密衝突判定を再試行中',
    en: 'Retrying exact collision check',
  }),
  retryAriaLabel: Object.freeze({
    ja: '厳密衝突判定を再試行',
    en: 'Retry exact collision check',
  }),
  retrying: Object.freeze({ ja: '再判定中', en: 'Checking again' }),
  retry: Object.freeze({ ja: '再試行', en: 'Retry' }),
})
