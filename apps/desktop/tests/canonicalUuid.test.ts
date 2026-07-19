import assert from 'node:assert/strict'
import test from 'node:test'

import { isCanonicalNonNilUuid } from '../src/lib/canonicalUuid.ts'

test('accepts every canonical non-nil UUID version and variant admitted by Rust', () => {
  for (const value of [
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-7000-000000000001',
    '00000000-0000-4000-8000-000000000001',
    'ffffffff-ffff-ffff-ffff-ffffffffffff',
  ]) {
    assert.equal(isCanonicalNonNilUuid(value), true, value)
  }
})

test('rejects nil, noncanonical text, and non-string values', () => {
  for (const value of [
    '00000000-0000-0000-0000-000000000000',
    'abcdef00-0000-4000-8000-000000000001'.toUpperCase(),
    '00000000000040008000000000000001',
    '00000000-0000-4000-8000-00000000001',
    'not-a-uuid',
    null,
    1,
  ]) {
    assert.equal(isCanonicalNonNilUuid(value), false, String(value))
  }
})
