import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS,
  MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_HINGE_TARGETS,
  MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH,
  resolveFoldPreviewKeyboardSelection,
  type FoldPreviewKeyboardSelectionEvent,
  type FoldPreviewKeyboardSelectionInput,
} from '../src/lib/foldPreviewKeyboardSelection.ts'

const HINGE_IDS = Object.freeze(['hinge-z', 'hinge-a', 'hinge-m'])
const FACE_IDS = Object.freeze(['face-2', 'face-0', 'face-1'])
const IGNORED = Object.freeze({ handled: false, command: null })

test('H and Shift+H cycle hinges in the supplied deterministic order', () => {
  assert.deepEqual(resolve(input({
    event: key('h', 'KeyH'),
    selectedHingeId: 'hinge-z',
  })), {
    handled: true,
    command: { kind: 'select_hinge', edgeId: 'hinge-a' },
  })
  assert.deepEqual(resolve(input({
    event: key('H', 'KeyH', { shiftKey: true }),
    selectedHingeId: 'hinge-z',
  })), {
    handled: true,
    command: { kind: 'select_hinge', edgeId: 'hinge-m' },
  })
  assert.deepEqual(resolve(input({
    selectedHingeId: 'hinge-m',
  })).command, {
    kind: 'select_hinge',
    edgeId: 'hinge-z',
  })
  assert.deepEqual(resolve(input({
    event: key('H', 'KeyH', { shiftKey: true }),
    selectedHingeId: 'hinge-a',
  })).command, {
    kind: 'select_hinge',
    edgeId: 'hinge-z',
  })
})

test('F and Shift+F cycle fixed faces without sorting caller IDs', () => {
  assert.deepEqual(resolve(input({
    event: key('f', 'KeyF'),
    fixedFaceId: 'face-2',
  })).command, {
    kind: 'choose_fixed_face',
    faceId: 'face-0',
  })
  assert.deepEqual(resolve(input({
    event: key('F', 'KeyF', { shiftKey: true }),
    fixedFaceId: 'face-2',
  })).command, {
    kind: 'choose_fixed_face',
    faceId: 'face-1',
  })
  assert.deepEqual(resolve(input({
    event: key('f', 'KeyF'),
    fixedFaceId: 'face-1',
  })).command, {
    kind: 'choose_fixed_face',
    faceId: 'face-2',
  })
})

test('null and stale current IDs use the documented end for each direction', () => {
  for (const selectedHingeId of [null, 'stale-hinge']) {
    assert.deepEqual(resolve(input({
      event: key('h', 'KeyH'),
      selectedHingeId,
    })).command, {
      kind: 'select_hinge',
      edgeId: 'hinge-z',
    })
    assert.deepEqual(resolve(input({
      event: key('H', 'KeyH', { shiftKey: true }),
      selectedHingeId,
    })).command, {
      kind: 'select_hinge',
      edgeId: 'hinge-m',
    })
  }

  for (const fixedFaceId of [null, 'stale-face']) {
    assert.deepEqual(resolve(input({
      event: key('f', 'KeyF'),
      fixedFaceId,
    })).command, {
      kind: 'choose_fixed_face',
      faceId: 'face-2',
    })
    assert.deepEqual(resolve(input({
      event: key('F', 'KeyF', { shiftKey: true }),
      fixedFaceId,
    })).command, {
      kind: 'choose_fixed_face',
      faceId: 'face-1',
    })
  }
})

test('one target is a no-op in both directions', () => {
  for (const event of [
    key('h', 'KeyH'),
    key('H', 'KeyH', { shiftKey: true }),
  ]) {
    assert.deepEqual(resolve(input({
      event,
      hingeIds: ['only-hinge'],
      selectedHingeId: 'only-hinge',
    })), IGNORED)
  }
  for (const event of [
    key('f', 'KeyF'),
    key('F', 'KeyF', { shiftKey: true }),
  ]) {
    assert.deepEqual(resolve(input({
      event,
      faceIds: ['only-face'],
      fixedFaceId: 'only-face',
    })), IGNORED)
  }
  assert.equal(resolve(input({
    hingeIds: ['only-hinge'],
    selectedHingeId: null,
  })).handled, true)
  assert.equal(resolve(input({
    event: key('f', 'KeyF'),
    faceIds: ['only-face'],
    fixedFaceId: 'stale-face',
  })).handled, true)
})

test('Escape clears current and stale hinges but ignores an already clear state', () => {
  for (const selectedHingeId of ['hinge-z', 'stale-hinge']) {
    assert.deepEqual(resolve(input({
      event: key('Escape', 'Escape'),
      selectedHingeId,
    })), {
      handled: true,
      command: { kind: 'clear_hinge' },
    })
  }
  assert.deepEqual(resolve(input({
    event: key('Escape', 'Escape'),
    selectedHingeId: null,
  })), IGNORED)
})

test('event key is authoritative across layouts and remapped physical codes', () => {
  assert.equal(resolve(input({
    event: key('h', ''),
  })).handled, true)
  assert.deepEqual(resolve(input({
    event: key('', 'KeyH'),
  })), IGNORED)
  assert.deepEqual(resolve(input({
    event: key('Unidentified', 'KeyF'),
  })), IGNORED)
  assert.equal(resolve(input({
    event: key('h', 'KeyJ'),
  })).handled, true)
  assert.equal(resolve(input({
    event: key('h', 'KeyF'),
  })).handled, true)
  assert.deepEqual(resolve(input({
    event: key('f', 'KeyH'),
  })).command, {
    kind: 'choose_fixed_face',
    faceId: 'face-0',
  })
  assert.deepEqual(resolve(input({
    event: key('Escape', 'KeyH'),
  })).command, {
    kind: 'clear_hinge',
  })
  assert.deepEqual(resolve(input({
    event: key('x', 'KeyX'),
  })), IGNORED)
})

test('modifiers, repeat, composition, and shifted Escape never issue commands', () => {
  const cases: FoldPreviewKeyboardSelectionEvent[] = [
    key('h', 'KeyH', { altKey: true }),
    key('h', 'KeyH', { ctrlKey: true }),
    key('h', 'KeyH', { metaKey: true }),
    key('h', 'KeyH', { repeat: true }),
    key('h', 'KeyH', { isComposing: true }),
    key('Escape', 'Escape', { shiftKey: true }),
  ]
  for (const event of cases) {
    assert.deepEqual(resolve(input({ event })), IGNORED)
  }
})

test('malformed event fields and excessive key tokens fail closed', () => {
  const malformed = [
    null,
    {},
    { ...key('h', 'KeyH'), key: 1 },
    { ...key('h', 'KeyH'), code: null },
    { ...key('h', 'KeyH'), altKey: 'false' },
    { ...key('h', 'KeyH'), ctrlKey: 0 },
    { ...key('h', 'KeyH'), metaKey: null },
    { ...key('h', 'KeyH'), shiftKey: 'yes' },
    { ...key('h', 'KeyH'), repeat: 0 },
    { ...key('h', 'KeyH'), isComposing: undefined },
    { ...key('h', 'KeyH'), key: 'h'.repeat(65) },
    { ...key('h', 'KeyH'), code: 'K'.repeat(65) },
  ]
  for (const event of malformed) {
    assert.deepEqual(resolve(input({
      event: event as FoldPreviewKeyboardSelectionEvent,
    })), IGNORED)
  }
})

test('callback availability gates actions before target data is accessed', () => {
  const throwingIds = throwingArrayProxy()
  assert.deepEqual(resolve(input({
    hasSelectHingeCallback: false,
    hingeIds: throwingIds,
  })), IGNORED)
  assert.deepEqual(resolve(input({
    event: key('f', 'KeyF'),
    hasChooseFixedFaceCallback: false,
    faceIds: throwingIds,
  })), IGNORED)
  assert.deepEqual(resolve(input({
    event: key('Escape', 'Escape'),
    hasSelectHingeCallback: false,
    selectedHingeId: 'hinge-z',
    hingeIds: throwingIds,
  })), IGNORED)
  assert.deepEqual(resolve(input({
    hasSelectHingeCallback: 'yes',
  } as never)), IGNORED)
  assert.deepEqual(resolve(input({
    event: key('f', 'KeyF'),
    hasChooseFixedFaceCallback: 1,
  } as never)), IGNORED)
})

test('only the target collection relevant to the shortcut is inspected', () => {
  assert.equal(resolve(input({
    hingeIds: HINGE_IDS,
    faceIds: throwingArrayProxy(),
  })).handled, true)
  assert.equal(resolve(input({
    event: key('f', 'KeyF'),
    hingeIds: throwingArrayProxy(),
    faceIds: FACE_IDS,
  })).handled, true)
  assert.equal(resolve(input({
    event: key('Escape', 'Escape'),
    hingeIds: throwingArrayProxy(),
    faceIds: throwingArrayProxy(),
  })).handled, true)
})

test('empty, duplicate, sparse, non-string, and oversized IDs fail closed', () => {
  const tooLong = 'x'.repeat(
    MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH + 1,
  )
  const invalidHinges = [
    [],
    ['duplicate', 'duplicate'],
    ['', 'valid'],
    [' \t\n', 'valid'],
    [tooLong],
    ['valid', 1],
    new Array(1),
    { 0: 'valid', length: 1 },
  ]
  for (const hingeIds of invalidHinges) {
    assert.deepEqual(resolve(input({
      hingeIds: hingeIds as readonly string[],
    })), IGNORED)
  }

  const invalidFaces = [
    [],
    ['duplicate', 'duplicate'],
    ['valid', ''],
    ['valid', '\u3000'],
    [tooLong],
    [Symbol('face')],
    new Array(2),
  ]
  for (const faceIds of invalidFaces) {
    assert.deepEqual(resolve(input({
      event: key('f', 'KeyF'),
      faceIds: faceIds as readonly string[],
    })), IGNORED)
  }
})

test('malformed current IDs fail closed while well-formed stale IDs remain usable', () => {
  for (const selectedHingeId of [
    '',
    ' \t',
    'x'.repeat(MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH + 1),
    1,
    undefined,
  ]) {
    assert.deepEqual(resolve(input({
      selectedHingeId: selectedHingeId as string | null,
    })), IGNORED)
  }
  for (const fixedFaceId of [
    '',
    '\u3000',
    'x'.repeat(MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_ID_LENGTH + 1),
    false,
    undefined,
  ]) {
    assert.deepEqual(resolve(input({
      event: key('f', 'KeyF'),
      fixedFaceId: fixedFaceId as string | null,
    })), IGNORED)
  }
})

test('collection limits are inclusive and reject oversized proxies before indexes', () => {
  const faceIds = Array.from(
    { length: MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS },
    (_, index) => `face-${index}`,
  )
  assert.deepEqual(resolve(input({
    event: key('f', 'KeyF'),
    faceIds,
    fixedFaceId: faceIds.at(-1) ?? null,
  })).command, {
    kind: 'choose_fixed_face',
    faceId: 'face-0',
  })

  for (const [maximum, field, event] of [
    [
      MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_HINGE_TARGETS,
      'hingeIds',
      key('h', 'KeyH'),
    ],
    [
      MAX_FOLD_PREVIEW_KEYBOARD_SELECTION_FACE_TARGETS,
      'faceIds',
      key('f', 'KeyF'),
    ],
  ] as const) {
    const reads = { length: 0, index: 0 }
    const oversized = oversizedArrayProxy(maximum + 1, reads)
    assert.deepEqual(resolve(input({
      event,
      [field]: oversized,
    })), IGNORED)
    assert.deepEqual(reads, { length: 1, index: 0 })
  }
})

test('throwing and revoked proxies are contained at every public boundary', () => {
  const revoked = Proxy.revocable<string[]>([], {})
  revoked.revoke()
  const inputs = [
    throwingProxy<FoldPreviewKeyboardSelectionInput>(),
    input({ event: throwingProxy<FoldPreviewKeyboardSelectionEvent>() }),
    input({ hingeIds: throwingArrayProxy() }),
    input({
      event: key('f', 'KeyF'),
      faceIds: revoked.proxy,
    }),
  ]
  for (const candidate of inputs) {
    assert.doesNotThrow(() => resolve(candidate))
    assert.deepEqual(resolve(candidate), IGNORED)
  }
})

test('stateful getters are captured exactly once into a stable decision', () => {
  const reads: Record<string, number> = {
    inputEvent: 0,
    availability: 0,
    hingeIds: 0,
    selectedHingeId: 0,
    length: 0,
    index0: 0,
    index1: 0,
    key: 0,
    code: 0,
    altKey: 0,
    ctrlKey: 0,
    metaKey: 0,
    shiftKey: 0,
    repeat: 0,
    isComposing: 0,
  }
  const firstEventValues = {
    key: 'h',
    code: 'KeyH',
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
  } as const
  const statefulEvent = Object.fromEntries(
    Object.entries(firstEventValues).map(([field, value]) => [
      field,
      {
        enumerable: true,
        get() {
          reads[field] += 1
          return reads[field] === 1 ? value : invalidEventValue(field)
        },
      },
    ]),
  )
  const event = Object.defineProperties(
    {},
    statefulEvent,
  ) as FoldPreviewKeyboardSelectionEvent
  const hingeIds = new Proxy(['hinge-z', 'hinge-a'], {
    get(target, property, receiver) {
      if (property === 'length') reads.length += 1
      else if (property === '0') reads.index0 += 1
      else if (property === '1') reads.index1 += 1
      const count = property === 'length'
        ? reads.length
        : property === '0'
          ? reads.index0
          : property === '1'
            ? reads.index1
            : 1
      return count === 1
        ? Reflect.get(target, property, receiver)
        : throwingProxy()
    },
  })
  const guardedInput = {
    get event() {
      reads.inputEvent += 1
      return reads.inputEvent === 1 ? event : throwingProxy()
    },
    get hasSelectHingeCallback() {
      reads.availability += 1
      return reads.availability === 1
    },
    get hingeIds() {
      reads.hingeIds += 1
      return reads.hingeIds === 1 ? hingeIds : throwingArrayProxy()
    },
    get selectedHingeId() {
      reads.selectedHingeId += 1
      return reads.selectedHingeId === 1 ? 'hinge-z' : ''
    },
  } as FoldPreviewKeyboardSelectionInput

  assert.deepEqual(resolve(guardedInput), {
    handled: true,
    command: { kind: 'select_hinge', edgeId: 'hinge-a' },
  })
  for (const count of Object.values(reads)) assert.equal(count, 1)
})

test('commands and results are deeply frozen and retain no mutable target list', () => {
  const hingeIds = ['hinge-z', 'hinge-a']
  const handled = resolve(input({
    hingeIds,
    selectedHingeId: 'hinge-z',
  }))
  hingeIds[1] = 'mutated'
  assert.deepEqual(handled, {
    handled: true,
    command: { kind: 'select_hinge', edgeId: 'hinge-a' },
  })
  assertDeeplyFrozen(handled)
  assertDeeplyFrozen(resolve(input({
    event: key('x', 'KeyX'),
  })))
})

function input(
  overrides: Partial<FoldPreviewKeyboardSelectionInput> = {},
): FoldPreviewKeyboardSelectionInput {
  return {
    event: key('h', 'KeyH'),
    hingeIds: HINGE_IDS,
    faceIds: FACE_IDS,
    selectedHingeId: 'hinge-z',
    fixedFaceId: 'face-2',
    hasSelectHingeCallback: true,
    hasChooseFixedFaceCallback: true,
    ...overrides,
  }
}

function resolve(value: FoldPreviewKeyboardSelectionInput) {
  return resolveFoldPreviewKeyboardSelection(value)
}

function key(
  keyValue: string,
  code: string,
  overrides: Partial<FoldPreviewKeyboardSelectionEvent> = {},
): FoldPreviewKeyboardSelectionEvent {
  return {
    key: keyValue,
    code,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
    ...overrides,
  }
}

function invalidEventValue(field: string) {
  return field === 'key' || field === 'code' ? null : 'invalid'
}

function throwingProxy<Value>(): Value {
  return new Proxy({}, {
    get() {
      throw new Error('unexpected input access')
    },
  }) as Value
}

function throwingArrayProxy(): readonly string[] {
  return new Proxy<string[]>([], {
    get() {
      throw new Error('unexpected array access')
    },
  })
}

function oversizedArrayProxy(
  length: number,
  reads: { length: number; index: number },
): readonly string[] {
  return new Proxy<string[]>([], {
    get(target, property, receiver) {
      if (property === 'length') {
        reads.length += 1
        return length
      }
      if (typeof property === 'string' && /^\d+$/u.test(property)) {
        reads.index += 1
        throw new Error('oversized arrays must not be indexed')
      }
      return Reflect.get(target, property, receiver)
    },
  })
}

function assertDeeplyFrozen(value: unknown): void {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const property of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[property],
    )
  }
}
