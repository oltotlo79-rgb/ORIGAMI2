import assert from 'node:assert/strict'
import test from 'node:test'

import {
  applyFoldPreviewCameraCommand,
  createFoldPreviewSelectionGesture,
  resolveFoldPreviewCameraCommand,
  type FoldPreviewCameraControls,
} from '../src/lib/foldPreviewCameraInteraction.ts'

test('a primary stationary pointer produces one selection gesture', () => {
  const gesture = createFoldPreviewSelectionGesture()
  gesture.pointerDown(start(1, 10, 20))
  assert.equal(gesture.pointerUp(sample(1, 10, 20)), true)
  assert.equal(gesture.pointerUp(sample(1, 10, 20)), false)
})

test('tap radius is inclusive and a returned drag stays rejected', () => {
  const gesture = createFoldPreviewSelectionGesture(5)
  gesture.pointerDown(start(1, 0, 0))
  gesture.pointerMove(sample(1, 3, 4))
  assert.equal(gesture.pointerUp(sample(1, 3, 4)), true)

  gesture.pointerDown(start(2, 0, 0))
  gesture.pointerMove(sample(2, 6, 0))
  gesture.pointerMove(sample(2, 0, 0))
  assert.equal(gesture.pointerUp(sample(2, 0, 0)), false)
})

test('a pointer-up beyond the tap radius rejects the selection', () => {
  const gesture = createFoldPreviewSelectionGesture(5)
  gesture.pointerDown(start(1, 0, 0))
  assert.equal(gesture.pointerUp(sample(1, 3, 5)), false)
})

test('right, middle, and non-primary pointers never select', () => {
  const gesture = createFoldPreviewSelectionGesture()
  gesture.pointerDown(start(1, 0, 0, 1))
  assert.equal(gesture.pointerUp(sample(1, 0, 0)), false)
  gesture.pointerDown(start(2, 0, 0, 2))
  assert.equal(gesture.pointerUp(sample(2, 0, 0)), false)
  gesture.pointerDown(start(3, 0, 0, 0, false))
  assert.equal(gesture.pointerUp(sample(3, 0, 0)), false)
})

test('multi-pointer and cancelled gestures fail closed until released', () => {
  const gesture = createFoldPreviewSelectionGesture()
  gesture.pointerDown(start(1, 0, 0))
  gesture.pointerDown(start(2, 1, 1, 0, false))
  assert.equal(gesture.pointerUp(sample(1, 0, 0)), false)
  assert.equal(gesture.pointerUp(sample(2, 1, 1)), false)

  gesture.pointerDown(start(3, 0, 0))
  assert.equal(gesture.pointerUp(sample(3, 0, 0)), true)

  gesture.pointerDown(start(4, 0, 0))
  gesture.pointerDown(start(5, 1, 1, 0, false))
  gesture.pointerCancel(5)
  assert.equal(gesture.pointerUp(sample(4, 0, 0)), false)
  gesture.pointerDown(start(6, 0, 0))
  assert.equal(gesture.pointerUp(sample(6, 0, 0)), true)
})

test('invalid pointer samples fail closed and recover for the next tap', () => {
  const gesture = createFoldPreviewSelectionGesture()
  gesture.pointerDown(start(1, 0, 0))
  gesture.pointerMove(sample(1, Number.NaN, 0))
  assert.equal(gesture.pointerUp(sample(1, 0, 0)), false)

  gesture.pointerDown(start(2, 0, 0))
  assert.equal(gesture.pointerUp(null as never), false)
  gesture.pointerDown(start(3, 0, 0))
  assert.equal(gesture.pointerUp(sample(3, 0, 0)), true)

  gesture.pointerDown({ ...start(4, 0, 0), isPrimary: 'yes' } as never)
  assert.equal(gesture.pointerUp(sample(4, 0, 0)), false)
})

test('tap threshold accepts its safe range and rejects invalid or excessive values', () => {
  assert.doesNotThrow(() => createFoldPreviewSelectionGesture(0))
  assert.doesNotThrow(() => createFoldPreviewSelectionGesture(64))
  for (const threshold of [-1, 65, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_VALUE]) {
    assert.throws(() => createFoldPreviewSelectionGesture(threshold), RangeError)
  }
})

test('camera key map separates every pan and rotate direction', () => {
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowUp')), 'pan_up')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowDown')), 'pan_down')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowLeft')), 'pan_left')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowRight')), 'pan_right')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowUp', { shiftKey: true })), 'rotate_up')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowDown', { shiftKey: true })), 'rotate_down')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowLeft', { shiftKey: true })), 'rotate_left')
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowRight', { shiftKey: true })), 'rotate_right')
})

test('camera key map supports zoom and reset across physical keyboard layouts', () => {
  assert.equal(resolveFoldPreviewCameraCommand(key('Equal')), 'zoom_in')
  assert.equal(resolveFoldPreviewCameraCommand(key('Semicolon', {
    key: '+',
    shiftKey: true,
  })), 'zoom_in')
  assert.equal(resolveFoldPreviewCameraCommand(key('NumpadSubtract')), 'zoom_out')
  assert.equal(resolveFoldPreviewCameraCommand(key('Home')), 'reset')
  assert.equal(resolveFoldPreviewCameraCommand(key('Digit0')), 'reset')
  assert.equal(resolveFoldPreviewCameraCommand(key('Digit0', {
    key: ')',
    shiftKey: true,
  })), null)
  assert.equal(resolveFoldPreviewCameraCommand(key('Minus', {
    key: '_',
    shiftKey: true,
  })), null)
})

test('camera key map preserves shortcuts, unrelated keys, and malformed input', () => {
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowUp', { ctrlKey: true })), null)
  assert.equal(resolveFoldPreviewCameraCommand(key('Equal', { metaKey: true })), null)
  assert.equal(resolveFoldPreviewCameraCommand(key('ArrowLeft', { altKey: true })), null)
  assert.equal(resolveFoldPreviewCameraCommand(key('KeyA')), null)
  assert.equal(resolveFoldPreviewCameraCommand(null as never), null)
  assert.equal(resolveFoldPreviewCameraCommand({
    ...key('ArrowUp'),
    shiftKey: 'yes',
  } as never), null)
})

test('camera commands apply deterministic pan, rotate, zoom, and reset operations', () => {
  const { controls, calls } = cameraControls()
  const commands = [
    'pan_up',
    'pan_down',
    'pan_left',
    'pan_right',
    'rotate_up',
    'rotate_down',
    'rotate_left',
    'rotate_right',
    'zoom_in',
    'zoom_out',
    'reset',
  ] as const
  for (const command of commands) {
    assert.equal(applyFoldPreviewCameraCommand(controls, command, 100), true)
  }
  const rotationStep = 2 * Math.PI / 100
  assert.deepEqual(calls, [
    ['pan', 0, 7],
    ['pan', 0, -7],
    ['pan', 7, 0],
    ['pan', -7, 0],
    ['rotateUp', rotationStep],
    ['rotateUp', -rotationStep],
    ['rotateLeft', rotationStep],
    ['rotateLeft', -rotationStep],
    ['dollyIn', 0.9],
    ['dollyOut', 0.9],
    ['reset'],
  ])
})

test('camera commands reject unsafe dimensions and speeds before changing the view', () => {
  const { controls, calls } = cameraControls()
  for (const height of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(applyFoldPreviewCameraCommand(controls, 'pan_up', height), false)
    assert.equal(applyFoldPreviewCameraCommand(controls, 'rotate_up', height), false)
  }
  assert.deepEqual(calls, [])

  assert.equal(applyFoldPreviewCameraCommand({
    ...controls,
    keyPanSpeed: Number.NaN,
  }, 'pan_left', 100), false)
  assert.equal(applyFoldPreviewCameraCommand({
    ...controls,
    keyRotateSpeed: Number.POSITIVE_INFINITY,
  }, 'rotate_left', 100), false)
  assert.equal(applyFoldPreviewCameraCommand(
    controls,
    'unknown' as never,
    100,
  ), false)
  assert.deepEqual(calls, [])

  assert.equal(applyFoldPreviewCameraCommand(controls, 'zoom_in', 0), true)
  assert.equal(applyFoldPreviewCameraCommand(controls, 'reset', 0), true)
  assert.deepEqual(calls, [['dollyIn', 0.9], ['reset']])
})

function start(
  pointerId: number,
  clientX: number,
  clientY: number,
  button = 0,
  isPrimary = true,
) {
  return { pointerId, clientX, clientY, button, isPrimary }
}

function sample(pointerId: number, clientX: number, clientY: number) {
  return { pointerId, clientX, clientY }
}

function key(
  code: string,
  overrides: Partial<{
    key: string
    altKey: boolean
    ctrlKey: boolean
    metaKey: boolean
    shiftKey: boolean
  }> = {},
) {
  return {
    code,
    key: keyValue(code),
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    ...overrides,
  }
}

function keyValue(code: string) {
  if (code === 'Equal') return '='
  if (code === 'Minus' || code === 'NumpadSubtract') return '-'
  if (code === 'NumpadAdd') return '+'
  if (code === 'Digit0' || code === 'Numpad0') return '0'
  return code
}

function cameraControls() {
  const calls: Array<readonly [string, ...number[]]> = []
  const controls: FoldPreviewCameraControls = {
    keyPanSpeed: 7,
    keyRotateSpeed: 1,
    pan: (deltaX, deltaY) => calls.push(['pan', deltaX, deltaY]),
    rotateLeft: (angle) => calls.push(['rotateLeft', angle]),
    rotateUp: (angle) => calls.push(['rotateUp', angle]),
    dollyIn: (scale) => calls.push(['dollyIn', scale]),
    dollyOut: (scale) => calls.push(['dollyOut', scale]),
    reset: () => calls.push(['reset']),
  }
  return { controls, calls }
}
