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
const coreClient = source('../src/lib/coreClient.ts')
const app = source('../src/App.tsx')
const numericCore = source('../../../crates/ori-numeric/src/lib.rs')

test('numeric expression command is registered across the native and frontend boundary', () => {
  assert.match(nativeLib, /mod numeric_expression;/u)
  assert.match(
    nativeLib,
    /use numeric_expression::\{[\s\S]*?evaluate_numeric_expression,[\s\S]*?\};/u,
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

test('the first user-input slice connects both new-paper dimensions without trusting WebView values', () => {
  assert.match(app, /evaluatePositiveMillimetreExpression\(widthExpression\)/u)
  assert.match(app, /evaluatePositiveMillimetreExpression\(heightExpression\)/u)
  assert.match(app, /widthExpression,\s*heightExpression,/u)
  assert.match(
    app,
    /newProject\(\s*current\.project_instance_id,\s*current\.project_id,\s*current\.revision,/u,
  )
  assert.match(
    coreClient,
    /expectedProjectInstanceId[\s\S]*?expectedProjectInstanceId,/u,
  )
  assert.match(
    nativeModule,
    /The exact `BigRational` endpoints stay inside native memory/u,
  )
  assert.match(
    frontend,
    /response\.source !== source[\s\S]*?stale_response/u,
  )
  assert.match(
    nativeLib,
    /evaluate_positive_millimetre_pair_in_worker\(\s*width_expression\.clone\(\),\s*height_expression\.clone\(\),?\s*\)/u,
  )
  assert.match(nativeLib, /async fn new_project\(/u)
  assert.match(
    nativeLib,
    /evaluate_positive_millimetre_pair_in_worker\([\s\S]*?\)\s*\.await/u,
  )
  assert.match(
    nativeLib,
    /async fn new_project\([\s\S]*?expected_project_instance_id:\s*ProjectId/u,
  )
  assert.match(
    nativeLib,
    /replace_with_new_project\(\s*&mut project,\s*expected_project_instance_id,/u,
  )
  assert.match(
    nativeModule,
    /evaluate_positive_millimetre_pair_in_worker[\s\S]*?try_acquire\(\)[\s\S]*?spawn_blocking\(move \|\|[\s\S]*?run_guarded_worker\(permit/u,
  )
  assert.match(
    nativeLib,
    /validate_loaded_numeric_expression_bindings\(&document\)\?/u,
  )
  assert.match(
    nativeLib,
    /width_mm\.to_bits\(\) != binding\.adopted_width_mm\.to_bits\(\)[\s\S]*?height_mm\.to_bits\(\) != binding\.adopted_height_mm\.to_bits\(\)/u,
  )
  assert.match(app, /<CreationDimensionExpressionSummary/u)
  const submitStart = app.indexOf('async function submitNewProject')
  const submitEnd = app.indexOf('async function runFileOperation')
  assert.ok(submitStart >= 0 && submitEnd > submitStart)
  const newProjectSubmit = app.slice(submitStart, submitEnd)
  assert.doesNotMatch(newProjectSubmit, /String\(error\)/u)
  assert.match(
    newProjectSubmit,
    /newProjectExpressionErrorMessage\(error\)[\s\S]*?新しいプロジェクトを作成できませんでした/u,
  )
  assert.match(
    nativeLib,
    /spawn_blocking\(move \|\| load_project_file\(path\)\)[\s\S]*?PROJECT_OPEN_TASK_FAILED_MESSAGE/u,
  )
  assert.match(
    nativeLib,
    /map_loaded_numeric_expression_error[\s\S]*?PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE/u,
  )
  const nativeNewProjectStart = nativeLib.indexOf('async fn new_project(')
  const nativeNewProjectEnd = nativeLib.indexOf('async fn validate_project')
  assert.ok(
    nativeNewProjectStart >= 0 && nativeNewProjectEnd > nativeNewProjectStart,
  )
  assert.doesNotMatch(
    nativeLib.slice(nativeNewProjectStart, nativeNewProjectEnd),
    /\bwidth_mm:\s*f64\b|\bheight_mm:\s*f64\b/u,
  )
  assert.match(app, /numericExpressionNativeErrorCategory\(error\)/u)
  assert.doesNotMatch(newProjectSubmit, /instanceof NumericExpressionNativeError/u)
  assert.match(
    frontend,
    /value\.length > MAX_NUMERIC_EXPRESSION_SOURCE_BYTES[\s\S]*?utf8ByteLength\(value\)/u,
  )
  assert.doesNotMatch(frontend, /userInputEvaluationTail/u)
  assert.match(
    frontend,
    /latestPendingUserInputEvaluation\?\.reject\([\s\S]*?'stale_response'/u,
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
