import { describe, expect, it } from 'vitest'

import { sha256BytesV1 } from '../src/lib/sha256Bytes.ts'

function ascii(value: string): number[] {
  return Array.from(value, (character) => character.charCodeAt(0))
}

function hex(bytes: ReadonlyArray<number> | null): string {
  return bytes?.map((byte) => byte.toString(16).padStart(2, '0')).join('')
    ?? ''
}

describe('dependency-free SHA-256 byte contract', () => {
  it.each([
    [
      '',
      'e3b0c44298fc1c149afbf4c8996fb924'
        + '27ae41e4649b934ca495991b7852b855',
    ],
    [
      'abc',
      'ba7816bf8f01cfea414140de5dae2223'
        + 'b00361a396177a9cb410ff61f20015ad',
    ],
  ])('matches the published SHA-256 vector for %j', (input, expected) => {
    expect(hex(sha256BytesV1(ascii(input)))).toBe(expected)
  })

  it('fails closed for non-byte input', () => {
    expect(sha256BytesV1([0, -1, 256])).toBeNull()
  })
})
