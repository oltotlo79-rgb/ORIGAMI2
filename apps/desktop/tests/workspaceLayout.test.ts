import assert from 'node:assert/strict'
import test from 'node:test'

import {
  DEFAULT_WORKSPACE_LAYOUT,
  MAX_EDITOR_TWO_D_PERCENT,
  MAX_INSPECTOR_WIDTH_PX,
  MAX_TIMELINE_HEIGHT_PX,
  MIN_EDITOR_TWO_D_PERCENT,
  MIN_INSPECTOR_WIDTH_PX,
  MIN_TIMELINE_HEIGHT_PX,
  createWorkspaceLayoutStore,
  decodeWorkspaceLayout,
  encodeWorkspaceLayout,
  type WorkspaceLayoutEnvironment,
} from '../src/lib/workspaceLayout.ts'

test('default layout has deterministic panel positions and usable sizes', () => {
  assert.deepEqual(DEFAULT_WORKSPACE_LAYOUT, {
    editorTwoDPercent: 50,
    inspectorWidthPx: 248,
    timelineHeightPx: 192,
    panelOrder: 'two_d_first',
    inspectorSide: 'right',
  })
  assert.equal(Object.isFrozen(DEFAULT_WORKSPACE_LAYOUT), true)
})

test('the exact versioned layout wire round trips without extra fields', () => {
  const serialized = encodeWorkspaceLayout(DEFAULT_WORKSPACE_LAYOUT)
  assert.deepEqual(Object.keys(JSON.parse(serialized)), [
    'version',
    'editorTwoDPercent',
    'inspectorWidthPx',
    'timelineHeightPx',
    'panelOrder',
    'inspectorSide',
  ])
  assert.deepEqual(decodeWorkspaceLayout(serialized), DEFAULT_WORKSPACE_LAYOUT)
})

test('malformed old oversized and hostile stored layouts fail closed', () => {
  for (const value of [
    null,
    '',
    '{',
    '[]',
    '{}',
    JSON.stringify({
      version: 0,
      ...DEFAULT_WORKSPACE_LAYOUT,
    }),
    JSON.stringify({
      version: 1,
      ...DEFAULT_WORKSPACE_LAYOUT,
      surprise: true,
    }),
    JSON.stringify({
      version: 1,
      ...DEFAULT_WORKSPACE_LAYOUT,
      editorTwoDPercent: Number.NaN,
    }),
    JSON.stringify({
      version: 1,
      ...DEFAULT_WORKSPACE_LAYOUT,
      inspectorWidthPx: 200.5,
    }),
    JSON.stringify({
      version: 1,
      ...DEFAULT_WORKSPACE_LAYOUT,
      timelineHeightPx: 999,
    }),
    JSON.stringify({
      version: 1,
      ...DEFAULT_WORKSPACE_LAYOUT,
      panelOrder: 'unknown',
    }),
    JSON.stringify({
      version: 1,
      ...DEFAULT_WORKSPACE_LAYOUT,
      inspectorSide: 'bottom',
    }),
    'x'.repeat(1_025),
    Object.defineProperty({}, 'length', {
      get() {
        throw new Error('hostile')
      },
    }),
  ]) {
    assert.equal(decodeWorkspaceLayout(value), null)
  }
})

test('size setters clamp drag values and persist one canonical snapshot', () => {
  const fixture = environment()
  const store = createWorkspaceLayoutStore(fixture.value)
  assert.equal(store.setEditorTwoDPercent(-10), true)
  assert.equal(store.setInspectorWidthPx(10_000), true)
  assert.equal(store.setTimelineHeightPx(10_000), true)
  assert.deepEqual(store.getSnapshot(), {
    ...DEFAULT_WORKSPACE_LAYOUT,
    editorTwoDPercent: MIN_EDITOR_TWO_D_PERCENT,
    inspectorWidthPx: MAX_INSPECTOR_WIDTH_PX,
    timelineHeightPx: MAX_TIMELINE_HEIGHT_PX,
  })
  assert.deepEqual(
    decodeWorkspaceLayout(fixture.writes.at(-1)),
    store.getSnapshot(),
  )

  assert.equal(store.setEditorTwoDPercent(99), true)
  assert.equal(store.setInspectorWidthPx(-1), true)
  assert.equal(store.setTimelineHeightPx(-1), true)
  assert.equal(store.getSnapshot().editorTwoDPercent, MAX_EDITOR_TWO_D_PERCENT)
  assert.equal(store.getSnapshot().inspectorWidthPx, MIN_INSPECTOR_WIDTH_PX)
  assert.equal(store.getSnapshot().timelineHeightPx, MIN_TIMELINE_HEIGHT_PX)
})

test('position setters reject unknown values and notify only real changes', () => {
  const fixture = environment()
  const store = createWorkspaceLayoutStore(fixture.value)
  let notifications = 0
  const unsubscribe = store.subscribe(() => {
    notifications += 1
  })
  assert.equal(store.setPanelOrder('unknown'), false)
  assert.equal(store.setInspectorSide(null), false)
  assert.equal(store.setPanelOrder('two_d_first'), true)
  assert.equal(store.setInspectorSide('right'), true)
  assert.equal(notifications, 0)
  assert.equal(store.setPanelOrder('three_d_first'), true)
  assert.equal(store.setInspectorSide('left'), true)
  assert.equal(notifications, 2)
  unsubscribe()
  store.setPanelOrder('two_d_first')
  assert.equal(notifications, 2)
})

test('invalid numeric setters never change or persist layout', () => {
  const fixture = environment()
  const store = createWorkspaceLayoutStore(fixture.value)
  for (const value of [null, '50', Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(store.setEditorTwoDPercent(value), false)
    assert.equal(store.setInspectorWidthPx(value), false)
    assert.equal(store.setTimelineHeightPx(value), false)
  }
  assert.equal(store.getSnapshot(), DEFAULT_WORKSPACE_LAYOUT)
  assert.equal(fixture.writes.length, 0)
})

test('storage failures do not block editing reset or subscriptions', () => {
  const store = createWorkspaceLayoutStore({
    readStoredLayout() {
      throw new Error('blocked')
    },
    writeStoredLayout() {
      throw new Error('blocked')
    },
  })
  let notifications = 0
  store.subscribe(() => {
    notifications += 1
  })
  assert.equal(store.setEditorTwoDPercent(61.234), true)
  assert.equal(store.getSnapshot().editorTwoDPercent, 61.23)
  store.reset()
  assert.deepEqual(store.getSnapshot(), DEFAULT_WORKSPACE_LAYOUT)
  assert.equal(Object.isFrozen(store.getSnapshot()), true)
  assert.equal(notifications, 2)
})

test('dispose clears subscribers and reinitializes from current storage', () => {
  const fixture = environment()
  const store = createWorkspaceLayoutStore(fixture.value)
  let notifications = 0
  store.subscribe(() => {
    notifications += 1
  })
  store.setInspectorSide('left')
  assert.equal(notifications, 1)
  store.dispose()
  fixture.stored = encodeWorkspaceLayout({
    ...DEFAULT_WORKSPACE_LAYOUT,
    timelineHeightPx: 300,
  })
  assert.equal(store.initialize().timelineHeightPx, 300)
  store.setTimelineHeightPx(301)
  assert.equal(notifications, 1)
})

function environment(): {
  value: WorkspaceLayoutEnvironment
  writes: string[]
  stored: unknown
} {
  const fixture = {
    writes: [] as string[],
    stored: null as unknown,
    value: null as unknown as WorkspaceLayoutEnvironment,
  }
  fixture.value = {
    readStoredLayout: () => fixture.stored,
    writeStoredLayout(serialized) {
      fixture.writes.push(serialized)
      fixture.stored = serialized
    },
  }
  return fixture
}
