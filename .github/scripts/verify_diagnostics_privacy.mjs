import { readFileSync, readdirSync } from 'node:fs'
import { extname, join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..', '..')
const roots = [join(root, 'apps/desktop/src'), join(root, 'apps/desktop/src-tauri/src')]
let files = 0
for (const directory of roots) {
  for (const relative of readdirSync(directory, { recursive: true })) {
    if (!['.ts', '.tsx', '.rs'].includes(extname(relative))) continue
    files += 1
    if (files > 1024) throw new Error('diagnostics privacy source bound exceeded')
    const path = join(directory, relative)
    let source = readFileSync(path, 'utf8')
    if (extname(relative) === '.rs') {
      const tests = source.search(/\r?\n#\[cfg\(test\)\]\r?\nmod tests/u)
      if (tests >= 0) source = source.slice(0, tests)
    }
    if (/\bconsole\s*\.|\b(?:println|eprintln|dbg)!|\b(?:tracing|log)::/u.test(source)) {
      throw new Error(`production source contains an unredacted logging sink: ${relative}`)
    }
  }
}
const native = readFileSync(join(root, 'apps/desktop/src-tauri/src/diagnostics.rs'), 'utf8')
const frontend = readFileSync(join(root, 'apps/desktop/src/lib/diagnostics.ts'), 'utf8')
if (!/struct StoredDiagnosticCount \{\s*scope: DiagnosticScope,\s*count: DiagnosticCountBucket,\s*\}[\s\S]*struct StoredDiagnostics \{\s*schema: DiagnosticsSchema,\s*unexpected: Vec<StoredDiagnosticCount>,\s*\}/u.test(native)) {
  throw new Error('native diagnostics schema gained non-aggregate fields')
}
for (const forbidden of ['projectId', 'fileName', 'errorMessage', 'stackTrace']) {
  if (frontend.includes(forbidden)) throw new Error(`frontend diagnostics schema exposes private project data: ${forbidden}`)
}
if (!native.includes('origami2.redacted-diagnostics.v1') || !frontend.includes('origami2.redacted-diagnostics.v1')) {
  throw new Error('redacted diagnostics schema is not bound across runtimes')
}
process.stdout.write(`diagnostics privacy verified across ${files} production source files\n`)
