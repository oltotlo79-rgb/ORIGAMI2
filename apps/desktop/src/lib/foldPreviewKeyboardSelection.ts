export const MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS = 10_000
export const MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_HINGE_TARGETS = 100_000
export const MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH = 512

const MAX_KEYBOARD_EVENT_TOKEN_LENGTH = 64

export type FoldPreviewKeyboardSelectionEvent = Readonly<{
  key: string
  code: string
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
  repeat: boolean
  isComposing: boolean
}>

export type FoldPreviewKeyboardSelectionInput = Readonly<{
  event: FoldPreviewKeyboardSelectionEvent
  hingeIds: readonly string[]
  faceIds: readonly string[]
  selectedHingeId: string | null
  fixedFaceId: string | null
  hasSelectHingeCallback: boolean
  hasChooseFixedFaceCallback: boolean
}>

export type FoldPreviewKeyboardSelectionCommand =
  | Readonly<{
      kind: 'select_hinge'
      edgeId: string
    }>
  | Readonly<{
      kind: 'choose_fixed_face'
      faceId: string
    }>
  | Readonly<{
      kind: 'clear_hinge'
    }>

export type FoldPreviewKeyboardSelectionResult =
  | Readonly<{
      handled: false
      command: null
    }>
  | Readonly<{
      handled: true
      command: FoldPreviewKeyboardSelectionCommand
    }>

type SelectionShortcut =
  | Readonly<{
      kind: 'cycle_hinge'
      direction: 'next' | 'previous'
    }>
  | Readonly<{
      kind: 'cycle_face'
      direction: 'next' | 'previous'
    }>
  | Readonly<{
      kind: 'clear_hinge'
    }>

const IGNORED_RESULT: FoldPreviewKeyboardSelectionResult = Object.freeze({
  handled: false,
  command: null,
})

/**
 * Resolves one keyboard event captured by the focused 3D viewport.
 *
 * H / Shift+H cycle hinges, F / Shift+F cycle fixed faces, and Escape clears
 * the selected hinge. Target arrays are kept in caller order; this boundary
 * never sorts IDs. When the current ID is null or stale, `next` starts at the
 * first target and `previous` starts at the last target.
 *
 * Every relevant getter is read once into an immutable snapshot. Malformed,
 * duplicated, empty, oversized, throwing, composing, or repeated input is
 * ignored without retaining a caller-owned object in the result.
 */
export function resolveFoldPreviewKeyboardSelection(
  input: FoldPreviewKeyboardSelectionInput,
): FoldPreviewKeyboardSelectionResult {
  try {
    if (!isRecord(input)) return IGNORED_RESULT
    const rawEvent = input.event
    const shortcut = snapshotShortcut(rawEvent)
    if (!shortcut) return IGNORED_RESULT

    if (shortcut.kind === 'clear_hinge') {
      const rawAvailability = input.hasSelectHingeCallback
      if (rawAvailability !== true) return IGNORED_RESULT
      const rawSelectedHingeId = input.selectedHingeId
      if (rawSelectedHingeId === null) return IGNORED_RESULT
      if (!validId(rawSelectedHingeId)) return IGNORED_RESULT
      return handled(Object.freeze({ kind: 'clear_hinge' }))
    }

    if (shortcut.kind === 'cycle_hinge') {
      const rawAvailability = input.hasSelectHingeCallback
      if (rawAvailability !== true) return IGNORED_RESULT
      const rawHingeIds = input.hingeIds
      const rawSelectedHingeId = input.selectedHingeId
      const hingeIds = snapshotIds(
        rawHingeIds,
        MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_HINGE_TARGETS,
      )
      const selectedHingeId = snapshotCurrentId(rawSelectedHingeId)
      if (!hingeIds || selectedHingeId === undefined) return IGNORED_RESULT
      const edgeId = cycleTarget(
        hingeIds,
        selectedHingeId,
        shortcut.direction,
      )
      return edgeId === null || edgeId === selectedHingeId
        ? IGNORED_RESULT
        : handled(Object.freeze({ kind: 'select_hinge', edgeId }))
    }

    const rawAvailability = input.hasChooseFixedFaceCallback
    if (rawAvailability !== true) return IGNORED_RESULT
    const rawFaceIds = input.faceIds
    const rawFixedFaceId = input.fixedFaceId
    const faceIds = snapshotIds(
      rawFaceIds,
      MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS,
    )
    const fixedFaceId = snapshotCurrentId(rawFixedFaceId)
    if (!faceIds || fixedFaceId === undefined) return IGNORED_RESULT
    const faceId = cycleTarget(faceIds, fixedFaceId, shortcut.direction)
    return faceId === null || faceId === fixedFaceId
      ? IGNORED_RESULT
      : handled(Object.freeze({ kind: 'choose_fixed_face', faceId }))
  } catch {
    return IGNORED_RESULT
  }
}

function snapshotShortcut(
  value: unknown,
): SelectionShortcut | null {
  if (!isRecord(value)) return null
  const rawKey = value.key
  const rawCode = value.code
  const rawAltKey = value.altKey
  const rawCtrlKey = value.ctrlKey
  const rawMetaKey = value.metaKey
  const rawShiftKey = value.shiftKey
  const rawRepeat = value.repeat
  const rawIsComposing = value.isComposing
  if (
    !validEventToken(rawKey)
    || !validEventToken(rawCode)
    || typeof rawAltKey !== 'boolean'
    || typeof rawCtrlKey !== 'boolean'
    || typeof rawMetaKey !== 'boolean'
    || typeof rawShiftKey !== 'boolean'
    || typeof rawRepeat !== 'boolean'
    || typeof rawIsComposing !== 'boolean'
    || rawAltKey
    || rawCtrlKey
    || rawMetaKey
    || rawRepeat
    || rawIsComposing
  ) return null

  const kind = shortcutKindForKey(rawKey)
  if (!kind) return null
  if (kind === 'clear_hinge') {
    return rawShiftKey
      ? null
      : Object.freeze({ kind: 'clear_hinge' })
  }
  return Object.freeze({
    kind,
    direction: rawShiftKey ? 'previous' : 'next',
  })
}

function shortcutKindForKey(
  key: string,
): SelectionShortcut['kind'] | null {
  if (key === 'h' || key === 'H') return 'cycle_hinge'
  if (key === 'f' || key === 'F') return 'cycle_face'
  if (key === 'Escape') return 'clear_hinge'
  return null
}

function snapshotIds(
  value: unknown,
  maximum: number,
): readonly string[] | null {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length < 1
    || length > maximum
  ) return null
  const ids = new Array<string>(length)
  const uniqueIds = new Set<string>()
  for (let index = 0; index < length; index += 1) {
    const id = value[index]
    if (!validId(id) || uniqueIds.has(id)) return null
    uniqueIds.add(id)
    ids[index] = id
  }
  return Object.freeze(ids)
}

function snapshotCurrentId(
  value: unknown,
): string | null | undefined {
  return value === null || validId(value) ? value : undefined
}

function cycleTarget(
  ids: readonly string[],
  currentId: string | null,
  direction: 'next' | 'previous',
) {
  const currentIndex = currentId === null ? -1 : ids.indexOf(currentId)
  if (currentIndex < 0) {
    return direction === 'next' ? ids[0] ?? null : ids.at(-1) ?? null
  }
  const offset = direction === 'next' ? 1 : -1
  return ids[(currentIndex + offset + ids.length) % ids.length] ?? null
}

function handled(
  command: FoldPreviewKeyboardSelectionCommand,
): FoldPreviewKeyboardSelectionResult {
  return Object.freeze({
    handled: true,
    command,
  })
}

function validEventToken(value: unknown): value is string {
  return typeof value === 'string'
    && value.length <= MAX_KEYBOARD_EVENT_TOKEN_LENGTH
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH
    && value.trim().length > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
