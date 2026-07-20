import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const client = source('../src/lib/projectFolderClient.ts')
const coreClient = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/project_folder_io.rs')
const nativeRoot = source('../src-tauri/src/lib.rs')

test('the visible workflow uses the dedicated strict client end to end', () => {
  assert.match(
    app,
    /from '\.\/lib\/projectFolderClient'[\s\S]*?runProjectFolderOperation/u,
  )
  assert.doesNotMatch(coreClient, /openProjectFolder|saveProjectFolderAs/u)
  assert.match(app, /await openProjectFolder\(locale\)/u)
  assert.match(app, /await saveProjectFolderAs\(locale\)/u)
  assert.match(app, /if \(response\.canceled\)[\s\S]*?return[\s\S]*?applySnapshot/u)
  assert.match(app, /projectFolderClientErrorMessage\(error, 'ja'\)/u)
  assert.match(app, /projectFolderClientErrorMessage\(error, 'en'\)/u)
  assert.match(app, /展開フォルダーを開く/u)
  assert.match(app, /展開フォルダー保存/u)
  assert.match(app, /既存フォルダーは上書きしません/u)
})

test('IPC accepts only locale and returns an exact pathless snapshot envelope', () => {
  for (const command of ['open_project_folder', 'save_project_folder_as']) {
    assert.match(client, new RegExp(`'${command}'`, 'u'))
    assert.match(nativeRoot, new RegExp(`\\n\\s*${command},`, 'u'))
  }
  assert.match(client, /Object\.freeze\(\{ locale \}\)/u)
  assert.doesNotMatch(
    functionSection(
      client,
      'export function createProjectFolderClient(',
      'export function isNativeProjectFolderAvailable(',
    ),
    /\bpath\b|\bbytes\b|targetName/u,
  )
  assert.match(client, /exactRecord\(value, \['canceled', 'project'\]\)/u)
  assert.match(client, /parsePathlessProjectSnapshot\(record\.project\)/u)
  assert.match(native, /fn redacted_snapshot[\s\S]*?response\.current_path = None/u)
  assert.match(
    native,
    /pub\(super\) async fn open_project_folder\([\s\S]*?locale: String,[\s\S]*?\) -> Result<ProjectFolderFileResponse, String>/u,
  )
  assert.match(
    native,
    /pub\(super\) async fn save_project_folder_as\([\s\S]*?locale: String,[\s\S]*?\) -> Result<ProjectFolderFileResponse, String>/u,
  )
})

test('native errors and filesystem names remain closed categories', () => {
  assert.match(client, /const NATIVE_ERROR_CODES = Object\.freeze/u)
  assert.match(client, /project_folder_target_exists: 'target_exists'/u)
  assert.match(client, /project_folder_project_changed: 'project_changed'/u)
  assert.match(client, /return new ProjectFolderClientError\('invalid_response'\)/u)
  assert.doesNotMatch(
    functionSection(client, 'function mapNativeError(', '\n}'),
    /message|cause|String\(/u,
  )
  assert.match(native, /fn error_string[\s\S]*?error\.code\(\)\.to_owned\(\)/u)
})

function functionSection(text: string, start: string, end: string) {
  const startIndex = text.indexOf(start)
  const endIndex = text.indexOf(end, startIndex + start.length)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `${start} section`)
  return text.slice(startIndex, endIndex + end.length)
}

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
