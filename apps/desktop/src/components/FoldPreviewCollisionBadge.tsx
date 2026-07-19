import {
  collisionBadgeClass,
  collisionBadgeText,
  collisionDataStatus,
  type CollisionSummary,
} from '../lib/foldPreviewCollisionView'

export type FoldPreviewCollisionBadgeProps = Readonly<{
  summary: CollisionSummary | null
  description: string
}>

export function FoldPreviewCollisionBadge({
  summary,
  description,
}: FoldPreviewCollisionBadgeProps) {
  const status = collisionDataStatus(summary)
  const requiresSafetyReview = status === 'penetrating'
    || status === 'indeterminate'
    || status === 'hinge-unresolved'
    || status === 'unavailable'
  const text = collisionBadgeText(summary)
  return (
    <span
      className={`fold-preview-collision ${collisionBadgeClass(summary)}`}
      title={description}
      data-collision-status={status}
      data-collision-risk={requiresSafetyReview ? 'blocking' : 'informational'}
      role={requiresSafetyReview ? 'alert' : 'status'}
      aria-live={requiresSafetyReview ? 'assertive' : 'polite'}
      aria-atomic="true"
      aria-label={`${requiresSafetyReview ? '安全上の警告' : '衝突情報'}。表示姿勢。${text}`}
    >
      表示姿勢｜{text}
    </span>
  )
}
