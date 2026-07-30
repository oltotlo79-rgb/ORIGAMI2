import { beforeEach, describe, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import { getBeginnerSymmetricParameterEstimate } from '../src/lib/coreClient'

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const REVISION = 7

function completeInsectEstimate() {
  return {
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: REVISION,
    estimate: {
      protrusion_count: 10,
      scale_percent: 25,
      spacing_percent: 50,
    },
    candidates: [
      {
        id: 0,
        scale_percent: 25,
        spacing_percent: 50,
        approximation_score: 100,
        complexity_score: 125,
        required_protrusion_count: 10,
      },
      {
        id: 1,
        scale_percent: 20,
        spacing_percent: 40,
        approximation_score: 70,
        complexity_score: 124,
        required_protrusion_count: 10,
      },
      {
        id: 2,
        scale_percent: 30,
        spacing_percent: 60,
        approximation_score: 70,
        complexity_score: 126,
        required_protrusion_count: 10,
      },
    ],
  }
}

async function readEstimate() {
  return getBeginnerSymmetricParameterEstimate(
    PROJECT_ID,
    REVISION,
    INSTANCE_ID,
  )
}

beforeEach(() => {
  nativeInvoke.mockReset()
})

describe('beginner symmetric estimate strict client', () => {
  it('accepts all three complete-insect u8 complexity candidates', async () => {
    nativeInvoke.mockResolvedValue(completeInsectEstimate())

    const response = await readEstimate()

    expect(response.estimate.protrusion_count).toBe(10)
    expect(response.candidates.map((candidate) => candidate.id)).toEqual([
      0,
      1,
      2,
    ])
    expect(response.candidates.map(
      (candidate) => candidate.complexity_score,
    )).toEqual([125, 124, 126])
    expect(nativeInvoke).toHaveBeenCalledWith(
      'get_beginner_symmetric_parameter_estimate',
      {
        expectedProjectInstanceId: INSTANCE_ID,
        expectedProjectId: PROJECT_ID,
        expectedRevision: REVISION,
      },
    )
  })

  it('accepts the inclusive u8 maximum and rejects non-u8 complexity', async () => {
    const maximum = completeInsectEstimate()
    maximum.candidates.forEach((candidate) => {
      candidate.complexity_score = 255
    })
    nativeInvoke.mockResolvedValueOnce(maximum)
    await expect(readEstimate()).resolves.toBeTruthy()

    for (const invalid of [-1, 1.5, 256]) {
      const response = completeInsectEstimate()
      response.candidates[1]!.complexity_score = invalid
      nativeInvoke.mockResolvedValueOnce(response)
      await expect(readEstimate()).rejects.toThrow(
        'invalid symmetric parameter candidates',
      )
    }
  })

  it('keeps the existing count, cardinality, order, and score bounds closed', async () => {
    const invalidResponses = [
      (() => {
        const response = completeInsectEstimate()
        response.candidates.pop()
        return response
      })(),
      (() => {
        const response = completeInsectEstimate()
        response.candidates[1]!.id = 2
        return response
      })(),
      (() => {
        const response = completeInsectEstimate()
        response.candidates[1]!.required_protrusion_count = 5
        return response
      })(),
      (() => {
        const response = completeInsectEstimate()
        response.candidates[1]!.scale_percent = 46
        return response
      })(),
      (() => {
        const response = completeInsectEstimate()
        response.candidates[1]!.spacing_percent = 19
        return response
      })(),
      (() => {
        const response = completeInsectEstimate()
        response.candidates[1]!.approximation_score = 101
        return response
      })(),
    ]

    for (const response of invalidResponses) {
      nativeInvoke.mockResolvedValueOnce(response)
      await expect(readEstimate()).rejects.toThrow(
        /invalid symmetric parameter (?:estimate|candidates)/u,
      )
    }
  })
})
