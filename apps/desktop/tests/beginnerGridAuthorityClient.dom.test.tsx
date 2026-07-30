import { beforeEach, describe, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import {
  applyBeginnerParameterGridCandidate,
  type BeginnerGridEvaluationResponse,
} from '../src/lib/coreClient'

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const GENERATION_ID = '33333333-3333-4333-8333-333333333333'
const AUTHORITY_TOKEN = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'

function authorizedGrid(
  authorityToken = AUTHORITY_TOKEN,
  reason:
    | 'global_flat_foldability_proven'
    | 'native_fold_path_certified' =
      'global_flat_foldability_proven',
) {
  const candidate = {
    point: { id: 0 },
    assessment: {
      proof_scope: 'sufficient',
      reason,
      apply_allowed: true,
      expected_candidate_edge_id:
        '77777777-7777-4777-8777-777777777777',
    },
    contour_witness: {
      topology_authority_hash: Array(32).fill(0),
    },
  }
  const grid = {
    request_generation_id: GENERATION_ID,
    authority_token: authorityToken,
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: 7,
    grid_hash: Array(32).fill(0),
    candidates: [candidate],
  }
  return { grid, candidate }
}

function applyGrid(
  grid: BeginnerGridEvaluationResponse,
  candidate: BeginnerGridEvaluationResponse['candidates'][number],
) {
  return applyBeginnerParameterGridCandidate(
    PROJECT_ID,
    7,
    INSTANCE_ID,
    grid,
    {} as never,
    candidate,
  )
}

describe('beginner grid registry authority client', () => {
  beforeEach(() => nativeInvoke.mockReset())

  it('sends the registry-issued token separately from the generation ID', async () => {
    nativeInvoke.mockResolvedValue({})
    const { grid, candidate } = authorizedGrid()

    await applyGrid(
      grid as never,
      candidate as never,
    )

    expect(nativeInvoke).toHaveBeenCalledWith(
      'apply_beginner_parameter_grid_candidate',
      expect.objectContaining({
        requestGenerationId: GENERATION_ID,
        authorityToken: AUTHORITY_TOKEN,
      }),
    )
  })

  it.each([
    'global_flat_foldability_proven',
    'native_fold_path_certified',
  ] as const)(
    'treats %s as diagnostic while sufficient/apply_allowed is authoritative',
    async (reason) => {
      nativeInvoke.mockResolvedValue({})
      const { grid, candidate } = authorizedGrid(AUTHORITY_TOKEN, reason)

      await applyGrid(grid as never, candidate as never)

      expect(nativeInvoke).toHaveBeenCalledOnce()
    },
  )

  it.each([
    ['missing', undefined],
    ['nil', '00000000-0000-0000-0000-000000000000'],
    ['uppercase', AUTHORITY_TOKEN.toUpperCase()],
    ['non-UUID', 'stale-authority'],
  ])('rejects a %s authority token before IPC', async (_label, token) => {
    const { grid, candidate } = authorizedGrid()
    const malformed = { ...grid, authority_token: token }

    await expect(applyGrid(
      malformed as never,
      candidate as never,
    )).rejects.toThrow('grid candidate lacks a live sufficient proof')
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

})
