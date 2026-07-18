import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import { makeFoldPreviewCanonicalAxisRotation } from '../src/lib/foldPreviewCanonicalRotation.ts'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import type {
  FoldPreviewHingeContactConstraint,
} from '../src/lib/foldPreviewHingeCollision.ts'
import {
  prepareFoldPreviewNarrowPhase,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const CUTOFF_ANGLE_DEGREES = 90
const CUTOFF_ANGLE_DELTA_DEGREES = 1e-12

test('finite hinge support is invariant under a common large translation and face input order', () => {
  const fixture = hingeFixture(1)
  const poses = [
    {
      label: 'origin',
      world: new Matrix4(),
    },
    {
      label: 'common x=3e12 translation',
      world: new Matrix4().makeTranslation(3e12, 0, 0),
    },
  ] as const

  for (const reverseFaces of [false, true]) {
    const analyzer = prepareFoldPreviewNarrowPhase(
      reverseFaces ? [...fixture.faces].reverse() : fixture.faces,
      fixture.adjacencies,
      fixture.constraints,
    )
    assert.ok(analyzer)

    for (const { label, world } of poses) {
      const interaction = analyzeHinge(
        analyzer,
        175,
        0.1,
        world,
      )
      assert.equal(
        interaction.geometryClass,
        'penetrating',
        `${label}, reverseFaces=${reverseFaces}`,
      )
      assert.deepEqual(
        interaction.hingeDecision,
        {
          kind: 'indeterminate',
          hingeEdgeIds: ['hinge'],
          reason: 'layer_offset_unmodeled',
        },
        `${label}, reverseFaces=${reverseFaces}`,
      )
    }
  }
})

test('a real hinge-axis mismatch cannot be absorbed by a large world-coordinate margin', () => {
  const fixture = hingeFixture(400)
  const analyzer = prepareFoldPreviewNarrowPhase(
    fixture.faces,
    fixture.adjacencies,
    fixture.constraints,
  )
  assert.ok(analyzer)
  const rotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(0, 0, 1),
    Math.PI / 4,
  )
  assert.ok(rotation)
  const mismatchedRight = new Matrix4()
    .makeTranslation(0, 0.1, 0)
    .multiply(rotation)

  for (const translation of [0, 1e12, 3e12, 1e15]) {
    const world = new Matrix4().makeTranslation(
      translation,
      translation,
      translation,
    )
    const result = analyzer.analyze(new Map([
      ['left', world.clone()],
      ['right', world.clone().multiply(mismatchedRight)],
    ]), 3)
    assert.ok(result)
    assert.equal(result.interactions.length, 1)
    assert.deepEqual(
      result.interactions[0]?.hingeDecision,
      {
        kind: 'indeterminate',
        hingeEdgeIds: ['hinge'],
        reason: 'pose_mismatch',
      },
      `translation=${translation}`,
    )
  }
})

test('zero thickness treats only the exact 180-degree pose as a flat surface stack', () => {
  const fixture = hingeFixture(1)
  const analyzer = prepareFoldPreviewNarrowPhase(
    fixture.faces,
    fixture.adjacencies,
    fixture.constraints,
  )
  assert.ok(analyzer)

  for (const degrees of [179.999998, 179.999999]) {
    const interaction = analyzeHinge(analyzer, degrees, 0)
    assert.equal(
      interaction.geometryClass,
      'touching',
      `${degrees} degrees`,
    )
    assert.deepEqual(
      interaction.hingeDecision,
      {
        kind: 'allowed_by_hinge_model',
        hingeEdgeId: 'hinge',
        geometry: 'boundary_contact',
        thicknessRule: 'centered_mid_surface_v1',
      },
      `${degrees} degrees`,
    )
  }

  const flatFold = analyzeHinge(analyzer, 180, 0)
  assert.equal(flatFold.geometryClass, 'penetrating')
  assert.deepEqual(flatFold.hingeDecision, {
    kind: 'allowed_by_hinge_model',
    hingeEdgeId: 'hinge',
    geometry: 'flat_surface_stack',
    thicknessRule: 'centered_mid_surface_v1',
  })
})

test('finite hinge radius cutoff is closed at equality for every supported thickness', () => {
  for (const thickness of [0.1, 1, 3]) {
    // At 90 degrees cosine(theta / 2) is sqrt(1 / 2). Constructing the
    // hinge length with the identical binary64 expression gives a stable
    // R=L equality cell instead of relying on a rounded inverse cosine.
    const hingeLength = (thickness / 2) / Math.sqrt(1 / 2)
    const fixture = hingeFixture(hingeLength)
    const analyzer = prepareFoldPreviewNarrowPhase(
      fixture.faces,
      fixture.adjacencies,
      fixture.constraints,
    )
    assert.ok(analyzer)

    const below = analyzeHinge(
      analyzer,
      CUTOFF_ANGLE_DEGREES - CUTOFF_ANGLE_DELTA_DEGREES,
      thickness,
    )
    assert.equal(below.geometryClass, 'penetrating')
    assert.deepEqual(
      below.hingeDecision,
      allowedCorridorDecision(),
      `${thickness} mm immediately below R=L`,
    )

    const equal = analyzeHinge(
      analyzer,
      CUTOFF_ANGLE_DEGREES,
      thickness,
    )
    assert.equal(equal.geometryClass, 'penetrating')
    assert.deepEqual(
      equal.hingeDecision,
      allowedCorridorDecision(),
      `${thickness} mm at R=L`,
    )

    const above = analyzeHinge(
      analyzer,
      CUTOFF_ANGLE_DEGREES + CUTOFF_ANGLE_DELTA_DEGREES,
      thickness,
    )
    assert.ok(
      above.geometryClass === 'penetrating'
        || above.geometryClass === 'indeterminate',
      `${thickness} mm immediately above R=L: ${above.geometryClass}`,
    )
    assert.deepEqual(
      above.hingeDecision,
      {
        kind: 'indeterminate',
        hingeEdgeIds: ['hinge'],
        reason: 'layer_offset_unmodeled',
      },
      `${thickness} mm immediately above R=L`,
    )
  }
})

function hingeFixture(length: number): Readonly<{
  faces: readonly FoldPreviewCollisionPoseFace[]
  adjacencies: readonly FoldPreviewCollisionAdjacency[]
  constraints: readonly FoldPreviewHingeContactConstraint[]
}> {
  const start = {
    vertexId: 'hinge-start',
    x: 0,
    z: 0,
  } as const
  const end = {
    vertexId: 'hinge-end',
    x: 0,
    z: length,
  } as const
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'left',
      polygon: [
        start,
        { vertexId: 'left-bottom', x: -length, z: 0 },
        { vertexId: 'left-top', x: -length, z: length },
        end,
      ],
    },
    {
      id: 'right',
      polygon: [
        end,
        { vertexId: 'right-top', x: length, z: length },
        { vertexId: 'right-bottom', x: length, z: 0 },
        start,
      ],
    },
  ]
  return {
    faces,
    adjacencies: [{
      edgeId: 'hinge',
      firstFaceId: 'left',
      secondFaceId: 'right',
    }],
    constraints: [{
      edgeId: 'hinge',
      leftFaceId: 'left',
      rightFaceId: 'right',
      start,
      end,
      thicknessRule: 'centered_mid_surface_v1',
    }],
  }
}

function analyzeHinge(
  analyzer: NonNullable<ReturnType<typeof prepareFoldPreviewNarrowPhase>>,
  degrees: number,
  thickness: number,
  world = new Matrix4(),
) {
  const rotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(0, 0, 1),
    degrees * Math.PI / 180,
  )
  assert.ok(rotation)
  const result = analyzer.analyze(new Map([
    ['left', world.clone()],
    ['right', world.clone().multiply(rotation)],
  ]), thickness)
  assert.ok(result)
  assert.equal(result.interactions.length, 1)
  const interaction = result.interactions[0]
  assert.ok(interaction)
  assert.equal(interaction.relation, 'hinge_adjacent')
  return interaction
}

function allowedCorridorDecision() {
  return {
    kind: 'allowed_by_hinge_model',
    hingeEdgeId: 'hinge',
    geometry: 'corridor_overlap',
    thicknessRule: 'centered_mid_surface_v1',
  } as const
}
