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
  assert.notEqual(cycle[0], corpus['current-cycle-pose-progress-v1'])
  assert.equal(Object.isFrozen(cycle[0]), true)
  const stacked: unknown[] = []
  await runtime.listenStackedFoldReadProgressV1((value) => stacked.push(value))
  runtime.deliver(corpus['stacked-fold-read-progress-v1'])
  assert.deepEqual(stacked, [corpus['stacked-fold-read-progress-v1']])
  assert.notEqual(stacked[0], corpus['stacked-fold-read-progress-v1'])
  assert.equal(Object.isFrozen(stacked[0]), true)
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
  // JSON cannot carry signed zero, so the strict wire boundary rejects it.
  runtime.deliver({ ...valid, exploredStateCount: -0, evaluatedTransitionCount: -0 })
  assert.equal(accepted.length, 1)
  assert.equal(Object.isFrozen(accepted[0]), true)
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
  runtime.deliver({ ...valid, status: 'certified', completedWork: 1 })
  runtime.deliver({ ...valid, status: 'running', completedWork: 2 })
  runtime.deliver({ ...valid, completedWork: -0 })
  const inherited = Object.create(valid) as Record<string, unknown>
  inherited.a = 1; inherited.b = 2; inherited.c = 3
  inherited.d = 4; inherited.e = 5; inherited.f = 6
  runtime.deliver(inherited)
  let getterCalls = 0
  runtime.deliver(Object.defineProperty({ ...valid }, 'status', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 'running'
    },
  }))
  assert.doesNotThrow(() => runtime.deliver(new Proxy(valid, {
    ownKeys() {
      throw new Error('hostile event proxy')
    },
  })))
  assert.equal(accepted, 1)
  assert.equal(getterCalls, 0)
})

test('DOM consumers reject foreign ABA ids and regressing progress before rendering', () => {
  const stackedFoldGuard
    = 'progress.requestId !== stackedFoldReadScopeRef.current'
  const currentCycleGuard
    = 'progress.requestId !== currentCycleReadScopeRef.current'
  assert.equal(
    (panelSource.match(
      /progress\.requestId !== stackedFoldReadScopeRef\.current/gu,
    ) ?? []).length,
    1,
  )
  assert.equal(
    (panelSource.match(
      /progress\.requestId !== currentCycleReadScopeRef\.current/gu,
    ) ?? []).length,
    1,
  )
  assert.match(panelSource, /progress\.exploredStateCount < previous\.exploredStateCount/u)
  assert.match(panelSource, /progress\.evaluatedTransitionCount < previous\.evaluatedTransitionCount/u)
  assert.match(panelSource, /progress\.completedWork < previous\.completedWork/u)
  assert.match(panelSource, /previous\.status !== 'running'/u)
  assert.match(panelSource, /if \(tokenRef\.current === token\)/u)
  assert.match(
    panelSource,
    /cyclePosePreview\?\.targetLayerOrder\.slice\(\s*0,\s*MAX_RENDERED_PERSISTED_LAYER_ORDER_PAIRS/u,
  )

  const stackedFoldListener = panelSource.indexOf(
    'listenStackedFoldReadProgressV1((progress) => {',
  )
  const stackedFoldGuardIndex = panelSource.indexOf(
    stackedFoldGuard,
    stackedFoldListener,
  )
  const stackedFoldUpdate = panelSource.indexOf(
    'setPathProgress((previous) => {',
    stackedFoldListener,
  )
  const stackedFoldListenerEnd = panelSource.indexOf(
    '}).then((value) => {',
    stackedFoldListener,
  )
  assert.ok(
    stackedFoldListener >= 0
    && stackedFoldGuardIndex > stackedFoldListener
    && stackedFoldUpdate > stackedFoldGuardIndex
    && stackedFoldListenerEnd > stackedFoldUpdate,
  )

  const currentCycleListener = panelSource.indexOf(
    'listenCurrentCyclePoseProgressV1((progress) => {',
  )
  const currentCycleGuardIndex = panelSource.indexOf(
    currentCycleGuard,
    currentCycleListener,
  )
  const currentCycleUpdate = panelSource.indexOf(
    'setCyclePoseProgress((previous) => {',
    currentCycleListener,
  )
  const currentCycleListenerEnd = panelSource.indexOf(
    '}).then((value) => {',
    currentCycleListener,
  )
  assert.ok(
    currentCycleListener >= 0
    && currentCycleGuardIndex > currentCycleListener
    && currentCycleUpdate > currentCycleGuardIndex
    && currentCycleListenerEnd > currentCycleUpdate,
  )
})

function compileListeners() {
  const names = ['listenCurrentCyclePoseProgressV1', 'listenStackedFoldReadProgressV1']
  const helpers = ['snapshotCoreDataRecord', 'exactCoreDataRecord']
    .map((name) => extractFunction(clientSource, name)).join('\n')
  const functions = names.map((name) => extractFunction(clientSource, name)).join('\n')
  const source = `let callback; const listen = (_name, next) => { callback = next; return Promise.resolve(() => {}) };
${helpers}
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
  const declaration = new RegExp(
    `(?:export\\s+)?function\\s+${name}(?:<[^>{}]*>)?\\s*\\(`,
    'u',
  ).exec(source)
  assert.ok(declaration, `missing function ${name}`)
  const start = declaration.index
  const brace = source.indexOf('{', start)
  let depth = 0
  for (let index = brace; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1
    if (source[index] === '}' && --depth === 0) return source.slice(start, index + 1)
  }
  throw new Error(`unterminated ${name}`)
}
