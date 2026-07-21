import assert from 'node:assert/strict'
import { existsSync, readFileSync, realpathSync } from 'node:fs'
import { dirname, resolve, sep } from 'node:path'
import test from 'node:test'

const workflow = readFileSync('../../.github/workflows/ci.yml', 'utf8')
const rustTest = readFileSync('src-tauri/tests/event_schema_corpus.rs', 'utf8')

test('frontend formal and Rust matrix jobs all execute the event corpus contracts', () => {
  const frontend = jobBody('frontend', 'rust')
  const rust = jobBody('rust', 'windows-bundle')
  assert.match(frontend, /- run: npm test/u)
  assert.match(frontend, /node --test \.\.\/\.\.\/\.github\/tests\/formal-release\.test\.mjs/u)
  assert.match(rust, /matrix:\s*\n\s+os: \[windows-latest, macos-latest\]/u)
  assert.match(rust, /cargo test -p "\$package" --locked --all-targets/u)
  assert.match(rust, /origami2-desktop/u)
  assert.match(rust, /cargo test --workspace --locked --all-targets/u)
})

test('the Rust integration target packages the one canonical fixture at compile time', () => {
  const include = rustTest.match(/include_str!\(\s*"([^"]+)"\s*\)/u)
  assert.ok(include)
  assert.equal(include[1].includes('\\'), false, 'fixture source path must be platform-neutral')
  assert.equal(include[1].startsWith('/'), false)
  assert.equal(/^[A-Za-z]:/u.test(include[1]), false)
  const testDirectory = realpathSync('src-tauri/tests')
  const fixture = resolve(testDirectory, include[1])
  assert.equal(existsSync(fixture), true)
  assert.equal(realpathSync(fixture), realpathSync('tests/fixtures/tauri-event-v1-corpus.json'))
  assert.equal(dirname(fixture).split(sep).at(-1), 'fixtures')
})

test('CI keeps Rust 1.90 formatting and all-target integration discovery explicit', () => {
  const rust = jobBody('rust', 'windows-bundle')
  assert.match(rust, /toolchain: 1\.90\.0/u)
  assert.match(rust, /cargo fmt --all -- --check/u)
  assert.ok((rust.match(/--all-targets/gu) ?? []).length >= 2)
  assert.doesNotMatch(rustTest, /std::env::current_dir|C:\\|\/Users\//u)
})

function jobBody(name: string, next: string): string {
  const start = workflow.indexOf(`  ${name}:`)
  const end = workflow.indexOf(`\n  ${next}:`, start)
  assert.ok(start >= 0 && end > start)
  return workflow.slice(start, end)
}
