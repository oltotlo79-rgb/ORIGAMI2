import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { AnnotationPanel } from '../src/components/AnnotationPanel.tsx'
import { UnderlayPanel } from '../src/components/UnderlayPanel.tsx'

afterEach(cleanup)

describe('small localized panels', () => {
  it('retranslates underlay ARIA and visible labels on rerender', () => {
    const view = render(<UnderlayPanel
      locale="ja" underlays={[]} layers={[]} onImport={() => {}}
      onUpdate={() => {}} onRemove={() => {}}
    />)
    expect(screen.getByText('下絵')).toBeTruthy()
    view.rerender(<UnderlayPanel
      locale="en" underlays={[]} layers={[]} onImport={() => {}}
      onUpdate={() => {}} onRemove={() => {}}
    />)
    expect(screen.getByText('Underlays')).toBeTruthy()
    expect(screen.getByRole('list', { name: 'Underlay list' })).toBeTruthy()
  })

  it('retranslates annotation ARIA and visible labels on rerender', () => {
    const props = {
      annotations: [], layers: [], vertices: [],
      onAdd: () => {}, onUpdate: () => {}, onRemove: () => {},
    }
    const view = render(<AnnotationPanel locale="ja" {...props} />)
    expect(screen.getByText('注釈')).toBeTruthy()
    view.rerender(<AnnotationPanel locale="en" {...props} />)
    expect(screen.getByText('Annotations')).toBeTruthy()
    expect(screen.getByRole('list', { name: 'Annotation list' })).toBeTruthy()
  })
})
