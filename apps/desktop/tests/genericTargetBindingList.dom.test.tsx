import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { GenericTargetBindingList } from '../src/components/GenericTargetBindingList'

const target = (
  id: number,
  count: number,
  symmetry: 'none' | 'bilateral' | 'radial',
) => ({
  id, count, symmetry, length_tenths_mm: id * 100, thickness_tenths_mm: id * 10,
  position_tenths_mm: [0, 0, 0] as [number, number, number],
  direction_milli: [1000, 0, 0] as [number, number, number], curvature_degrees: 0,
  joint: 'fixed' as const, motion_degrees: [0, 0] as [number, number],
  side: 'either' as const, priority: 50,
})
const valid = [target(1, 4, 'bilateral'), target(2, 2, 'bilateral')]
const mixed = [target(1, 1, 'none'), target(2, 2, 'bilateral')]
afterEach(cleanup)

describe('GenericTargetBindingList', () => {
  it('rerenders both status words and ARIA in place without changing row identity', () => {
    const { rerender } = render(<GenericTargetBindingList locale="ja" protrusions={mixed} />)
    const list = screen.getByRole('list', { name: '上限付き汎用対象binding寸法' })
    const [firstRow, secondRow] = [...list.children]
    expect(firstRow?.textContent).toBe('binding 1・非対称単独・数 1・長さ 100・厚さ 10')
    expect(secondRow?.textContent).toBe('binding 2・左右対称・数 2・長さ 200・厚さ 20')

    rerender(<GenericTargetBindingList locale="en" protrusions={mixed} />)

    const rerenderedList = screen.getByRole('list', {
      name: 'Bounded generic target binding dimensions',
    })
    expect(rerenderedList).toBe(list)
    expect(rerenderedList.children[0]).toBe(firstRow)
    expect(rerenderedList.children[1]).toBe(secondRow)
    expect(firstRow?.textContent)
      .toBe('Binding 1 · asymmetric single · count 1 · length 100 · thickness 10')
    expect(secondRow?.textContent)
      .toBe('Binding 2 · bilateral · count 2 · length 200 · thickness 20')
  })

  it('accepts the inclusive upper bound with canonical bilateral quadruples', () => {
    const maximum = Array.from(
      { length: 8 },
      (_, index) => target(index + 1, 4, 'bilateral'),
    )
    render(<GenericTargetBindingList locale="en" protrusions={maximum} />)
    const list = screen.getByRole('list', {
      name: 'Bounded generic target binding dimensions',
    })
    expect(list.children).toHaveLength(8)
    expect(list.children[7]?.textContent)
      .toBe('Binding 8 · bilateral · count 4 · length 800 · thickness 80')
  })

  it('accepts radial and all domain-supported bilateral counts', () => {
    const radial = [target(1, 3, 'radial'), target(2, 2, 'radial')]
    const { unmount } = render(
      <GenericTargetBindingList locale="en" protrusions={radial} />,
    )
    let list = screen.getByRole('list', {
      name: 'Bounded generic target binding dimensions',
    })
    expect(list.children[0]?.textContent)
      .toBe('Binding 1 · radial · count 3 · length 100 · thickness 10')
    expect(list.children[1]?.textContent)
      .toBe('Binding 2 · radial · count 2 · length 200 · thickness 20')
    unmount()

    const bilateral = [target(1, 6, 'bilateral'), target(2, 8, 'bilateral')]
    render(<GenericTargetBindingList locale="en" protrusions={bilateral} />)
    list = screen.getByRole('list', {
      name: 'Bounded generic target binding dimensions',
    })
    expect(list.children[0]?.textContent)
      .toBe('Binding 1 · bilateral · count 6 · length 100 · thickness 10')
    expect(list.children[1]?.textContent)
      .toBe('Binding 2 · bilateral · count 8 · length 200 · thickness 20')
  })

  it('accepts strictly increasing nonconsecutive binding ids', () => {
    const sparse = [target(2, 3, 'radial'), target(7, 6, 'bilateral')]
    render(<GenericTargetBindingList locale="en" protrusions={sparse} />)
    const list = screen.getByRole('list', {
      name: 'Bounded generic target binding dimensions',
    })
    expect(list.children[0]?.textContent)
      .toBe('Binding 2 · radial · count 3 · length 200 · thickness 20')
    expect(list.children[1]?.textContent)
      .toBe('Binding 7 · bilateral · count 6 · length 700 · thickness 70')
  })

  it('rejects out-of-range and unsupported symmetry/count combinations', () => {
    for (const forged of [
      valid.slice(0, 1),
      Array.from({ length: 9 }, (_, i) => target(i + 1, 2, 'bilateral')),
      [valid[1], valid[0]],
      [target(2, 4, 'bilateral'), target(2, 2, 'bilateral')],
      [target(1, 1, 'radial'), valid[1]],
      [target(1, 9, 'radial'), valid[1]],
      [target(1, 3, 'bilateral'), valid[1]],
      [target(1, 5, 'bilateral'), valid[1]],
      [target(1, 7, 'bilateral'), valid[1]],
      [target(1, 2, 'none'), valid[1]],
    ]) {
      const { unmount } = render(<GenericTargetBindingList locale="en" protrusions={forged} />)
      expect(screen.queryByRole('list')).toBeNull(); unmount()
    }
  })
})
