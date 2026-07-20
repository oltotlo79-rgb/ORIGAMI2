import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { appConfirmationText } from '../src/lib/appMessages.ts'

const app = source('../src/App.tsx')
const client = source('../src/lib/foldTechniqueFileClient.ts')
const native = source('../src-tauri/src/fold_technique_file_io.rs')
const nativeRoot = source('../src-tauri/src/lib.rs')
const persistence = source('../src-tauri/src/project_persistence.rs')
const appMessages = source('../src/lib/appMessages.ts')
const requirements = source('../../../docs/requirements-status.md')

test('native dialogs own every path and byte boundary', () => {
  assert.match(native, /blocking_pick_file\(\)/u)
  assert.match(native, /blocking_save_file\(\)/u)
  assert.match(native, /MAX_FOLD_TECHNIQUE_FILE_BYTES/u)
  assert.match(native, /open_regular_file_no_follow\(path\)/u)
  assert.match(native, /metadata_is_plain_regular_file\(&opened_metadata\)/u)
  assert.match(native, /read_fold_technique_file_v1\(&bytes\)/u)
  assert.match(native, /document_json\.len\(\)\s*>\s*MAX_FOLD_TECHNIQUE_FILE_BYTES/u)
  assert.match(
    native,
    /read_fold_technique_file_v1\(document_json\.as_bytes\(\)\)/u,
  )
  assert.match(native, /write_fold_technique_file_v1\(&file\)/u)
  assert.match(native, /persist_export_bytes_to_destination\(destination, bytes\)/u)
  assert.match(
    persistence,
    /O_NOFOLLOW\s*\|\s*libc::O_CLOEXEC\s*\|\s*libc::O_NONBLOCK/u,
  )
  assert.match(persistence, /FILE_FLAG_OPEN_REPARSE_POINT/u)

  const responseTypes = native.slice(
    native.indexOf('pub(super) struct OpenFoldTechniqueFileResponse'),
    native.indexOf('#[tauri::command]'),
  )
  assert.doesNotMatch(responseTypes, /\bpath\b|\bbytes\b/u)
})

test('single-flight ownership extends into detached blocking work', () => {
  assert.match(native, /struct FoldTechniqueFileIoPermit\s*\{\s*busy:\s*Arc<AtomicBool>/u)
  assert.match(
    native,
    /spawn_blocking\(move \|\|\s*\{\s*let _permit = _permit;\s*load_fold_technique_document/u,
  )
  assert.match(
    native,
    /spawn_blocking\(move \|\|\s*\{\s*let _permit = _permit;\s*persist_fold_technique_file/u,
  )
  assert.match(native, /owned_permit_holds_single_flight_until_detached_worker_finishes/u)
})

test('strict TypeScript admission rejects stale or malformed native documents', () => {
  assert.match(client, /invoke<unknown>\('open_fold_technique_file'/u)
  assert.match(client, /invoke<unknown>\('save_fold_technique_file_as'/u)
  assert.match(client, /documentJson,/u)
  assert.doesNotMatch(
    client,
    /invoke<unknown>\('save_fold_technique_file_as',\s*\{[\s\S]*?\bdocument:\s*admitted/u,
  )
  assert.match(
    client,
    /requestId !== expectedRequestId[\s\S]*throw new FoldTechniqueFileClientError\('invalid_response'\)/u,
  )
  assert.match(client, /admitFoldTechniqueDocumentV1\(record\.document\)/u)
  assert.match(client, /exactRecord\(value, \['request_id', 'canceled', 'document'\]\)/u)
  assert.doesNotMatch(client, /readFile|writeFile|FileReader|showOpenFilePicker/u)
})

test('App exposes the complete inert create/import/edit/save-as workflow', () => {
  for (const symbol of [
    'openNewFoldTechniqueEditor',
    'importFoldTechniqueFile',
    'openCurrentFoldTechniqueEditor',
    'confirmFoldTechniqueEditor',
    'saveCurrentFoldTechniqueAs',
  ]) assert.match(app, new RegExp(String.raw`\b${symbol}\b`, 'u'))
  assert.match(app, /複数の説明手順を宣言データとして作成・共有します/u)
  assert.match(app, /折り操作、プロジェクト変更、外部取得は自動実行しません/u)
  assert.match(app, /未対応の物理操作としてファイル内に明示/u)
  assert.match(app, /response\.canceled[\s\S]*内容は変更していません/u)
  assert.match(nativeRoot, /open_fold_technique_file,\s*save_fold_technique_file_as,/u)
})

test('App guards dirty workspaces, raw editor drafts, busy saves, and focus', () => {
  assert.match(
    app,
    /const foldTechniqueBusyRef = useRef\(foldTechniqueBusy\)/u,
  )
  assert.match(
    app,
    /if \(foldTechniqueBusyRef\.current\) return 'core'/u,
  )
  assert.match(
    app,
    /is_dirty: current\.is_dirty[\s\S]*?foldTechniqueWorkspaceRef\.current\?\.dirty === true[\s\S]*?foldTechniqueEditorDirtyRef\.current/u,
  )
  assert.equal(
    app.match(
      /appConfirmationText\(locale, 'replaceFoldTechnique'\)/gu,
    )?.length,
    2,
  )
  assert.match(
    app,
    /appConfirmationText\(locale, 'discardFoldTechniqueDraft'\)/u,
  )
  assert.match(app, /busy=\{foldTechniqueBusy \|\| coreBusy\}/u)
  assert.match(app, /onDirtyChange=\{noteFoldTechniqueEditorDirty\}/u)
  assert.match(
    app,
    /returnFocusTo=\{foldTechniqueEditorOpenerRef\.current\}/u,
  )
  assert.doesNotMatch(app, /foldTechniqueCreateButtonRef/u)
  assert.match(appMessages, /replaceFoldTechnique:\s*\{/u)
  assert.match(appMessages, /discardFoldTechniqueDraft:\s*\{/u)
  assert.match(
    appConfirmationText('ja', 'replaceFoldTechnique'),
    /未保存の折り技法/u,
  )
  assert.match(
    appConfirmationText('en', 'replaceFoldTechnique'),
    /unsaved fold-technique/iu,
  )
  assert.match(
    appConfirmationText('ja', 'discardFoldTechniqueDraft'),
    /破棄して閉じますか/u,
  )
  assert.match(
    appConfirmationText('en', 'discardFoldTechniqueDraft'),
    /discard them and close/iu,
  )
})

test('requirements distinguish file sharing completion from timeline application', () => {
  assert.match(requirements, /\| INS-008 \| 部分実装 \|/u)
  assert.match(requirements, /\| INS-009 \| 実装済み \|/u)
  assert.match(requirements, /timeline手順を生成・適用/u)
  assert.match(requirements, /path\/raw bytesはWebViewへ渡さず/u)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
