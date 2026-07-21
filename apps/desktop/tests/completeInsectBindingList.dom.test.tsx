import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { CompleteInsectBindingList } from '../src/components/CompleteInsectBindingList'

const target = (id: number, direction: [number, number, number], y: number) => ({
  id, count: 2, length_tenths_mm: id * 100, thickness_tenths_mm: id * 10,
  position_tenths_mm: [0, y, 0] as [number, number, number], direction_milli: direction,
  symmetry: 'bilateral' as const, curvature_degrees: 0, joint: 'fixed' as const,
  motion_degrees: [0, 0] as [number, number], side: 'either' as const, priority: 50,
})
const valid = [
  target(1, [1000, 0, 0], 0), target(2, [0, -1000, 0], 0),
  target(3, [1000, 0, 0], -30), target(4, [1000, 0, 0], 0),
  target(5, [1000, 0, 0], 30),
]

afterEach(cleanup)

describe('CompleteInsectBindingList', () => {
  it('renders five semantic pairs and retranslates immediately', () => {
    const { rerender } = render(<CompleteInsectBindingList locale="ja" protrusions={valid} />)
    expect(screen.getByRole('list', { name: '完全昆虫の五組binding寸法' }).children).toHaveLength(5)
    rerender(<CompleteInsectBindingList locale="en" protrusions={valid} />)
    expect(screen.getByText('Wing pair · binding 1 · length 100 · thickness 10')).toBeTruthy()
    expect(screen.getByText('Leg pair 3 · binding 5 · length 500 · thickness 50')).toBeTruthy()
  })

  it('fails closed for missing, duplicate, reordered, asymmetric, or ABA leg pairs', () => {
    for (const forged of [
      valid.slice(0, 4),
      [valid[0], valid[1], valid[2], valid[3], { ...valid[4], id: 4 }],
      [valid[1], valid[0], valid[2], valid[3], valid[4]],
      [valid[0], valid[1], valid[2], { ...valid[3], symmetry: 'none' as const }, valid[4]],
      [valid[0], valid[1], valid[2], { ...valid[3], position_tenths_mm: [0, -30, 0] as [number, number, number] }, valid[4]],
    ]) {
      const { unmount } = render(<CompleteInsectBindingList locale="en" protrusions={forged} />)
      expect(screen.queryByRole('list')).toBeNull()
      unmount()
    }
  })
})
