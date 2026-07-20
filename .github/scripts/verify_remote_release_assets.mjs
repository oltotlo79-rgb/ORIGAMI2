import { createHash } from 'node:crypto'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'

const directory = resolve(process.argv[2])
const responsePath = process.argv[3]
const allowSubset = process.argv[4] === '--allow-subset'
if (statSync(responsePath).size > 1_048_576) throw new Error('release response is oversized')
const response = JSON.parse(readFileSync(responsePath, 'utf8'))
if (
  !Array.isArray(response.assets)
  || (!allowSubset && response.assets.length !== 9)
  || response.assets.length > 9
) {
  throw new Error('remote release asset count mismatch')
}
const local = readdirSync(directory).sort().map((name) => {
  const bytes = readFileSync(join(directory, name))
  return {
    name,
    size: bytes.length,
    digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}`,
  }
})
if (local.length !== 9) throw new Error('local release asset count mismatch')
const remote = response.assets.map(({ name, size, digest }) => ({ name, size, digest }))
  .sort((left, right) => left.name.localeCompare(right.name))
const expected = allowSubset
  ? local.filter(({ name }) => remote.some((asset) => asset.name === name))
  : local
if (JSON.stringify(remote) !== JSON.stringify(expected)) {
  throw new Error('remote release assets differ from verified local assets')
}
console.log('verified remote draft release assets')
