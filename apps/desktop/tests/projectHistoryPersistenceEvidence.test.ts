import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const formats = read('../../../crates/ori-formats/src/lib.rs')
const ori2 = read('../../../crates/ori-formats/src/ori2.rs')
const folder = read('../../../crates/ori-formats/src/project_folder.rs')
const history = read('../../../crates/ori-core/src/editor/history_persistence.rs')
const recovery = read('../src-tauri/src/recovery.rs')
const native = read('../src-tauri/src/lib.rs')

test('PRJ and HIS use one authenticated document and history authority in every store', () => {
  assert.match(formats, /pub struct ProjectDocument/u)
  assert.match(ori2, /EditorHistoryV1/u)
  assert.match(folder, /EditorHistoryV1/u)
  assert.match(history, /project_id/u)
  assert.match(ori2, /project_sha256/u)
  assert.match(recovery, /history/u)
  assert.match(native, /get_history_entry_limit/u)
})

test('IO-003 folder preview and entries remain bounded, hashed, and read-only', () => {
  assert.match(folder, /generate_safe_preview_svg/u)
  assert.match(folder, /data-origami-preview=\\"read-only\\"/u)
  assert.match(folder, /sha256/u)
  assert.match(folder, /MAX_PREVIEW_VERTICES/u)
  assert.match(folder, /MAX_PREVIEW_EDGES/u)
  assert.match(folder, /symlink|reparse/u)
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
