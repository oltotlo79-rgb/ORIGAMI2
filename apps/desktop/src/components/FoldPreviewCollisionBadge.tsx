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
  return (
    <span
      className={`fold-preview-collision ${collisionBadgeClass(summary)}`}
      title={description}
      data-collision-status={status}
      data-collision-risk={requiresSafetyReview ? 'blocking' : 'informational'}
    >
      表示姿勢｜{collisionBadgeText(summary)}
    </span>
  )
}
