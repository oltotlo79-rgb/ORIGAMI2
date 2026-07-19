import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

function source(relativePath: string): string {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

const nativeLib = source('../src-tauri/src/lib.rs')
const nativeModule = source('../src-tauri/src/numeric_expression.rs')
const nativeCargo = source('../src-tauri/Cargo.toml')
const frontend = source('../src/lib/numericExpressionNative.ts')
const app = source('../src/App.tsx')
const numericCore = source('../../../crates/ori-numeric/src/lib.rs')

test('numeric expression command is registered across the native and frontend boundary', () => {
  assert.match(nativeLib, /mod numeric_expression;/u)
  assert.match(
    nativeLib,
    /use numeric_expression::evaluate_numeric_expression;/u,
  )
  assert.match(
    nativeLib,
    /generate_handler!\[[\s\S]*?evaluate_numeric_expression,/u,
  )
  assert.match(nativeModule, /#\[tauri::command\]/u)
  assert.match(
    nativeModule,
    /async fn evaluate_numeric_expression\(/u,
  )
  assert.match(
    frontend,
    /nativeInvoke\(\s*'evaluate_numeric_expression',\s*\{/u,
  )
  assert.match(
    nativeCargo,
    /ori-numeric = \{ path = "\.\.\/\.\.\/\.\.\/crates\/ori-numeric" \}/u,
  )
})

test('the IPC slice stays disconnected from editor UI and persisted project state', () => {
  assert.doesNotMatch(app, /numericExpressionNative/u)
  assert.match(
    nativeModule,
    /The exact `BigRational` endpoints stay inside native memory/u,
  )
  assert.match(
    frontend,
    /response\.source !== source[\s\S]*?stale_response/u,
  )
})

test('frontend IPC ceilings stay pinned to the native public hard ceilings', () => {
  for (const [name, value] of [
    ['MIN_PRECISION_BITS', '32'],
    ['MAX_PRECISION_BITS', '512'],
    ['HARD_MAX_SOURCE_BYTES', '4_096'],
    ['HARD_MAX_OPERATIONS', '20_000'],
  ] as const) {
    assert.match(
      numericCore,
      new RegExp(`pub const ${name}: usize = ${value};`, 'u'),
    )
  }
  assert.match(
    frontend,
    /MIN_NUMERIC_EXPRESSION_PRECISION_BITS = 32/u,
  )
  assert.match(
    frontend,
    /MAX_NUMERIC_EXPRESSION_PRECISION_BITS = 512/u,
  )
  assert.match(
    frontend,
    /MAX_NUMERIC_EXPRESSION_SOURCE_BYTES = 4_096/u,
  )
  assert.match(
    frontend,
    /MAX_NUMERIC_EXPRESSION_OPERATIONS = 20_000/u,
  )
})
