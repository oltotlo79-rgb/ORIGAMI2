import assert from 'node:assert/strict'
import test from 'node:test'

import {
  adoptFiniteAdjacentInterval,
  adoptPositiveAdjacentInterval,
  createNumericExpressionNativeTransport,
  evaluateFiniteNumericExpression,
  evaluatePositiveMillimetreExpression,
  MAX_NUMERIC_EXPRESSION_SOURCE_BYTES,
  numericExpressionNativeErrorCategory,
  NumericExpressionNativeError,
  NUMERIC_EXPRESSION_SCHEMA,
  parseNumericExpressionResponseDto,
  type NumericExpressionEvaluation,
  type NumericExpressionNativeTransport,
} from '../src/lib/numericExpressionNative.ts'

function display(value: number): string {
  return value.toExponential(17).replace('e+', 'e')
}

function response(
  overrides: Readonly<Record<string, unknown>> = {},
): Readonly<Record<string, unknown>> {
  return {
    schema: NUMERIC_EXPRESSION_SCHEMA,
    source: '1 / 10',
    requestedPrecisionBits: 96,
    exact: true,
    operations: 1,
    lowerBound: 0.09999999999999999,
    upperBound: 0.1,
    lowerDisplay: display(0.09999999999999999),
    upperDisplay: display(0.1),
    ...overrides,
  }
}

test('transport sends the closed nested command and accepts its bounded enclosure', async () => {
  const calls: Array<readonly [
    string,
    Readonly<Record<string, unknown>> | undefined,
  ]> = []
  const transport = createNumericExpressionNativeTransport(
    (command, arguments_) => {
      calls.push([command, arguments_])
      return response()
    },
  )

  const result = await transport.evaluate('1 / 10', 96)

  assert.deepEqual(calls, [[
    'evaluate_numeric_expression',
    {
      request: {
        source: '1 / 10',
        precisionBits: 96,
      },
    },
  ]])
  assert.deepEqual(result, response())
  assert.equal(Object.isFrozen(result), true)
})

test('strict parser rejects unknown fields, raw integers, and malformed scalars', () => {
  const rejected = [
    response({ privateNumerator: '123456789' }),
    response({ lowerBound: 1n }),
    response({ lowerBound: Number.NaN }),
    response({ lowerBound: Number.NEGATIVE_INFINITY }),
    response({ upperBound: Number.POSITIVE_INFINITY }),
    response({ lowerBound: 2, upperBound: 1 }),
    response({ lowerBound: -0, lowerDisplay: display(-0) }),
    response({ operations: 20_001 }),
    response({ operations: 1.5 }),
    response({ exact: 1 }),
    response({ source: 'x'.repeat(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES + 1) }),
    response({ requestedPrecisionBits: 31 }),
    response({ requestedPrecisionBits: 513 }),
    response({ lowerDisplay: '0.1' }),
    response({ lowerDisplay: display(0.1) }),
    response({ schema: 'origami2.numeric-expression-evaluation.v2' }),
  ]

  for (const value of rejected) {
    assert.equal(parseNumericExpressionResponseDto(value), null)
  }
})

test('request bounds reject oversize, non-scalar text, and invalid precision before IPC', async () => {
  let calls = 0
  const transport = createNumericExpressionNativeTransport(() => {
    calls += 1
    return response()
  })
  const invalid: Array<readonly [unknown, unknown]> = [
    ['x'.repeat(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES + 1), 96],
    ['あ'.repeat(1_366), 96],
    ['\ud800', 96],
    ['1', 31],
    ['1', 513],
    ['1', 96.5],
    ['1', Number.NaN],
    ['1', Number.POSITIVE_INFINITY],
    [1, 96],
  ]

  for (const [source, precision] of invalid) {
    await assert.rejects(
      transport.evaluate(
        source as string,
        precision as number,
      ),
      (error: unknown) => (
        error instanceof NumericExpressionNativeError
        && error.category === 'invalid_request'
      ),
    )
  }
  assert.equal(calls, 0)
})

test('oversize source is rejected before TextEncoder and hostile encoding stays closed', async () => {
  const OriginalTextEncoder = globalThis.TextEncoder
  let encoderConstructions = 0
  const hostileTextEncoder = new Proxy(OriginalTextEncoder, {
    construct() {
      encoderConstructions += 1
      throw new Error('C:\\private\\text-encoder.txt')
    },
  })
  Object.defineProperty(globalThis, 'TextEncoder', {
    configurable: true,
    writable: true,
    value: hostileTextEncoder,
  })
  let calls = 0
  const transport = createNumericExpressionNativeTransport(() => {
    calls += 1
    return response()
  })
  try {
    await assert.rejects(
      transport.evaluate(
        'x'.repeat(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES + 1),
        96,
      ),
      hasNumericExpressionCategory('invalid_request'),
    )
    assert.equal(encoderConstructions, 0)

    await assert.rejects(
      transport.evaluate(
        'x'.repeat(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES),
        96,
      ),
      hasNumericExpressionCategory('invalid_request'),
    )
    assert.equal(encoderConstructions, 1)
    assert.equal(calls, 0)
  } finally {
    Object.defineProperty(globalThis, 'TextEncoder', {
      configurable: true,
      writable: true,
      value: OriginalTextEncoder,
    })
  }
})

test('UTF-8 scalar source accepts the exact byte ceiling without code-unit ambiguity', async () => {
  const source = 'π'.repeat(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES / 2)
  const transport = createNumericExpressionNativeTransport(
    () => response({ source }),
  )

  const result = await transport.evaluate(source, 96)
  assert.equal(result.source, source)

  await assert.rejects(
    transport.evaluate(`${source}π`, 96),
    (error: unknown) => (
      error instanceof NumericExpressionNativeError
      && error.category === 'invalid_request'
    ),
  )
})

test('source and precision echoes reject stale or swapped native responses', async () => {
  for (const stale of [
    response({ source: '2 / 10' }),
    response({ requestedPrecisionBits: 97 }),
  ]) {
    const transport = createNumericExpressionNativeTransport(() => stale)
    await assert.rejects(
      transport.evaluate('1 / 10', 96),
      (error: unknown) => (
        error instanceof NumericExpressionNativeError
        && error.category === 'stale_response'
      ),
    )
  }
})

test('only closed native categories cross the error boundary', async () => {
  for (const category of [
    'invalid_request',
    'invalid_expression',
    'resource_limit',
    'result_out_of_range',
    'internal_failure',
  ] as const) {
    const transport = createNumericExpressionNativeTransport(() => {
      throw { category }
    })
    await assert.rejects(
      transport.evaluate('1 / 10', 96),
      (error: unknown) => (
        error instanceof NumericExpressionNativeError
        && error.category === category
      ),
    )
  }

  const privatePath = 'C:\\Users\\alice\\private-expression.txt'
  const transport = createNumericExpressionNativeTransport(() => {
    throw {
      category: 'invalid_expression',
      message: privatePath,
    }
  })
  await assert.rejects(
    transport.evaluate('1 / 10', 96),
    (error: unknown) => (
      error instanceof NumericExpressionNativeError
      && error.category === 'native_unavailable'
      && !String(error).includes(privatePath)
    ),
  )
})

test('hostile DTOs and synchronous or asynchronous exceptions fail closed', async () => {
  const accessor = Object.create(null) as Record<string, unknown>
  Object.defineProperty(accessor, 'schema', {
    enumerable: true,
    get() {
      throw new Error('private accessor')
    },
  })
  const hostile = [
    accessor,
    new Proxy({}, {
      ownKeys() {
        throw new Error('C:\\private\\secret.ori')
      },
    }),
  ]
  for (const value of hostile) {
    const transport = createNumericExpressionNativeTransport(() => value)
    await assert.rejects(
      transport.evaluate('1 / 10', 96),
      (error: unknown) => (
        error instanceof NumericExpressionNativeError
        && error.category === 'invalid_response'
      ),
    )
  }

  for (const invoke of [
    () => {
      throw new Error('C:\\private\\secret.ori')
    },
    () => Promise.reject(new Error('C:\\private\\secret.ori')),
  ]) {
    const transport = createNumericExpressionNativeTransport(invoke)
    await assert.rejects(
      transport.evaluate('1 / 10', 96),
      (error: unknown) => (
        error instanceof NumericExpressionNativeError
        && error.category === 'native_unavailable'
        && !String(error).includes('secret.ori')
      ),
    )
  }

  const hostileRejection = new Proxy({}, {
    getPrototypeOf() {
      throw new Error('C:\\private\\rejection-payload.txt')
    },
  })
  const hostileTransport = createNumericExpressionNativeTransport(
    () => Promise.reject(hostileRejection),
  )
  await assert.rejects(
    hostileTransport.evaluate('1 / 10', 96),
    (error: unknown) => (
      error instanceof NumericExpressionNativeError
      && error.category === 'native_unavailable'
      && !String(error).includes('rejection-payload.txt')
    ),
  )
})

test('known local errors are copied into a fresh bounded error', async () => {
  const injected = new NumericExpressionNativeError('resource_limit')
  injected.message = 'C:\\private\\mutated-message.txt'
  const transport = createNumericExpressionNativeTransport(() => {
    throw injected
  })

  await assert.rejects(
    transport.evaluate('1 / 10', 96),
    (error: unknown) => (
      error instanceof NumericExpressionNativeError
      && error !== injected
      && error.category === 'resource_limit'
      && !String(error).includes('mutated-message.txt')
    ),
  )
})

test('error-category extraction contains hostile instanceof and proxy traps', () => {
  const hostile = new Proxy({}, {
    getPrototypeOf() {
      throw new Error('C:\\private\\instanceof-trap.txt')
    },
  })
  assert.equal(numericExpressionNativeErrorCategory(hostile), null)
  assert.equal(
    numericExpressionNativeErrorCategory(
      new NumericExpressionNativeError('invalid_expression'),
    ),
    'invalid_expression',
  )
})

test('browser default never attempts native evaluation and returns one safe category', async () => {
  const transport = createNumericExpressionNativeTransport()
  await assert.rejects(
    transport.evaluate('1', 96),
    (error: unknown) => (
      error instanceof NumericExpressionNativeError
      && error.category === 'native_unavailable'
    ),
  )
})

test('millimetre adoption accepts only positive exact or adjacent-f64 enclosures', async () => {
  assert.equal(adoptPositiveAdjacentInterval(400, 400), 400)
  for (const [lower, upper] of [
    [floatFromBits(1n), floatFromBits(2n)],
    [1, 1.0000000000000002],
    [floatFromBits(floatBits(2) - 1n), 2],
    [floatFromBits(floatBits(Number.MAX_VALUE) - 1n), Number.MAX_VALUE],
  ] as const) {
    assert.equal(
      Object.is(adoptPositiveAdjacentInterval(lower, upper), lower),
      true,
    )
  }
  assert.equal(adoptPositiveAdjacentInterval(1, 1.0000000000000004), null)
  assert.equal(adoptPositiveAdjacentInterval(0, Number.MIN_VALUE), null)
  assert.equal(adoptPositiveAdjacentInterval(-1, -1), null)
  assert.equal(adoptPositiveAdjacentInterval(2, 1), null)

  const transport = createNumericExpressionNativeTransport((_command, arguments_) => {
    const request = (arguments_?.request ?? {}) as Record<string, unknown>
    return response({
      source: request.source,
      requestedPrecisionBits: request.precisionBits,
      lowerBound: 400,
      upperBound: 400,
      lowerDisplay: display(400),
      upperDisplay: display(400),
    })
  })
  const adopted = await evaluatePositiveMillimetreExpression('200 * 2', transport)
  assert.equal(adopted.value, 400)
  assert.equal(adopted.source, '200 * 2')
  assert.equal(adopted.evaluation.requestedPrecisionBits, 192)
  assert.equal(Object.isFrozen(adopted), true)
})

test('general scalar adoption accepts signed zero and adjacent finite enclosures', async () => {
  assert.equal(adoptFiniteAdjacentInterval(-12.5, -12.5), -12.5)
  assert.equal(Object.is(adoptFiniteAdjacentInterval(-0, 0), 0), true)
  const adjacentNegativeLower = floatFromBits(floatBits(-2) + 1n)
  assert.equal(
    adoptFiniteAdjacentInterval(adjacentNegativeLower, -2),
    adjacentNegativeLower,
  )
  assert.equal(adoptFiniteAdjacentInterval(-2, -1.9999999999999996), null)
  assert.equal(adoptFiniteAdjacentInterval(Number.NEGATIVE_INFINITY, -1), null)

  const transport: NumericExpressionNativeTransport = {
    async evaluate(source, precisionBits) {
      return response({
        source,
        requestedPrecisionBits: precisionBits,
        lowerBound: -45,
        upperBound: -45,
        lowerDisplay: display(-45),
        upperDisplay: display(-45),
      }) as unknown as NumericExpressionEvaluation
    },
  }
  const adopted = await evaluateFiniteNumericExpression('-90 / 2', transport)
  assert.equal(adopted.value, -45)
  assert.equal(adopted.source, '-90 / 2')
})

test('user-input burst keeps one running and only the latest pending native job', async () => {
  const calls: string[] = []
  let resolveFirst: ((value: NumericExpressionEvaluation) => void) | undefined
  const firstResponse = new Promise<NumericExpressionEvaluation>((resolve) => {
    resolveFirst = resolve
  })
  const transport: NumericExpressionNativeTransport = {
    evaluate(source, precisionBits) {
      calls.push(source)
      if (source === '1') return firstResponse
      return Promise.resolve(adoptableResponse(source, precisionBits))
    },
  }

  const first = evaluatePositiveMillimetreExpression('1', transport)
  assert.deepEqual(calls, ['1'])

  let latest = evaluatePositiveMillimetreExpression('2', transport)
  const staleAssertions: Array<Promise<void>> = []
  for (let value = 3; value <= 32; value += 1) {
    staleAssertions.push(assert.rejects(
      latest,
      hasNumericExpressionCategory('stale_response'),
    ))
    latest = evaluatePositiveMillimetreExpression(String(value), transport)
  }
  assert.deepEqual(calls, ['1'], 'pending jobs must not invoke native')

  resolveFirst?.(adoptableResponse('1', 192))
  const [firstResult, latestResult] = await Promise.all([first, latest])
  await Promise.all(staleAssertions)

  assert.equal(firstResult.source, '1')
  assert.equal(latestResult.source, '32')
  assert.deepEqual(calls, ['1', '32'])
})

function adoptableResponse(
  source: string,
  requestedPrecisionBits: number,
): NumericExpressionEvaluation {
  return {
    schema: NUMERIC_EXPRESSION_SCHEMA,
    source,
    requestedPrecisionBits,
    exact: true,
    operations: 1,
    lowerBound: 400,
    upperBound: 400,
    lowerDisplay: display(400),
    upperDisplay: display(400),
  }
}

function hasNumericExpressionCategory(category: string) {
  return (error: unknown) => (
    error instanceof NumericExpressionNativeError
    && error.category === category
  )
}

function floatBits(value: number): bigint {
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setFloat64(0, value, false)
  return view.getBigUint64(0, false)
}

function floatFromBits(bits: bigint): number {
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setBigUint64(0, bits, false)
  return view.getFloat64(0, false)
}
