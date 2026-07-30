import { readdirSync, readFileSync } from 'node:fs'

export function readDesktopRustUnitTestSources(): string {
  const root = new URL('../src-tauri/src/', import.meta.url)
  return [
    readFileSync(new URL('tests.rs', root), 'utf8'),
    ...readRustSourcesRecursively(new URL('tests/', root)),
  ].join('\n')
}

function readRustSourcesRecursively(directory: URL): string[] {
  const sources: string[] = []
  const entries = readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name, 'en'))
  for (const entry of entries) {
    const target = new URL(entry.isDirectory() ? `${entry.name}/` : entry.name, directory)
    if (entry.isDirectory()) {
      sources.push(...readRustSourcesRecursively(target))
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      sources.push(readFileSync(target, 'utf8'))
    }
  }
  return sources
}
