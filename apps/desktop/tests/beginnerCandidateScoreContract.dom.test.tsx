import { describe, expect, it } from 'vitest'

import {
  beginnerReferenceConsensusPairDigestV1,
} from '../src/lib/beginnerCandidateScoreContract.ts'

function hex(bytes: ReadonlyArray<number> | null): string {
  return bytes?.map((byte) => byte.toString(16).padStart(2, '0')).join('')
    ?? ''
}

describe('beginner candidate score contract', () => {
  it('matches the native consensus-pair serialization digest', () => {
    const digest = beginnerReferenceConsensusPairDigestV1(
      '88888888-8888-4888-8888-888888888888',
      Array(32).fill(1),
      '99999999-9999-4999-8999-999999999999',
      Array(32).fill(2),
      {
        componentError: 0,
        normalizedExtentError: 0,
        branchError: 2,
        agreementScore: 80,
        disagrees: false,
      },
    )

    expect(hex(digest)).toBe(
      '4784fa626dc39470a36e27670c6c382c'
        + '1a975271ac45ce4001d5983f8b250d35',
    )
  })

  it('fails closed for noncanonical pair inputs', () => {
    expect(beginnerReferenceConsensusPairDigestV1(
      '88888888-8888-4888-8888-888888888888',
      Array(31).fill(1),
      '99999999-9999-4999-8999-999999999999',
      Array(32).fill(2),
      {
        componentError: 0,
        normalizedExtentError: 0,
        branchError: 2,
        agreementScore: 80,
        disagrees: false,
      },
    )).toBeNull()
  })

  it('accepts the unrestricted non-nil UUID bits used by native IDs', () => {
    expect(beginnerReferenceConsensusPairDigestV1(
      '88888888-8888-7888-0888-888888888888',
      Array(32).fill(1),
      '99999999-9999-0999-f999-999999999999',
      Array(32).fill(2),
      {
        componentError: 0,
        normalizedExtentError: 0,
        branchError: 2,
        agreementScore: 80,
        disagrees: false,
      },
    )).toHaveLength(32)
  })
})
