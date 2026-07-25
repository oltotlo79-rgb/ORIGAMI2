import {
  collisionBadgeClass,
  collisionBadgeText,
  collisionDataStatus,
  type CollisionSummary,
} from '../lib/foldPreviewCollisionView'
import {
  formatLocalizedText,
  localeStore,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'
import { FOLD_PREVIEW_COLLISION_BADGE_TEXT } from '../lib/foldPreviewCollisionBadgeText.ts'

export type FoldPreviewCollisionBadgeProps = Readonly<{
  summary: CollisionSummary | null
  description: string
  localeStore?: LocaleStore
}>

export function FoldPreviewCollisionBadge({
  summary,
  description,
  localeStore: localeStore_ = localeStore,
}: FoldPreviewCollisionBadgeProps) {
  const locale = useLocale(localeStore_)
  const status = collisionDataStatus(summary)
  const requiresSafetyReview = status === 'penetrating'
    || status === 'indeterminate'
    || status === 'hinge-unresolved'
    || status === 'unavailable'
  const text = collisionBadgeText(summary, locale)
  const ariaLabel = formatLocalizedText(
    locale,
    requiresSafetyReview
      ? FOLD_PREVIEW_COLLISION_BADGE_TEXT.warningAriaLabel
      : FOLD_PREVIEW_COLLISION_BADGE_TEXT.informationAriaLabel,
    { text },
  )
  return (
    <span
      className={`fold-preview-collision ${collisionBadgeClass(summary)}`}
      title={description}
      data-collision-status={status}
      data-collision-risk={requiresSafetyReview ? 'blocking' : 'informational'}
      role={requiresSafetyReview ? 'alert' : 'status'}
      aria-live={requiresSafetyReview ? 'assertive' : 'polite'}
      aria-atomic="true"
      aria-label={ariaLabel}
    >
      {formatLocalizedText(
        locale,
        FOLD_PREVIEW_COLLISION_BADGE_TEXT.visible,
        { text },
      )}
    </span>
  )
}
