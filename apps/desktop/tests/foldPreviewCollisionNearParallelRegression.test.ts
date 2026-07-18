import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type {
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  provesFoldPreviewBinary64TransversalTriangleIntersection,
} from '../src/lib/foldPreviewExactTriangleIntersection.ts'
import {
  prepareFoldPreviewNarrowPhase,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const shallowRadians = Number.EPSILON * 96
const hingeAxis = new Vector3(1, 0, 1).normalize()
const hingePerpendicular = new Vector3(1, 0, -1).normalize()
const skinnyAxis = new Vector3(
  Math.cos(5 * Math.PI / 3),
  0,
  Math.sin(5 * Math.PI / 3),
)

for (const scale of [1, 400, 1_000_000]) {
  for (const reverseInput of [false, true]) {
    test(
      'exact transversal proof outranks near-parallel floating separation'
        + ` at scale ${scale} with ${reverseInput ? 'reversed' : 'forward'}`
        + ' face input',
      () => {
        const polygon = [
          { x: -scale, z: scale },
          { x: scale, z: -scale },
          { x: scale, z: scale },
        ] as const
        const firstTransform = new Matrix4()
        const secondTransform = shallowRotationThroughTriangle(scale)
        assert.equal(
          provesFoldPreviewBinary64TransversalTriangleIntersection(
            transformedTriangle(polygon, firstTransform),
            transformedTriangle(polygon, secondTransform),
          ),
          true,
          `the stored binary64 triangles must provably cross at scale ${scale}`,
        )

        const canonicalFaces: readonly FoldPreviewCollisionPoseFace[] = [
          { id: 'near-parallel-first', polygon },
          { id: 'near-parallel-second', polygon },
        ]
        const faces = reverseInput
          ? [...canonicalFaces].reverse()
          : canonicalFaces
        const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
        assert.ok(analyzer)
        const result = analyzer.analyze(new Map([
          ['near-parallel-first', firstTransform],
          ['near-parallel-second', secondTransform],
        ]), 0)
        assert.ok(result)
        assert.equal(result.broadPhaseCandidates, 1)
        assert.equal(result.trianglePairTests, 1)
        assert.equal(
          result.interactions[0]?.geometryClass,
          'penetrating',
          [
            `scale=${scale}`,
            `firstInput=${faces[0]?.id}`,
            `exactAttempts=${result.exactTransversalProofWork.attempted}`,
            `interactions=${JSON.stringify(result.interactions)}`,
          ].join(', '),
        )
        assert.equal(result.exactTransversalProofWork.attempted, 1)
      },
    )
  }
}

for (const scale of [1, 400, 1_000_000]) {
  for (const degrees of [90, 179]) {
    for (const reverseInput of [false, true]) {
      test(
        'exact transversal proof outranks an ill-conditioned section-range'
          + ` separation at scale ${scale}, ${degrees} degrees, and`
          + ` ${reverseInput ? 'reversed' : 'forward'} face input`,
        () => {
          const relativeHeight = 1e-10
          const polygon = [
            { x: -scale, z: -relativeHeight * scale },
            { x: scale, z: -relativeHeight * scale },
            { x: 0.4 * scale, z: 2 * relativeHeight * scale },
          ] as const
          const firstTransform = new Matrix4()
          const secondTransform = skinnyRotationThroughTriangle(
            scale,
            degrees,
          )
          assert.equal(
            provesFoldPreviewBinary64TransversalTriangleIntersection(
              transformedTriangle(polygon, firstTransform),
              transformedTriangle(polygon, secondTransform),
            ),
            true,
            `the skinny binary64 triangles must cross at ${scale}:${degrees}`,
          )

          const canonicalFaces: readonly FoldPreviewCollisionPoseFace[] = [
            { id: 'skinny-first', polygon },
            { id: 'skinny-second', polygon },
          ]
          const faces = reverseInput
            ? [...canonicalFaces].reverse()
            : canonicalFaces
          const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
          assert.ok(analyzer)
          const result = analyzer.analyze(new Map([
            ['skinny-first', firstTransform],
            ['skinny-second', secondTransform],
          ]), 0)
          assert.ok(result)
          assert.equal(result.broadPhaseCandidates, 1)
          assert.equal(result.trianglePairTests, 1)
          assert.equal(
            result.interactions[0]?.geometryClass,
            'penetrating',
            [
              `scale=${scale}`,
              `degrees=${degrees}`,
              `firstInput=${faces[0]?.id}`,
              `exactAttempts=${result.exactTransversalProofWork.attempted}`,
              `interactions=${JSON.stringify(result.interactions)}`,
            ].join(', '),
          )
          assert.equal(result.exactTransversalProofWork.attempted, 1)
        },
      )
    }
  }
}

function shallowRotationThroughTriangle(scale: number) {
  const pointOnAxis = hingePerpendicular.clone().multiplyScalar(-1.3 * scale)
  return new Matrix4()
    .makeTranslation(pointOnAxis.x, pointOnAxis.y, pointOnAxis.z)
    .multiply(new Matrix4().makeRotationAxis(hingeAxis, shallowRadians))
    .multiply(new Matrix4().makeTranslation(
      -pointOnAxis.x,
      -pointOnAxis.y,
      -pointOnAxis.z,
    ))
}

function skinnyRotationThroughTriangle(
  scale: number,
  degrees: number,
) {
  const centroidX = 0.4 * scale / 3
  return new Matrix4()
    .makeTranslation(centroidX, 0, 0)
    .multiply(new Matrix4().makeRotationAxis(
      skinnyAxis,
      degrees * Math.PI / 180,
    ))
    .multiply(new Matrix4().makeTranslation(-centroidX, 0, 0))
}

function transformedTriangle(
  polygon: readonly Readonly<{ x: number; z: number }>[],
  transform: Matrix4,
) {
  return polygon.map((point) =>
    new Vector3(point.x, 0, point.z).applyMatrix4(transform))
}
