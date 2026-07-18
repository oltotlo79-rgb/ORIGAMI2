import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createFoldPreviewKeyboardCoordinator,
  type FoldPreviewKeyboardCoordinatorOptions,
  type FoldPreviewKeyboardEvent,
} from '../src/lib/foldPreviewKeyboardCoordinator.ts'

test('selection shortcuts dispatch callbacks, announcements, and prevention in order', () => {
  const fixture = coordinatorFixture()

  fixture.handle(key('h', fixture.host))
  fixture.handle(key('f', fixture.host))
  fixture.state.selectedHingeId = 'hinge-b'
  fixture.handle(key('Escape', fixture.host))

  assert.deepEqual(fixture.calls, [
    'select:hinge-b',
    'announce:ヒンジ 2/2 を選択しました',
    'prevent:h',
    'fixed:face-b',
    'announce:面 2/2 を固定面に設定しました',
    'prevent:f',
    'select:null',
    'announce:ヒンジ選択を解除しました',
    'prevent:Escape',
  ])
})

test('dirty gestures are reset before routing any selection or camera command', () => {
  const fixture = coordinatorFixture()
  fixture.state.gesturesClean = false

  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(fixture.calls, ['reset-gestures', 'prevent:ArrowUp'])
  assert.equal(fixture.cameraCalls.length, 0)
})

test('dirty gesture cancellation reads only target and prevention from the event', () => {
  const fixture = coordinatorFixture()
  fixture.state.gesturesClean = false
  let keyReads = 0
  const event = {
    target: fixture.host,
    get key() {
      keyReads += 1
      throw new Error('routing fields must stay unread')
    },
    get preventDefault() {
      return () => {
        fixture.calls.push('prevent:dirty-hostile')
      }
    },
  } as unknown as FoldPreviewKeyboardEvent

  fixture.handle(event)

  assert.equal(keyReads, 0)
  assert.deepEqual(fixture.calls, [
    'reset-gestures',
    'prevent:dirty-hostile',
  ])
})

test('throwing gesture-state inspection fails closed through reset and prevention', () => {
  const fixture = coordinatorFixture({
    foldGesturesAreClean: () => {
      throw new Error('gesture state failure')
    },
  })

  assert.doesNotThrow(() => {
    fixture.handle(key('ArrowUp', fixture.host))
  })

  assert.deepEqual(fixture.calls, [
    'reset-gestures',
    'prevent:ArrowUp',
  ])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('foreign targets and stale scene contexts are ignored without side effects', () => {
  const fixture = coordinatorFixture()

  fixture.handle(key('h', new EventTarget()))
  fixture.state.current = false
  fixture.handle(key('h', fixture.host))
  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(fixture.calls, [])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('selection callbacks cannot publish an announcement after invalidating context', () => {
  let current = true
  const selectionCalls: string[] = []
  const fixture = coordinatorFixture({
    contextIsCurrent: () => current,
    canAnnounce: () => current,
    getSelectHingeCallback: () => (edgeId) => {
      selectionCalls.push(`select:${edgeId ?? 'null'}`)
      current = false
    },
  })

  fixture.handle(key('h', fixture.host))

  assert.deepEqual(selectionCalls, ['select:hinge-b'])
  assert.deepEqual(fixture.calls, ['prevent:h'])
})

test('throwing selection callbacks are consumed and never fall through to camera', () => {
  const fixture = coordinatorFixture({
    getSelectHingeCallback: () => () => {
      throw new Error('callback failure')
    },
  })

  fixture.handle(key('h', fixture.host))

  assert.deepEqual(fixture.calls, ['prevent:h'])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('camera commands are routed only after selection resolution declines the key', () => {
  const fixture = coordinatorFixture()

  fixture.handle(key('ArrowUp', fixture.host))
  fixture.handle(key('+', fixture.host))
  fixture.handle(key('x', fixture.host))

  assert.deepEqual(fixture.cameraCalls, [
    'pan:0,7',
    'dolly-in:0.9',
  ])
  assert.deepEqual(fixture.calls, [
    'prevent:ArrowUp',
    'prevent:+',
  ])
})

test('snapshotted camera methods preserve their original receiver', () => {
  const cameraCalls: string[] = []
  const controls = {
    keyPanSpeed: 7,
    keyRotateSpeed: 4,
    pan(deltaX: number, deltaY: number) {
      assert.equal(this, controls)
      cameraCalls.push(`pan:${deltaX},${deltaY}`)
    },
    rotateLeft() {},
    rotateUp() {},
    dollyIn() {},
    dollyOut() {},
    reset() {},
  }
  const fixture = coordinatorFixture({ cameraControls: controls })

  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(cameraCalls, ['pan:0,7'])
  assert.deepEqual(fixture.calls, ['prevent:ArrowUp'])
})

test('camera failures are isolated behind the supplied lifecycle callback', () => {
  const fixture = coordinatorFixture({
    cameraControls: {
      keyPanSpeed: 7,
      keyRotateSpeed: 4,
      pan() {
        throw new Error('camera failure')
      },
      rotateLeft() {},
      rotateUp() {},
      dollyIn() {},
      dollyOut() {},
      reset() {},
    },
  })

  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(fixture.calls, ['camera-failure'])
})

test('construction detaches target arrays, option callbacks, and camera methods', () => {
  const hingeIds = ['hinge-a', 'hinge-b']
  const faceIds = ['face-a', 'face-b']
  const fixture = coordinatorFixture({ hingeIds, faceIds })

  hingeIds.splice(0, hingeIds.length, 'hostile-hinge')
  faceIds.splice(0, faceIds.length, 'hostile-face')
  Object.assign(fixture.options, {
    getSelectHingeCallback: () => () => {
      throw new Error('mutated callback')
    },
    cameraControls: {
      keyPanSpeed: 999,
      keyRotateSpeed: 999,
      pan() {
        throw new Error('mutated controls')
      },
    },
  })

  fixture.handle(key('h', fixture.host))
  fixture.handle(key('f', fixture.host))
  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(fixture.calls, [
    'select:hinge-b',
    'announce:ヒンジ 2/2 を選択しました',
    'prevent:h',
    'fixed:face-b',
    'announce:面 2/2 を固定面に設定しました',
    'prevent:f',
    'prevent:ArrowUp',
  ])
  assert.deepEqual(fixture.cameraCalls, ['pan:0,7'])
})

test('caller-owned ID accessors are read once at construction', () => {
  let firstHingeReads = 0
  const hingeIds = ['hinge-a', 'hinge-b']
  Object.defineProperty(hingeIds, 0, {
    configurable: true,
    get() {
      firstHingeReads += 1
      return firstHingeReads === 1 ? 'hinge-a' : 'changed-hinge'
    },
  })
  const fixture = coordinatorFixture({ hingeIds })

  fixture.handle(key('h', fixture.host))

  assert.equal(firstHingeReads, 1)
  assert.deepEqual(fixture.calls, [
    'select:hinge-b',
    'announce:ヒンジ 2/2 を選択しました',
    'prevent:h',
  ])
})

test('one event snapshot prevents stateful accessors from changing route mid-key', () => {
  const fixture = coordinatorFixture()
  let keyReads = 0
  const event = {
    target: fixture.host,
    get key() {
      keyReads += 1
      return keyReads === 1 ? 'ArrowUp' : 'x'
    },
    code: '',
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
    preventDefault() {
      fixture.calls.push('prevent:stateful')
    },
  } as FoldPreviewKeyboardEvent

  fixture.handle(event)

  assert.equal(keyReads, 1)
  assert.deepEqual(fixture.cameraCalls, ['pan:0,7'])
  assert.deepEqual(fixture.calls, ['prevent:stateful'])
})

test('an unrelated key never reads a hostile preventDefault accessor', () => {
  const fixture = coordinatorFixture()
  let preventionReads = 0
  const event = {
    target: fixture.host,
    key: 'x',
    code: '',
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
    get preventDefault() {
      preventionReads += 1
      throw new Error('unused prevention getter')
    },
  } as unknown as FoldPreviewKeyboardEvent

  fixture.handle(event)

  assert.equal(preventionReads, 0)
  assert.deepEqual(fixture.calls, [])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('hostile option and event accessors are contained without routing', () => {
  let inert:
    ReturnType<typeof createFoldPreviewKeyboardCoordinator> | undefined
  const hostileOptions = new Proxy(
    {} as FoldPreviewKeyboardCoordinatorOptions,
    {
      get() {
        throw new Error('option getter')
      },
    },
  )
  assert.doesNotThrow(() => {
    inert = createFoldPreviewKeyboardCoordinator(hostileOptions)
  })
  assert.doesNotThrow(() => {
    inert?.handleKeyDown(new Proxy(
      {} as FoldPreviewKeyboardEvent,
      {
        get() {
          throw new Error('inert event getter')
        },
      },
    ))
  })

  const fixture = coordinatorFixture()
  const hostileTarget = Object.defineProperty(
    {},
    'target',
    {
      get() {
        throw new Error('target getter')
      },
    },
  ) as FoldPreviewKeyboardEvent
  const hostileKey = Object.defineProperty(
    {
      target: fixture.host,
    },
    'key',
    {
      get() {
        throw new Error('key getter')
      },
    },
  ) as FoldPreviewKeyboardEvent

  assert.doesNotThrow(() => fixture.handle(hostileTarget))
  assert.doesNotThrow(() => fixture.handle(hostileKey))
  assert.deepEqual(fixture.calls, [])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('runtime getter re-entry cannot dispatch against an invalidated context', () => {
  let current = true
  const getterCalls: string[] = []
  const fixture = coordinatorFixture({
    contextIsCurrent: () => current,
    canAnnounce: () => current,
    getSelectedHingeId: () => {
      getterCalls.push('selected')
      current = false
      return 'hinge-a'
    },
    getFixedFaceId: () => {
      getterCalls.push('fixed')
      return 'face-a'
    },
    getSelectHingeCallback: () => {
      getterCalls.push('select-callback')
      return () => undefined
    },
  })

  fixture.handle(key('h', fixture.host))

  assert.deepEqual(getterCalls, ['selected'])
  assert.deepEqual(fixture.calls, [])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('a nested selection supersedes the outer announcement', () => {
  let nested = false
  let fixture: ReturnType<typeof coordinatorFixture>
  fixture = coordinatorFixture({
    getSelectHingeCallback: () => (edgeId) => {
      fixture.calls.push(`select:${edgeId ?? 'null'}`)
      fixture.state.selectedHingeId = edgeId
      if (!nested) {
        nested = true
        fixture.handle(key('h', fixture.host))
      }
    },
  })

  fixture.handle(key('h', fixture.host))

  assert.deepEqual(fixture.calls, [
    'select:hinge-b',
    'select:hinge-a',
    'announce:ヒンジ 1/2 を選択しました',
    'prevent:h',
    'prevent:h',
  ])
})

test('dispose during selection suppresses publication and makes retained handlers inert', () => {
  let fixture: ReturnType<typeof coordinatorFixture>
  fixture = coordinatorFixture({
    getSelectHingeCallback: () => (edgeId) => {
      fixture.calls.push(`select:${edgeId ?? 'null'}`)
      fixture.dispose()
    },
  })

  fixture.handle(key('h', fixture.host))
  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(fixture.calls, [
    'select:hinge-b',
    'prevent:h',
  ])
  assert.deepEqual(fixture.cameraCalls, [])
})

test('announcement and preventDefault failures never escape or become camera failures', () => {
  let announcementAttempts = 0
  const fixture = coordinatorFixture({
    announce: () => {
      announcementAttempts += 1
      throw new Error('announcement failure')
    },
  })

  assert.doesNotThrow(() => fixture.handle(key('h', fixture.host)))
  const hostilePrevention = {
    ...key('ArrowUp', fixture.host),
    preventDefault() {
      throw new Error('preventDefault failure')
    },
  }
  assert.doesNotThrow(() => fixture.handle(hostilePrevention))

  assert.equal(announcementAttempts, 1)
  assert.deepEqual(fixture.calls, [
    'select:hinge-b',
    'prevent:h',
  ])
  assert.deepEqual(fixture.cameraCalls, ['pan:0,7'])
})

test('camera failure deactivates before one throwing lifecycle callback', () => {
  let fixture: ReturnType<typeof coordinatorFixture>
  let cameraAttempts = 0
  let failureAttempts = 0
  fixture = coordinatorFixture({
    cameraControls: {
      keyPanSpeed: 7,
      keyRotateSpeed: 4,
      pan() {
        cameraAttempts += 1
        throw new Error('camera failure')
      },
      rotateLeft() {},
      rotateUp() {},
      dollyIn() {},
      dollyOut() {},
      reset() {},
    },
    onCameraFailure: () => {
      failureAttempts += 1
      fixture.handle(key('ArrowUp', fixture.host))
      throw new Error('lifecycle failure')
    },
  })

  assert.doesNotThrow(() => fixture.handle(key('ArrowUp', fixture.host)))
  fixture.handle(key('ArrowUp', fixture.host))

  assert.equal(cameraAttempts, 1)
  assert.equal(failureAttempts, 1)
  assert.deepEqual(fixture.calls, [])
})

test('camera viewport re-entry cannot mutate a stale scene', () => {
  let current = true
  const fixture = coordinatorFixture({
    contextIsCurrent: () => current,
    getViewportHeight: () => {
      current = false
      return 400
    },
  })

  fixture.handle(key('ArrowUp', fixture.host))

  assert.deepEqual(fixture.cameraCalls, [])
  assert.deepEqual(fixture.calls, [])
})

test('a camera error after context invalidation cannot publish a stale failure', () => {
  let current = true
  let cameraAttempts = 0
  let failureAttempts = 0
  const fixture = coordinatorFixture({
    contextIsCurrent: () => current,
    cameraControls: {
      keyPanSpeed: 7,
      keyRotateSpeed: 4,
      pan() {
        cameraAttempts += 1
        current = false
        throw new Error('stale camera failure')
      },
      rotateLeft() {},
      rotateUp() {},
      dollyIn() {},
      dollyOut() {},
      reset() {},
    },
    onCameraFailure: () => {
      failureAttempts += 1
    },
  })

  fixture.handle(key('ArrowUp', fixture.host))
  current = true
  fixture.handle(key('ArrowUp', fixture.host))

  assert.equal(cameraAttempts, 1)
  assert.equal(failureAttempts, 0)
  assert.deepEqual(fixture.calls, [])
})

function coordinatorFixture(
  overrides: Partial<FoldPreviewKeyboardCoordinatorOptions> = {},
) {
  const host = new EventTarget()
  const calls: string[] = []
  const cameraCalls: string[] = []
  const state = {
    gesturesClean: true,
    current: true,
    selectedHingeId: 'hinge-a' as string | null,
    fixedFaceId: 'face-a' as string | null,
  }
  const options: FoldPreviewKeyboardCoordinatorOptions = {
    host,
    hingeIds: ['hinge-a', 'hinge-b'],
    faceIds: ['face-a', 'face-b'],
    foldGesturesAreClean: () => state.gesturesClean,
    resetFoldGestures: () => {
      calls.push('reset-gestures')
    },
    contextIsCurrent: () => state.current,
    canAnnounce: () => state.current,
    getSelectedHingeId: () => state.selectedHingeId,
    getFixedFaceId: () => state.fixedFaceId,
    getSelectHingeCallback: () => (edgeId) => {
      calls.push(`select:${edgeId ?? 'null'}`)
    },
    getChooseFixedFaceCallback: () => (faceId) => {
      calls.push(`fixed:${faceId}`)
    },
    announce: (text) => {
      calls.push(`announce:${text}`)
    },
    cameraControls: {
      keyPanSpeed: 7,
      keyRotateSpeed: 4,
      pan(deltaX, deltaY) {
        cameraCalls.push(`pan:${deltaX},${deltaY}`)
      },
      rotateLeft(angle) {
        cameraCalls.push(`rotate-left:${angle}`)
      },
      rotateUp(angle) {
        cameraCalls.push(`rotate-up:${angle}`)
      },
      dollyIn(scale) {
        cameraCalls.push(`dolly-in:${scale}`)
      },
      dollyOut(scale) {
        cameraCalls.push(`dolly-out:${scale}`)
      },
      reset() {
        cameraCalls.push('reset-camera')
      },
    },
    getViewportHeight: () => 400,
    onCameraFailure: () => {
      calls.push('camera-failure')
    },
    ...overrides,
  }
  const coordinator = createFoldPreviewKeyboardCoordinator(options)
  return {
    host,
    calls,
    cameraCalls,
    state,
    options,
    handle(event: FoldPreviewKeyboardEvent) {
      testCalls.set(event, calls)
      coordinator.handleKeyDown(event)
    },
    dispose() {
      coordinator.dispose()
    },
  }
}

function key(
  value: string,
  target: EventTarget,
): FoldPreviewKeyboardEvent {
  return {
    target,
    key: value,
    code: '',
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
    preventDefault() {
      testCalls.get(this)?.push(`prevent:${value}`)
    },
  }
}

const testCalls = new WeakMap<object, string[]>()
