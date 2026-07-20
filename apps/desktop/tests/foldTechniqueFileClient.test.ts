import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FoldTechniqueFileClientError,
  encodeFoldTechniqueDocumentForSaveV1,
  foldTechniqueFileClientErrorCode,
  normalizeFoldTechniqueFileResponseV1,
} from '../src/lib/foldTechniqueFileClient.ts'
import { createInitialFoldTechniqueDocumentV1 } from '../src/lib/foldTechniqueEditor.ts'

test('strictly admits a native validated document and freezes it again', () => {
  const document = createInitialFoldTechniqueDocumentV1()
  const response = normalizeFoldTechniqueFileResponseV1({
    request_id: 7,
    canceled: false,
    document,
  }, 7)

  assert.equal(response.requestId, 7)
  assert.equal(response.canceled, false)
  assert.deepEqual(response.document, document)
  assert.equal(Object.isFrozen(response), true)
  assert.equal(Object.isFrozen(response.document?.techniques), true)
})

test('requires cancellation to carry no document', () => {
  assert.deepEqual(
    normalizeFoldTechniqueFileResponseV1({
      request_id: 1,
      canceled: true,
      document: null,
    }, 1),
    { requestId: 1, canceled: true, document: null },
  )
  assert.throws(
    () => normalizeFoldTechniqueFileResponseV1({
      request_id: 1,
      canceled: true,
      document: createInitialFoldTechniqueDocumentV1(),
    }, 1),
    invalidResponse,
  )
})

test('rejects stale, malformed, extra-field, and invalid-document responses', () => {
  const valid = {
    request_id: 3,
    canceled: false,
    document: createInitialFoldTechniqueDocumentV1(),
  }
  assert.throws(
    () => normalizeFoldTechniqueFileResponseV1(valid, 4),
    invalidResponse,
  )
  assert.throws(
    () => normalizeFoldTechniqueFileResponseV1({ ...valid, path: 'C:\\secret' }, 3),
    invalidResponse,
  )
  assert.throws(
    () => normalizeFoldTechniqueFileResponseV1({
      ...valid,
      document: { ...valid.document, execute: 'script' },
    }, 3),
    invalidResponse,
  )
})

test('does not invoke hostile response getters', () => {
  let getterCalls = 0
  const hostile = Object.defineProperty({}, 'request_id', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 1
    },
  })
  Object.defineProperties(hostile, {
    canceled: { enumerable: true, value: true },
    document: { enumerable: true, value: null },
  })

  assert.throws(
    () => normalizeFoldTechniqueFileResponseV1(hostile, 1),
    invalidResponse,
  )
  assert.equal(getterCalls, 0)
})

test('maps only closed client error categories and never reflects raw text', () => {
  assert.equal(
    foldTechniqueFileClientErrorCode(
      new FoldTechniqueFileClientError('not_regular_file'),
    ),
    'not_regular_file',
  )
  assert.equal(
    foldTechniqueFileClientErrorCode(
      new Error(String.raw`C:\Users\alice\secret.json`),
    ),
    'invalid_response',
  )
})

test('encodes only a strictly admitted canonical document for native save', () => {
  const candidate = JSON.parse(
    JSON.stringify(createInitialFoldTechniqueDocumentV1()),
  )
  candidate.metadata.authors = ['Zulu', 'Alpha']
  candidate.techniques[0].names.reverse()

  const encoded = encodeFoldTechniqueDocumentForSaveV1(candidate)
  const decoded = JSON.parse(encoded)
  assert.deepEqual(decoded.metadata.authors, ['Alpha', 'Zulu'])
  assert.deepEqual(
    decoded.techniques[0].names.map(({ locale }: { locale: string }) => locale),
    ['en', 'ja'],
  )
  assert.equal(encoded.includes('\n'), false)
  assert.throws(
    () => encodeFoldTechniqueDocumentForSaveV1({
      ...candidate,
      execute: 'hostile',
    }),
    (error) => error instanceof FoldTechniqueFileClientError
      && error.code === 'invalid_document',
  )
})

function invalidResponse(error: unknown) {
  return error instanceof FoldTechniqueFileClientError
    && error.code === 'invalid_response'
}
