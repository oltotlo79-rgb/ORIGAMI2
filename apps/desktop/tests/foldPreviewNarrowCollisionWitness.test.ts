import assert from 'node:assert/strict'
import test from 'node:test'

import {
  deriveFoldPreviewTrianglePrismWitness,
  MAX_FOLD_PREVIEW_WITNESS_POSITION_CANDIDATES,
  type FoldPreviewTrianglePrismWitnessInput,
  type FoldPreviewWitnessFrame,
  type FoldPreviewWitnessPoint,
} from '../src/lib/foldPreviewNarrowCollisionWitness.ts'

const MARGIN = 1e-7
const FRAME = Object.freeze({
  xAxis: Object.freeze({ x: 1, y: 0, z: 0 }),
  yAxis: Object.freeze({ x: 0, y: 1, z: 0 }),
  zAxis: Object.freeze({ x: 0, y: 0, z: 1 }),
})

test('identical prisms expose a covariant ambiguous minimum without auto-apply', () => {
  const vertices = prismVertices([
    point(0, 0),
    point(4, 0),
    point(0, 4),
  ], 0.2)
  const witness = derive(input(vertices, vertices))

  assert.ok(witness)
  assert.equal(witness.geometryClass, 'penetrating')
  assertPointClose(witness.normal.vector, { x: 0, y: 1, z: 0 })
  assertClose(witness.escapeDistance, 0.2)
  assert.equal(witness.toleratedGap, 0)
  assert.equal(witness.normal.uniqueness, 'one_of_multiple')
  assert.equal(witness.positionRegion.sourcePose, 'analyzed_input_pose')
  assert.equal(witness.localSeparationHint.autoApplicable, false)
  assert.equal(
    witness.localSeparationHint.scope,
    'selected_triangle_prism_pair_only',
  )
  assertPointClose(
    witness.localSeparationHint.translation,
    { x: 0, y: 0.2, z: 0 },
  )
  assertDeeplyFrozen(witness)
})

test('exact side and face contact return bounded support midpoint regions', () => {
  const first = standardPrism()
  const sideContact = derive(
    input(
      first,
      translateVertices(first, { x: 2, y: 0, z: 0 }),
      'touching',
    ),
  )
  const faceContact = derive(
    input(
      first,
      translateVertices(first, { x: 0, y: 0.2, z: 0 }),
      'touching',
    ),
  )

  assert.ok(sideContact)
  assert.equal(sideContact.geometryClass, 'touching')
  assert.equal(sideContact.escapeDistance, 0)
  assert.equal(sideContact.toleratedGap, 0)
  assertPointClose(sideContact.normal.vector, { x: 1, y: 0, z: 0 })
  assert.equal(sideContact.firstSupport.length, 2)
  assert.equal(sideContact.secondSupport.length, 4)
  assert.equal(sideContact.positionRegion.generators.length, 8)

  assert.ok(faceContact)
  assert.equal(faceContact.geometryClass, 'touching')
  assert.equal(faceContact.escapeDistance, 0)
  assertPointClose(faceContact.normal.vector, { x: 0, y: 1, z: 0 })
  assert.equal(faceContact.firstSupport.length, 3)
  assert.equal(faceContact.secondSupport.length, 3)
  assert.equal(faceContact.positionRegion.generators.length, 9)
})

test('a tolerated micro-gap stays distinct from penetration and separation', () => {
  const first = standardPrism()
  const gap = MARGIN / 2
  const witness = derive(input(
    first,
    translateVertices(first, { x: 0, y: 0.2 + gap, z: 0 }),
    'touching',
  ))

  assert.ok(witness)
  assert.equal(witness.geometryClass, 'touching')
  assert.equal(witness.escapeDistance, 0)
  assertClose(witness.toleratedGap, gap)
  assertPointClose(witness.normal.vector, { x: 0, y: 1, z: 0 })

  assert.equal(derive(input(
    first,
    translateVertices(first, {
      x: 0,
      y: 0.2 + MARGIN * 2,
      z: 0,
    }),
    'touching',
  )), null)
})

test('a tolerated micro-gap outranks a margin-tied overlap witness', () => {
  const first = prismVertices([
    point(0, 0),
    point(1, 0),
    point(0, 1),
  ], 0.2)
  const second = translateVertices(
    first.map((value) => rotateAroundY(value, 0.1)),
    {
      x: -0.994998166108028,
      y: 0,
      z: 1.0999323167134116,
    },
  )
  const witness = derive({
    ...input(first, second, 'touching'),
    numericalMargin: 1e-4,
  })

  assert.ok(witness)
  assert.equal(witness.geometryClass, 'touching')
  assert.equal(witness.escapeDistance, 0)
  assert.ok(witness.toleratedGap > 0)
  assert.ok(witness.toleratedGap <= 1e-4)
  assert.equal(witness.localSeparationHint.distance, 0)
})

test('contained intervals use escape distance instead of overlap length', () => {
  const outer = prismVertices([
    point(0, 0),
    point(10, 0),
    point(0, 10),
  ], 10)
  const inner = prismVertices([
    point(2, 2),
    point(3, 2),
    point(2, 3),
  ], 2)
  const witness = derive(input(outer, inner))

  assert.ok(witness)
  assert.equal(witness.geometryClass, 'penetrating')
  assertClose(witness.escapeDistance, 3)
  assertPointClose(witness.normal.vector, { x: -1, y: 0, z: 0 })
  assert.equal(witness.normal.uniqueness, 'one_of_multiple')
})

test('the local hint reaches contact for only the selected prism pair', () => {
  const first = standardPrism()
  const second = translateVertices(first, { x: 1.5, y: 0, z: 0 })
  const penetrating = derive(input(first, second))
  assert.ok(penetrating)
  assert.equal(penetrating.geometryClass, 'penetrating')
  assert.ok(penetrating.escapeDistance > 0)

  const moved = translateVertices(
    second,
    penetrating.localSeparationHint.translation,
  )
  const contact = derive(input(first, moved, 'touching'))
  assert.ok(contact)
  assert.equal(contact.geometryClass, 'touching')
  assert.equal(contact.escapeDistance, 0)
})

test('a common rigid transform rotates the normal and moves every generator', () => {
  const first = standardPrism()
  const second = translateVertices(first, { x: 1.5, y: 0, z: 0 })
  const baseline = derive(input(first, second))
  assert.ok(baseline)

  const transformedFirst = first.map(rigidTransform)
  const transformedSecond = second.map(rigidTransform)
  const transformedFrame = transformFrame(FRAME)
  const transformed = derive({
    ...input(transformedFirst, transformedSecond),
    firstFrame: transformedFrame,
  })
  assert.ok(transformed)

  assertClose(transformed.escapeDistance, baseline.escapeDistance)
  assertClose(transformed.toleratedGap, baseline.toleratedGap)
  assertPointClose(
    transformed.normal.vector,
    rotateVector(baseline.normal.vector),
  )
  assert.equal(
    transformed.positionRegion.generators.length,
    baseline.positionRegion.generators.length,
  )
  for (
    let index = 0;
    index < baseline.positionRegion.generators.length;
    index += 1
  ) {
    assertPointClose(
      transformed.positionRegion.generators[index],
      rigidTransform(baseline.positionRegion.generators[index]),
    )
  }
})

test('cyclic and reversed cap order preserve the canonical witness', () => {
  const first = standardPrism()
  const second = translateVertices(first, { x: 1.5, y: 0, z: 0 })
  const baseline = derive(input(first, second))
  const cyclicOrder = [1, 2, 0, 4, 5, 3]
  const reversedOrder = [0, 2, 1, 3, 5, 4]

  assert.ok(baseline)
  assert.deepEqual(derive(input(
    reorderVertices(first, cyclicOrder),
    reorderVertices(second, cyclicOrder),
  )), baseline)
  assert.deepEqual(derive(input(
    reorderVertices(first, reversedOrder),
    reorderVertices(second, reversedOrder),
  )), baseline)
})

test('four-by-four support reaches the fixed maximum without truncation', () => {
  const first = standardPrism()
  const second = prismVertices([
    point(-2, 0),
    point(0, 0),
    point(0, 2),
  ], 0.2)
  const witness = derive(input(first, second, 'touching'))

  assert.ok(witness)
  assert.equal(witness.geometryClass, 'touching')
  assertPointClose(witness.normal.vector, { x: -1, y: 0, z: 0 })
  assert.equal(witness.firstSupport.length, 4)
  assert.equal(witness.secondSupport.length, 4)
  assert.equal(
    witness.positionRegion.generators.length,
    MAX_FOLD_PREVIEW_WITNESS_POSITION_CANDIDATES,
  )

  assert.equal(derive({
    ...input(first, first),
    numericalMargin: 100,
  }), null)
})

test('zero thickness, near-parallel uncertainty, and bad frames fail closed', () => {
  const first = standardPrism()
  const zeroThickness = prismVertices([
    point(0, 0),
    point(2, 0),
    point(0, 2),
  ], 0)
  assert.equal(derive(input(zeroThickness, zeroThickness)), null)

  const tinyRotation = first.map((value) =>
    rotateAroundY(value, 5e-11))
  assert.equal(derive(input(first, tinyRotation)), null)

  const classifierBoundaryRotation = translateVertices(
    first.map((value) =>
      rotateAroundY(value, -2.843470732187364e-14)),
    {
      x: 5.903359691923659e-8,
      y: 0,
      z: 2.880402478302726e-8,
    },
  )
  assert.equal(derive({
    ...input(first, classifierBoundaryRotation),
    authoritativeGeometryClass: 'indeterminate' as never,
    numericalMargin: 1.1368684107728752e-13,
  }), null)

  const collinear = prismVertices([
    point(0, 0),
    point(1, 0),
    point(2, 1e-12),
  ], 0.2)
  assert.equal(derive(input(collinear, collinear)), null)

  assert.equal(derive({
    ...input(first, first),
    firstFrame: {
      ...FRAME,
      zAxis: FRAME.xAxis,
    },
  }), null)
  assert.equal(derive({
    ...input(first, first),
    firstFrame: {
      xAxis: FRAME.xAxis,
      yAxis: FRAME.zAxis,
      zAxis: FRAME.yAxis,
    },
  }), null)
  assert.equal(derive({
    ...input(first, first),
    firstFrame: {
      ...FRAME,
      xAxis: { x: 2, y: 0, z: 0 },
    },
  }), null)
})

test('translated common rotations retain classifier near-parallel uncertainty', () => {
  const firstMatrix = [
    0.8927127067006053, 0.1616528968632289, -0.4206332894945492, 0,
    -0.3202393711622924, 0.8842965166770353, -0.339803495789024, 0,
    0.3170343332398851, 0.4380502385898083, 0.8411903589667165, 0,
    11.614431224156219, -57.348580009503266, -64.99877976867418, 1,
  ] as const
  const secondMatrix = [
    ...firstMatrix.slice(0, 12),
    11.550383349922779, -57.17172070616515, -65.06674046783303, 1,
  ]
  // The production triangulator emits this triangle as [1, 2, 0].
  const local = reorderVertices(
    standardPrism(),
    [1, 2, 0, 4, 5, 3],
  )
  const first = applyMatrixToVertices(local, firstMatrix)
  const second = applyMatrixToVertices(local, secondMatrix)

  assert.equal(derive({
    ...input(first, second),
    authoritativeGeometryClass: 'indeterminate' as never,
    firstFrame: {
      xAxis: {
        x: firstMatrix[0],
        y: firstMatrix[1],
        z: firstMatrix[2],
      },
      yAxis: {
        x: firstMatrix[4],
        y: firstMatrix[5],
        z: firstMatrix[6],
      },
      zAxis: {
        x: firstMatrix[8],
        y: firstMatrix[9],
        z: firstMatrix[10],
      },
    },
    numericalMargin: 3.748368010087747e-12,
  }), null)
})

test('the authoritative class is mandatory and must match the local class', () => {
  const first = standardPrism()
  const contact = translateVertices(first, { x: 2, y: 0, z: 0 })

  assert.equal(derive(input(first, first, 'touching')), null)
  assert.equal(derive(input(first, contact, 'penetrating')), null)
  assert.equal(derive({
    ...input(first, first),
    authoritativeGeometryClass: 'indeterminate' as never,
  }), null)
})

test('large faces cannot hide a mismatched opposite cap', () => {
  const first = prismVertices([
    point(0, 0),
    point(1e9, 0),
    point(0, 1e9),
  ], 0.2)
  const mismatched = first.map((value) => ({ ...value }))
  mismatched[4].x += 0.5

  assert.equal(derive({
    ...input(first, mismatched),
    numericalMargin: 1e-4,
  }), null)
})

test('malformed, throwing, revoked, and non-finite inputs return null', () => {
  const first = standardPrism()
  const revoked = Proxy.revocable<FoldPreviewWitnessPoint[]>([], {})
  revoked.revoke()
  const candidates = [
    null,
    {},
    input(first.slice(0, 5), first),
    input([...first, first[0]], first),
    input(new Array(6), first),
    input(first, [{ x: Number.NaN, y: 0, z: 0 }, ...first.slice(1)]),
    input(first, [
      throwingProxy<FoldPreviewWitnessPoint>(),
      ...first.slice(1),
    ]),
    { ...input(first, first), numericalMargin: -1 },
    { ...input(first, first), numericalMargin: Number.POSITIVE_INFINITY },
    { ...input(first, first), authoritativeGeometryClass: 'indeterminate' },
    {
      ...input(first, first),
      firstFrame: {
        ...FRAME,
        xAxis: throwingProxy<FoldPreviewWitnessPoint>(),
      },
    },
    { ...input(first, first), firstVertices: revoked.proxy },
    throwingProxy<FoldPreviewTrianglePrismWitnessInput>(),
  ]
  for (const candidate of candidates) {
    assert.doesNotThrow(() =>
      deriveFoldPreviewTrianglePrismWitness(candidate as never))
    assert.equal(
      deriveFoldPreviewTrianglePrismWitness(candidate as never),
      null,
    )
  }
})

test('every caller-owned getter is snapshotted exactly once', () => {
  const reads = new Map<string, number>()
  const first = guardedVertices('first', standardPrism(), reads)
  const second = guardedVertices(
    'second',
    translateVertices(standardPrism(), { x: 1.5, y: 0, z: 0 }),
    reads,
  )
  const frame = guardedFrame(FRAME, reads)
  const guardedInput = Object.defineProperties({}, {
    firstVertices: onceProperty('input.firstVertices', first, reads),
    secondVertices: onceProperty('input.secondVertices', second, reads),
    firstFrame: onceProperty('input.firstFrame', frame, reads),
    numericalMargin: onceProperty('input.numericalMargin', MARGIN, reads),
    authoritativeGeometryClass: onceProperty(
      'input.authoritativeGeometryClass',
      'penetrating',
      reads,
    ),
  }) as FoldPreviewTrianglePrismWitnessInput

  assert.ok(derive(guardedInput))
  assert.ok(reads.size > 0)
  for (const [name, count] of reads) {
    assert.equal(count, 1, `${name} was read ${count} times`)
  }
})

test('negative zero margin is canonical and extreme arithmetic fails closed', () => {
  const first = standardPrism()
  const witness = derive({
    ...input(first, first),
    numericalMargin: -0,
  })
  assert.ok(witness)
  assert.equal(Object.is(witness.numericalMargin, -0), false)

  const extreme = first.map((value) => ({ ...value }))
  extreme[0].x = Number.MAX_VALUE
  assert.doesNotThrow(() => derive(input(extreme, first)))
  assert.equal(derive(input(extreme, first)), null)
})

test('the result snapshots mutable input and freezes every nested value', () => {
  const first = standardPrism().map((value) => ({ ...value }))
  const second = translateVertices(first, { x: 1.5, y: 0, z: 0 })
    .map((value) => ({ ...value }))
  const witness = derive(input(first, second))
  assert.ok(witness)
  const snapshot = JSON.stringify(witness)

  first[0].x = 999
  second[0].y = 999
  assert.equal(JSON.stringify(witness), snapshot)
  assertDeeplyFrozen(witness)
})

function derive(value: FoldPreviewTrianglePrismWitnessInput) {
  return deriveFoldPreviewTrianglePrismWitness(value)
}

function input(
  firstVertices: readonly FoldPreviewWitnessPoint[],
  secondVertices: readonly FoldPreviewWitnessPoint[],
  authoritativeGeometryClass: 'touching' | 'penetrating' = 'penetrating',
): FoldPreviewTrianglePrismWitnessInput {
  return {
    firstVertices,
    secondVertices,
    firstFrame: FRAME,
    numericalMargin: MARGIN,
    authoritativeGeometryClass,
  }
}

function standardPrism() {
  return prismVertices([
    point(0, 0),
    point(2, 0),
    point(0, 2),
  ], 0.2)
}

function point(x: number, z: number) {
  return { x, z }
}

function prismVertices(
  triangle: readonly Readonly<{ x: number; z: number }>[],
  thickness: number,
): FoldPreviewWitnessPoint[] {
  const half = thickness / 2
  return [
    ...triangle.map(({ x, z }) => ({ x, y: half, z })),
    ...triangle.map(({ x, z }) => ({ x, y: -half, z })),
  ]
}

function translateVertices(
  vertices: readonly FoldPreviewWitnessPoint[],
  translation: FoldPreviewWitnessPoint,
) {
  return vertices.map((value) => ({
    x: value.x + translation.x,
    y: value.y + translation.y,
    z: value.z + translation.z,
  }))
}

function applyMatrixToVertices(
  vertices: readonly FoldPreviewWitnessPoint[],
  elements: readonly number[],
) {
  return vertices.map((value) => ({
    x: elements[0] * value.x
      + elements[4] * value.y
      + elements[8] * value.z
      + elements[12],
    y: elements[1] * value.x
      + elements[5] * value.y
      + elements[9] * value.z
      + elements[13],
    z: elements[2] * value.x
      + elements[6] * value.y
      + elements[10] * value.z
      + elements[14],
  }))
}

function reorderVertices(
  vertices: readonly FoldPreviewWitnessPoint[],
  order: readonly number[],
) {
  return order.map((index) => vertices[index])
}

function rigidTransform(value: FoldPreviewWitnessPoint) {
  const rotated = rotateVector(value)
  return {
    x: rotated.x + 10,
    y: rotated.y - 4,
    z: rotated.z + 3,
  }
}

function rotateVector(value: FoldPreviewWitnessPoint) {
  return {
    x: -value.y,
    y: value.x,
    z: value.z,
  }
}

function transformFrame(
  frame: FoldPreviewWitnessFrame,
): FoldPreviewWitnessFrame {
  return {
    xAxis: rotateVector(frame.xAxis),
    yAxis: rotateVector(frame.yAxis),
    zAxis: rotateVector(frame.zAxis),
  }
}

function rotateAroundY(
  value: FoldPreviewWitnessPoint,
  angle: number,
) {
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  return {
    x: cosine * value.x + sine * value.z,
    y: value.y,
    z: -sine * value.x + cosine * value.z,
  }
}

function assertPointClose(
  actual: FoldPreviewWitnessPoint,
  expected: FoldPreviewWitnessPoint,
) {
  assertClose(actual.x, expected.x)
  assertClose(actual.y, expected.y)
  assertClose(actual.z, expected.z)
}

function assertClose(actual: number, expected: number) {
  assert.ok(
    Math.abs(actual - expected) <= 1e-9,
    `${actual} is not close to ${expected}`,
  )
}

function throwingProxy<Value>(): Value {
  return new Proxy({}, {
    get() {
      throw new Error('unexpected property access')
    },
  }) as Value
}

function guardedVertices(
  prefix: string,
  vertices: readonly FoldPreviewWitnessPoint[],
  reads: Map<string, number>,
) {
  const guarded = vertices.map((value, index) =>
    guardedPoint(`${prefix}[${index}]`, value, reads))
  return new Proxy(guarded, {
    get(target, property, receiver) {
      if (
        property === 'length'
        || (
          typeof property === 'string'
          && /^\d+$/u.test(property)
        )
      ) {
        recordRead(`${prefix}.${String(property)}`, reads)
      }
      return Reflect.get(target, property, receiver)
    },
  })
}

function guardedFrame(
  frame: FoldPreviewWitnessFrame,
  reads: Map<string, number>,
) {
  return Object.defineProperties({}, {
    xAxis: onceProperty(
      'frame.xAxis',
      guardedPoint('frame.xAxis.point', frame.xAxis, reads),
      reads,
    ),
    yAxis: onceProperty(
      'frame.yAxis',
      guardedPoint('frame.yAxis.point', frame.yAxis, reads),
      reads,
    ),
    zAxis: onceProperty(
      'frame.zAxis',
      guardedPoint('frame.zAxis.point', frame.zAxis, reads),
      reads,
    ),
  }) as FoldPreviewWitnessFrame
}

function guardedPoint(
  prefix: string,
  value: FoldPreviewWitnessPoint,
  reads: Map<string, number>,
) {
  return Object.defineProperties({}, {
    x: onceProperty(`${prefix}.x`, value.x, reads),
    y: onceProperty(`${prefix}.y`, value.y, reads),
    z: onceProperty(`${prefix}.z`, value.z, reads),
  }) as FoldPreviewWitnessPoint
}

function onceProperty(
  name: string,
  value: unknown,
  reads: Map<string, number>,
) {
  return {
    enumerable: true,
    get() {
      recordRead(name, reads)
      return value
    },
  }
}

function recordRead(name: string, reads: Map<string, number>) {
  const count = (reads.get(name) ?? 0) + 1
  reads.set(name, count)
  if (count > 1) throw new Error(`${name} was read repeatedly`)
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
