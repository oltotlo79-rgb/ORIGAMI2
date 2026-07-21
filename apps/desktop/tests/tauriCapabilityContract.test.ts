import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import test from 'node:test'

const capability = JSON.parse(readFileSync('src-tauri/capabilities/default.json', 'utf8'))
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8')
const native = readFileSync('src-tauri/src/lib.rs', 'utf8')

test('the webview capability is an exact minimal close-guard allowlist', () => {
  assert.deepEqual(capability.windows, ['main'])
  assert.deepEqual(capability.permissions, [
    'core:event:allow-listen',
    'core:event:allow-unlisten',
    'core:window:allow-destroy',
  ])
  const serialized = JSON.stringify(capability.permissions)
  for (const forbidden of ['shell', 'http', 'fs', 'process', 'dialog', '*', 'default']) {
    assert.equal(serialized.includes(forbidden), false, `${forbidden} permission must stay absent`)
  }
})

test('native plugins expose dialog only behind Rust commands and no ambient host APIs', () => {
  for (const dependency of ['tauri-plugin-shell', 'tauri-plugin-http', 'tauri-plugin-fs', 'tauri-plugin-process']) {
    assert.doesNotMatch(cargo, new RegExp(`^${dependency}\\s*=`, 'mu'))
  }
  assert.match(cargo, /^tauri-plugin-dialog\s*=\s*"2"$/mu)
  assert.match(native, /\.plugin\(tauri_plugin_dialog::init\(\)\)/u)
})

test('every literal frontend invoke is registered and unknown commands stay rejected', () => {
  const marker = 'tauri::generate_handler!['
  const start = native.indexOf(marker)
  assert.notEqual(start, -1)
  const end = native.indexOf('])', start)
  assert.notEqual(end, -1)
  const registered = new Set(native.slice(start + marker.length, end).match(/\b[a-z][a-z0-9_]*\b/gu) ?? [])
  assert.equal(registered.has('__contract_unknown_command__'), false)
  assert.doesNotMatch(native.slice(start, end), /\*|\.\.\.|["'`]/u)

  const frontend = collectFiles('src', '.ts')
    .concat(collectFiles('src', '.tsx'))
    .flatMap((path) => [...readFileSync(path, 'utf8').matchAll(/\binvoke(?:<[^>]+>)?\(\s*['"]([a-z][a-z0-9_]*)['"]/gu)])
    .map((match) => match[1])
  assert.ok(frontend.length > 0)
  for (const command of frontend) assert.ok(registered.has(command), `${command} must be registered`)
})

function collectFiles(directory: string, suffix: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name)
    return entry.isDirectory() ? collectFiles(path, suffix) : entry.name.endsWith(suffix) ? [path] : []
  })
}
