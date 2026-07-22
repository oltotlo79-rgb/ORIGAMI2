import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { FoldPreview } from '../src/components/FoldPreview.tsx'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('FoldPreview internationalization', () => {
  it('live-translates unavailable text, group ARIA, viewport ARIA, and controls', () => {
    const localeStore = localeFixture('ja')
    const { container } = render(
      <FoldPreview
        angle={0}
        statusMessage="2面・3ヒンジ"
        localeStore={localeStore}
      />,
    )
    expect(screen.getByRole('group', {
      name: '3D折りプレビュー',
    })).toBeTruthy()
    expect(screen.getByRole('img').getAttribute('aria-label')).toContain(
      '2面・3ヒンジ',
    )
    expect(screen.getByRole('button', {
      name: '視点をリセット',
    }).getAttribute('title')).toBe('カメラを初期位置へ戻す')
    expect(container.querySelector('.fold-preview-empty')?.textContent).toBe(
      '2面・3ヒンジ',
    )

    act(() => {
      localeStore.setLocale('en')
    })

    expect(screen.getByRole('group', {
      name: '3D fold preview',
    })).toBeTruthy()
    expect(screen.getByRole('region', {
      name: '3D measurement',
    })).toBeTruthy()
    expect((screen.getByRole('button', {
      name: '3D measurement mode',
    }) as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByRole('img').getAttribute('aria-label')).toContain(
      '2 faces · 3 hinges',
    )
    expect(screen.getByRole('button', {
      name: 'Reset view',
    }).getAttribute('title')).toBe(
      'Return the camera to its initial position',
    )
    expect(container.querySelector('.fold-preview-empty')?.textContent).toBe(
      '2 faces · 3 hinges',
    )
  })

  it('does not expose external error payloads in English UI or ARIA', () => {
    const localeStore = localeFixture('en')
    const { container } = render(
      <FoldPreview
        angle={0}
        statusMessage="3D analysis error: native-secret-payload"
        localeStore={localeStore}
      />,
    )
    expect(container.textContent).toContain('3D analysis failed.')
    expect(container.textContent).not.toContain('native-secret-payload')
    expect(screen.getByRole('img').getAttribute('aria-label')).not.toContain(
      'native-secret-payload',
    )
  })
})
