import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'

const npmLicenseAllowlist = new Set([
  '0BSD', 'Apache-2.0', 'Apache-2.0 OR MIT', 'BSD-2-Clause', 'BSD-3-Clause',
  'BlueOak-1.0.0', 'CC0-1.0', 'ISC', 'MIT', 'MIT-0', 'MPL-2.0',
])

const digest = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')

export function buildDependencyPolicy() {
  const cargoBytes = readFileSync('Cargo.lock', 'utf8')
  const cargoPackages = cargoBytes.split(/\r?\n\[\[package\]\]\r?\n/u).slice(1)
  if (cargoPackages.length < 1 || cargoPackages.length > 10000) {
    throw new Error('Cargo.lock package count is outside policy bounds')
  }
  let cargoRegistryPackages = 0
  for (const entry of cargoPackages) {
    const source = /^source = "([^"]+)"$/mu.exec(entry)?.[1]
    if (source?.startsWith('git+')) throw new Error('Cargo git dependency is not allowed')
    if (source?.startsWith('registry+')) {
      cargoRegistryPackages += 1
      if (!/^checksum = "[0-9a-f]{64}"$/mu.test(entry)) {
        throw new Error('Cargo registry dependency lacks a SHA-256 checksum')
      }
    }
  }

  const packageLock = JSON.parse(readFileSync('apps/desktop/package-lock.json', 'utf8'))
  if (packageLock.lockfileVersion !== 3 || packageLock.requires !== true) {
    throw new Error('npm lockfile policy requires lockfileVersion 3')
  }
  const npmPackages = Object.entries(packageLock.packages ?? {}).filter(([path]) => path !== '')
  if (npmPackages.length < 1 || npmPackages.length > 10000) {
    throw new Error('npm package count is outside policy bounds')
  }
  for (const [path, pkg] of npmPackages) {
    if (!/^sha512-[A-Za-z0-9+/]+={0,2}$/u.test(pkg.integrity ?? '')) {
      throw new Error(`npm dependency lacks SHA-512 integrity: ${path}`)
    }
    if (!npmLicenseAllowlist.has(pkg.license)) {
      throw new Error(`npm dependency license is not allowed: ${path}`)
    }
    if (pkg.resolved !== undefined && !/^https:\/\/registry\.npmjs\.org\//u.test(pkg.resolved)) {
      throw new Error(`npm dependency source is not allowed: ${path}`)
    }
  }

  return {
    schema: 'origami2.dependency-policy.v1',
    policy: 'locked-integrity-and-license-allowlist',
    cargoLockSha256: digest('Cargo.lock'),
    packageLockSha256: digest('apps/desktop/package-lock.json'),
    cargoPackages: cargoPackages.length,
    cargoRegistryPackages,
    npmPackages: npmPackages.length,
    npmIntegrity: 'sha512-required',
    npmLicenses: [...new Set(npmPackages.map(([, pkg]) => pkg.license))].sort(),
    cargoSources: 'registry-checksum-required;git-forbidden',
    result: 'pass',
  }
}

if (process.argv[1]?.endsWith('dependency_policy.mjs')) {
  const bytes = `${JSON.stringify(buildDependencyPolicy())}\n`
  if (process.argv[2]) writeFileSync(process.argv[2], bytes, 'utf8')
  else process.stdout.write(bytes)
}
