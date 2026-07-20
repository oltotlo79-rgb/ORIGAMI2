import { readFileSync, statSync } from 'node:fs'

const metadataPath = process.argv[2]
const metadataSize = statSync(metadataPath).size
if (metadataSize <= 0 || metadataSize > 1_048_576) {
  throw new Error('workflow artifact metadata size is invalid')
}
const metadata = JSON.parse(readFileSync(metadataPath, 'utf8'))
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
    || !Number.isSafeInteger(artifact.size_in_bytes)
    || artifact.size_in_bytes <= 0
    || artifact.size_in_bytes > 2_147_483_648
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
