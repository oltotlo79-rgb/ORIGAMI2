import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AnnotationPanel } from '../src/components/AnnotationPanel.tsx'
import type { LayerRecordV1 } from '../src/lib/projectLayers.ts'

const LOCKED_LAYER_ID = '10000000-0000-4000-8000-000000000001'
const FIRST_UNLOCKED_LAYER_ID = '20000000-0000-4000-8000-000000000001'
const SECOND_UNLOCKED_LAYER_ID = '30000000-0000-4000-8000-000000000001'

function annotationLayer(
  id: string,
  name: string,
  locked: boolean,
): LayerRecordV1 {
  return {
    id,
    name,
    content_kind: 'annotation',
    visible: true,
    locked,
    opacity: 1,
  }
}

function renderPanel(
  layers: readonly LayerRecordV1[],
  onAdd = vi.fn(),
) {
  render(<AnnotationPanel
    locale="en"
    annotations={[]}
    layers={layers}
    vertices={[]}
    onAdd={onAdd}
    onUpdate={vi.fn()}
    onRemove={vi.fn()}
  />)
  return onAdd
}

afterEach(cleanup)

describe('AnnotationPanel new annotation layer selection', () => {
  it('skips a locked first layer and passes the unlocked stored layer ID through on save', () => {
    const onAdd = renderPanel([
      annotationLayer(LOCKED_LAYER_ID, 'Locked first', true),
      annotationLayer(FIRST_UNLOCKED_LAYER_ID, 'Writable second', false),
    ])

    fireEvent.click(screen.getByRole('button', { name: 'New' }))

    const layerSelect = screen.getByRole('combobox', { name: 'Layer' }) as HTMLSelectElement
    expect(layerSelect.value).toBe(FIRST_UNLOCKED_LAYER_ID)
    expect(
      (screen.getByRole('option', { name: 'Locked first (locked)' }) as HTMLOptionElement).disabled,
    ).toBe(true)
    expect(
      (screen.getByRole('option', { name: 'Writable second' }) as HTMLOptionElement).disabled,
    ).toBe(false)

    fireEvent.change(screen.getByRole('textbox', { name: 'Text' }), {
      target: { value: 'Stored on the writable layer' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))

    expect(onAdd).toHaveBeenCalledOnce()
    expect(onAdd).toHaveBeenCalledWith(expect.objectContaining({
      text: 'Stored on the writable layer',
      layer: FIRST_UNLOCKED_LAYER_ID,
    }))
  })

  it('rejects new annotation creation when every annotation layer is locked', () => {
    renderPanel([
      annotationLayer(LOCKED_LAYER_ID, 'Locked only', true),
    ])

    expect((screen.getByRole('button', { name: 'New' }) as HTMLButtonElement).disabled).toBe(true)
    expect(screen.queryByRole('form', { name: 'Edit annotation' })).toBeNull()
  })

  it('preserves stored order when choosing among writable annotation layers', () => {
    renderPanel([
      annotationLayer(FIRST_UNLOCKED_LAYER_ID, 'Writable first', false),
      annotationLayer(SECOND_UNLOCKED_LAYER_ID, 'Writable second', false),
      annotationLayer(LOCKED_LAYER_ID, 'Locked last', true),
    ])

    fireEvent.click(screen.getByRole('button', { name: 'New' }))

    expect(
      (screen.getByRole('combobox', { name: 'Layer' }) as HTMLSelectElement).value,
    ).toBe(FIRST_UNLOCKED_LAYER_ID)
  })

  it('promotes a native-accepted new annotation to edit mode instead of adding its ID twice', () => {
    const layer = annotationLayer(
      FIRST_UNLOCKED_LAYER_ID,
      'Writable',
      false,
    )
    const onAdd = vi.fn()
    const onUpdate = vi.fn()
    const props = {
      locale: 'en' as const,
      layers: [layer],
      vertices: [],
      onAdd,
      onUpdate,
      onRemove: vi.fn(),
    }
    const view = render(<AnnotationPanel annotations={[]} {...props} />)

    fireEvent.click(screen.getByRole('button', { name: 'New' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'Text' }), {
      target: { value: 'Accepted annotation' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    const accepted = onAdd.mock.calls[0]?.[0]
    expect(accepted?.layer).toBe(FIRST_UNLOCKED_LAYER_ID)

    view.rerender(<AnnotationPanel annotations={[accepted]} {...props} />)
    expect(
      screen.getByRole('button', { name: 'Accepted annotation' }).getAttribute('aria-pressed'),
    ).toBe('true')

    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(onAdd).toHaveBeenCalledOnce()
    expect(onUpdate).toHaveBeenCalledOnce()
    expect(onUpdate).toHaveBeenCalledWith(expect.objectContaining({
      id: accepted.id,
      layer: FIRST_UNLOCKED_LAYER_ID,
    }))
  })
})
