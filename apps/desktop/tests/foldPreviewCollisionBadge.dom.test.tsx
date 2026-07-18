import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { FoldPreviewCollisionBadge } from '../src/components/FoldPreviewCollisionBadge'
import type { CollisionSummary } from '../src/lib/foldPreviewCollisionView'

type ReadySummary = Extract<CollisionSummary, { kind: 'ready' }>

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('FoldPreviewCollisionBadge', () => {
  it('shows indeterminate intersections as an explicit blocking collision risk', () => {
    const { container } = render(
      <FoldPreviewCollisionBadge
        summary={ready({ indeterminateInteractions: 2 })}
        description="交差の可能性・判定保留2件。判定保留は安全確認が必要です。"
      />,
    )

    const badge = screen.getByText(
      '表示姿勢｜交差の可能性・判定保留 2・安全確認が必要',
    )
    expect(badge.classList.contains('fold-preview-collision')).toBe(true)
    expect(badge.classList.contains('has-indeterminate')).toBe(true)
    expect(badge.getAttribute('data-collision-status')).toBe('indeterminate')
    expect(badge.getAttribute('data-collision-risk')).toBe('blocking')
    expect(badge.getAttribute('title')).toContain('判定保留は安全確認が必要')
    expect(container.querySelector('.has-penetrations')).toBeNull()
  })

  it('marks penetration and indeterminate badges with the same blocking contract', () => {
    const { rerender } = render(
      <FoldPreviewCollisionBadge
        summary={ready({ nonAdjacentPenetrations: 1 })}
        description="貫通1件"
      />,
    )
    expect(
      screen.getByText(/表示姿勢｜貫通 1/u)
        .getAttribute('data-collision-risk'),
    ).toBe('blocking')

    rerender(
      <FoldPreviewCollisionBadge
        summary={ready({ indeterminateInteractions: 1 })}
        description="交差の可能性・判定保留1件"
      />,
    )
    expect(
      screen.getByText(/表示姿勢｜交差の可能性・判定保留 1/u)
        .getAttribute('data-collision-risk'),
    ).toBe('blocking')
  })

  it('keeps the indeterminate warning visible beside a definitive penetration', () => {
    render(
      <FoldPreviewCollisionBadge
        summary={ready({
          narrowInteractions: 3,
          nonAdjacentPenetrations: 1,
          indeterminateInteractions: 2,
        })}
        description="非隣接貫通1件、交差の可能性・判定保留2件。判定保留は安全確認が必要です。"
      />,
    )

    const badge = screen.getByText(
      '表示姿勢｜貫通 1（ヒンジ外 0）・接触 0・交差の可能性・判定保留 2・安全確認が必要',
    )
    expect(badge.classList.contains('has-penetrations')).toBe(true)
    expect(badge.getAttribute('data-collision-status')).toBe('penetrating')
    expect(badge.getAttribute('data-collision-risk')).toBe('blocking')
    expect(badge.getAttribute('title')).toContain('判定保留2件')
    expect(badge.getAttribute('title')).toContain('安全確認が必要')
  })
})

function ready(overrides: Partial<ReadySummary> = {}): ReadySummary {
  return {
    kind: 'ready',
    requestKey: 'pose',
    totalCandidates: 1,
    nonAdjacentCandidates: 1,
    hingeAdjacentCandidates: 0,
    narrowInteractions: 1,
    nonAdjacentPenetrations: 0,
    nonAdjacentContacts: 0,
    hingeInteractions: 0,
    hingeModelAllowedContacts: 0,
    hingeModelCorridorOverlaps: 0,
    hingeModelFlatSurfaceStacks: 0,
    hingeLayerOffsetUnmodeled: 0,
    hingeOutsidePenetrations: 0,
    hingeOutsideContacts: 0,
    hingeUnresolvedInteractions: 0,
    indeterminateInteractions: 0,
    ...overrides,
  }
}
