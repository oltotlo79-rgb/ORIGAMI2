import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useRef, useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  finishBeginnerGridCancellation,
  runBeginnerGridApplyWorkflow,
} from '../src/lib/beginnerGridWorkflow'

afterEach(cleanup)

function WorkflowHarness({ confirm, apply }: {
  confirm: () => boolean
  apply: () => Promise<boolean>
}) {
  const [preview, setPreview] = useState(true)
  const evaluateRef = useRef<HTMLButtonElement>(null)
  const restoreFocus = () => requestAnimationFrame(() => evaluateRef.current?.focus())
  return <>
    <button ref={evaluateRef}>Evaluate complete animal grid</button>
    {preview && <section aria-label="Complete animal candidate preview">
      <button onClick={() => void runBeginnerGridApplyWorkflow({
        confirm,
        apply,
        clearPreview: () => setPreview(false),
        restoreFocus,
      })}>Apply complete animal</button>
      <button onClick={() => finishBeginnerGridCancellation(
        () => setPreview(false), restoreFocus,
      )}>Cancel complete animal grid</button>
    </section>}
  </>
}

describe('complete animal grid user workflow', () => {
  it('keeps the preview and never applies when confirmation is rejected', async () => {
    const apply = vi.fn(async () => true)
    render(<WorkflowHarness confirm={() => false} apply={apply} />)
    fireEvent.click(screen.getByRole('button', { name: 'Apply complete animal' }))
    await waitFor(() => expect(apply).not.toHaveBeenCalled())
    expect(screen.getByRole('region', { name: 'Complete animal candidate preview' })).toBeTruthy()
  })

  it('keeps the preview when native apply fails', async () => {
    render(<WorkflowHarness confirm={() => true} apply={async () => false} />)
    fireEvent.click(screen.getByRole('button', { name: 'Apply complete animal' }))
    await waitFor(() => expect(screen.getByRole('region', {
      name: 'Complete animal candidate preview',
    })).toBeTruthy())
  })

  it('clears the preview and restores focus after successful apply', async () => {
    render(<WorkflowHarness confirm={() => true} apply={async () => true} />)
    fireEvent.click(screen.getByRole('button', { name: 'Apply complete animal' }))
    await waitFor(() => expect(screen.queryByRole('region', {
      name: 'Complete animal candidate preview',
    })).toBeNull())
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', {
      name: 'Evaluate complete animal grid',
    })))
  })

  it('clears the preview and restores focus after cancellation', async () => {
    render(<WorkflowHarness confirm={() => true} apply={async () => true} />)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel complete animal grid' }))
    await waitFor(() => expect(screen.queryByRole('region', {
      name: 'Complete animal candidate preview',
    })).toBeNull())
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', {
      name: 'Evaluate complete animal grid',
    })))
  })
})
