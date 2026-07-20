import { readFileSync, statSync } from 'node:fs'

const metadataPath = process.argv[2]
const metadataSize = statSync(metadataPath).size
if (metadataSize <= 0 || metadataSize > 1_048_576) {
  throw new Error('workflow artifact metadata size is invalid')
}
const metadata = JSON.parse(readFileSync(metadataPath, 'utf8'))
const runId = Number(process.env.GITHUB_RUN_ID)
const releaseCommit = process.env.RELEASE_COMMIT
if (!Number.isSafeInteger(runId) || runId < 1 || !/^[0-9a-f]{40}$/u.test(releaseCommit ?? '')) {
  throw new Error('workflow artifact expected identity is invalid')
}
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
  const createdAt = Date.parse(artifact?.created_at)
  const expiresAt = Date.parse(artifact?.expires_at)
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
    || artifact.workflow_run?.id !== runId
    || artifact.workflow_run?.head_sha !== releaseCommit
    || !Number.isFinite(createdAt)
    || !Number.isFinite(expiresAt)
    || expiresAt <= createdAt
    || expiresAt - createdAt > 26 * 60 * 60 * 1000
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
