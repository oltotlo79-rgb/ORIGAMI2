import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { UnderlayPanel } from '../src/components/UnderlayPanel'

afterEach(cleanup)

describe('UnderlayPanel', () => {
  it('imports, selects, transforms, and removes with Japanese ARIA', () => {
    const onImport = vi.fn()
    const onUpdate = vi.fn()
    const onRemove = vi.fn()
    render(<UnderlayPanel
      locale="ja"
      layers={[{
        id: '10000000-0000-4000-8000-000000000001',
        name: '下絵',
        content_kind: 'underlay',
        visible: true,
        locked: false,
        opacity: 1,
      }]}
      underlays={[{
        id: '20000000-0000-4000-8000-000000000001',
        asset: '30000000-0000-4000-8000-000000000001',
        transform: {
          position: { x: 1, y: 2 },
          scale_x: 0.1,
          scale_y: 0.2,
          rotation_degrees: 5,
        },
        opacity: 0.8,
        layer: '10000000-0000-4000-8000-000000000001',
      }]}
      onImport={onImport}
      onUpdate={onUpdate}
      onRemove={onRemove}
    />)
    fireEvent.click(screen.getByRole('button', { name: '画像を追加' }))
    expect(onImport).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '下絵 1' }))
    const form = screen.getByRole('form', { name: '下絵の配置と変形' })
    expect(form).toBeTruthy()
    fireEvent.submit(form)
    expect(onUpdate).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '削除' }))
    expect(onRemove).toHaveBeenCalledWith('20000000-0000-4000-8000-000000000001')
  })

  it('disables mutation for a locked layer', () => {
    render(<UnderlayPanel locale="en" underlays={[]} layers={[{
      id: '10000000-0000-4000-8000-000000000001',
      name: 'Locked',
      content_kind: 'underlay',
      visible: true,
      locked: true,
      opacity: 1,
    }]} onImport={vi.fn()} onUpdate={vi.fn()} onRemove={vi.fn()} />)
    expect((screen.getByRole('button', { name: 'Add image' }) as HTMLButtonElement).disabled).toBe(true)
  })
})
