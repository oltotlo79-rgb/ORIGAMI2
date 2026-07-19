import {
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useRef,
  useSyncExternalStore,
} from 'react'

import {
  DEFAULT_WORKSPACE_LAYOUT,
  MAX_EDITOR_TWO_D_PERCENT,
  MAX_INSPECTOR_WIDTH_PX,
  MAX_TIMELINE_HEIGHT_PX,
  MIN_EDITOR_TWO_D_PERCENT,
  MIN_INSPECTOR_WIDTH_PX,
  MIN_TIMELINE_HEIGHT_PX,
  workspaceLayoutStore,
  type WorkspaceLayoutSnapshot,
  type WorkspaceLayoutStore,
} from '../lib/workspaceLayout'

export type WorkspaceLayoutSeparatorKind =
  | 'editor'
  | 'inspector'
  | 'timeline'

type WorkspaceLayoutSeparatorProps = Readonly<{
  kind: WorkspaceLayoutSeparatorKind
  store?: WorkspaceLayoutStore
}>

type ActiveDrag = Readonly<{
  pointerId: number
  startX: number
  startY: number
  startValue: number
  extent: number
  layout: WorkspaceLayoutSnapshot
}>

export function WorkspaceLayoutSeparator({
  kind,
  store = workspaceLayoutStore,
}: WorkspaceLayoutSeparatorProps) {
  const layout = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )
  const dragRef = useRef<ActiveDrag | null>(null)
  const contract = separatorContract(kind, layout)

  const finishDrag = (
    event: ReactPointerEvent<HTMLDivElement>,
  ) => {
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return
    dragRef.current = null
    try {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId)
      }
    } catch {
      // Hosts without complete pointer capture support still finish safely.
    }
  }

  return (
    <div
      className={`workspace-separator is-${kind}`}
      role="separator"
      tabIndex={0}
      aria-label={contract.label}
      aria-orientation={contract.orientation}
      aria-valuemin={contract.minimum}
      aria-valuemax={contract.maximum}
      aria-valuenow={contract.value}
      aria-valuetext={contract.valueText}
      aria-controls={contract.controls}
      onDoubleClick={() => resetSeparator(kind, store)}
      onPointerDown={(event) => {
        if (
          event.button !== 0
          || !event.isPrimary
          || event.altKey
          || event.ctrlKey
          || event.metaKey
          || event.shiftKey
          || !Number.isInteger(event.pointerId)
          || !Number.isFinite(event.clientX)
          || !Number.isFinite(event.clientY)
        ) return
        const bounds = event.currentTarget.parentElement?.getBoundingClientRect()
        const extent = kind === 'editor' ? bounds?.width : bounds?.height
        if (kind === 'editor' && (!extent || !Number.isFinite(extent))) return
        dragRef.current = {
          pointerId: event.pointerId,
          startX: event.clientX,
          startY: event.clientY,
          startValue: contract.value,
          extent: extent ?? 1,
          layout,
        }
        try {
          event.currentTarget.setPointerCapture(event.pointerId)
        } catch {
          // Keyboard resizing and uncaptured pointer events remain available.
        }
        event.preventDefault()
      }}
      onPointerMove={(event) => {
        const drag = dragRef.current
        if (
          !drag
          || drag.pointerId !== event.pointerId
          || !Number.isFinite(event.clientX)
          || !Number.isFinite(event.clientY)
        ) return
        applyPointerDrag(kind, drag, event.clientX, event.clientY, store)
        event.preventDefault()
      }}
      onPointerUp={finishDrag}
      onPointerCancel={finishDrag}
      onLostPointerCapture={() => {
        dragRef.current = null
      }}
      onKeyDown={(event) => {
        if (applySeparatorKey(kind, layout, event, store)) {
          event.preventDefault()
        }
      }}
    >
      <span aria-hidden="true" />
    </div>
  )
}

function separatorContract(
  kind: WorkspaceLayoutSeparatorKind,
  layout: WorkspaceLayoutSnapshot,
) {
  if (kind === 'editor') {
    return {
      label: '2Dと3Dの幅を変更',
      orientation: 'vertical' as const,
      minimum: MIN_EDITOR_TWO_D_PERCENT,
      maximum: MAX_EDITOR_TWO_D_PERCENT,
      value: layout.editorTwoDPercent,
      valueText: `2D ${layout.editorTwoDPercent}%`,
      controls: 'crease-editor-panel fold-preview-panel',
    }
  }
  if (kind === 'inspector') {
    return {
      label: 'プロパティパネルの幅を変更',
      orientation: 'vertical' as const,
      minimum: MIN_INSPECTOR_WIDTH_PX,
      maximum: MAX_INSPECTOR_WIDTH_PX,
      value: layout.inspectorWidthPx,
      valueText: `${layout.inspectorWidthPx}px`,
      controls: 'workspace-editor-panels workspace-inspector-panel',
    }
  }
  return {
    label: '折り手順パネルの高さを変更',
    orientation: 'horizontal' as const,
    minimum: MIN_TIMELINE_HEIGHT_PX,
    maximum: MAX_TIMELINE_HEIGHT_PX,
    value: layout.timelineHeightPx,
    valueText: `${layout.timelineHeightPx}px`,
    controls: 'workspace-main instruction-timeline-panel',
  }
}

function applyPointerDrag(
  kind: WorkspaceLayoutSeparatorKind,
  drag: ActiveDrag,
  clientX: number,
  clientY: number,
  store: WorkspaceLayoutStore,
) {
  if (kind === 'editor') {
    const direction = drag.layout.panelOrder === 'two_d_first' ? 1 : -1
    store.setEditorTwoDPercent(
      drag.startValue
      + direction * ((clientX - drag.startX) / drag.extent) * 100,
    )
    return
  }
  if (kind === 'inspector') {
    const direction = drag.layout.inspectorSide === 'left' ? 1 : -1
    store.setInspectorWidthPx(
      drag.startValue + direction * (clientX - drag.startX),
    )
    return
  }
  store.setTimelineHeightPx(drag.startValue - (clientY - drag.startY))
}

function applySeparatorKey(
  kind: WorkspaceLayoutSeparatorKind,
  layout: WorkspaceLayoutSnapshot,
  event: KeyboardEvent<HTMLDivElement>,
  store: WorkspaceLayoutStore,
) {
  if (
    event.altKey
    || event.ctrlKey
    || event.metaKey
    || event.shiftKey
    || keyboardEventIsComposing(event)
  ) return false

  if (kind === 'editor') {
    if (event.key === 'Home') {
      return store.setEditorTwoDPercent(MIN_EDITOR_TWO_D_PERCENT)
    }
    if (event.key === 'End') {
      return store.setEditorTwoDPercent(MAX_EDITOR_TWO_D_PERCENT)
    }
    const visualDirection = event.key === 'ArrowLeft'
      ? -1
      : event.key === 'ArrowRight' ? 1 : 0
    if (visualDirection === 0) return false
    const twoDDirection = layout.panelOrder === 'two_d_first'
      ? visualDirection
      : -visualDirection
    return store.setEditorTwoDPercent(
      layout.editorTwoDPercent + twoDDirection * 2,
    )
  }

  if (kind === 'inspector') {
    if (event.key === 'Home') {
      return store.setInspectorWidthPx(MIN_INSPECTOR_WIDTH_PX)
    }
    if (event.key === 'End') {
      return store.setInspectorWidthPx(MAX_INSPECTOR_WIDTH_PX)
    }
    const visualDirection = event.key === 'ArrowLeft'
      ? -1
      : event.key === 'ArrowRight' ? 1 : 0
    if (visualDirection === 0) return false
    const widthDirection = layout.inspectorSide === 'left'
      ? visualDirection
      : -visualDirection
    return store.setInspectorWidthPx(
      layout.inspectorWidthPx + widthDirection * 10,
    )
  }

  if (event.key === 'Home') {
    return store.setTimelineHeightPx(MIN_TIMELINE_HEIGHT_PX)
  }
  if (event.key === 'End') {
    return store.setTimelineHeightPx(MAX_TIMELINE_HEIGHT_PX)
  }
  if (event.key === 'ArrowUp') {
    return store.setTimelineHeightPx(layout.timelineHeightPx + 10)
  }
  if (event.key === 'ArrowDown') {
    return store.setTimelineHeightPx(layout.timelineHeightPx - 10)
  }
  return false
}

function keyboardEventIsComposing(
  event: KeyboardEvent<HTMLDivElement>,
): boolean {
  try {
    return Reflect.get(event, 'isComposing') === true
      || Reflect.get(event.nativeEvent, 'isComposing') === true
      || Reflect.get(event, 'keyCode') === 229
  } catch {
    return true
  }
}

function resetSeparator(
  kind: WorkspaceLayoutSeparatorKind,
  store: WorkspaceLayoutStore,
) {
  if (kind === 'editor') {
    store.setEditorTwoDPercent(DEFAULT_WORKSPACE_LAYOUT.editorTwoDPercent)
  } else if (kind === 'inspector') {
    store.setInspectorWidthPx(DEFAULT_WORKSPACE_LAYOUT.inspectorWidthPx)
  } else {
    store.setTimelineHeightPx(DEFAULT_WORKSPACE_LAYOUT.timelineHeightPx)
  }
}
