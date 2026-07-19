import assert from 'node:assert/strict'
import { afterEach, describe, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import {
  NativeStaticCollisionBadge,
  PoseBoundNativeStaticCollisionBadge,
} from '../src/components/NativeStaticCollisionBadge'
import type { FoldPreviewAppliedPoseSnapshot } from '../src/lib/foldPreviewAppliedPose'

afterEach(cleanup)

describe('NativeStaticCollisionBadge', () => {
  it('hides an old green certificate in the first paint of a newly rendered pose', () => {
    const oldPose = pose(10, 'stable')
    const nextPose = pose(45, 'stable')
    const { rerender } = render(
      <PoseBoundNativeStaticCollisionBadge
        state={certified()}
        observedPose={oldPose}
        renderedPose={oldPose}
      />,
    )
    assert.equal(
      screen.getByRole('status').getAttribute('data-native-collision-status'),
      'certified_nonblocking',
    )

    rerender(
      <PoseBoundNativeStaticCollisionBadge
        state={certified()}
        observedPose={oldPose}
        renderedPose={nextPose}
      />,
    )

    const badge = screen.getByRole('status')
    assert.equal(badge.getAttribute('data-native-collision-status'), 'checking')
    assert.equal(badge.getAttribute('data-collision-risk'), 'blocking')
    assert.equal(badge.getAttribute('aria-live'), 'polite')
    assert.equal(
      screen.queryByText('厳密判定｜ゼロ厚み面貫通・重なりなし'),
      null,
    )
  })

  it('hides a matching green certificate while the rendered pose is moving', () => {
    const observedPose = pose(10, 'stable')
    render(
      <PoseBoundNativeStaticCollisionBadge
        state={certified()}
        observedPose={observedPose}
        renderedPose={pose(10, 'running')}
      />,
    )

    const badge = screen.getByRole('status')
    assert.equal(badge.getAttribute('data-native-collision-status'), 'checking')
    assert.equal(badge.getAttribute('aria-live'), 'polite')
    assert.match(badge.textContent ?? '', /姿勢確定待ち/)
  })

  it('renders a certified result as a polite informational status', () => {
    render(
      <NativeStaticCollisionBadge
        state={{
          kind: 'ready',
          diagnostic: {
            status: 'certified_nonblocking',
            reason: null,
            expectedUnorderedFacePairs: 0,
            provenPenetratingPairs: 0,
            firstProvenPenetratingPair: null,
          },
        }}
      />,
    )

    const badge = screen.getByRole('status')
    assert.equal(badge.getAttribute('data-native-collision-status'), 'certified_nonblocking')
    assert.equal(badge.getAttribute('data-collision-risk'), 'informational')
    assert.equal(badge.getAttribute('aria-live'), 'polite')
  })

  it('renders proven zero-thickness penetration or overlap as an assertive blocking alert', () => {
    render(
      <NativeStaticCollisionBadge
        state={{
          kind: 'ready',
          diagnostic: {
            status: 'blocking',
            reason: 'proven_zero_thickness_penetration',
            expectedUnorderedFacePairs: 3,
            provenPenetratingPairs: 1,
            firstProvenPenetratingPair: {
              firstFaceId: 'face-a',
              secondFaceId: 'face-b',
            },
          },
        }}
      />,
    )

    const badge = screen.getByRole('alert')
    assert.equal(badge.getAttribute('data-native-collision-status'), 'penetrating')
    assert.equal(badge.getAttribute('data-collision-risk'), 'blocking')
    assert.equal(badge.getAttribute('aria-live'), 'assertive')
    assert.match(
      badge.textContent ?? '',
      /ゼロ厚み面貫通・重なり 1・安全認定不可/,
    )
  })

  it('renders proven positive-thickness material penetration with the existing red blocking class', () => {
    render(
      <NativeStaticCollisionBadge
        state={{
          kind: 'ready',
          diagnostic: {
            status: 'blocking',
            reason: 'proven_positive_thickness_penetration',
            expectedUnorderedFacePairs: 3,
            provenPenetratingPairs: 1,
            firstProvenPenetratingPair: {
              firstFaceId: '00000000-0000-4000-8000-000000000001',
              secondFaceId: '00000000-0000-4000-8000-000000000002',
            },
          },
        }}
      />,
    )

    const badge = screen.getByRole('alert')
    assert.match(badge.className, /(?:^|\s)is-blocked(?:\s|$)/u)
    assert.equal(badge.getAttribute('data-native-collision-status'), 'penetrating')
    assert.equal(badge.getAttribute('data-collision-risk'), 'blocking')
    assert.equal(badge.getAttribute('aria-live'), 'assertive')
    assert.equal(
      badge.textContent,
      '厳密判定｜紙厚を含む材料貫通 1・安全認定不可',
    )
    const accessible =
      '現在の表示姿勢で紙厚を含む材料の貫通1件を厳密証明したため、安全認定を遮断しました。'
    assert.equal(badge.getAttribute('title'), accessible)
    assert.equal(
      badge.getAttribute('aria-label'),
      `native厳密衝突判定。${accessible}`,
    )
  })

  it('does not hide an unavailable result', () => {
    render(
      <NativeStaticCollisionBadge
        state={{
          kind: 'ready',
          diagnostic: {
            status: 'unavailable',
            reason: 'pose_authority_unavailable',
            expectedUnorderedFacePairs: null,
            provenPenetratingPairs: null,
            firstProvenPenetratingPair: null,
          },
        }}
      />,
    )

    const badge = screen.getByRole('alert')
    assert.equal(badge.getAttribute('data-native-collision-status'), 'unavailable')
    assert.match(badge.textContent ?? '', /安全確認が必要/)
  })

  it('announces in-progress checks politely instead of as assertive alerts', () => {
    const { rerender } = render(
      <NativeStaticCollisionBadge state={{ kind: 'checking' }} />,
    )

    let badge = screen.getByRole('status')
    assert.equal(badge.getAttribute('data-collision-risk'), 'blocking')
    assert.equal(badge.getAttribute('aria-live'), 'polite')
    assert.equal(screen.queryByRole('alert'), null)

    rerender(<NativeStaticCollisionBadge state={{ kind: 'waiting' }} />)
    badge = screen.getByRole('status')
    assert.equal(badge.getAttribute('data-collision-risk'), 'blocking')
    assert.equal(badge.getAttribute('aria-live'), 'polite')
    assert.equal(screen.queryByRole('alert'), null)
  })

  it('offers a keyboard-accessible explicit retry after an execution failure', () => {
    let retries = 0
    const { rerender } = render(
      <NativeStaticCollisionBadge
        state={{ kind: 'failed' }}
        onRetry={() => {
          retries += 1
        }}
      />,
    )

    const retry = screen.getByRole('button', {
      name: '厳密衝突判定を再試行',
    })
    retry.focus()
    fireEvent.click(retry)
    assert.equal(retries, 1)
    assert.equal(retry.hasAttribute('disabled'), true)
    assert.equal(document.activeElement, retry)
    assert.equal(screen.getByRole('alert').getAttribute('aria-live'), 'assertive')

    rerender(
      <NativeStaticCollisionBadge
        state={{ kind: 'checking' }}
        onRetry={() => {
          retries += 1
        }}
      />,
    )
    const retrying = screen.getByRole('button', {
      name: '厳密衝突判定を再試行中',
    })
    assert.equal(retrying, retry)
    assert.equal(retrying.hasAttribute('disabled'), true)
    assert.equal(document.activeElement, retrying)

    rerender(
      <NativeStaticCollisionBadge
        state={{ kind: 'failed' }}
        onRetry={() => {
          retries += 1
        }}
      />,
    )
    const retryAgain = screen.getByRole('button', {
      name: '厳密衝突判定を再試行',
    })
    assert.equal(retryAgain, retry)
    assert.equal(retryAgain.hasAttribute('disabled'), false)
    assert.equal(document.activeElement, retryAgain)
  })

  for (const [reason, visibleReason] of [
    ['evidence_unavailable', '証拠不足'],
    ['resource_limit_exceeded', '資源上限'],
    ['inconsistent_state', '状態不整合'],
  ] as const) {
    it(`shows ${reason} visibly as a terminal assertive hold`, () => {
      render(
        <NativeStaticCollisionBadge
          state={{
            kind: 'ready',
            diagnostic: {
              status: 'blocking',
              reason,
              expectedUnorderedFacePairs:
                reason === 'evidence_unavailable' ? 3 : null,
              provenPenetratingPairs: null,
              firstProvenPenetratingPair: null,
            },
          }}
        />,
      )

      const badge = screen.getByRole('alert')
      assert.equal(badge.getAttribute('data-native-collision-status'), 'indeterminate')
      assert.equal(badge.getAttribute('data-collision-risk'), 'blocking')
      assert.equal(badge.getAttribute('aria-live'), 'assertive')
      assert.match(badge.textContent ?? '', new RegExp(visibleReason, 'u'))
    })
  }
})

function certified() {
  return {
    kind: 'ready',
    diagnostic: {
      status: 'certified_nonblocking',
      reason: null,
      expectedUnorderedFacePairs: 0,
      provenPenetratingPairs: 0,
      firstProvenPenetratingPair: null,
    },
  } as const
}

function pose(
  angleDegrees: number,
  state: FoldPreviewAppliedPoseSnapshot['state'],
): FoldPreviewAppliedPoseSnapshot {
  return {
    projectId: 'project',
    revision: 1,
    fixedFaceId: 'face',
    hingeAngles: [{ edgeId: 'hinge', angleDegrees }],
    state,
  }
}
