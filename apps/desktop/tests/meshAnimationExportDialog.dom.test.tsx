import { StrictMode } from 'react'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { MeshAnimationExportDialog } from '../src/components/MeshAnimationExportDialog'
import { localeStore } from '../src/lib/i18n.ts'
import type { MeshAnimationPreviewResponse } from '../src/lib/meshAnimationExport.ts'

const PREVIEW: MeshAnimationPreviewResponse = {
  exportId: '11111111-1111-4111-8111-111111111111',
  projectInstanceId: '22222222-2222-4222-8222-222222222222',
  projectId: '33333333-3333-4333-8333-333333333333',
  revision: 4,
  sourceFingerprint: 'a'.repeat(64),
  frameCount: 3,
  vertexCount: 12,
  triangleCount: 6,
  durationSeconds: 2.5,
  byteCount: 4096,
  mediaType: 'model/gltf-binary',
  fileExtension: 'glb',
  suggestedFileName: 'model-instruction-animation.glb',
}

afterEach(() => {
  cleanup()
  localeStore.setLocale('ja')
  localeStore.dispose()
})

describe('MeshAnimationExportDialog', () => {
  it('shows bounded metadata and requires explicit warning acknowledgement', () => {
    const onSave = vi.fn()
    render(<MeshAnimationExportDialog preview={PREVIEW} busy={false} error={null} notice={null} onRetry={vi.fn()} onSave={onSave} onCancel={vi.fn()} />)
    expect(screen.getByText(PREVIEW.suggestedFileName)).toBeTruthy()
    expect(screen.getByText(/線形補間/u)).toBeTruthy()
    const save = screen.getByRole('button', { name: '保存先を選ぶ' }) as HTMLButtonElement
    expect(save.disabled).toBe(true)
    fireEvent.click(screen.getByLabelText('制限と情報損失を確認しました'))
    expect(save.disabled).toBe(false)
    fireEvent.click(save)
    expect(onSave).toHaveBeenCalledTimes(1)
    expect(document.body.textContent).not.toMatch(/sourceFingerprint|C:\\|file:\/\//u)
  })

  it('supports retry and cleans up cancel listeners under StrictMode disposal', () => {
    const onRetry = vi.fn()
    const onCancel = vi.fn()
    const view = render(
      <StrictMode>
        <MeshAnimationExportDialog preview={null} busy={false} error="stale" notice={null} onRetry={onRetry} onSave={vi.fn()} onCancel={onCancel} />
      </StrictMode>,
    )
    fireEvent.click(screen.getByRole('button', { name: '現在の手順から再作成' }))
    expect(onRetry).toHaveBeenCalledTimes(1)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
    view.unmount()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it('fails closed against save reentry while busy', () => {
    const onSave = vi.fn()
    render(<MeshAnimationExportDialog preview={PREVIEW} busy error={null} notice={null} onRetry={vi.fn()} onSave={onSave} onCancel={vi.fn()} />)
    const save = screen.getByRole('button', { name: '処理中…' }) as HTMLButtonElement
    expect(save.disabled).toBe(true)
    fireEvent.click(save)
    expect(onSave).not.toHaveBeenCalled()
  })

  it('keeps keyboard focus inside the modal in both tab directions', () => {
    render(<MeshAnimationExportDialog preview={PREVIEW} busy={false} error={null} notice={null} onRetry={vi.fn()} onSave={vi.fn()} onCancel={vi.fn()} />)
    const buttons = (screen.getAllByRole('button') as HTMLButtonElement[])
      .filter((button) => !button.disabled)
    const first = buttons[0]
    const last = buttons.at(-1) as HTMLButtonElement
    last.focus()
    fireEvent.keyDown(document, { key: 'Tab' })
    expect(document.activeElement).toBe(first)
    first.focus()
    fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(last)
  })
})
