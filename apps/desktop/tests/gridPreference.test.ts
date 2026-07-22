import assert from 'node:assert/strict'
import test from 'node:test'

import {
  loadGridDivisionPreference,
  loadGridDivisionPreferenceFromHost,
  saveGridDivisionPreference,
  saveGridDivisionPreferenceToHost,
  updateGridPreferenceInput,
} from '../src/lib/gridPreference.ts'

function storage(initial: string | null = null) {
  let value = initial
  return {
    getItem: () => value,
    setItem: (_key: string, next: string) => { value = next },
    value: () => value,
  }
}

test('grid division preference round-trips one strict versioned terminal record', () => {
  const target = storage()
  assert.equal(saveGridDivisionPreference(target, { divisions: 3, diagonals: true }), true)
  assert.equal(target.value(), '{"version":1,"divisions":3,"diagonals":true}')
  assert.deepEqual(loadGridDivisionPreference(target), { divisions: 3, diagonals: true })
  assert.equal(saveGridDivisionPreference(target, { divisions: null, diagonals: false }), true)
  assert.deepEqual(loadGridDivisionPreference(target), { divisions: null, diagonals: false })
  for (const divisions of [2, 63]) {
    assert.equal(saveGridDivisionPreference(target, { divisions, diagonals: false }), true)
    assert.deepEqual(loadGridDivisionPreference(target), { divisions, diagonals: false })
  }
})

test('grid division preference fails closed on invalid, future, and hostile storage', () => {
  for (const raw of [
    '', '{}', '{"version":2,"divisions":3,"diagonals":false}',
    '{"version":1,"divisions":1,"diagonals":false}',
    '{"version":1,"divisions":64,"diagonals":false}',
    '{"version":1,"divisions":2.5,"diagonals":false}',
    '{"version":1,"divisions":3,"diagonals":"yes"}',
    '{"version":1,"divisions":null,"diagonals":true}',
    '{"version":1,"divisions":3,"diagonals":false,"future":true}', 'x'.repeat(97),
  ]) assert.equal(loadGridDivisionPreference(storage(raw)), null)
  assert.equal(loadGridDivisionPreference({ getItem: () => { throw new Error('private') } }), null)
  assert.equal(saveGridDivisionPreference(storage(), { divisions: 64, diagonals: false }), false)
  assert.equal(saveGridDivisionPreference(storage(), { divisions: null, diagonals: true }), false)
  assert.equal(saveGridDivisionPreference(
    { setItem: () => { throw new Error('private') } },
    { divisions: 8, diagonals: true },
  ), false)
  const hostileHost = Object.defineProperty({}, 'localStorage', {
    get() { throw new Error('private') },
  })
  assert.equal(loadGridDivisionPreferenceFromHost(hostileHost as { localStorage: Storage }), null)
  assert.equal(saveGridDivisionPreferenceToHost(
    hostileHost as { localStorage: Storage },
    { divisions: 8, diagonals: false },
  ), false)
  const hostilePreference = new Proxy({}, {
    getPrototypeOf() { throw new Error('private') },
  })
  assert.equal(saveGridDivisionPreference(
    storage(),
    hostilePreference as { divisions: number | null; diagonals: boolean },
  ), false)
})

test('clearing an N division disables diagonals before persistence', () => {
  assert.deepEqual(updateGridPreferenceInput('3', false), { input: '3', diagonals: false })
  assert.deepEqual(updateGridPreferenceInput('3', true), { input: '3', diagonals: true })
  assert.deepEqual(updateGridPreferenceInput('', true), { input: '', diagonals: false })
  assert.equal(updateGridPreferenceInput('123', true), null)
})
