import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const native = source('../src-tauri/src/lib.rs')
const registry = source('../src-tauri/src/recent_projects.rs')
const client = source('../src/lib/recentProjectsClient.ts')

test('normal open updates recent only after load and project adoption succeed', () => {
  const body = section(native, 'async fn open_project(', '\n#[tauri::command]\nasync fn save_project(')
  assertOrdered(body,
    'blocking_pick_file()',
    'load_project_file(path)',
    'apply_loaded_project_file(',
    'clear_after_normal_completion',
    'remember_current_project',
  )
  assert.match(body, /let Some\(selected\)[\s\S]*?else \{\s*return canceled_file_response/u)
  assert.equal(occurrences(body, 'remember_current_project'), 1)
})

test('save and save-as cover current-path success, dialog success, cancel, and failure without premature MRU mutation', () => {
  const save = section(native, 'async fn save_project(', '\n#[tauri::command]\nasync fn save_project_as(')
  assert.match(save, /save_project_to_path\(&mut project, path\)\?/u)
  assert.match(save, /if let Some\(response\) = saved_to_current_path[\s\S]*?remember_current_project/u)
  assert.match(save, /save_project_with_dialog\(&app, &state\)\?[\s\S]*?if !response\.canceled[\s\S]*?remember_current_project/u)
  assert.equal(occurrences(save, 'remember_current_project'), 2)

  const saveAs = section(native, 'async fn save_project_as(', '\nfn recent_storage(')
  assert.match(saveAs, /save_project_with_dialog\(&app, &state\)\?/u)
  assert.match(saveAs, /if !response\.canceled[\s\S]*?remember_current_project/u)
  assert.equal(occurrences(saveAs, 'remember_current_project'), 1)
})

test('recent selection invalidates identity drift before load and successful open re-enters MRU', () => {
  const openRecent = section(native, 'async fn open_recent_project(', '\n#[tauri::command]')
  assertOrdered(openRecent, '.select(', 'OpenRecentProjectResponse::Invalidated', 'load_project_file(path)', 'apply_loaded_project_file(', 'remember_current_project')
  assert.match(registry, /probe_regular_no_follow\(&entry\.path\)[\s\S]*?== Some\(entry\.identity\)/u)
  assert.match(registry, /next\.remove\(index\)[\s\S]*?persist\(&next, storage\)\?[\s\S]*?self\.entries = next/u)
})

test('CAS conflict retries are bounded and exhaustion cannot publish stale bytes', () => {
  const remember = section(native, 'fn remember_current_project(', '\n#[tauri::command]')
  assert.match(remember, /for _ in 0\.\.2/u)
  assert.match(remember, /RecentProjectRegistry::load\(&storage\)/u)
  assert.match(registry, /let expected = self\.observed[\s\S]*?let actual = read_current_digest[\s\S]*?if actual != expected[\s\S]*?return Err/u)
  assert.match(registry, /stale_prelease_snapshot_cannot_overwrite_a_newer_process_commit/u)
  assert.match(registry, /exhausted_retry_under_live_foreign_lease_keeps_terminal_file_unchanged/u)
})

test('the webview contract remains bounded, pathless, exact, and single-flight', () => {
  assert.match(client, /value\.length > 10/u)
  assert.match(client, /\^r1-\[0-9a-f\]\{32\}\$/u)
  assert.match(client, /nativeInvoke\('open_recent_project', Object\.freeze\(\{ opaqueId: item\.opaque_id \}\)\)/u)
  assert.match(client, /if \(active\) throw new RecentProjectsClientError\('busy'\)/u)
  assert.doesNotMatch(section(client, 'async open(', '\n    },'), /\bpath\b|volume|file_index|identity/u)
})

function assertOrdered(text: string, ...needles: string[]) {
  let previous = -1
  for (const needle of needles) {
    const index = text.indexOf(needle, previous + 1)
    assert.ok(index > previous, `${needle} must follow the preceding lifecycle boundary`)
    previous = index
  }
}
function occurrences(text: string, needle: string) { return text.split(needle).length - 1 }
function section(text: string, start: string, end: string) {
  const from = text.indexOf(start); const to = text.indexOf(end, from + start.length)
  assert.ok(from >= 0 && to > from, `${start} section`); return text.slice(from, to)
}
function source(relative: string) { return readFileSync(new URL(relative, import.meta.url), 'utf8') }
