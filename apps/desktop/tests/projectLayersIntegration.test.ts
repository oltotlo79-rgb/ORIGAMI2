import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const client = source('../src/lib/coreClient.ts')
const recovery = source('../src/lib/recoveryClient.ts')
const native = source('../src-tauri/src/lib.rs')
const editor = source('../../../crates/ori-core/src/editor.rs')
const formats = source('../../../crates/ori-formats/src/lib.rs')
const ori2 = source('../../../crates/ori-formats/src/ori2.rs')

test('the native snapshot and strict TypeScript contract expose project layers', () => {
  assert.match(client, /project_layers:\s*ProjectLayerDocumentV1/u)
  assert.doesNotMatch(client, /project_layers\?:/u)
  assert.match(native, /project_layers:\s*ProjectLayerDocumentV1/u)
  assert.match(
    native,
    /project_layers:\s*project\.editor\.project_layers\(\)\.clone\(\)/u,
  )
  assert.match(recovery, /'project_layers'/u)
  assert.match(
    recovery,
    /normalizeProjectLayerDocument\(\s*record\.project_layers,\s*creasePattern\?\.edges \?\? \[\],\s*\)/u,
  )
  assert.match(recovery, /project_layers:\s*projectLayers/u)
})

test('document, dirty state, normal open, recovery, and history endpoints retain layers', () => {
  assert.match(
    native,
    /layers:\s*self\.editor\.project_layers\(\)\.clone\(\)/u,
  )
  assert.match(
    native,
    /saved\.layers\s*!=\s*\*self\.editor\.project_layers\(\)/u,
  )
  assert.match(
    native,
    /with_all_document_parts_and_memo\([\s\S]*?document\.layers/u,
  )
  assert.match(
    native,
    /with_all_document_parts_memo_and_history_v1\([\s\S]*?project\.document\.layers\.clone\(\)/u,
  )
  assert.equal(
    native.match(
      /\.layers\s*=\s*(?:editor|undo_endpoint)\.project_layers\(\)\.clone\(\)/gu,
    )?.length,
    2,
  )
})

test('core history and formats bind the versioned layer document without silent loss', () => {
  for (const command of [
    'CreateLayer',
    'RenameLayer',
    'UpdateLayerPresentation',
    'MoveLayer',
    'DeleteLayer',
    'AssignEdgeToLayer',
  ]) {
    assert.match(editor, new RegExp(`Command::${command}`, 'u'))
  }
  assert.match(editor, /project_layers:\s*ProjectLayerDocumentV1/u)
  assert.match(formats, /pub layers:\s*ProjectLayerDocumentV1/u)
  assert.match(ori2, /ORI2_FEATURE_LAYERS_V1/u)
  assert.match(
    ori2,
    /!document\.layers\.is_default\(\)[\s\S]*?required_features\.push\(ORI2_FEATURE_LAYERS_V1/u,
  )
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
