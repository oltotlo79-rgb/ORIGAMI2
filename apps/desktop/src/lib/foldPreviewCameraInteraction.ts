export type FoldPreviewPointerSample = Readonly<{
  pointerId: number
  clientX: number
  clientY: number
}>

export type FoldPreviewPointerStart = FoldPreviewPointerSample & Readonly<{
  button: number
  isPrimary: boolean
}>

export type FoldPreviewSelectionGesture = Readonly<{
  pointerDown(sample: FoldPreviewPointerStart): void
  pointerMove(sample: FoldPreviewPointerSample): void
  pointerUp(sample: FoldPreviewPointerSample): boolean
  pointerCancel(pointerId: number): void
  reset(): void
}>

export type FoldPreviewCameraCommand =
  | 'pan_up'
  | 'pan_down'
  | 'pan_left'
  | 'pan_right'
  | 'rotate_up'
  | 'rotate_down'
  | 'rotate_left'
  | 'rotate_right'
  | 'zoom_in'
  | 'zoom_out'
  | 'reset'

export type FoldPreviewCameraControls = Readonly<{
  keyPanSpeed: number
  keyRotateSpeed: number
  pan(deltaX: number, deltaY: number): void
  rotateLeft(angle: number): void
  rotateUp(angle: number): void
  dollyIn(scale: number): void
  dollyOut(scale: number): void
  reset(): void
}>

const MAXIMUM_TAP_DISTANCE_LIMIT = 64

/**
 * Accepts only one primary-button pointer that never leaves a CSS-pixel tap
 * radius. Once a gesture becomes a drag, multi-pointer action, or cancellation,
 * returning to the start point cannot turn it back into a selection.
 */
export function createFoldPreviewSelectionGesture(
  maximumTapDistance = 6,
): FoldPreviewSelectionGesture {
  if (
    !Number.isFinite(maximumTapDistance)
    || maximumTapDistance < 0
    || maximumTapDistance > MAXIMUM_TAP_DISTANCE_LIMIT
  ) {
    throw new RangeError('maximum tap distance must be between 0 and 64 CSS pixels')
  }
  const maximumDistanceSquared = maximumTapDistance * maximumTapDistance
  const activePointers = new Set<number>()
  let candidate: Readonly<{
    pointerId: number
    startX: number
    startY: number
  }> | null = null
  const reset = () => {
    activePointers.clear()
    candidate = null
  }

  return Object.freeze({
    pointerDown(sample) {
      if (!validPointerStart(sample)) {
        reset()
        return
      }
      if (activePointers.has(sample.pointerId)) {
        candidate = null
        return
      }
      activePointers.add(sample.pointerId)
      if (activePointers.size !== 1) {
        candidate = null
        return
      }
      candidate = sample.isPrimary === true && sample.button === 0
        ? {
            pointerId: sample.pointerId,
            startX: sample.clientX,
            startY: sample.clientY,
          }
        : null
    },
    pointerMove(sample) {
      if (!candidate) return
      if (!validPointerSample(sample)) {
        candidate = null
        return
      }
      if (sample.pointerId !== candidate.pointerId) return
      if (exceedsTapDistance(candidate, sample, maximumDistanceSquared)) candidate = null
    },
    pointerUp(sample) {
      if (!validPointerSample(sample)) {
        reset()
        return false
      }
      const accepted = Boolean(
        candidate
        && sample.pointerId === candidate.pointerId
        && activePointers.size === 1
        && !exceedsTapDistance(candidate, sample, maximumDistanceSquared),
      )
      activePointers.delete(sample.pointerId)
      if (candidate?.pointerId === sample.pointerId) candidate = null
      if (activePointers.size === 0) candidate = null
      return accepted
    },
    pointerCancel(pointerId) {
      if (!validPointerId(pointerId)) {
        reset()
        return
      }
      activePointers.delete(pointerId)
      candidate = null
    },
    reset,
  })
}

export function resolveFoldPreviewCameraCommand(input: Readonly<{
  code: string
  key: string
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
}>): FoldPreviewCameraCommand | null {
  if (
    !validKeyboardInput(input)
    || input.altKey
    || input.ctrlKey
    || input.metaKey
  ) return null
  if (input.key === '+' || input.key === '=') return 'zoom_in'
  if (input.key === '-') return 'zoom_out'
  if (input.key === 'Home' || input.key === '0') return 'reset'
  if (input.key === 'ArrowUp') return input.shiftKey ? 'rotate_up' : 'pan_up'
  if (input.key === 'ArrowDown') return input.shiftKey ? 'rotate_down' : 'pan_down'
  if (input.key === 'ArrowLeft') return input.shiftKey ? 'rotate_left' : 'pan_left'
  if (input.key === 'ArrowRight') return input.shiftKey ? 'rotate_right' : 'pan_right'
  return null
}

export function applyFoldPreviewCameraCommand(
  controls: FoldPreviewCameraControls,
  command: FoldPreviewCameraCommand,
  viewportHeight: number,
) {
  if (command === 'zoom_in') {
    controls.dollyIn(0.9)
    return true
  }
  if (command === 'zoom_out') {
    controls.dollyOut(0.9)
    return true
  }
  if (command === 'reset') {
    controls.reset()
    return true
  }
  if (!isPositiveFinite(viewportHeight)) return false
  if (
    command === 'pan_up'
    || command === 'pan_down'
    || command === 'pan_left'
    || command === 'pan_right'
  ) {
    if (!isNonNegativeFinite(controls.keyPanSpeed)) return false
    if (command === 'pan_up') controls.pan(0, controls.keyPanSpeed)
    else if (command === 'pan_down') controls.pan(0, -controls.keyPanSpeed)
    else if (command === 'pan_left') controls.pan(controls.keyPanSpeed, 0)
    else controls.pan(-controls.keyPanSpeed, 0)
    return true
  }
  if (!isNonNegativeFinite(controls.keyRotateSpeed)) return false
  const rotationStep = 2 * Math.PI * controls.keyRotateSpeed / viewportHeight
  if (!Number.isFinite(rotationStep)) return false
  if (command === 'rotate_up') controls.rotateUp(rotationStep)
  else if (command === 'rotate_down') controls.rotateUp(-rotationStep)
  else if (command === 'rotate_left') controls.rotateLeft(rotationStep)
  else if (command === 'rotate_right') controls.rotateLeft(-rotationStep)
  else return false
  return true
}

function exceedsTapDistance(
  candidate: Readonly<{ startX: number; startY: number }>,
  sample: FoldPreviewPointerSample,
  maximumDistanceSquared: number,
) {
  const deltaX = sample.clientX - candidate.startX
  const deltaY = sample.clientY - candidate.startY
  const distanceSquared = deltaX * deltaX + deltaY * deltaY
  return !Number.isFinite(distanceSquared) || distanceSquared > maximumDistanceSquared
}

function validPointerSample(sample: FoldPreviewPointerSample) {
  return Boolean(
    sample
    && validPointerId(sample.pointerId)
    && Number.isFinite(sample.clientX)
    && Number.isFinite(sample.clientY),
  )
}

function validPointerStart(sample: FoldPreviewPointerStart) {
  return validPointerSample(sample)
    && Number.isSafeInteger(sample.button)
    && sample.button >= 0
    && typeof sample.isPrimary === 'boolean'
}

function validPointerId(pointerId: number) {
  return Number.isSafeInteger(pointerId) && pointerId >= 0
}

function isPositiveFinite(value: number) {
  return Number.isFinite(value) && value > 0
}

function isNonNegativeFinite(value: number) {
  return Number.isFinite(value) && value >= 0
}

function validKeyboardInput(input: Readonly<{
  code: string
  key: string
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
}>) {
  return Boolean(
    input
    && typeof input.code === 'string'
    && typeof input.key === 'string'
    && typeof input.altKey === 'boolean'
    && typeof input.ctrlKey === 'boolean'
    && typeof input.metaKey === 'boolean'
    && typeof input.shiftKey === 'boolean',
  )
}
