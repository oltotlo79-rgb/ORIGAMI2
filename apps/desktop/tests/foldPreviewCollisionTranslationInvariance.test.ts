import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type {
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  prepareFoldPreviewNarrowPhase,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const positiveOverlap = 0.01
const farTranslationX = 1e12
const triangleArea = 8
const thicknesses = [0.1, 1, 3] as const
const boundaryTranslations = [
  { label: 'origin', x: 0 },
  { label: 'translated-x-1e12', x: farTranslationX },
] as const

const faces: readonly FoldPreviewCollisionPoseFace[] = [
  {
    id: 'parallel-first',
    polygon: [
      { x: -2, z: -2 },
      { x: 2, z: -2 },
      { x: 0, z: 2 },
    ],
  },
  {
    id: 'parallel-second',
    polygon: [
      { x: -2, z: -2 },
      { x: 2, z: -2 },
      { x: 0, z: 2 },
    ],
  },
]

const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
assert.ok(analyzer)

test('a mismatched current shared vertex cannot become allowed after translation', () => {
  const shared = { vertexId: 'shared', x: 0, z: 0 } as const
  const mismatchFaces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'a',
      polygon: [
        shared,
        { vertexId: 'a-low', x: -2, z: -1 },
        { vertexId: 'a-high', x: -2, z: 1 },
      ],
    },
    {
      id: 'b',
      polygon: [
        shared,
        { vertexId: 'b-low', x: 2, z: -1 },
        { vertexId: 'b-high', x: 2, z: 1 },
      ],
    },
  ]

  for (const reverseFaces of [false, true]) {
    const mismatchAnalyzer = prepareFoldPreviewNarrowPhase(
      reverseFaces ? [...mismatchFaces].reverse() : mismatchFaces,
      [],
    )
    assert.ok(mismatchAnalyzer)
    for (const commonTranslation of [0, 1e12, 3e12, 1e15]) {
      const common = new Matrix4().makeTranslation(
        commonTranslation,
        commonTranslation,
        commonTranslation,
      )
      const result = mismatchAnalyzer.analyze(new Map([
        [
          'a',
          common.clone().multiply(
            new Matrix4().makeRotationZ(Math.PI / 4),
          ),
        ],
        [
          'b',
          common.clone().multiply(
            new Matrix4().makeTranslation(0, 0.1, 0),
          ),
        ],
      ]), 1)
      assert.ok(result)
      const interaction = result.interactions[0]
      const diagnostic = JSON.stringify({
        reverseFaces,
        commonTranslation,
        numericalMargin: result.numericalMargin,
        interaction,
      })
      assert.equal(result.interactions.length, 1, diagnostic)
      assert.equal(interaction?.relation, 'non_adjacent', diagnostic)
      assert.equal(interaction?.geometryClass, 'indeterminate', diagnostic)
      assert.equal(interaction?.topologyContact, undefined, diagnostic)
    }
  }
})

test('world-ULP-collapsed overlap and gap stay indeterminate in every input order', () => {
  const permutations = trianglePermutations([
    { x: -800, z: -800 },
    { x: 800, z: -800 },
    { x: 0, z: 800 },
  ])
  const overlapPose = parallelSlabPose({
    rotation: new Matrix4().makeRotationZ(135 * Math.PI / 180),
    commonTranslation: new Vector3(1e15, 0, 0),
    centerSeparation: 0.075,
  })
  const gapPose = parallelSlabPose({
    rotation: new Matrix4().makeRotationX(Math.PI / 4),
    commonTranslation: new Vector3(1e15, 1e15, 1e15),
    centerSeparation: 3.25,
  })
  const scenarios = [
    {
      label: '0.025 mm positive overlap',
      thickness: 0.1,
      pose: overlapPose,
    },
    {
      label: '0.25 mm positive gap',
      thickness: 3,
      pose: gapPose,
    },
  ] as const

  for (const scenario of scenarios) {
    for (let firstOrder = 0; firstOrder < permutations.length; firstOrder += 1) {
      for (
        let secondOrder = 0;
        secondOrder < permutations.length;
        secondOrder += 1
      ) {
        for (const reverseFaces of [false, true]) {
          const faces: FoldPreviewCollisionPoseFace[] = [
            {
              id: 'slab-first',
              polygon: permutations[firstOrder],
            },
            {
              id: 'slab-second',
              polygon: permutations[secondOrder],
            },
          ]
          if (reverseFaces) faces.reverse()
          const currentAnalyzer = prepareFoldPreviewNarrowPhase(faces, [])
          assert.ok(currentAnalyzer)
          const result = currentAnalyzer.analyze(
            scenario.pose,
            scenario.thickness,
          )
          const diagnostic = JSON.stringify({
            scenario: scenario.label,
            firstOrder,
            secondOrder,
            reverseFaces,
            result,
          })
          assert.ok(result, diagnostic)
          assert.equal(result.interactions.length, 1, diagnostic)
          assert.equal(
            result.interactions[0]?.geometryClass,
            'indeterminate',
            'a stored projection zero under a world-ULP-dominated margin '
              + 'is not affirmative boundary-contact evidence: ' + diagnostic,
          )
        }
      }
    }
  }
})

test('rigid-basis SAT axes make positive slab overlap permutation invariant', () => {
  const permutations = trianglePermutations([
    { x: -2, z: -2 },
    { x: 2, z: -2 },
    { x: 0, z: 2 },
  ])

  for (const translation of [0, 1e12, 3e12]) {
    const pose = parallelSlabPose({
      rotation: new Matrix4().makeRotationX(Math.PI / 4),
      commonTranslation: new Vector3(
        translation,
        translation,
        translation,
      ),
      centerSeparation: 0.75,
    })
    for (let firstOrder = 0; firstOrder < permutations.length; firstOrder += 1) {
      for (
        let secondOrder = 0;
        secondOrder < permutations.length;
        secondOrder += 1
      ) {
        for (const reverseFaces of [false, true]) {
          const faces: FoldPreviewCollisionPoseFace[] = [
            {
              id: 'slab-first',
              polygon: permutations[firstOrder],
            },
            {
              id: 'slab-second',
              polygon: permutations[secondOrder],
            },
          ]
          if (reverseFaces) faces.reverse()
          const currentAnalyzer = prepareFoldPreviewNarrowPhase(faces, [])
          assert.ok(currentAnalyzer)
          const result = currentAnalyzer.analyze(pose, 1)
          const diagnostic = JSON.stringify({
            translation,
            firstOrder,
            secondOrder,
            reverseFaces,
            result,
          })
          assert.ok(result, diagnostic)
          assert.equal(result.interactions.length, 1, diagnostic)
          assert.equal(
            result.interactions[0]?.geometryClass,
            'penetrating',
            'the congruent slabs have a proved 0.25 mm positive overlap: '
              + diagnostic,
          )
        }
      }
    }
  }
})

for (const thickness of thicknesses) {
  test(
    `positive-volume parallel prisms are translation invariant at thickness ${thickness}`,
    () => {
      assert.ok(positiveOverlap > 0)
      assert.ok(positiveOverlap < thickness)
      assert.ok(triangleArea * positiveOverlap > 0)

      const atOrigin = classify(thickness, 0)
      const translated = classify(thickness, farTranslationX)
      const diagnostic = JSON.stringify({
        thickness,
        positiveOverlap,
        atOrigin,
        translated,
      })

      assert.equal(
        translated.geometryClass,
        atOrigin.geometryClass,
        'a common translation must not change the collision class: '
          + diagnostic,
      )
      assert.ok(
        isPositiveVolumeClass(atOrigin.geometryClass),
        'positive-volume overlap must be penetrating, or indeterminate when '
          + 'it cannot be proved: ' + diagnostic,
      )
      assert.ok(
        isPositiveVolumeClass(translated.geometryClass),
        'positive-volume overlap must not be downgraded to touching or '
          + 'silently omitted: ' + diagnostic,
      )
    },
  )
}

for (const thickness of thicknesses) {
  for (const translation of boundaryTranslations) {
    const cell = `thickness ${thickness} at ${translation.label}`

    test(`parallel slab ${cell}: exact contact is touching`, () => {
      const contact = observeNormalGap(thickness, translation.x, 0)
      const diagnostic = JSON.stringify({ cell, contact })

      assert.equal(contact.storedSignedNormalGap, 0, diagnostic)
      assert.equal(contact.interactionCount, 1, diagnostic)
      assert.equal(contact.geometryClass, 'touching', diagnostic)
    })

    test(
      `parallel slab ${cell}: positive sub-margin overlap is indeterminate`,
      () => {
        const contact = observeNormalGap(thickness, translation.x, 0)
        const localContact = observeNormalGap(thickness, 0, 0)
        const requestedOverlap = localContact.numericalMargin / 16
        const overlap = observeNormalGap(
          thickness,
          translation.x,
          -requestedOverlap,
        )
        const storedPositiveOverlap = -overlap.storedSignedNormalGap
        const diagnostic = JSON.stringify({
          cell,
          requestedOverlap,
          storedPositiveOverlap,
          contact,
          localContact,
          overlap,
        })

        assert.ok(storedPositiveOverlap > 0, diagnostic)
        assert.ok(
          storedPositiveOverlap < localContact.numericalMargin,
          diagnostic,
        )
        assert.ok(
          storedPositiveOverlap < overlap.numericalMargin,
          diagnostic,
        )
        assert.equal(overlap.interactionCount, 1, diagnostic)
        assert.equal(
          overlap.geometryClass,
          'indeterminate',
          'strictly positive overlap below the reported narrow-phase margin '
            + 'must not be called touching: ' + diagnostic,
        )
      },
    )

    test(
      `parallel slab ${cell}: overlap above margin is penetrating`,
      () => {
        const requestedOverlap = Math.min(thickness * 0.99, 0.25)
        const overlap = observeNormalGap(
          thickness,
          translation.x,
          -requestedOverlap,
        )
        const storedPositiveOverlap = -overlap.storedSignedNormalGap
        const diagnostic = JSON.stringify({
          cell,
          requestedOverlap,
          storedPositiveOverlap,
          overlap,
        })

        assert.ok(storedPositiveOverlap > overlap.numericalMargin, diagnostic)
        assert.equal(overlap.interactionCount, 1, diagnostic)
        assert.equal(overlap.geometryClass, 'penetrating', diagnostic)
      },
    )

    test(
      `parallel slab ${cell}: positive sub-margin gap is indeterminate`,
      () => {
        const contact = observeNormalGap(thickness, translation.x, 0)
        const localContact = observeNormalGap(thickness, 0, 0)
        const requestedGap = localContact.numericalMargin / 16
        const gap = observeNormalGap(
          thickness,
          translation.x,
          requestedGap,
        )
        const diagnostic = JSON.stringify({
          cell,
          requestedGap,
          contact,
          localContact,
          gap,
        })

        assert.ok(gap.storedSignedNormalGap > 0, diagnostic)
        assert.ok(
          gap.storedSignedNormalGap < localContact.numericalMargin,
          diagnostic,
        )
        assert.ok(
          gap.storedSignedNormalGap < gap.numericalMargin,
          diagnostic,
        )
        assert.equal(gap.interactionCount, 1, diagnostic)
        assert.equal(
          gap.geometryClass,
          'indeterminate',
          'strictly positive separation below the reported narrow-phase '
            + 'margin must not be called touching: ' + diagnostic,
        )
      },
    )

    test(
      `parallel slab ${cell}: sufficiently positive gap has no interaction`,
      () => {
        const gap = observeNormalGap(thickness, translation.x, 0.25)
        const diagnostic = JSON.stringify({ cell, gap })

        assert.ok(gap.storedSignedNormalGap > gap.numericalMargin, diagnostic)
        assert.equal(gap.broadPhaseCandidates, 0, diagnostic)
        assert.equal(gap.interactionCount, 0, diagnostic)
        assert.equal(gap.geometryClass, null, diagnostic)
      },
    )
  }
}

function classify(thickness: number, translationX: number) {
  // The two congruent centered slabs overlap by exactly positiveOverlap along
  // their shared normal, so their analytic intersection volume is
  // triangleArea * positiveOverlap.
  const observation = observeNormalGap(
    thickness,
    translationX,
    -positiveOverlap,
  )
  assert.equal(observation.broadPhaseCandidates, 1)
  assert.equal(observation.interactionCount, 1)
  const geometryClass = observation.geometryClass
  assert.ok(
    geometryClass,
    'positive-volume overlap must not disappear from narrow-phase output',
  )
  return {
    geometryClass,
    numericalMargin: observation.numericalMargin,
    exactTransversalProofAttempts:
      observation.exactTransversalProofAttempts,
  }
}

function observeNormalGap(
  thickness: number,
  translationX: number,
  requestedSignedNormalGap: number,
) {
  const centerSeparation = thickness + requestedSignedNormalGap
  const storedSignedNormalGap = centerSeparation - thickness
  const result = analyzer.analyze(new Map([
    [
      'parallel-first',
      new Matrix4().makeTranslation(translationX, 0, 0),
    ],
    [
      'parallel-second',
      new Matrix4().makeTranslation(
        translationX,
        centerSeparation,
        0,
      ),
    ],
  ]), thickness)
  assert.ok(result)
  const interaction = result.interactions.find((candidate) =>
    candidate.firstFaceId === 'parallel-first'
    && candidate.secondFaceId === 'parallel-second')
  return {
    requestedSignedNormalGap,
    storedSignedNormalGap,
    geometryClass: interaction?.geometryClass ?? null,
    broadPhaseCandidates: result.broadPhaseCandidates,
    interactionCount: result.interactions.length,
    numericalMargin: result.numericalMargin,
    exactTransversalProofAttempts:
      result.exactTransversalProofWork.attempted,
  }
}

function isPositiveVolumeClass(
  geometryClass: string,
): geometryClass is 'penetrating' | 'indeterminate' {
  return geometryClass === 'penetrating' || geometryClass === 'indeterminate'
}

function trianglePermutations(
  points: readonly [
    Readonly<{ x: number; z: number }>,
    Readonly<{ x: number; z: number }>,
    Readonly<{ x: number; z: number }>,
  ],
) {
  const [first, second, third] = points
  return [
    [first, second, third],
    [second, third, first],
    [third, first, second],
    [first, third, second],
    [third, second, first],
    [second, first, third],
  ] as const
}

function parallelSlabPose({
  rotation,
  commonTranslation,
  centerSeparation,
}: Readonly<{
  rotation: Matrix4
  commonTranslation: Vector3
  centerSeparation: number
}>) {
  const first = new Matrix4()
    .makeTranslation(
      commonTranslation.x,
      commonTranslation.y,
      commonTranslation.z,
    )
    .multiply(rotation)
  const normal = new Vector3(0, 1, 0).transformDirection(first)
  const second = new Matrix4()
    .makeTranslation(
      normal.x * centerSeparation,
      normal.y * centerSeparation,
      normal.z * centerSeparation,
    )
    .multiply(first)
  return new Map([
    ['slab-first', first],
    ['slab-second', second],
  ])
}
