import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { CompleteInsectBindingList } from '../src/components/CompleteInsectBindingList'

const target = (
  id: number,
  direction: [number, number, number],
  y: number,
  priority: number,
) => ({
  id, count: 2, length_tenths_mm: id * 100, thickness_tenths_mm: id * 10,
  position_tenths_mm: [0, y, 0] as [number, number, number], direction_milli: direction,
  symmetry: 'bilateral' as const, curvature_degrees: 0, joint: 'fixed' as const,
  motion_degrees: [0, 0] as [number, number], side: 'either' as const, priority,
})
const valid = [
  target(1, [1000, 0, 0], 0, 60), target(2, [0, -1000, 0], 0, 60),
  target(3, [1000, 0, 0], -30, 50), target(4, [1000, 0, 0], 0, 50),
  target(5, [1000, 0, 0], 30, 50),
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

  it('fails closed for missing, duplicate, ambiguous, asymmetric, or ABA leg pairs', () => {
    for (const forged of [
      valid.slice(0, 4),
      [valid[0], valid[1], valid[2], valid[3], { ...valid[4], id: 4 }],
      [valid[0], valid[1], valid[2], { ...valid[3], symmetry: 'none' as const }, valid[4]],
      [valid[0], valid[1], valid[2], { ...valid[3], position_tenths_mm: [0, -30, 0] as [number, number, number] }, valid[4]],
      [valid[0], valid[1], { ...valid[2], priority: 60 }, valid[3], valid[4]],
      [...valid, target(6, [1000, 0, 0], 40, 50)],
    ]) {
      const { unmount } = render(<CompleteInsectBindingList locale="en" protrusions={forged} />)
      expect(screen.queryByRole('list')).toBeNull()
      unmount()
    }
  })

  it('renders semantic roles and strict leg order independently of storage order', () => {
    render(<CompleteInsectBindingList
      locale="en"
      protrusions={[valid[4], valid[1], valid[2], valid[0], valid[3]]}
    />)
    const rows = Array.from(screen.getByRole('list').children)
    expect(rows.map((row) => row.textContent?.split(' · ')[0])).toEqual([
      'Wing pair',
      'Antenna pair',
      'Leg pair 1',
      'Leg pair 2',
      'Leg pair 3',
    ])
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining('binding 1'),
      expect.stringContaining('binding 2'),
      expect.stringContaining('binding 3'),
      expect.stringContaining('binding 4'),
      expect.stringContaining('binding 5'),
    ])
  })
})
