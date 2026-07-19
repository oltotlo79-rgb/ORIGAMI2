import {
  act,
  cleanup,
  createEvent,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import {
  type CSSProperties,
  useSyncExternalStore,
} from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { WorkspaceLayoutControl } from '../src/components/WorkspaceLayoutControl.tsx'
import { WorkspaceLayoutSeparator } from '../src/components/WorkspaceLayoutSeparator.tsx'
import {
  DEFAULT_WORKSPACE_LAYOUT,
  createWorkspaceLayoutStore,
  type WorkspaceLayoutStore,
} from '../src/lib/workspaceLayout.ts'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

function store(): WorkspaceLayoutStore {
  return createWorkspaceLayoutStore({
    readStoredLayout: () => null,
    writeStoredLayout: () => undefined,
  })
}

function IntegratedWorkspaceLayout({
  target,
}: Readonly<{ target: WorkspaceLayoutStore }>) {
  const layout = useSyncExternalStore(
    target.subscribe,
    target.getSnapshot,
    target.getServerSnapshot,
  )
  const style = {
    '--workspace-editor-two-d-share': `${layout.editorTwoDPercent}fr`,
    '--workspace-editor-three-d-share':
      `${100 - layout.editorTwoDPercent}fr`,
    '--workspace-inspector-width': `${layout.inspectorWidthPx}px`,
    '--workspace-timeline-height': `${layout.timelineHeightPx}px`,
  } as CSSProperties

  return (
    <main data-testid="integrated-layout" style={style}>
      <section
        id="workspace-main"
        data-inspector-side={layout.inspectorSide}
      >
        <section
          id="workspace-editor-panels"
          data-panel-order={layout.panelOrder}
        >
          <article id="crease-editor-panel" />
          <WorkspaceLayoutSeparator kind="editor" store={target} />
          <article id="fold-preview-panel" />
        </section>
        <WorkspaceLayoutSeparator kind="inspector" store={target} />
        <aside id="workspace-inspector-panel" />
      </section>
      <WorkspaceLayoutSeparator kind="timeline" store={target} />
      <section id="instruction-timeline-panel" />
      <footer>
        <WorkspaceLayoutControl store={target} />
      </footer>
    </main>
  )
}

describe('workspace layout integration', () => {
  it('keeps CSS values, panel positions, and separator targets synchronized', () => {
    const target = store()
    render(<IntegratedWorkspaceLayout target={target} />)

    const root = screen.getByTestId('integrated-layout')
    const editorRegion = document.getElementById('workspace-editor-panels')
    const workspace = document.getElementById('workspace-main')
    const separators = screen.getAllByRole('separator')
    expect(separators).toHaveLength(3)
    expect(root.style.getPropertyValue('--workspace-editor-two-d-share'))
      .toBe('50fr')
    expect(root.style.getPropertyValue('--workspace-inspector-width'))
      .toBe('248px')
    expect(root.style.getPropertyValue('--workspace-timeline-height'))
      .toBe('192px')

    for (const separator of separators) {
      const controls = separator.getAttribute('aria-controls')?.split(' ') ?? []
      expect(controls.length).toBe(2)
      for (const id of controls) {
        expect(document.getElementById(id), id).not.toBeNull()
      }
    }

    fireEvent.keyDown(screen.getByRole('separator', {
      name: '2Dと3Dの幅を変更',
    }), { key: 'ArrowRight' })
    expect(root.style.getPropertyValue('--workspace-editor-two-d-share'))
      .toBe('52fr')
    expect(root.style.getPropertyValue('--workspace-editor-three-d-share'))
      .toBe('48fr')

    fireEvent.keyDown(screen.getByRole('separator', {
      name: '折り手順パネルの高さを変更',
    }), { key: 'ArrowUp' })
    expect(root.style.getPropertyValue('--workspace-timeline-height'))
      .toBe('202px')

    fireEvent.click(screen.getByRole('button', {
      name: '2Dと3Dを入れ替え',
    }))
    fireEvent.click(screen.getByRole('button', {
      name: 'プロパティを左へ',
    }))
    expect(editorRegion?.getAttribute('data-panel-order'))
      .toBe('three_d_first')
    expect(workspace?.getAttribute('data-inspector-side')).toBe('left')
  })
})

describe('WorkspaceLayoutControl', () => {
  it('localizes layout actions, status, and separator labels in English', () => {
    const target = store()
    const english = localeFixture('en')
    render(
      <>
        <WorkspaceLayoutControl store={target} localeStore={english} />
        <WorkspaceLayoutSeparator
          kind="editor"
          store={target}
          localeStore={english}
        />
        <WorkspaceLayoutSeparator
          kind="inspector"
          store={target}
          localeStore={english}
        />
        <WorkspaceLayoutSeparator
          kind="timeline"
          store={target}
          localeStore={english}
        />
      </>,
    )

    expect(screen.getByRole('group', {
      name: 'Workspace layout',
    })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Swap 2D and 3D',
    })).toBeTruthy()
    expect(screen.getByRole('status', {
      name: 'Current workspace layout',
    }).textContent).toContain('Properties 248px · Timeline 192px')
    expect(screen.getByRole('separator', {
      name: 'Resize 2D and 3D panels',
    })).toBeTruthy()
    expect(screen.getByRole('separator', {
      name: 'Resize properties panel',
    })).toBeTruthy()
    expect(screen.getByRole('separator', {
      name: 'Resize instruction timeline panel',
    })).toBeTruthy()
  })

  it('moves both position choices and restores the complete default', () => {
    const target = store()
    target.setEditorTwoDPercent(63)
    target.setInspectorWidthPx(320)
    target.setTimelineHeightPx(280)
    render(<WorkspaceLayoutControl store={target} />)

    fireEvent.click(screen.getByRole('button', {
      name: '2Dと3Dを入れ替え',
    }))
    fireEvent.click(screen.getByRole('button', {
      name: 'プロパティを左へ',
    }))
    expect(target.getSnapshot().panelOrder).toBe('three_d_first')
    expect(target.getSnapshot().inspectorSide).toBe('left')
    expect(screen.getByRole('button', {
      name: 'プロパティを右へ',
    })).toBeTruthy()
    expect(screen.getByRole('status', {
      name: '現在の作業レイアウト',
    }).textContent).toContain('2D 63%')

    fireEvent.click(screen.getByRole('button', {
      name: '初期配置に戻す',
    }))
    expect(target.getSnapshot()).toEqual(DEFAULT_WORKSPACE_LAYOUT)
  })
})

describe('WorkspaceLayoutSeparator', () => {
  it('exposes bounded separator semantics and keyboard resizing', () => {
    const target = store()
    render(
      <>
        <WorkspaceLayoutSeparator kind="editor" store={target} />
        <WorkspaceLayoutSeparator kind="inspector" store={target} />
        <WorkspaceLayoutSeparator kind="timeline" store={target} />
      </>,
    )
    const editor = screen.getByRole('separator', {
      name: '2Dと3Dの幅を変更',
    })
    const inspector = screen.getByRole('separator', {
      name: 'プロパティパネルの幅を変更',
    })
    const timeline = screen.getByRole('separator', {
      name: '折り手順パネルの高さを変更',
    })
    expect(editor.getAttribute('aria-valuemin')).toBe('25')
    expect(editor.getAttribute('aria-valuemax')).toBe('75')
    expect(editor.getAttribute('aria-valuenow')).toBe('50')

    fireEvent.keyDown(editor, { key: 'ArrowRight' })
    expect(target.getSnapshot().editorTwoDPercent).toBe(52)
    act(() => {
      target.setPanelOrder('three_d_first')
    })
    fireEvent.keyDown(editor, { key: 'ArrowRight' })
    expect(target.getSnapshot().editorTwoDPercent).toBe(50)

    fireEvent.keyDown(inspector, { key: 'ArrowLeft' })
    expect(target.getSnapshot().inspectorWidthPx).toBe(258)
    act(() => {
      target.setInspectorSide('left')
    })
    fireEvent.keyDown(inspector, { key: 'ArrowRight' })
    expect(target.getSnapshot().inspectorWidthPx).toBe(268)

    fireEvent.keyDown(timeline, { key: 'ArrowUp' })
    expect(target.getSnapshot().timelineHeightPx).toBe(202)
    fireEvent.keyDown(timeline, { key: 'Home' })
    expect(target.getSnapshot().timelineHeightPx).toBe(140)
    fireEvent.keyDown(timeline, { key: 'End' })
    expect(target.getSnapshot().timelineHeightPx).toBe(360)
    fireEvent.doubleClick(timeline)
    expect(target.getSnapshot().timelineHeightPx).toBe(192)
  })

  it('ignores modified and composing keys without changing layout', () => {
    const target = store()
    render(<WorkspaceLayoutSeparator kind="editor" store={target} />)
    const separator = screen.getByRole('separator')
    for (const event of [
      { key: 'ArrowRight', altKey: true },
      { key: 'ArrowRight', ctrlKey: true },
      { key: 'ArrowRight', metaKey: true },
      { key: 'ArrowRight', shiftKey: true },
      { key: 'PageDown' },
    ]) {
      fireEvent.keyDown(separator, event)
    }
    const composing = createEvent.keyDown(separator, {
      key: 'ArrowRight',
      keyCode: 229,
    })
    Object.defineProperty(composing, 'isComposing', { value: true })
    fireEvent(separator, composing)
    expect(target.getSnapshot()).toEqual(DEFAULT_WORKSPACE_LAYOUT)
  })

  it('maps pointer motion to the actual panel regardless of visual side', () => {
    const target = store()
    const view = render(
      <div data-testid="parent">
        <WorkspaceLayoutSeparator kind="editor" store={target} />
      </div>,
    )
    const parent = screen.getByTestId('parent')
    parent.getBoundingClientRect = () => ({
      x: 0,
      y: 0,
      width: 1_000,
      height: 600,
      top: 0,
      right: 1_000,
      bottom: 600,
      left: 0,
      toJSON: () => ({}),
    })
    const separator = screen.getByRole('separator')
    fireEvent.pointerDown(separator, {
      button: 0,
      isPrimary: true,
      pointerId: 1,
      clientX: 500,
      clientY: 0,
    })
    fireEvent.pointerMove(separator, {
      pointerId: 1,
      clientX: 600,
      clientY: 0,
    })
    fireEvent.pointerUp(separator, { pointerId: 1 })
    expect(target.getSnapshot().editorTwoDPercent).toBe(60)

    act(() => {
      target.setPanelOrder('three_d_first')
    })
    fireEvent.pointerDown(separator, {
      button: 0,
      isPrimary: true,
      pointerId: 2,
      clientX: 500,
      clientY: 0,
    })
    fireEvent.pointerMove(separator, {
      pointerId: 2,
      clientX: 600,
      clientY: 0,
    })
    fireEvent.pointerUp(separator, { pointerId: 2 })
    expect(target.getSnapshot().editorTwoDPercent).toBe(50)
    view.unmount()
  })
})
