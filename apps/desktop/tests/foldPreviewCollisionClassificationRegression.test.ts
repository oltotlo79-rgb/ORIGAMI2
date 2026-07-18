import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import { makeFoldPreviewCanonicalAxisRotation } from '../src/lib/foldPreviewCanonicalRotation.ts'
import type { FoldPreviewHingeContactConstraint } from '../src/lib/foldPreviewHingeCollision.ts'
import {
  findFoldPreviewNarrowPhaseInteractions,
  prepareFoldPreviewNarrowPhase,
} from '../src/lib/foldPreviewNarrowCollision.ts'
import {
  summarizeFoldPreviewCollision,
} from '../src/lib/foldPreviewCollisionPresentation.ts'
import {
  collisionBadgeClass,
  collisionDataStatus,
} from '../src/lib/foldPreviewCollisionView.ts'

const origin = { vertexId: 'origin', x: 0, z: 0 } as const
const leftHingeEnd = {
  vertexId: 'left-hinge-end',
  x: 0,
  z: 400,
} as const
const rightHingeEnd = {
  vertexId: 'right-hinge-end',
  x: 400,
  z: 0,
} as const

const reportedVFoldFaces: readonly FoldPreviewCollisionPoseFace[] = [
  {
    id: 'left',
    polygon: [
      origin,
      { vertexId: 'left-boundary', x: -400, z: 0 },
      leftHingeEnd,
    ],
  },
  {
    id: 'middle',
    polygon: [origin, leftHingeEnd, rightHingeEnd],
  },
  {
    id: 'right',
    polygon: [
      origin,
      rightHingeEnd,
      { vertexId: 'right-boundary', x: 0, z: -400 },
    ],
  },
]

const reportedVFoldAdjacencies: readonly FoldPreviewCollisionAdjacency[] = [
  {
    edgeId: 'left-hinge',
    firstFaceId: 'left',
    secondFaceId: 'middle',
  },
  {
    edgeId: 'right-hinge',
    firstFaceId: 'middle',
    secondFaceId: 'right',
  },
]

const squareApex = {
  vertexId: 'square-apex',
  x: 200,
  z: 0,
} as const
const squareLeftHingeEnd = {
  vertexId: 'square-left-hinge-end',
  x: 0,
  z: 200 * Math.sqrt(3),
} as const
const squareRightHingeEnd = {
  vertexId: 'square-right-hinge-end',
  x: 400,
  z: 200 * Math.sqrt(3),
} as const
const reported400MillimetreSheetFaces:
readonly FoldPreviewCollisionPoseFace[] = [
  {
    id: 'square-left',
    polygon: [
      squareApex,
      { vertexId: 'square-bottom-left', x: 0, z: 0 },
      squareLeftHingeEnd,
    ],
  },
  {
    id: 'square-middle',
    polygon: [
      squareApex,
      squareLeftHingeEnd,
      { vertexId: 'square-top-left', x: 0, z: 400 },
      { vertexId: 'square-top-right', x: 400, z: 400 },
      squareRightHingeEnd,
    ],
  },
  {
    id: 'square-right',
    polygon: [
      squareApex,
      squareRightHingeEnd,
      { vertexId: 'square-bottom-right', x: 400, z: 0 },
    ],
  },
]
const reported400MillimetreSheetAdjacencies:
readonly FoldPreviewCollisionAdjacency[] = [
  {
    edgeId: 'square-left-hinge',
    firstFaceId: 'square-left',
    secondFaceId: 'square-middle',
  },
  {
    edgeId: 'square-right-hinge',
    firstFaceId: 'square-middle',
    secondFaceId: 'square-right',
  },
]
const reported400MillimetreSheetConstraints:
readonly FoldPreviewHingeContactConstraint[] = [
  {
    edgeId: 'square-left-hinge',
    leftFaceId: 'square-left',
    rightFaceId: 'square-middle',
    start: squareApex,
    end: squareLeftHingeEnd,
    thicknessRule: 'centered_mid_surface_v1',
  },
  {
    edgeId: 'square-right-hinge',
    leftFaceId: 'square-middle',
    rightFaceId: 'square-right',
    start: squareApex,
    end: squareRightHingeEnd,
    thicknessRule: 'centered_mid_surface_v1',
  },
]

test('reported A: a zero-thickness 10 degree V fold keeps non-adjacent shared-vertex contact out of penetration', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reportedVFoldFaces,
    reportedVFoldAdjacencies,
  )
  assert.ok(analyzer)

  const result = analyzer.analyze(new Map([
    ['left', hingeRotation(leftHingeEnd, 10)],
    ['middle', new Matrix4()],
    ['right', new Matrix4()],
  ]), 0)
  assert.ok(result)

  const outerFaces = result.interactions.find((interaction) =>
    interaction.firstFaceId === 'left'
    && interaction.secondFaceId === 'right')
  assert.ok(outerFaces)
  assert.equal(outerFaces.relation, 'non_adjacent')
  assert.equal(outerFaces.geometryClass, 'touching')
})

test('reported B: fully overlapping zero-thickness faces at 180 degrees remain penetrating', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reportedVFoldFaces,
    reportedVFoldAdjacencies,
  )
  assert.ok(analyzer)

  const result = analyzer.analyze(new Map([
    ['left', hingeRotation(leftHingeEnd, 180)],
    ['middle', new Matrix4()],
    ['right', hingeRotation(rightHingeEnd, 180)],
  ]), 0)
  assert.ok(result)

  const outerFaces = result.interactions.find((interaction) =>
    interaction.firstFaceId === 'left'
    && interaction.secondFaceId === 'right')
  assert.ok(outerFaces)
  assert.equal(outerFaces.relation, 'non_adjacent')
  assert.equal(outerFaces.geometryClass, 'penetrating')
})

test('reported A/B remain correct through the UI summary on a non-origin 400 mm square', () => {
  assert.ok(
    Math.abs(Math.hypot(
      squareLeftHingeEnd.x - squareApex.x,
      squareLeftHingeEnd.z - squareApex.z,
    ) - 400) < 1e-12,
  )
  assert.ok(
    Math.abs(Math.hypot(
      squareRightHingeEnd.x - squareApex.x,
      squareRightHingeEnd.z - squareApex.z,
    ) - 400) < 1e-12,
  )
  const analyzer = prepareFoldPreviewNarrowPhase(
    reported400MillimetreSheetFaces,
    reported400MillimetreSheetAdjacencies,
    reported400MillimetreSheetConstraints,
  )
  assert.ok(analyzer)
  const analyze = (leftDegrees: number, rightDegrees: number) => {
    const result = analyzer.analyze(new Map([
      [
        'square-left',
        hingeRotationAround(
          squareApex,
          squareLeftHingeEnd,
          leftDegrees,
        ),
      ],
      ['square-middle', new Matrix4()],
      [
        'square-right',
        hingeRotationAround(
          squareApex,
          squareRightHingeEnd,
          rightDegrees,
        ),
      ],
    ]), 0)
    assert.ok(result)
    const presentation = summarizeFoldPreviewCollision(result)
    return {
      result,
      presentation,
      summary: {
        kind: 'ready' as const,
        requestKey: `${leftDegrees}:${rightDegrees}`,
        ...presentation,
      },
    }
  }

  const reportedA = analyze(10, 0)
  assert.equal(reportedA.presentation.nonAdjacentPenetrations, 0)
  assert.equal(reportedA.presentation.nonAdjacentContacts, 1)
  assert.equal(reportedA.presentation.hingeModelAllowedContacts, 2)
  assert.equal(reportedA.presentation.indeterminateInteractions, 0)
  assert.equal(collisionDataStatus(reportedA.summary), 'contact')
  assert.equal(collisionBadgeClass(reportedA.summary), 'has-contact')

  const reportedB = analyze(180, 180)
  assert.equal(reportedB.presentation.nonAdjacentPenetrations, 1)
  assert.equal(collisionDataStatus(reportedB.summary), 'penetrating')
  assert.equal(collisionBadgeClass(reportedB.summary), 'has-penetrations')
})

test('the faithful 400 mm V-fold outer pair has an explicit same-angle 3 by 4 table', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reported400MillimetreSheetFaces,
    reported400MillimetreSheetAdjacencies,
    reported400MillimetreSheetConstraints,
  )
  assert.ok(analyzer)
  const expected = new Map<string, Readonly<{
    geometryClass: 'touching' | 'penetrating' | 'indeterminate'
    status: 'contact' | 'penetrating' | 'hinge-unresolved'
    indeterminateInteractions: number
  }>>([
    ['0:10', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0:90', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0:179', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0:180', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['0.1:10', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0.1:90', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0.1:179', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['0.1:180', { geometryClass: 'indeterminate', status: 'hinge-unresolved', indeterminateInteractions: 3 }],
    ['3:10', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['3:90', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['3:179', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['3:180', { geometryClass: 'indeterminate', status: 'hinge-unresolved', indeterminateInteractions: 3 }],
  ])

  for (const thickness of [0, 0.1, 3]) {
    for (const degrees of [10, 90, 179, 180]) {
      const result = analyzer.analyze(new Map([
        [
          'square-left',
          hingeRotationAround(
            squareApex,
            squareLeftHingeEnd,
            degrees,
          ),
        ],
        ['square-middle', new Matrix4()],
        [
          'square-right',
          hingeRotationAround(
            squareApex,
            squareRightHingeEnd,
            degrees,
          ),
        ],
      ]), thickness)
      assert.ok(result)
      const outerFaces = result.interactions.find((interaction) =>
        interaction.firstFaceId === 'square-left'
        && interaction.secondFaceId === 'square-right')
      assert.ok(outerFaces, `missing outer pair for ${thickness}:${degrees}`)
      const wanted = expected.get(`${thickness}:${degrees}`)
      assert.ok(wanted)
      assert.equal(
        outerFaces.geometryClass,
        wanted.geometryClass,
        `${thickness} mm at ${degrees} degrees`,
      )
      const presentation = summarizeFoldPreviewCollision(result)
      assert.equal(
        presentation.indeterminateInteractions,
        wanted.indeterminateInteractions,
      )
      assert.equal(collisionDataStatus({
        kind: 'ready',
        requestKey: `${thickness}:${degrees}`,
        ...presentation,
      }), wanted.status)
    }
  }
})

test('the faithful 400 mm V-fold outer pair has a left-only 3 by 4 table', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reported400MillimetreSheetFaces,
    reported400MillimetreSheetAdjacencies,
    reported400MillimetreSheetConstraints,
  )
  assert.ok(analyzer)
  const expected = new Map<string, Readonly<{
    geometryClass: 'touching' | 'penetrating' | 'indeterminate'
    status: 'contact' | 'penetrating' | 'indeterminate' | 'hinge-unresolved'
    indeterminateInteractions: number
  }>>([
    ['0:10', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0:90', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0:179', { geometryClass: 'touching', status: 'contact', indeterminateInteractions: 0 }],
    ['0:180', { geometryClass: 'indeterminate', status: 'indeterminate', indeterminateInteractions: 1 }],
    ['0.1:10', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['0.1:90', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['0.1:179', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['0.1:180', { geometryClass: 'indeterminate', status: 'hinge-unresolved', indeterminateInteractions: 2 }],
    ['3:10', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['3:90', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['3:179', { geometryClass: 'penetrating', status: 'penetrating', indeterminateInteractions: 0 }],
    ['3:180', { geometryClass: 'indeterminate', status: 'hinge-unresolved', indeterminateInteractions: 2 }],
  ])
  for (const thickness of [0, 0.1, 3]) {
    for (const leftDegrees of [10, 90, 179, 180]) {
      const result = analyzer.analyze(new Map([
        [
          'square-left',
          hingeRotationAround(
            squareApex,
            squareLeftHingeEnd,
            leftDegrees,
          ),
        ],
        ['square-middle', new Matrix4()],
        ['square-right', new Matrix4()],
      ]), thickness)
      assert.ok(result)
      const outerFaces = result.interactions.find((interaction) =>
        interaction.firstFaceId === 'square-left'
        && interaction.secondFaceId === 'square-right')
      assert.ok(
        outerFaces,
        `missing outer pair for ${thickness}:${leftDegrees}:0`,
      )
      const wanted = expected.get(`${thickness}:${leftDegrees}`)
      assert.ok(wanted)
      assert.equal(
        outerFaces.geometryClass,
        wanted.geometryClass,
        `${thickness} mm with left=${leftDegrees}, right=0 degrees`,
      )
      const presentation = summarizeFoldPreviewCollision(result)
      assert.equal(
        presentation.indeterminateInteractions,
        wanted.indeterminateInteractions,
      )
      assert.equal(collisionDataStatus({
        kind: 'ready',
        requestKey: `${thickness}:${leftDegrees}:0`,
        ...presentation,
      }), wanted.status)
    }
  }
})

test('both faithful 400 mm V-fold hinges have an explicit 3 by 4 policy table', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reported400MillimetreSheetFaces,
    reported400MillimetreSheetAdjacencies,
    reported400MillimetreSheetConstraints,
  )
  assert.ok(analyzer)
  const expected = new Map<string, Readonly<{
    geometryClass: 'touching' | 'penetrating' | 'indeterminate'
    decisionGeometry?: 'boundary_contact' | 'corridor_overlap' | 'flat_surface_stack'
    indeterminateReason?: 'layer_offset_unmodeled'
  }>>([
    ['0:10', { geometryClass: 'touching', decisionGeometry: 'boundary_contact' }],
    ['0:90', { geometryClass: 'touching', decisionGeometry: 'boundary_contact' }],
    ['0:179', { geometryClass: 'touching', decisionGeometry: 'boundary_contact' }],
    ['0:180', { geometryClass: 'penetrating', decisionGeometry: 'flat_surface_stack' }],
    ['0.1:10', { geometryClass: 'penetrating', decisionGeometry: 'corridor_overlap' }],
    ['0.1:90', { geometryClass: 'penetrating', decisionGeometry: 'corridor_overlap' }],
    ['0.1:179', { geometryClass: 'penetrating', decisionGeometry: 'corridor_overlap' }],
    ['0.1:180', { geometryClass: 'indeterminate', indeterminateReason: 'layer_offset_unmodeled' }],
    ['3:10', { geometryClass: 'penetrating', decisionGeometry: 'corridor_overlap' }],
    ['3:90', { geometryClass: 'penetrating', decisionGeometry: 'corridor_overlap' }],
    ['3:179', { geometryClass: 'penetrating', decisionGeometry: 'corridor_overlap' }],
    ['3:180', { geometryClass: 'indeterminate', indeterminateReason: 'layer_offset_unmodeled' }],
  ])

  for (const thickness of [0, 0.1, 3]) {
    for (const degrees of [10, 90, 179, 180]) {
      const result = analyzer.analyze(
        new Map([
          [
            'square-left',
            hingeRotationAround(
              squareApex,
              squareLeftHingeEnd,
              degrees,
            ),
          ],
          ['square-middle', new Matrix4()],
          [
            'square-right',
            hingeRotationAround(
              squareApex,
              squareRightHingeEnd,
              degrees,
            ),
          ],
        ]),
        thickness,
      )
      assert.ok(result)
      const interactions = result.interactions.filter((interaction) =>
        interaction.relation === 'hinge_adjacent')
      const wanted = expected.get(`${thickness}:${degrees}`)
      assert.equal(interactions.length, 2)
      assert.ok(wanted)
      for (const interaction of interactions) {
        assert.equal(
          interaction.geometryClass,
          wanted.geometryClass,
          `${thickness} mm at ${degrees} degrees`,
        )
        if (wanted.decisionGeometry) {
          assert.equal(interaction.hingeDecision?.kind, 'allowed_by_hinge_model')
          assert.equal(
            interaction.hingeDecision?.kind === 'allowed_by_hinge_model'
              ? interaction.hingeDecision.geometry
              : null,
            wanted.decisionGeometry,
          )
        } else {
          assert.equal(interaction.hingeDecision?.kind, 'indeterminate')
          assert.equal(
            interaction.hingeDecision?.kind === 'indeterminate'
              ? interaction.hingeDecision.reason
              : null,
            wanted.indeterminateReason,
          )
        }
      }
    }
  }
})

test('a centered-slab corridor exceeding the finite hinge length fails closed', () => {
  const fixture = longHingeFixture(1)
  const result = fixture.analyzer.analyze(
    new Map([
      ['hinge-left', new Matrix4()],
      ['hinge-right', hingeRotation(fixture.end, 179)],
    ]),
    0.1,
  )
  assert.ok(result)
  assert.deepEqual(result.interactions[0]?.hingeDecision, {
    kind: 'indeterminate',
    hingeEdgeIds: ['finite-hinge'],
    reason: 'layer_offset_unmodeled',
  })
})

test('a raw sin(pi) residue is not promoted from near-parallel to coplanar overlap', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reportedVFoldFaces,
    reportedVFoldAdjacencies,
  )
  assert.ok(analyzer)
  const result = analyzer.analyze(new Map([
    ['left', rawHingeRotation(leftHingeEnd, Math.PI)],
    ['middle', new Matrix4()],
    ['right', rawHingeRotation(rightHingeEnd, Math.PI)],
  ]), 0)
  assert.ok(result)
  const outerFaces = result.interactions.find((interaction) =>
    interaction.firstFaceId === 'left'
    && interaction.secondFaceId === 'right')
  assert.equal(outerFaces?.geometryClass, 'indeterminate')
})

test('zero-thickness sync, resumable, and witness accounting agree without requiring a prism witness', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reportedVFoldFaces,
    reportedVFoldAdjacencies,
  )
  assert.ok(analyzer)
  const pose = new Map([
    ['left', hingeRotation(leftHingeEnd, 10)],
    ['middle', new Matrix4()],
    ['right', new Matrix4()],
  ])
  const preparedResult = analyzer.analyze(pose, 0)
  const oneShotResult = findFoldPreviewNarrowPhaseInteractions(
    reportedVFoldFaces,
    pose,
    0,
    reportedVFoldAdjacencies,
  )
  assert.ok(preparedResult && oneShotResult)
  assert.deepEqual(oneShotResult, preparedResult)
  assert.deepEqual(preparedResult.witnessCoverage, {
    scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
    eligiblePairCount: 1,
    attemptedPairCount: 1,
    unavailablePairCount: 1,
    omittedByLimitCount: 0,
    authoritativePairScanComplete: true,
  })
  assert.deepEqual(preparedResult.witnessSamples, [])

  const job = analyzer.createAnalysisJob(pose, 0)
  assert.ok(job)
  let step = job.step(1)
  for (let index = 0; step.kind === 'pending' && index < 16; index += 1) {
    step = job.step(1)
  }
  assert.equal(step.kind, 'complete')
  if (step.kind !== 'complete') assert.fail('zero-thickness job did not complete')
  assert.deepEqual(step.result, preparedResult)
  assert.deepEqual(step.work, {
    totalWorkUnits: 4,
    trianglePairTests: 3,
    witnessDerivations: 1,
  })
})

test('shared topology is evidence for a contact feature, never a blanket collision exemption', () => {
  const shared = { vertexId: 'shared', x: 0, z: 0 } as const
  const overlapping = prepareFoldPreviewNarrowPhase([
    {
      id: 'overlap-a',
      polygon: [
        shared,
        { vertexId: 'a-x', x: 2, z: 0 },
        { vertexId: 'a-z', x: 0, z: 2 },
      ],
    },
    {
      id: 'overlap-b',
      polygon: [
        shared,
        { vertexId: 'b-x', x: 2, z: 0 },
        { vertexId: 'b-z', x: 0, z: 2 },
      ],
    },
  ], [])
  assert.ok(overlapping)
  const overlapResult = overlapping.analyze(new Map([
    ['overlap-a', new Matrix4()],
    ['overlap-b', new Matrix4()],
  ]), 0)
  assert.equal(
    overlapResult?.interactions[0]?.geometryClass,
    'penetrating',
  )

  const edgeStart = { vertexId: 'edge-start', x: 1, z: 0 } as const
  const edgeEnd = { vertexId: 'edge-end', x: 1, z: 1 } as const
  const edgeContact = prepareFoldPreviewNarrowPhase([
    {
      id: 'edge-left',
      polygon: [
        { vertexId: 'left-bottom', x: 0, z: 0 },
        edgeStart,
        edgeEnd,
        { vertexId: 'left-top', x: 0, z: 1 },
      ],
    },
    {
      id: 'edge-right',
      polygon: [
        edgeStart,
        { vertexId: 'right-bottom', x: 2, z: 0 },
        { vertexId: 'right-top', x: 2, z: 1 },
        edgeEnd,
      ],
    },
  ], [])
  assert.ok(edgeContact)
  const edgeResult = edgeContact.analyze(new Map([
    ['edge-left', new Matrix4()],
    ['edge-right', new Matrix4()],
  ]), 0)
  assert.equal(edgeResult?.interactions[0]?.geometryClass, 'touching')
})

test('coplanar positive-area overlap no larger than the margin stays indeterminate', () => {
  const classifyShift = (overlap: number) => {
    const analyzer = prepareFoldPreviewNarrowPhase([
      {
        id: 'skinny-a',
        polygon: [
          { x: 0, z: 0 },
          { x: 1, z: 0 },
          { x: 0, z: 1 },
        ],
      },
      {
        id: 'skinny-b',
        polygon: [
          { x: 1 - overlap, z: 0 },
          { x: 2, z: 0 },
          { x: 1 - overlap, z: 1 },
        ],
      },
    ], [])
    assert.ok(analyzer)
    return analyzer.analyze(new Map([
      ['skinny-a', new Matrix4()],
      ['skinny-b', new Matrix4()],
    ]), 0)
  }
  assert.equal(
    classifyShift(1e-14)?.interactions[0]?.geometryClass,
    'indeterminate',
  )
  assert.equal(
    classifyShift(1e-8)?.interactions[0]?.geometryClass,
    'penetrating',
  )
})

test('parallel surfaces with a positive sub-margin gap are not promoted to coplanar penetration', () => {
  const triangle = [
    { x: 0, z: 0 },
    { x: 1, z: 0 },
    { x: 0, z: 1 },
  ] as const
  const analyzer = prepareFoldPreviewNarrowPhase([
    { id: 'parallel-a', polygon: triangle },
    { id: 'parallel-b', polygon: triangle },
  ], [])
  assert.ok(analyzer)
  const result = analyzer.analyze(new Map([
    ['parallel-a', new Matrix4()],
    ['parallel-b', new Matrix4().makeTranslation(0, 1e-15, 0)],
  ]), 0)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'indeterminate')
})

test('non-coplanar surface crossings supported only by sub-margin plane distances fail closed', () => {
  const triangle = [
    { x: -1, z: -1 },
    { x: 1, z: -1 },
    { x: 0, z: 1 },
  ] as const
  const analyzer = prepareFoldPreviewNarrowPhase([
    { id: 'nearly-flat-a', polygon: triangle },
    { id: 'nearly-flat-b', polygon: triangle },
  ], [])
  assert.ok(analyzer)
  const radians = 1e-8
  const transform = new Matrix4()
    .makeRotationZ(radians)
    .setPosition(0, -(1 - 1e-7) * Math.sin(radians), 0)
  const result = analyzer.analyze(new Map([
    ['nearly-flat-a', new Matrix4()],
    ['nearly-flat-b', transform],
  ]), 0)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'indeterminate')
})

test('a positive line overlap collapsed by point de-duplication stays indeterminate', () => {
  const delta = 5e-14
  const analyzer = prepareFoldPreviewNarrowPhase([
    {
      id: 'short-section-a',
      polygon: [
        { x: 0, z: 0 },
        { x: delta, z: 1 },
        { x: delta, z: -1 },
      ],
    },
    {
      id: 'short-section-b',
      polygon: [
        { x: -1, z: 0 },
        { x: 1, z: -1 },
        { x: 1, z: 1 },
      ],
    },
  ], [])
  assert.ok(analyzer)
  const exactQuarterTurn = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(1, 0, 0),
    Math.PI / 2,
  )
  assert.ok(exactQuarterTurn)
  const result = analyzer.analyze(new Map([
    ['short-section-a', new Matrix4()],
    ['short-section-b', exactQuarterTurn],
  ]), 0)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'indeterminate')
})

test('common-origin projection preserves large translations and fails closed after precision is exhausted', () => {
  const analyzer = prepareFoldPreviewNarrowPhase(
    reportedVFoldFaces,
    reportedVFoldAdjacencies,
  )
  assert.ok(analyzer)
  const classifyAt = (translation: number) => {
    const world = new Matrix4().makeTranslation(
      translation,
      -translation,
      translation,
    )
    const result = analyzer.analyze(new Map([
      [
        'left',
        world.clone().multiply(hingeRotation(leftHingeEnd, 10)),
      ],
      ['middle', world.clone()],
      ['right', world.clone()],
    ]), 0)
    return result?.interactions.find((interaction) =>
      interaction.firstFaceId === 'left'
      && interaction.secondFaceId === 'right')
  }
  assert.equal(classifyAt(1e12)?.geometryClass, 'touching')
  assert.equal(classifyAt(1e15)?.geometryClass, 'indeterminate')
})

function hingeRotation(
  endpoint: Readonly<{ x: number; z: number }>,
  degrees: number,
) {
  const rotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(endpoint.x, 0, endpoint.z).normalize(),
    degrees * Math.PI / 180,
  )
  assert.ok(rotation)
  return rotation
}

function rawHingeRotation(
  endpoint: Readonly<{ x: number; z: number }>,
  radians: number,
) {
  return new Matrix4().makeRotationAxis(
    new Vector3(endpoint.x, 0, endpoint.z).normalize(),
    radians,
  )
}

function hingeRotationAround(
  start: Readonly<{ x: number; z: number }>,
  end: Readonly<{ x: number; z: number }>,
  degrees: number,
) {
  const axisRotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(end.x - start.x, 0, end.z - start.z).normalize(),
    degrees * Math.PI / 180,
  )
  assert.ok(axisRotation)
  return new Matrix4()
    .makeTranslation(start.x, 0, start.z)
    .multiply(axisRotation)
    .multiply(new Matrix4().makeTranslation(-start.x, 0, -start.z))
}

function longHingeFixture(length = 400) {
  const hingeStart = {
    vertexId: 'hinge-start',
    x: 0,
    z: 0,
  } as const
  const hingeEnd = {
    vertexId: 'hinge-end',
    x: 0,
    z: length,
  } as const
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'hinge-left',
      polygon: [
        hingeStart,
        { vertexId: 'hinge-left-bottom', x: -length, z: 0 },
        { vertexId: 'hinge-left-top', x: -length, z: length },
        hingeEnd,
      ],
    },
    {
      id: 'hinge-right',
      polygon: [
        hingeEnd,
        { vertexId: 'hinge-right-top', x: length, z: length },
        { vertexId: 'hinge-right-bottom', x: length, z: 0 },
        hingeStart,
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'finite-hinge',
    firstFaceId: 'hinge-left',
    secondFaceId: 'hinge-right',
  }]
  const constraints: readonly FoldPreviewHingeContactConstraint[] = [{
    edgeId: 'finite-hinge',
    leftFaceId: 'hinge-left',
    rightFaceId: 'hinge-right',
    start: hingeStart,
    end: hingeEnd,
    thicknessRule: 'centered_mid_surface_v1',
  }]
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacencies,
    constraints,
  )
  assert.ok(analyzer)
  return { analyzer, end: hingeEnd }
}
