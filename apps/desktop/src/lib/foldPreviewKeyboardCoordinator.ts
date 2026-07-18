import {
  applyFoldPreviewCameraCommand,
  resolveFoldPreviewCameraCommand,
  type FoldPreviewCameraControls,
} from './foldPreviewCameraInteraction.ts'
import {
  MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS,
  MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_HINGE_TARGETS,
  MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH,
  resolveFoldPreviewKeyboardSelection,
  type FoldPreviewKeyboardSelectionEvent,
} from './foldPreviewKeyboardSelection.ts'

export type FoldPreviewKeyboardEvent =
  FoldPreviewKeyboardSelectionEvent
  & Readonly<{
    target: EventTarget | null
    preventDefault(): void
  }>

export type FoldPreviewKeyboardCoordinator = Readonly<{
  handleKeyDown(event: FoldPreviewKeyboardEvent): void
  dispose(): void
}>

export type FoldPreviewKeyboardCoordinatorOptions = Readonly<{
  host: EventTarget
  hingeIds: readonly string[]
  faceIds: readonly string[]
  foldGesturesAreClean(): boolean
  resetFoldGestures(): void
  contextIsCurrent(): boolean
  canAnnounce(): boolean
  getSelectedHingeId(): string | null
  getFixedFaceId(): string | null
  getSelectHingeCallback(): ((edgeId: string | null) => void) | undefined
  getChooseFixedFaceCallback(): ((faceId: string) => void) | undefined
  announce(text: string): void
  cameraControls: FoldPreviewCameraControls
  getViewportHeight(): number
  onCameraFailure(): void
}>

type FoldPreviewKeyboardCoordinatorSnapshot = Readonly<{
  host: EventTarget
  hingeIds: readonly string[]
  faceIds: readonly string[]
  foldGesturesAreClean(): boolean
  resetFoldGestures(): void
  contextIsCurrent(): boolean
  canAnnounce(): boolean
  getSelectedHingeId(): string | null
  getFixedFaceId(): string | null
  getSelectHingeCallback(): ((edgeId: string | null) => void) | undefined
  getChooseFixedFaceCallback(): ((faceId: string) => void) | undefined
  announce(text: string): void
  cameraControls: FoldPreviewCameraControls
  getViewportHeight(): number
  onCameraFailure(): void
}>

type FoldPreviewKeyboardEventTargetSnapshot = Readonly<{
  source: object
  target: unknown
}>

type FoldPreviewKeyboardSelectionRuntime = Readonly<{
  selectedHingeId: string | null
  fixedFaceId: string | null
  selectHinge: ((edgeId: string | null) => void) | undefined
  chooseFixedFace: ((faceId: string) => void) | undefined
}>

const EMPTY_IDS: readonly string[] = Object.freeze([])
const INERT_COORDINATOR: FoldPreviewKeyboardCoordinator = Object.freeze({
  handleKeyDown() {},
  dispose() {},
})

/**
 * Owns keyboard routing for one FoldPreview scene lifecycle.
 *
 * Selection shortcuts take priority over camera shortcuts. Caller-owned
 * options, target arrays, camera methods, and each keyboard event are detached
 * before routing. Context authority is checked after every re-entrant boundary
 * and again before dispatch or publication. `dispose` permanently invalidates
 * retained handlers in addition to the caller removing its event listener.
 */
export function createFoldPreviewKeyboardCoordinator(
  options: FoldPreviewKeyboardCoordinatorOptions,
): FoldPreviewKeyboardCoordinator {
  const snapshot = snapshotCoordinatorOptions(options)
  if (!snapshot) return INERT_COORDINATOR

  let current: FoldPreviewKeyboardCoordinatorSnapshot | null = snapshot
  let routingGeneration = 0

  const isAuthoritative = (
    expected: FoldPreviewKeyboardCoordinatorSnapshot,
    generation: number,
  ) => current === expected && routingGeneration === generation

  const contextRemainsCurrent = (
    expected: FoldPreviewKeyboardCoordinatorSnapshot,
    generation: number,
  ) => {
    if (!isAuthoritative(expected, generation)) return false
    let isCurrent = false
    try {
      isCurrent = expected.contextIsCurrent() === true
    } catch {
      return false
    }
    return isCurrent && isAuthoritative(expected, generation)
  }

  const failCamera = (
    expected: FoldPreviewKeyboardCoordinatorSnapshot,
  ) => {
    if (current !== expected) return
    let isCurrent = false
    try {
      isCurrent = expected.contextIsCurrent() === true
    } catch {
      // A context that cannot prove authority must not mutate shared UI state.
    }
    if (current !== expected) return
    current = null
    routingGeneration += 1
    if (!isCurrent) return
    try {
      expected.onCameraFailure()
    } catch {
      // Failure reporting and lifecycle cleanup are external best-effort work.
    }
  }

  const coordinator: FoldPreviewKeyboardCoordinator = Object.freeze({
    handleKeyDown(event) {
      const expected = current
      if (!expected) return

      const generationBeforeTarget = routingGeneration
      const targetSnapshot = snapshotEventTarget(event)
      if (
        !targetSnapshot
        || current !== expected
        || targetSnapshot.target !== expected.host
        || generationBeforeTarget !== routingGeneration
      ) return

      const generation = routingGeneration + 1
      routingGeneration = generation
      let gesturesAreClean = false
      try {
        gesturesAreClean = expected.foldGesturesAreClean() === true
      } catch {
        // An unreadable gesture state is unsafe and follows the reset path.
      }
      if (!isAuthoritative(expected, generation)) return
      if (!gesturesAreClean) {
        try {
          expected.resetFoldGestures()
        } catch {
          // A rejected reset cannot make routing the pending key safe.
        }
        safePreventTargetEvent(targetSnapshot)
        return
      }

      if (!contextRemainsCurrent(expected, generation)) return
      const routedEvent = snapshotRoutingEvent(
        targetSnapshot,
        () => contextRemainsCurrent(expected, generation),
      )
      if (
        !routedEvent
        || !contextRemainsCurrent(expected, generation)
      ) return

      const runtime = snapshotSelectionRuntime(
        expected,
        () => contextRemainsCurrent(expected, generation),
      )
      if (
        !runtime
        || !contextRemainsCurrent(expected, generation)
      ) return
      const selectionResult = resolveFoldPreviewKeyboardSelection({
        event: routedEvent,
        hingeIds: expected.hingeIds,
        faceIds: expected.faceIds,
        selectedHingeId: runtime.selectedHingeId,
        fixedFaceId: runtime.fixedFaceId,
        hasSelectHingeCallback: Boolean(runtime.selectHinge),
        hasChooseFixedFaceCallback: Boolean(runtime.chooseFixedFace),
      })
      if (selectionResult.handled) {
        let announcement: string | null = null
        try {
          const command = selectionResult.command
          if (command.kind === 'select_hinge') {
            const index = expected.hingeIds.indexOf(command.edgeId)
            if (!runtime.selectHinge || index < 0) return
            runtime.selectHinge(command.edgeId)
            announcement =
              `ヒンジ ${index + 1}/${expected.hingeIds.length} を選択しました`
          } else if (command.kind === 'choose_fixed_face') {
            const index = expected.faceIds.indexOf(command.faceId)
            if (!runtime.chooseFixedFace || index < 0) return
            runtime.chooseFixedFace(command.faceId)
            announcement =
              `面 ${index + 1}/${expected.faceIds.length} を固定面に設定しました`
          } else {
            if (!runtime.selectHinge) return
            runtime.selectHinge(null)
            announcement = 'ヒンジ選択を解除しました'
          }
        } catch {
          safePreventTargetEvent(targetSnapshot)
          return
        }

        if (
          announcement
          && contextRemainsCurrent(expected, generation)
        ) {
          let canAnnounce = false
          try {
            canAnnounce = expected.canAnnounce() === true
          } catch {
            // A failed publication guard suppresses only the announcement.
          }
          if (canAnnounce && isAuthoritative(expected, generation)) {
            try {
              expected.announce(announcement)
            } catch {
              // The selection has already committed; announcement is optional.
            }
          }
        }
        safePreventTargetEvent(targetSnapshot)
        return
      }

      const cameraCommand = resolveFoldPreviewCameraCommand(routedEvent)
      if (!cameraCommand) return
      let viewportHeight: number
      try {
        viewportHeight = expected.getViewportHeight()
      } catch {
        failCamera(expected)
        return
      }
      if (!contextRemainsCurrent(expected, generation)) return
      try {
        if (!applyFoldPreviewCameraCommand(
          expected.cameraControls,
          cameraCommand,
          viewportHeight,
        )) return
      } catch {
        failCamera(expected)
        return
      }
      safePreventTargetEvent(targetSnapshot)
    },
    dispose() {
      if (!current) return
      current = null
      routingGeneration += 1
    },
  })
  return coordinator
}

function snapshotCoordinatorOptions(
  value: unknown,
): FoldPreviewKeyboardCoordinatorSnapshot | null {
  try {
    if (!isObjectRecord(value)) return null
    const host = value.host
    const rawHingeIds = value.hingeIds
    const rawFaceIds = value.faceIds
    const foldGesturesAreClean = value.foldGesturesAreClean
    const resetFoldGestures = value.resetFoldGestures
    const contextIsCurrent = value.contextIsCurrent
    const canAnnounce = value.canAnnounce
    const getSelectedHingeId = value.getSelectedHingeId
    const getFixedFaceId = value.getFixedFaceId
    const getSelectHingeCallback = value.getSelectHingeCallback
    const getChooseFixedFaceCallback = value.getChooseFixedFaceCallback
    const announce = value.announce
    const rawCameraControls = value.cameraControls
    const getViewportHeight = value.getViewportHeight
    const onCameraFailure = value.onCameraFailure
    if (
      !isObjectRecord(host)
      || typeof foldGesturesAreClean !== 'function'
      || typeof resetFoldGestures !== 'function'
      || typeof contextIsCurrent !== 'function'
      || typeof canAnnounce !== 'function'
      || typeof getSelectedHingeId !== 'function'
      || typeof getFixedFaceId !== 'function'
      || typeof getSelectHingeCallback !== 'function'
      || typeof getChooseFixedFaceCallback !== 'function'
      || typeof announce !== 'function'
      || typeof getViewportHeight !== 'function'
      || typeof onCameraFailure !== 'function'
    ) return null

    const cameraControls = snapshotCameraControls(rawCameraControls)
    if (!cameraControls) return null
    const snapshot: FoldPreviewKeyboardCoordinatorSnapshot = {
      host: host as unknown as EventTarget,
      hingeIds: snapshotIds(
        rawHingeIds,
        MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_HINGE_TARGETS,
      ) ?? EMPTY_IDS,
      faceIds: snapshotIds(
        rawFaceIds,
        MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS,
      ) ?? EMPTY_IDS,
      foldGesturesAreClean: () =>
        Reflect.apply(foldGesturesAreClean, undefined, []) as boolean,
      resetFoldGestures: () => {
        Reflect.apply(resetFoldGestures, undefined, [])
      },
      contextIsCurrent: () =>
        Reflect.apply(contextIsCurrent, undefined, []) as boolean,
      canAnnounce: () =>
        Reflect.apply(canAnnounce, undefined, []) as boolean,
      getSelectedHingeId: () =>
        Reflect.apply(getSelectedHingeId, undefined, []) as string | null,
      getFixedFaceId: () =>
        Reflect.apply(getFixedFaceId, undefined, []) as string | null,
      getSelectHingeCallback: () =>
        callbackOrUndefined<(edgeId: string | null) => void>(
          Reflect.apply(getSelectHingeCallback, undefined, []),
        ),
      getChooseFixedFaceCallback: () =>
        callbackOrUndefined<(faceId: string) => void>(
          Reflect.apply(getChooseFixedFaceCallback, undefined, []),
        ),
      announce: (text: string) => {
        Reflect.apply(announce, undefined, [text])
      },
      cameraControls,
      getViewportHeight: () =>
        Reflect.apply(getViewportHeight, undefined, []) as number,
      onCameraFailure: () => {
        Reflect.apply(onCameraFailure, undefined, [])
      },
    }
    return Object.freeze(snapshot)
  } catch {
    return null
  }
}

function snapshotCameraControls(
  value: unknown,
): FoldPreviewCameraControls | null {
  try {
    if (!isObjectRecord(value)) return null
    const keyPanSpeed = value.keyPanSpeed
    const keyRotateSpeed = value.keyRotateSpeed
    const pan = value.pan
    const rotateLeft = value.rotateLeft
    const rotateUp = value.rotateUp
    const dollyIn = value.dollyIn
    const dollyOut = value.dollyOut
    const reset = value.reset
    if (
      typeof keyPanSpeed !== 'number'
      || typeof keyRotateSpeed !== 'number'
      || typeof pan !== 'function'
      || typeof rotateLeft !== 'function'
      || typeof rotateUp !== 'function'
      || typeof dollyIn !== 'function'
      || typeof dollyOut !== 'function'
      || typeof reset !== 'function'
    ) return null
    return Object.freeze({
      keyPanSpeed,
      keyRotateSpeed,
      pan: (deltaX, deltaY) => {
        Reflect.apply(pan, value, [deltaX, deltaY])
      },
      rotateLeft: (angle) => {
        Reflect.apply(rotateLeft, value, [angle])
      },
      rotateUp: (angle) => {
        Reflect.apply(rotateUp, value, [angle])
      },
      dollyIn: (scale) => {
        Reflect.apply(dollyIn, value, [scale])
      },
      dollyOut: (scale) => {
        Reflect.apply(dollyOut, value, [scale])
      },
      reset: () => {
        Reflect.apply(reset, value, [])
      },
    })
  } catch {
    return null
  }
}

function snapshotEventTarget(
  value: unknown,
): FoldPreviewKeyboardEventTargetSnapshot | null {
  try {
    if (!isObjectRecord(value)) return null
    return Object.freeze({
      source: value,
      target: value.target,
    })
  } catch {
    return null
  }
}

function snapshotRoutingEvent(
  value: FoldPreviewKeyboardEventTargetSnapshot | null,
  remainsAuthoritative: () => boolean,
): FoldPreviewKeyboardEvent | null {
  if (!value) return null
  try {
    const source = value.source as Record<string, unknown>
    const key = source.key
    if (!remainsAuthoritative()) return null
    const code = source.code
    if (!remainsAuthoritative()) return null
    const altKey = source.altKey
    if (!remainsAuthoritative()) return null
    const ctrlKey = source.ctrlKey
    if (!remainsAuthoritative()) return null
    const metaKey = source.metaKey
    if (!remainsAuthoritative()) return null
    const shiftKey = source.shiftKey
    if (!remainsAuthoritative()) return null
    const repeat = source.repeat
    if (!remainsAuthoritative()) return null
    const isComposing = source.isComposing
    if (!remainsAuthoritative()) return null
    if (
      typeof key !== 'string'
      || typeof code !== 'string'
      || typeof altKey !== 'boolean'
      || typeof ctrlKey !== 'boolean'
      || typeof metaKey !== 'boolean'
      || typeof shiftKey !== 'boolean'
      || typeof repeat !== 'boolean'
      || typeof isComposing !== 'boolean'
    ) return null
    return Object.freeze({
      target: value.target as EventTarget | null,
      key,
      code,
      altKey,
      ctrlKey,
      metaKey,
      shiftKey,
      repeat,
      isComposing,
      // Routing never invokes this placeholder. Suppression reads the original
      // event method only after a command has committed, matching DOM ordering.
      preventDefault() {},
    })
  } catch {
    return null
  }
}

function snapshotSelectionRuntime(
  options: FoldPreviewKeyboardCoordinatorSnapshot,
  remainsAuthoritative: () => boolean,
): FoldPreviewKeyboardSelectionRuntime | null {
  try {
    const selectedHingeId = options.getSelectedHingeId()
    if (!remainsAuthoritative()) return null
    const fixedFaceId = options.getFixedFaceId()
    if (!remainsAuthoritative()) return null
    const selectHinge = options.getSelectHingeCallback()
    if (!remainsAuthoritative()) return null
    const chooseFixedFace = options.getChooseFixedFaceCallback()
    if (!remainsAuthoritative()) return null
    return Object.freeze({
      selectedHingeId,
      fixedFaceId,
      selectHinge,
      chooseFixedFace,
    })
  } catch {
    return null
  }
}

function snapshotIds(
  value: unknown,
  maximum: number,
): readonly string[] | null {
  try {
    if (!Array.isArray(value)) return null
    const length = value.length
    if (
      !Number.isSafeInteger(length)
      || length < 0
      || length > maximum
    ) return null
    const result = new Array<string>(length)
    const unique = new Set<string>()
    for (let index = 0; index < length; index += 1) {
      const id = value[index]
      if (
        typeof id !== 'string'
        || id.length < 1
        || id.length > MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH
        || id.trim().length < 1
        || unique.has(id)
      ) return null
      unique.add(id)
      result[index] = id
    }
    return Object.freeze(result)
  } catch {
    return null
  }
}

function callbackOrUndefined<T>(
  value: unknown,
): T | undefined {
  return typeof value === 'function' ? value as T : undefined
}

function safePreventTargetEvent(
  value: FoldPreviewKeyboardEventTargetSnapshot,
) {
  try {
    const preventDefault =
      (value.source as Record<string, unknown>).preventDefault
    if (typeof preventDefault === 'function') {
      Reflect.apply(preventDefault, value.source, [])
    }
  } catch {
    // A pending gesture still blocks keyboard routing if suppression fails.
  }
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
