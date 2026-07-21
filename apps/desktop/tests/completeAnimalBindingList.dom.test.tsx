import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { CompleteAnimalBindingList } from '../src/components/CompleteAnimalBindingList'

const target = (id: number, count: number, direction: [number, number, number], symmetry: 'none' | 'bilateral') => ({
  id, count, length_tenths_mm: id * 100, thickness_tenths_mm: id * 10,
  position_tenths_mm: [0, 0, 0] as [number, number, number], direction_milli: direction,
  symmetry, curvature_degrees: 0, joint: 'fixed' as const,
  motion_degrees: [0, 0] as [number, number], side: 'either' as const, priority: 50,
})

const valid = [
  target(1, 1, [0, -1000, 0], 'none'),
  target(2, 1, [1000, 0, 0], 'none'),
  target(3, 2, [1000, 0, 0], 'bilateral'),
  target(4, 4, [0, 1000, 0], 'bilateral'),
]

afterEach(cleanup)

describe('CompleteAnimalBindingList', () => {
  it('renders four strict rows and retranslates immediately', () => {
    const { rerender } = render(<CompleteAnimalBindingList locale="ja" protrusions={valid} />)
    expect(screen.getByRole('list', { name: '完全動物の四部位binding寸法' }).children).toHaveLength(4)
    expect(screen.getByText('binding 4・数 4・長さ 400・厚さ 40')).toBeTruthy()
    rerender(<CompleteAnimalBindingList locale="en" protrusions={valid} />)
    expect(screen.getByRole('list', { name: 'Four complete-animal binding dimensions' })).toBeTruthy()
    expect(screen.getByText('Binding 1 · count 1 · length 100 · thickness 10')).toBeTruthy()
  })

  it('hides the whole list for duplicate, overflow, or noncanonical bindings', () => {
    for (const forged of [
      valid.slice(0, 3),
      [valid[0], valid[1], valid[2], { ...valid[3], id: 3 }],
      [valid[1], valid[0], valid[2], valid[3]],
      [valid[0], valid[1], valid[2], { ...valid[3], count: 9 }],
    ]) {
      const { unmount } = render(<CompleteAnimalBindingList locale="en" protrusions={forged} />)
      expect(screen.queryByRole('list')).toBeNull()
      unmount()
    }
  })

  it('adds no focus target beside a disabled apply action', () => {
    render(<><CompleteAnimalBindingList locale="en" protrusions={valid} /><button disabled>Apply</button></>)
    const apply = screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement
    apply.focus()
    expect(apply.disabled).toBe(true)
    expect(screen.getByRole('list').querySelector('[tabindex]')).toBeNull()
  })
})
