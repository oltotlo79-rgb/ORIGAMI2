import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import test from 'node:test'

const runtimeOperationNames = new Set([
  'atan2',
  'cos',
  'hypot',
  'sin',
  'sin_cos',
  'sincos',
  'tan',
  'to_degrees',
  'to_radians',
])
const frozenKernelPath =
  'crates/ori-numeric/src/deterministic_transcendental.rs'
const explicitRuntimeComparisonTestSources = new Set([
  'crates/ori-core/src/constraint_solver_deterministic_proof_tests.rs',
  'crates/ori-core/src/editor/tests/editor_deterministic_constraint_admission_tests.rs',
])

interface RustToken {
  index: number
  value: string
}

interface ForbiddenRuntimeReference {
  index: number
  operation: string
  receiver?: string
}

function readWorkspaceSource(relativePath: string): string {
  return readFileSync(
    new URL(`../../../${relativePath}`, import.meta.url),
    'utf8',
  )
}

function listRustSources(relativeRoot: string): string[] {
  const paths: string[] = []

  function visit(directory: URL, relativeDirectory: string): void {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const relativePath = `${relativeDirectory}/${entry.name}`
      if (entry.isDirectory()) {
        visit(new URL(`${entry.name}/`, directory), relativePath)
      } else if (entry.isFile() && entry.name.endsWith('.rs')) {
        paths.push(relativePath)
      }
    }
  }

  visit(
    new URL(`../../../${relativeRoot}/`, import.meta.url),
    relativeRoot,
  )
  return paths.sort()
}

function listWorkspaceRustSources(): string[] {
  return [
    ...listRustSources('crates')
      .filter((relativePath) => relativePath.includes('/src/')),
    ...listRustSources('apps/desktop/src-tauri/src'),
  ].sort()
}

function withoutInlineTests(source: string): string {
  const code = rustCodeOnly(source)
  const marker =
    /(?:^|\r?\n)[ \t]*#\[cfg\(test\)\]\r?\n[ \t]*(?:pub(?:\([^)]*\))?\s+)?mod tests\s*\{/gmu
  const ranges: Array<{ end: number; start: number }> = []
  for (const match of code.matchAll(marker)) {
    const openingBrace = code.indexOf('{', match.index)
    let depth = 0
    let end = -1
    for (let index = openingBrace; index < code.length; index += 1) {
      if (code[index] === '{') {
        depth += 1
      } else if (code[index] === '}') {
        depth -= 1
        if (depth === 0) {
          end = index + 1
          break
        }
      }
    }
    assert.notEqual(end, -1, 'unterminated #[cfg(test)] mod tests block')
    ranges.push({ end, start: match.index })
  }
  return ranges
    .reverse()
    .reduce(
      (result, { end, start }) =>
        result.slice(0, start)
        + result.slice(start, end).replace(/[^\r\n]/gu, ' ')
        + result.slice(end),
      source,
    )
}

function rustCodeOnly(source: string): string {
  let result = ''
  let index = 0
  while (index < source.length) {
    if (source.startsWith('//', index)) {
      while (index < source.length && source[index] !== '\n') {
        result += ' '
        index += 1
      }
      continue
    }
    if (source.startsWith('/*', index)) {
      let depth = 1
      result += '  '
      index += 2
      while (index < source.length && depth > 0) {
        if (source.startsWith('/*', index)) {
          depth += 1
          result += '  '
          index += 2
        } else if (source.startsWith('*/', index)) {
          depth -= 1
          result += '  '
          index += 2
        } else {
          result += source[index] === '\n' ? '\n' : ' '
          index += 1
        }
      }
      continue
    }

    const rawString = rawStringOpening(source, index)
    if (rawString) {
      const terminator = `"${'#'.repeat(rawString.hashes)}`
      for (let offset = index; offset < rawString.contentStart; offset += 1) {
        result += ' '
      }
      index = rawString.contentStart
      while (
        index < source.length
        && !source.startsWith(terminator, index)
      ) {
        result += source[index] === '\n' ? '\n' : ' '
        index += 1
      }
      for (let offset = 0; offset < terminator.length; offset += 1) {
        result += ' '
      }
      index += terminator.length
      continue
    }

    const characterLiteralEnd = rustCharacterLiteralEnd(source, index)
    if (characterLiteralEnd !== null) {
      while (index < characterLiteralEnd) {
        result += source[index] === '\n' ? '\n' : ' '
        index += 1
      }
      continue
    }

    if (source[index] === '"') {
      result += ' '
      index += 1
      let escaped = false
      while (index < source.length) {
        const character = source[index]
        result += character === '\n' ? '\n' : ' '
        index += 1
        if (!escaped && character === '"') break
        if (!escaped && character === '\\') {
          escaped = true
        } else {
          escaped = false
        }
      }
      continue
    }

    result += source[index]
    index += 1
  }
  return result
}

function rustCharacterLiteralEnd(
  source: string,
  index: number,
): number | null {
  if (source[index] !== '\'' || index + 2 >= source.length) return null
  if (source[index + 1] !== '\\') {
    const codePoint = source.codePointAt(index + 1)
    if (codePoint === undefined) return null
    const width = codePoint > 0xffff ? 2 : 1
    return source[index + 1 + width] === '\''
      ? index + 2 + width
      : null
  }

  let cursor = index + 2
  let escaped = true
  while (cursor < source.length && source[cursor] !== '\n') {
    const character = source[cursor]
    cursor += 1
    if (!escaped && character === '\'') return cursor
    escaped = !escaped && character === '\\'
  }
  return null
}

function rawStringOpening(
  source: string,
  index: number,
): { contentStart: number; hashes: number } | null {
  let cursor = index
  if (source.startsWith('br', cursor)) {
    cursor += 2
  } else if (source[cursor] === 'r') {
    cursor += 1
  } else {
    return null
  }
  let hashes = 0
  while (source[cursor] === '#') {
    hashes += 1
    cursor += 1
  }
  return source[cursor] === '"'
    ? { contentStart: cursor + 1, hashes }
    : null
}

function rustTokens(source: string): RustToken[] {
  const code = rustCodeOnly(source)
  const tokens: RustToken[] = []
  let index = 0
  while (index < code.length) {
    const character = code[index]
    if (/[A-Za-z_]/u.test(character)) {
      const start = index
      index += 1
      while (index < code.length && /[A-Za-z0-9_]/u.test(code[index])) {
        index += 1
      }
      tokens.push({ index: start, value: code.slice(start, index) })
      continue
    }
    if (code.startsWith('::', index)) {
      tokens.push({ index, value: '::' })
      index += 2
      continue
    }
    if (!/\s/u.test(character)) {
      tokens.push({ index, value: character })
    }
    index += 1
  }
  return tokens
}

function findForbiddenRuntimeReferences(
  source: string,
): ForbiddenRuntimeReference[] {
  const tokens = rustTokens(source)
  const references: ForbiddenRuntimeReference[] = []
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index]
    const previous = tokens[index - 1]
    const receiver = tokens[index - 2]
    const next = tokens[index + 1]
    const afterNext = tokens[index + 2]

    if (token.value === 'libm') {
      if (next?.value !== '::' || afterNext === undefined) {
        references.push({ index: token.index, operation: 'libm' })
      } else if (afterNext.value === '{') {
        let depth = 0
        let importsRuntimeOperation = false
        for (let cursor = index + 2; cursor < tokens.length; cursor += 1) {
          if (tokens[cursor].value === '{') depth += 1
          if (
            depth > 0
            && runtimeOperationNames.has(tokens[cursor].value)
          ) {
            importsRuntimeOperation = true
          }
          if (tokens[cursor].value === '}') {
            depth -= 1
            if (depth === 0) break
          }
        }
        if (importsRuntimeOperation) {
          references.push({ index: token.index, operation: 'libm' })
        }
      } else if (afterNext.value === '*') {
        references.push({ index: token.index, operation: 'libm' })
      }
      continue
    }
    if (!runtimeOperationNames.has(token.value)) continue

    if (
      previous?.value === '.'
      && (next?.value === '(' || next?.value === '::')
    ) {
      references.push({
        index: token.index,
        operation: `.${token.value}`,
        receiver: receiver?.value,
      })
    } else if (previous?.value === '::') {
      references.push({
        index: token.index,
        operation: receiver?.value === 'libm'
          ? `libm::${token.value}`
          : `::${token.value}`,
        receiver: receiver?.value,
      })
    } else if (next?.value === '(' && previous?.value !== 'fn') {
      references.push({
        index: token.index,
        operation: token.value,
      })
    }
  }
  return references.sort((left, right) => left.index - right.index)
}

function findForbiddenRuntimeOperations(source: string): string[] {
  return findForbiddenRuntimeReferences(source)
    .map(({ operation }) => operation)
}

function extractRustBlock(
  source: string,
  signature: string,
): { block: string; end: number; remainder: string; start: number } {
  const code = rustCodeOnly(source)
  const start = code.indexOf(signature)
  assert.notEqual(start, -1, `missing Rust block: ${signature}`)
  assert.equal(
    code.indexOf(signature, start + signature.length),
    -1,
    `ambiguous Rust block: ${signature}`,
  )

  const openingBrace = code.indexOf('{', start)
  assert.notEqual(openingBrace, -1, `missing opening brace: ${signature}`)
  let depth = 0
  for (let index = openingBrace; index < code.length; index += 1) {
    if (code[index] === '{') {
      depth += 1
    } else if (code[index] === '}') {
      depth -= 1
      if (depth === 0) {
        const end = index + 1
        const blankedBlock = source
          .slice(start, end)
          .replace(/[^\r\n]/gu, ' ')
        return {
          block: source.slice(start, end),
          end,
          remainder: source.slice(0, start) + blankedBlock + source.slice(end),
          start,
        }
      }
    }
  }

  assert.fail(`missing closing brace: ${signature}`)
}

function sourceWithoutAuditedRuntimePreviewBlocks(
  relativePath: string,
): string {
  let source = withoutInlineTests(readWorkspaceSource(relativePath))
  if (relativePath === 'crates/ori-core/src/constraints.rs') {
    source = extractRustBlock(
      source,
      'pub(crate) fn fixed_angle_residual_binary64_v1',
    ).remainder
  }
  if (relativePath === 'crates/ori-core/src/constraint_solver.rs') {
    source = extractRustBlock(
      source,
      'impl ResidualTranscendentalModelV1',
    ).remainder
    source = extractRustBlock(
      source,
      'fn residuals_with_transcendental_model_v1',
    ).remainder
  }
  return source
}

test('every new production Rust source is default-denied runtime transcendental access', () => {
  const workspaceSources = listWorkspaceRustSources()
  assert.ok(
    workspaceSources.length > 100,
    'the policy must discover the workspace recursively rather than use a hand-maintained file list',
  )

  for (const relativePath of workspaceSources) {
    if (
      relativePath === frozenKernelPath
      || explicitRuntimeComparisonTestSources.has(relativePath)
    ) {
      continue
    }
    const productionSource =
      sourceWithoutAuditedRuntimePreviewBlocks(relativePath)
    assert.deepEqual(
      findForbiddenRuntimeOperations(productionSource),
      [],
      `${relativePath} bypasses the default-deny frozen transcendental policy`,
    )
  }
})

test('fixed-angle runtime conversion stays inside its preview-only helper', () => {
  const source = withoutInlineTests(
    readWorkspaceSource('crates/ori-core/src/constraints.rs'),
  )
  const previewHelper = extractRustBlock(
    source,
    'pub(crate) fn fixed_angle_residual_binary64_v1',
  )
  const previewDocumentation = source.slice(
    Math.max(0, previewHelper.start - 600),
    previewHelper.start,
  )

  assert.match(previewDocumentation, /numerical solver preview/u)
  assert.match(previewDocumentation, /not proof authority/u)
  assert.deepEqual(
    findForbiddenRuntimeOperations(previewHelper.block),
    ['.to_radians'],
  )
  assert.match(
    previewHelper.block,
    /fixed_angle_residual_from_expected_radians_binary64_v1\s*\([\s\S]*angle_degrees\s*\.\s*to_radians\s*\(\s*\)/u,
  )
  assert.deepEqual(
    findForbiddenRuntimeOperations(previewHelper.remainder),
    [],
    'constraints.rs uses a runtime transcendental operation outside the preview-only helper',
  )
})

test('solver runtime transcendental calls stay isolated from proof residuals', () => {
  const source = withoutInlineTests(
    readWorkspaceSource('crates/ori-core/src/constraint_solver.rs'),
  )
  const modelImplementation = extractRustBlock(
    source,
    'impl ResidualTranscendentalModelV1',
  )
  const hypotMethod = extractRustBlock(modelImplementation.block, 'fn hypot')
  const atan2Method = extractRustBlock(modelImplementation.block, 'fn atan2')
  const sinCosMethod = extractRustBlock(
    modelImplementation.block,
    'fn sin_cos_degrees',
  )

  assert.deepEqual(
    findForbiddenRuntimeOperations(modelImplementation.block),
    ['.hypot', '.atan2', '.to_radians', '.sin', '.cos'],
  )
  assert.deepEqual(
    findForbiddenRuntimeOperations(hypotMethod.block),
    ['.hypot'],
  )
  assert.deepEqual(
    findForbiddenRuntimeOperations(atan2Method.block),
    ['.atan2'],
  )
  assert.deepEqual(
    findForbiddenRuntimeOperations(sinCosMethod.block),
    ['.to_radians', '.sin', '.cos'],
  )
  assert.match(
    hypotMethod.block,
    /Self::RuntimePreview\s*=>\s*Ok\s*\(\s*x\s*\.\s*hypot\s*\(\s*y\s*\)\s*\)/u,
  )
  assert.match(
    hypotMethod.block,
    /Self::DeterministicProofV1\s*=>[\s\S]*deterministic_hypot_v1\s*\(\s*x\s*,\s*y\s*\)/u,
  )
  assert.match(
    atan2Method.block,
    /Self::RuntimePreview\s*=>\s*Ok\s*\(\s*y\s*\.\s*atan2\s*\(\s*x\s*\)\s*\)/u,
  )
  assert.match(
    atan2Method.block,
    /Self::DeterministicProofV1\s*=>[\s\S]*deterministic_atan2_v1\s*\(\s*y\s*,\s*x\s*\)/u,
  )
  assert.match(
    sinCosMethod.block,
    /Self::RuntimePreview\s*=>\s*\{[\s\S]*degrees\s*\.\s*to_radians\s*\(\s*\)[\s\S]*radians\s*\.\s*sin\s*\(\s*\)[\s\S]*radians\s*\.\s*cos\s*\(\s*\)/u,
  )
  assert.match(
    sinCosMethod.block,
    /Self::DeterministicProofV1\s*=>\s*deterministic_sin_cos_degrees_v1\s*\(\s*degrees\s*\)/u,
  )

  const residualEvaluator = extractRustBlock(
    modelImplementation.remainder,
    'fn residuals_with_transcendental_model_v1',
  )
  const residualDispatchReferences =
    findForbiddenRuntimeReferences(residualEvaluator.block)
  assert.deepEqual(
    residualDispatchReferences.map(({ operation }) => operation),
    [
      '.hypot',
      '.hypot',
      '.hypot',
      '.hypot',
      '.atan2',
      '.hypot',
      '.hypot',
      '.hypot',
      '.hypot',
    ],
  )
  assert.ok(
    residualDispatchReferences.every(
      ({ receiver }) => receiver === 'transcendental_model',
    ),
    'shared residual algebra may call only the typed preview/proof model receiver',
  )
  assert.deepEqual(
    findForbiddenRuntimeOperations(residualEvaluator.remainder),
    [],
    'constraint solver bypasses its preview/proof transcendental dispatch',
  )

  const proofResiduals = extractRustBlock(
    source,
    'pub(super) fn deterministic_proof_residuals_v1',
  ).block
  assert.deepEqual(findForbiddenRuntimeOperations(proofResiduals), [])
  assert.match(
    proofResiduals,
    /residuals_with_transcendental_model_v1\s*\([\s\S]*ResidualTranscendentalModelV1::DeterministicProofV1/u,
  )
})

test('constrained mutation authority uses only the frozen residual path', () => {
  const editorSource = withoutInlineTests(
    readWorkspaceSource('crates/ori-core/src/editor.rs'),
  )
  const mutationGate = extractRustBlock(
    editorSource,
    'fn ensure_geometric_constraints_allow',
  ).block
  assert.match(
    mutationGate,
    /verify_deterministic_geometric_constraint_mutation_admission_v1\s*\(\s*&candidate\s*,\s*&self\s*\.\s*geometric_constraints\s*,?\s*\)/u,
  )
  assert.doesNotMatch(
    mutationGate,
    /\bverify_geometric_constraint_solution_v1\s*\(/u,
    'project mutation must not reuse the platform numerical-preview verifier',
  )

  const admissionSource = withoutInlineTests(
    readWorkspaceSource(
      'crates/ori-core/src/constraint_solver/mutation_admission.rs',
    ),
  )
  assert.deepEqual(findForbiddenRuntimeOperations(admissionSource), [])
  assert.match(
    admissionSource,
    /MUTATION_ADMISSION_RESIDUAL_TOLERANCE_V1\s*:\s*f64\s*=\s*f64\s*::\s*from_bits\s*\(\s*0x3e7a_d7f2_9abc_af48\s*\)/u,
  )
  assert.match(
    admissionSource,
    /deterministic_transcendental_model_supported_v1\s*\(\s*\)/u,
  )
  assert.match(
    admissionSource,
    /prepare_geometric_constraints_v1\s*\([\s\S]*GeometricConstraintLimitsV1\s*::\s*default\s*\(\s*\)/u,
  )
  const preflightPolicy = extractRustBlock(
    admissionSource,
    'fn preflight_permits_complete_deterministic_residual_evaluation_v1',
  ).block
  assert.match(
    preflightPolicy,
    /ConstraintPreflightV1\s*::\s*NoDirectConflict/u,
  )
  assert.match(
    preflightPolicy,
    /ConstraintPreflightV1\s*::\s*Unknown\s*\{[\s\S]*reason\s*:\s*GeometricConstraintUnknownReasonV1\s*::\s*SolverRequiredConstraintKinds/u,
    'only the solver-required preflight outcome may proceed to complete residual evaluation',
  )
  assert.equal(
    (preflightPolicy.match(/ConstraintPreflightV1\s*::/gu) ?? []).length,
    2,
    'the preflight allowlist must contain exactly NoDirectConflict and SolverRequiredConstraintKinds',
  )
  assert.match(
    admissionSource,
    /if\s+!\s*preflight_permits_complete_deterministic_residual_evaluation_v1\s*\([\s\S]*return\s+Err\s*\(\s*ConstraintSolveErrorV1\s*::\s*NonConvergent\s*\)/u,
    'every preflight outcome outside the narrow allowlist must fail closed',
  )
  assert.match(
    admissionSource,
    /deterministic_proof_residuals_v1\s*\(\s*candidate\s*,\s*constraints\s*,\s*&positions\s*\)/u,
  )
  assert.doesNotMatch(
    admissionSource,
    /\b(?:Binary64ExactConstraintSatisfactionV1|certificate|witness)\s*\{/u,
    'mutation admission must not mint reusable proof authority',
  )
})

test('direct libm calls remain confined to the frozen numeric kernel', () => {
  for (const relativePath of listWorkspaceRustSources()) {
    const directLibmCalls = findForbiddenRuntimeOperations(
      withoutInlineTests(readWorkspaceSource(relativePath)),
    )
      .filter((operation) => operation === 'libm'
        || operation.startsWith('libm::'))
      .sort()
    assert.deepEqual(
      directLibmCalls,
      relativePath === frozenKernelPath
        ? [
            'libm::atan2',
            'libm::cos',
            'libm::hypot',
            'libm::sin',
            'libm::sincos',
          ]
        : [],
      `${relativePath} bypasses the frozen numeric kernel`,
    )
  }
})

test('source policy token scan detects aliases UFCS and spaced calls', () => {
  assert.deepEqual(
    findForbiddenRuntimeOperations(`
      first . atan2 (second);
      angle.
        sin_cos ();
      length.hypot(other);
      degrees . to_radians ();
      radians.to_degrees();
      libm :: sin(value);
      let native_hypot = libm::hypot;
      let tangent = libm::tan(value);
      let cosine = f64 :: cos;
      let ufcs_angle = <f64 as Float> :: atan2;
      use std::primitive::f64::sin as platform_sine;
      use libm as renamed_libm;
      use libm::{sincos as pair};
      sin(value);
      value . tan ();
    `),
    [
      '.atan2',
      '.sin_cos',
      '.hypot',
      '.to_radians',
      '.to_degrees',
      'libm::sin',
      'libm::hypot',
      'libm::tan',
      '::cos',
      '::atan2',
      '::sin',
      'libm',
      'libm',
      'sin',
      '.tan',
    ],
  )
  assert.deepEqual(
    findForbiddenRuntimeOperations(String.raw`
      // ignored.sin()
      /* ignored.cos(); /* nested.tan() */ ignored.to_degrees() */
      const TEXT: &str = ".hypot()";
      const RAW: &str = r#".sin_cos()"#;
      const BYTES: &[u8] = br##".atan2()"##;
      const LIBM: &str = "libm::cos";
      const RAW_LIBM: &str = r#"libm::sincos"#;
      const TAN: &str = "libm::tan";
      let square = value.sqrt();
      let sine = deterministic_sin_v1(value);
      fn sin(value: f64) -> f64 { value }
    `),
    [],
  )
})

test('cfg test removal preserves production items declared after the test module', () => {
  const production = withoutInlineTests(`
    fn before() {
      let _ = deterministic_sin_v1(1.0);
    }

    #[cfg(test)]
    pub(crate) mod tests {
      fn runtime_comparison() {
        let _ = 1.0_f64.sin();
      }
    }

    fn after() {
      let platform_sine = f64::sin;
      let _ = platform_sine(1.0);
    }
  `)
  assert.deepEqual(findForbiddenRuntimeOperations(production), ['::sin'])
})
