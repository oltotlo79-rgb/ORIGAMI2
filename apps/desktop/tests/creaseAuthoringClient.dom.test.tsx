import { beforeEach, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import {
  addConnectedVertex,
  addEdge,
  addRayToFirstTarget,
} from '../src/lib/coreClient.ts'

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const TARGET_LAYER = '33333333-3333-4333-8333-333333333333'
const START = '44444444-4444-4444-8444-444444444444'
const END = '55555555-5555-4555-8555-555555555555'

beforeEach(() => {
  nativeInvoke.mockReset()
  nativeInvoke.mockResolvedValue({})
})

it('passes the exact crease authoring layer through every edge-authoring invoke', async () => {
  await addEdge(
    PROJECT_ID,
    7,
    INSTANCE_ID,
    START,
    END,
    'mountain',
    TARGET_LAYER,
  )
  await addRayToFirstTarget(
    PROJECT_ID,
    8,
    INSTANCE_ID,
    START,
    12_345_678,
    'valley',
    TARGET_LAYER,
  )
  await addConnectedVertex(
    PROJECT_ID,
    9,
    INSTANCE_ID,
    START,
    '10',
    '45',
    'auxiliary',
    TARGET_LAYER,
  )

  expect(nativeInvoke.mock.calls).toEqual([
    ['add_edge', {
      expectedProjectInstanceId: INSTANCE_ID,
      expectedProjectId: PROJECT_ID,
      expectedRevision: 7,
      start: START,
      end: END,
      kind: 'mountain',
      targetLayer: TARGET_LAYER,
    }],
    ['add_ray_to_first_target', {
      expectedProjectInstanceId: INSTANCE_ID,
      expectedProjectId: PROJECT_ID,
      expectedRevision: 8,
      start: START,
      angleMicrodegrees: 12_345_678,
      kind: 'valley',
      targetLayer: TARGET_LAYER,
    }],
    ['add_connected_vertex', {
      expectedProjectInstanceId: INSTANCE_ID,
      expectedProjectId: PROJECT_ID,
      expectedRevision: 9,
      start: START,
      lengthExpression: '10',
      angleDegreesExpression: '45',
      kind: 'auxiliary',
      targetLayer: TARGET_LAYER,
    }],
  ])
})
