import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import ts from 'typescript'

const clientSource = readFileSync('src/lib/coreClient.ts', 'utf8')
const panelSource = readFileSync('src/components/StackedFoldPanel.tsx', 'utf8')
const corpus = JSON.parse(readFileSync('tests/fixtures/tauri-event-v1-corpus.json', 'utf8'))

test('canonical Rust corpus roundtrips through both TypeScript strict parsers', async () => {
  const runtime = compileListeners()
  const cycle: unknown[] = []
  await runtime.listenCurrentCyclePoseProgressV1((value) => cycle.push(value))
  runtime.deliver(corpus['current-cycle-pose-progress-v1'])
  assert.deepEqual(cycle, [corpus['current-cycle-pose-progress-v1']])
  const stacked: unknown[] = []
  await runtime.listenStackedFoldReadProgressV1((value) => stacked.push(value))
  runtime.deliver(corpus['stacked-fold-read-progress-v1'])
  assert.deepEqual(stacked, [corpus['stacked-fold-read-progress-v1']])
})

test('strict event parsers reject unknown oversized and non-finite payloads', async () => {
  const runtime = compileListeners()
  const accepted: unknown[] = []
  await runtime.listenStackedFoldReadProgressV1((value: unknown) => accepted.push(value))
  const valid = {
    version: 1, requestId: 'request-a', exploredStateCount: 0,
    evaluatedTransitionCount: 0, stateLimit: 32, transitionLimit: 64,
    authorizesProjectMutation: false,
  }
  runtime.deliver(valid)
  runtime.deliver({ ...valid, unknown: true })
  runtime.deliver({ ...valid, requestId: 'x'.repeat(129) })
  for (const hostile of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
    runtime.deliver({ ...valid, exploredStateCount: hostile })
    runtime.deliver({ ...valid, evaluatedTransitionCount: hostile })
  }
  // Signed zero is observationally canonical zero and cannot bypass monotonic DOM checks.
  runtime.deliver({ ...valid, exploredStateCount: -0, evaluatedTransitionCount: -0 })
  assert.equal(accepted.length, 2)
  assert.equal(Object.is((accepted[1] as typeof valid).exploredStateCount, -0), true)
})

test('duplicate-like replacement and prototype-carried fields cannot form an accepted record', async () => {
  const runtime = compileListeners()
  let accepted = 0
  await runtime.listenCurrentCyclePoseProgressV1(() => { accepted += 1 })
  const valid = {
    version: 1, requestId: 'request-a', status: 'running', completedWork: 0,
    totalWork: 2, authorizesProjectMutation: false,
  }
  runtime.deliver(valid)
  runtime.deliver({ ...valid, version: 2 })
  runtime.deliver({ ...valid, status: 'running', extra: 'duplicate replacement' })
  const inherited = Object.create(valid) as Record<string, unknown>
  inherited.a = 1; inherited.b = 2; inherited.c = 3
  inherited.d = 4; inherited.e = 5; inherited.f = 6
  runtime.deliver(inherited)
  assert.equal(accepted, 1)
})

test('DOM consumers reject foreign ABA ids and regressing progress before rendering', () => {
  assert.equal((panelSource.match(/progress\.requestId !== progressRequestRef\.current/gu) ?? []).length, 2)
  assert.match(panelSource, /progress\.exploredStateCount < previous\.exploredStateCount/u)
  assert.match(panelSource, /progress\.evaluatedTransitionCount < previous\.evaluatedTransitionCount/u)
  const foreignIdCheck = panelSource.indexOf('progress.requestId !== progressRequestRef.current')
  const firstRender = panelSource.indexOf('setPathProgress', foreignIdCheck)
  assert.ok(foreignIdCheck >= 0 && firstRender > foreignIdCheck)
})

function compileListeners() {
  const names = ['listenCurrentCyclePoseProgressV1', 'listenStackedFoldReadProgressV1']
  const functions = names.map((name) => extractFunction(clientSource, name)).join('\n')
  const source = `let callback; const listen = (_name, next) => { callback = next; return Promise.resolve(() => {}) };
${functions}
export { ${names.join(', ')} };
export const deliver = (payload) => callback({ payload });`
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText
  const module = { exports: {} as Record<string, unknown> }
  Function('exports', 'module', output)(module.exports, module)
  return module.exports as {
    deliver(payload: unknown): void
    listenCurrentCyclePoseProgressV1(callback: (value: unknown) => void): Promise<() => void>
    listenStackedFoldReadProgressV1(callback: (value: unknown) => void): Promise<() => void>
  }
}

function extractFunction(source: string, name: string): string {
  const start = source.indexOf(`export function ${name}(`)
  assert.notEqual(start, -1)
  const brace = source.indexOf('{', start)
  let depth = 0
  for (let index = brace; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1
    if (source[index] === '}' && --depth === 0) return source.slice(start, index + 1)
  }
  throw new Error(`unterminated ${name}`)
}
