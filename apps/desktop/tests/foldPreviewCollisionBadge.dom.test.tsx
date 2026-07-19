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
    expect(badge.getAttribute('role')).toBe('alert')
    expect(badge.getAttribute('aria-live')).toBe('assertive')
    expect(badge.getAttribute('aria-atomic')).toBe('true')
    expect(badge.getAttribute('aria-label')).toMatch(
      /^安全上の警告。表示姿勢。交差の可能性・判定保留/u,
    )
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

  it('promotes unresolved hinge evidence to the same assertive danger contract', () => {
    render(
      <FoldPreviewCollisionBadge
        summary={ready({
          nonAdjacentContacts: 1,
          hingeInteractions: 1,
          hingeUnresolvedInteractions: 1,
        })}
        description="接触1件、ヒンジ未解決1件。安全確認が必要です。"
      />,
    )

    const badge = screen.getByRole('alert', {
      name: /^安全上の警告。表示姿勢。交差の可能性・判定保留/u,
    })
    expect(badge.classList.contains('has-indeterminate')).toBe(true)
    expect(badge.classList.contains('has-hinge-candidates')).toBe(false)
    expect(badge.getAttribute('data-collision-status')).toBe('hinge-unresolved')
    expect(badge.getAttribute('data-collision-risk')).toBe('blocking')
    expect(badge.getAttribute('aria-live')).toBe('assertive')
    expect(badge.textContent).toContain('ヒンジ未解決 1')
    expect(badge.textContent).toContain('安全確認が必要')
    expect(badge.textContent).toContain('接触 1')
    expect(badge.textContent).not.toContain('貫通 0')
  })

  it('treats unavailable collision evidence as an assertive safety hold', () => {
    render(
      <FoldPreviewCollisionBadge
        summary={{ kind: 'unavailable', requestKey: 'pose' }}
        description="現在姿勢の衝突判定は利用できません。安全確認が必要です。"
      />,
    )

    const badge = screen.getByRole('alert', {
      name: '安全上の警告。表示姿勢。衝突判定不能・安全確認が必要',
    })
    expect(badge.classList.contains('is-unavailable')).toBe(true)
    expect(badge.getAttribute('data-collision-status')).toBe('unavailable')
    expect(badge.getAttribute('data-collision-risk')).toBe('blocking')
    expect(badge.getAttribute('aria-live')).toBe('assertive')
  })

  it('shows an exact shared-vertex allowance without presenting penetration', () => {
    render(
      <FoldPreviewCollisionBadge
        summary={ready({
          nonAdjacentAllowedSharedVertexContacts: 1,
        })}
        description="共有頂点のみと証明した許容接触1件"
      />,
    )

    const badge = screen.getByText(
      '表示姿勢｜共有頂点の許容接触 1・貫通 0',
    )
    expect(badge.classList.contains('has-topology-allowance')).toBe(true)
    expect(badge.getAttribute('data-collision-status')).toBe('topology-model')
    expect(badge.getAttribute('data-collision-risk')).toBe('informational')
    expect(badge.getAttribute('role')).toBe('status')
    expect(badge.getAttribute('aria-live')).toBe('polite')
    expect(badge.getAttribute('aria-label')).toMatch(/^衝突情報。表示姿勢。/u)
    expect(badge.getAttribute('title')).toContain('共有頂点のみと証明')
  })

  it('keeps contact and allowed flat stacks nonblocking and politely announced', () => {
    const { rerender } = render(
      <FoldPreviewCollisionBadge
        summary={ready({ nonAdjacentContacts: 1 })}
        description="接触1件"
      />,
    )

    let badge = screen.getByRole('status', {
      name: /^衝突情報。表示姿勢。接触 1/u,
    })
    expect(badge.classList.contains('has-contact')).toBe(true)
    expect(badge.getAttribute('data-collision-risk')).toBe('informational')
    expect(badge.getAttribute('aria-live')).toBe('polite')

    rerender(
      <FoldPreviewCollisionBadge
        summary={ready({
          hingeInteractions: 1,
          hingeModelFlatSurfaceStacks: 1,
        })}
        description="厚さ0の許容平坦積層1件"
      />,
    )
    badge = screen.getByRole('status', {
      name: /^衝突情報。表示姿勢。厚さ0の許容平坦積層 1/u,
    })
    expect(badge.classList.contains('has-hinge-candidates')).toBe(true)
    expect(badge.getAttribute('data-collision-risk')).toBe('informational')
    expect(badge.getAttribute('aria-live')).toBe('polite')
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
    nonAdjacentAllowedSharedVertexContacts: 0,
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
