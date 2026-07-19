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
      ? COLLISION_BADGE_COMPONENT_TEXT.warningAriaLabel
      : COLLISION_BADGE_COMPONENT_TEXT.informationAriaLabel,
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
        COLLISION_BADGE_COMPONENT_TEXT.visible,
        { text },
      )}
    </span>
  )
}

const COLLISION_BADGE_COMPONENT_TEXT = Object.freeze({
  warningAriaLabel: Object.freeze({
    ja: '安全上の警告。表示姿勢。{text}',
    en: 'Safety warning. Current pose. {text}',
  }),
  informationAriaLabel: Object.freeze({
    ja: '衝突情報。表示姿勢。{text}',
    en: 'Collision information. Current pose. {text}',
  }),
  visible: Object.freeze({
    ja: '表示姿勢｜{text}',
    en: 'Current pose | {text}',
  }),
})
