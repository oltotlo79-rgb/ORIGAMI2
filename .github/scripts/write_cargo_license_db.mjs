import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..', '..')
const lock = readFileSync(resolve(root, 'Cargo.lock'))
const metadata = JSON.parse(execFileSync('cargo', [
  'metadata', '--locked', '--format-version', '1',
], { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }))
const packages = metadata.packages.map(({ name, version, license }) => {
  if (!license) throw new Error(`Cargo license is unknown: ${name}@${version}`)
  return { package: `${name}@${version}`, license }
}).sort((left, right) => left.package.localeCompare(right.package))
if (new Set(packages.map(({ package: name }) => name)).size !== packages.length) {
  throw new Error('Cargo license package identities are duplicated')
}
writeFileSync(resolve(root, '.github/cargo-license-db.json'), `${JSON.stringify({
  schema: 'origami2.cargo-license-db.v1',
  cargoLockSha256: createHash('sha256').update(lock).digest('hex'),
  packages,
})}\n`, { encoding: 'utf8' })
