import { readFileSync } from 'node:fs'

const metadata = JSON.parse(readFileSync(process.argv[2], 'utf8'))
const expectedNames = [
  'formal-release-macos-arm64',
  'formal-release-windows-x64',
]
if (
  metadata === null
  || typeof metadata !== 'object'
  || metadata.total_count !== expectedNames.length
  || !Array.isArray(metadata.artifacts)
  || metadata.artifacts.length !== expectedNames.length
) {
  throw new Error('workflow artifact count mismatch')
}
const admitted = metadata.artifacts.map((artifact) => {
  if (
    artifact === null
    || typeof artifact !== 'object'
    || !Number.isSafeInteger(artifact.id)
    || artifact.id <= 0
    || typeof artifact.name !== 'string'
    || artifact.expired !== false
    || typeof artifact.digest !== 'string'
    || !/^sha256:[0-9a-f]{64}$/u.test(artifact.digest)
  ) {
    throw new Error('workflow artifact metadata is invalid')
  }
  return artifact
}).sort((left, right) => left.name.localeCompare(right.name))
if (admitted.map(({ name }) => name).join('\n') !== expectedNames.join('\n')) {
  throw new Error('workflow artifact names are incomplete or ambiguous')
}
for (const { name, id, digest } of admitted) {
  process.stdout.write(`${name}\t${id}\t${digest.slice(7)}\n`)
}
