export const WORKSPACE_LAYOUT_STORAGE_KEY = 'origami2.workspace-layout'
export const WORKSPACE_LAYOUT_VERSION = 1

export const MIN_EDITOR_TWO_D_PERCENT = 25
export const MAX_EDITOR_TWO_D_PERCENT = 75
export const MIN_INSPECTOR_WIDTH_PX = 200
export const MAX_INSPECTOR_WIDTH_PX = 480
export const MIN_TIMELINE_HEIGHT_PX = 140
export const MAX_TIMELINE_HEIGHT_PX = 360

const MAX_STORED_LAYOUT_BYTES = 1_024

export type WorkspacePanelOrder = 'two_d_first' | 'three_d_first'
export type WorkspaceInspectorSide = 'left' | 'right'

export type WorkspaceLayoutSnapshot = Readonly<{
  editorTwoDPercent: number
  inspectorWidthPx: number
  timelineHeightPx: number
  panelOrder: WorkspacePanelOrder
  inspectorSide: WorkspaceInspectorSide
}>

export const DEFAULT_WORKSPACE_LAYOUT: WorkspaceLayoutSnapshot = Object.freeze({
  editorTwoDPercent: 50,
  inspectorWidthPx: 248,
  timelineHeightPx: 192,
  panelOrder: 'two_d_first',
  inspectorSide: 'right',
})

export type WorkspaceLayoutEnvironment = Readonly<{
  readStoredLayout: () => unknown
  writeStoredLayout: (serialized: string) => void
}>

export type WorkspaceLayoutStore = Readonly<{
  initialize: () => WorkspaceLayoutSnapshot
  getSnapshot: () => WorkspaceLayoutSnapshot
  getServerSnapshot: () => WorkspaceLayoutSnapshot
  subscribe: (listener: () => void) => () => void
  setEditorTwoDPercent: (value: unknown) => boolean
  setInspectorWidthPx: (value: unknown) => boolean
  setTimelineHeightPx: (value: unknown) => boolean
  setPanelOrder: (value: unknown) => boolean
  setInspectorSide: (value: unknown) => boolean
  reset: () => void
  dispose: () => void
}>

export function createWorkspaceLayoutStore(
  environment: WorkspaceLayoutEnvironment,
): WorkspaceLayoutStore {
  let initialized = false
  let snapshot = DEFAULT_WORKSPACE_LAYOUT
  const listeners = new Set<() => void>()

  const initialize = () => {
    if (initialized) return snapshot
    let stored: unknown = null
    try {
      stored = environment.readStoredLayout()
    } catch {
      stored = null
    }
    snapshot = decodeWorkspaceLayout(stored) ?? DEFAULT_WORKSPACE_LAYOUT
    initialized = true
    return snapshot
  }

  const notify = () => {
    for (const listener of [...listeners]) listener()
  }

  const persist = () => {
    try {
      environment.writeStoredLayout(encodeWorkspaceLayout(snapshot))
    } catch {
      // The active layout remains usable when persistence is unavailable.
    }
  }

  const replace = (next: WorkspaceLayoutSnapshot) => {
    initialize()
    if (workspaceLayoutsEqual(snapshot, next)) return true
    snapshot = Object.freeze({ ...next })
    persist()
    notify()
    return true
  }

  const updateNumber = (
    value: unknown,
    key: 'editorTwoDPercent' | 'inspectorWidthPx' | 'timelineHeightPx',
    minimum: number,
    maximum: number,
    precision: number,
  ) => {
    if (typeof value !== 'number' || !Number.isFinite(value)) return false
    initialize()
    const scale = 10 ** precision
    const normalized = Math.round(
      Math.min(maximum, Math.max(minimum, value)) * scale,
    ) / scale
    return replace({ ...snapshot, [key]: normalized })
  }

  return Object.freeze({
    initialize,
    getSnapshot: initialize,
    getServerSnapshot: () => DEFAULT_WORKSPACE_LAYOUT,
    subscribe(listener: () => void) {
      initialize()
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    setEditorTwoDPercent: (value) => updateNumber(
      value,
      'editorTwoDPercent',
      MIN_EDITOR_TWO_D_PERCENT,
      MAX_EDITOR_TWO_D_PERCENT,
      2,
    ),
    setInspectorWidthPx: (value) => updateNumber(
      value,
      'inspectorWidthPx',
      MIN_INSPECTOR_WIDTH_PX,
      MAX_INSPECTOR_WIDTH_PX,
      0,
    ),
    setTimelineHeightPx: (value) => updateNumber(
      value,
      'timelineHeightPx',
      MIN_TIMELINE_HEIGHT_PX,
      MAX_TIMELINE_HEIGHT_PX,
      0,
    ),
    setPanelOrder(value) {
      if (value !== 'two_d_first' && value !== 'three_d_first') return false
      initialize()
      return replace({ ...snapshot, panelOrder: value })
    },
    setInspectorSide(value) {
      if (value !== 'left' && value !== 'right') return false
      initialize()
      return replace({ ...snapshot, inspectorSide: value })
    },
    reset() {
      replace(DEFAULT_WORKSPACE_LAYOUT)
    },
    dispose() {
      listeners.clear()
      initialized = false
      snapshot = DEFAULT_WORKSPACE_LAYOUT
    },
  })
}

export function encodeWorkspaceLayout(
  snapshot: WorkspaceLayoutSnapshot,
): string {
  return JSON.stringify({
    version: WORKSPACE_LAYOUT_VERSION,
    editorTwoDPercent: snapshot.editorTwoDPercent,
    inspectorWidthPx: snapshot.inspectorWidthPx,
    timelineHeightPx: snapshot.timelineHeightPx,
    panelOrder: snapshot.panelOrder,
    inspectorSide: snapshot.inspectorSide,
  })
}

export function decodeWorkspaceLayout(
  serialized: unknown,
): WorkspaceLayoutSnapshot | null {
  if (
    typeof serialized !== 'string'
    || serialized.length === 0
    || serialized.length > MAX_STORED_LAYOUT_BYTES
  ) return null
  try {
    const value: unknown = JSON.parse(serialized)
    if (!isRecord(value)) return null
    const keys = Object.keys(value)
    if (
      keys.length !== 6
      || !keys.includes('version')
      || !keys.includes('editorTwoDPercent')
      || !keys.includes('inspectorWidthPx')
      || !keys.includes('timelineHeightPx')
      || !keys.includes('panelOrder')
      || !keys.includes('inspectorSide')
      || value.version !== WORKSPACE_LAYOUT_VERSION
      || !inRange(
        value.editorTwoDPercent,
        MIN_EDITOR_TWO_D_PERCENT,
        MAX_EDITOR_TWO_D_PERCENT,
      )
      || !integerInRange(
        value.inspectorWidthPx,
        MIN_INSPECTOR_WIDTH_PX,
        MAX_INSPECTOR_WIDTH_PX,
      )
      || !integerInRange(
        value.timelineHeightPx,
        MIN_TIMELINE_HEIGHT_PX,
        MAX_TIMELINE_HEIGHT_PX,
      )
      || (
        value.panelOrder !== 'two_d_first'
        && value.panelOrder !== 'three_d_first'
      )
      || (value.inspectorSide !== 'left' && value.inspectorSide !== 'right')
    ) return null
    return Object.freeze({
      editorTwoDPercent: value.editorTwoDPercent,
      inspectorWidthPx: value.inspectorWidthPx,
      timelineHeightPx: value.timelineHeightPx,
      panelOrder: value.panelOrder,
      inspectorSide: value.inspectorSide,
    })
  } catch {
    return null
  }
}

function workspaceLayoutsEqual(
  left: WorkspaceLayoutSnapshot,
  right: WorkspaceLayoutSnapshot,
) {
  return left.editorTwoDPercent === right.editorTwoDPercent
    && left.inspectorWidthPx === right.inspectorWidthPx
    && left.timelineHeightPx === right.timelineHeightPx
    && left.panelOrder === right.panelOrder
    && left.inspectorSide === right.inspectorSide
}

function inRange(value: unknown, minimum: number, maximum: number): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= minimum
    && value <= maximum
}

function integerInRange(
  value: unknown,
  minimum: number,
  maximum: number,
): value is number {
  return inRange(value, minimum, maximum) && Number.isInteger(value)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}

const browserWorkspaceLayoutEnvironment: WorkspaceLayoutEnvironment = {
  readStoredLayout() {
    if (typeof window === 'undefined') return null
    return window.localStorage.getItem(WORKSPACE_LAYOUT_STORAGE_KEY)
  },
  writeStoredLayout(serialized) {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(WORKSPACE_LAYOUT_STORAGE_KEY, serialized)
  },
}

export const workspaceLayoutStore = createWorkspaceLayoutStore(
  browserWorkspaceLayoutEnvironment,
)
