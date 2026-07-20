import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..', '..')
const lock = readFileSync(resolve(root, 'Cargo.lock'))
const lockText = lock.toString('utf8')
const lockedSources = new Map(lockText.split(/\r?\n\[\[package\]\]\r?\n/u).slice(1).map((entry) => {
  const name = /^name = "([^"]+)"$/mu.exec(entry)?.[1]
  const version = /^version = "([^"]+)"$/mu.exec(entry)?.[1]
  return [`${name}@${version}`, {
    source: /^source = "([^"]+)"$/mu.exec(entry)?.[1] ?? null,
    checksum: /^checksum = "([0-9a-f]{64})"$/mu.exec(entry)?.[1] ?? null,
  }]
}))
const metadata = JSON.parse(execFileSync('cargo', [
  'metadata', '--locked', '--format-version', '1',
], { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }))
const packages = metadata.packages.map(({ name, version, license }) => {
  if (!license) throw new Error(`Cargo license is unknown: ${name}@${version}`)
  const packageName = `${name}@${version}`
  const locked = lockedSources.get(packageName)
  if (!locked) throw new Error(`Cargo package is absent from lockfile: ${packageName}`)
  return { package: packageName, license, source: locked.source, checksum: locked.checksum }
}).sort((left, right) => left.package.localeCompare(right.package))
if (new Set(packages.map(({ package: name }) => name)).size !== packages.length) {
  throw new Error('Cargo license package identities are duplicated')
}
writeFileSync(resolve(root, '.github/cargo-license-db.json'), `${JSON.stringify({
  schema: 'origami2.cargo-license-db.v1',
  cargoLockSha256: createHash('sha256').update(lock).digest('hex'),
  packages,
})}\n`, { encoding: 'utf8' })
