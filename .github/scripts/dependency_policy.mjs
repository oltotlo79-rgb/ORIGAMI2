import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const repositoryRoot = resolve(import.meta.dirname, '..', '..')
const cargoLockPath = resolve(repositoryRoot, 'Cargo.lock')
const packageLockPath = resolve(repositoryRoot, 'apps', 'desktop', 'package-lock.json')
const packageManifestPath = resolve(repositoryRoot, 'apps', 'desktop', 'package.json')
const cargoLicenseDbPath = resolve(repositoryRoot, '.github', 'cargo-license-db.json')
const rustsecWarningPath = resolve(repositoryRoot, '.github', 'rustsec-warning-ledger.json')

const npmLicenseAllowlist = new Set([
  '0BSD', 'Apache-2.0', 'Apache-2.0 OR MIT', 'BSD-2-Clause', 'BSD-3-Clause',
  'BlueOak-1.0.0', 'CC0-1.0', 'ISC', 'MIT', 'MIT-0', 'MPL-2.0',
])
const cargoLicenseAllowlist = new Set([
  '0BSD', 'Apache-2.0', 'BSD-3-Clause', 'CC0-1.0', 'GPL-3.0', 'ISC',
  'LGPL-2.1-or-later', 'LLVM-exception', 'MIT', 'MIT-0', 'MPL-2.0',
  'Unicode-3.0', 'Unlicense', 'Zlib',
])

const digest = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')

export function buildRustsecReviewReport() {
  const ledger = JSON.parse(readFileSync(rustsecWarningPath, 'utf8'))
  return {
    schema: 'origami2.rustsec-warning-review.v1',
    databaseCommit: ledger.databaseCommit,
    ledgerSha256: digest(rustsecWarningPath),
    exceptions: ledger.entries,
    result: 'pass',
  }
}

export function buildDependencyPolicy() {
  const rustsecWarningLedger = JSON.parse(readFileSync(rustsecWarningPath, 'utf8'))
  const rustsecAllowedWarnings = rustsecWarningLedger.entries?.map(({ id }) => id) ?? []
  if (
    rustsecWarningLedger.schema !== 'origami2.rustsec-warning-ledger.v1'
    ||
    rustsecAllowedWarnings.length !== 18
    || new Set(rustsecAllowedWarnings).size !== rustsecAllowedWarnings.length
    || rustsecAllowedWarnings.some((id) => !/^RUSTSEC-[0-9]{4}-[0-9]{4}$/u.test(id))
    || rustsecAllowedWarnings.join('\n') !== [...rustsecAllowedWarnings].sort().join('\n')
  ) throw new Error('RustSec warning ledger is invalid')
  const cargoBytes = readFileSync(cargoLockPath, 'utf8')
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
  const cargoLicenseDb = JSON.parse(readFileSync(cargoLicenseDbPath, 'utf8'))
  if (
    cargoLicenseDb.schema !== 'origami2.cargo-license-db.v1'
    || cargoLicenseDb.cargoLockSha256 !== digest(cargoLockPath)
    || !Array.isArray(cargoLicenseDb.packages)
    || cargoLicenseDb.packages.length !== cargoPackages.length
  ) throw new Error('Cargo license database is stale or invalid')
  const lockedCargoIdentities = cargoPackages.map((entry) => {
    const name = /^name = "([^"]+)"$/mu.exec(entry)?.[1]
    const version = /^version = "([^"]+)"$/mu.exec(entry)?.[1]
    return `${name}@${version}`
  }).sort()
  const licensedCargoIdentities = cargoLicenseDb.packages.map((entry) => {
    if (typeof entry?.package !== 'string' || typeof entry?.license !== 'string') {
      throw new Error('Cargo license database entry is incomplete')
    }
    const identifiers = entry.license.match(/[A-Za-z0-9][A-Za-z0-9.-]*/gu) ?? []
    if (identifiers.some((id) => !['AND', 'OR', 'WITH'].includes(id) && !cargoLicenseAllowlist.has(id))) {
      throw new Error(`Cargo dependency license is not allowed: ${entry.package}`)
    }
    const lockEntry = cargoPackages.find((candidate) => (
      /^name = "([^"]+)"$/mu.exec(candidate)?.[1]
      + '@' + /^version = "([^"]+)"$/mu.exec(candidate)?.[1]
    ) === entry.package)
    const expectedSource = /^source = "([^"]+)"$/mu.exec(lockEntry ?? '')?.[1] ?? null
    const expectedChecksum = /^checksum = "([0-9a-f]{64})"$/mu.exec(lockEntry ?? '')?.[1] ?? null
    if (entry.source !== expectedSource || entry.checksum !== expectedChecksum) {
      throw new Error(`Cargo source provenance mismatch: ${entry.package}`)
    }
    if (entry.source !== null && (
      entry.source !== 'registry+https://github.com/rust-lang/crates.io-index'
      || !/^[0-9a-f]{64}$/u.test(entry.checksum ?? '')
    )) throw new Error(`Cargo source provenance is not allowed: ${entry.package}`)
    return entry.package
  }).sort()
  if (lockedCargoIdentities.join('\n') !== licensedCargoIdentities.join('\n')) {
    throw new Error('Cargo license database does not cover the complete lockfile')
  }

  const packageLock = JSON.parse(readFileSync(packageLockPath, 'utf8'))
  const packageManifest = JSON.parse(readFileSync(packageManifestPath, 'utf8'))
  if (packageLock.lockfileVersion !== 3 || packageLock.requires !== true) {
    throw new Error('npm lockfile policy requires lockfileVersion 3')
  }
  const lockedRoot = packageLock.packages?.['']
  const canonicalRoot = (value) => JSON.stringify({
    name: value?.name,
    version: value?.version,
    dependencies: value?.dependencies ?? {},
    devDependencies: value?.devDependencies ?? {},
  })
  if (canonicalRoot(lockedRoot) !== canonicalRoot(packageManifest)) {
    throw new Error('npm package manifest and lockfile root are out of sync')
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
    let resolved
    try {
      resolved = new URL(pkg.resolved)
    } catch {
      throw new Error(`npm dependency source is missing or invalid: ${path}`)
    }
    if (
      resolved.protocol !== 'https:'
      || resolved.hostname !== 'registry.npmjs.org'
      || resolved.port !== ''
      || resolved.username !== ''
      || resolved.password !== ''
      || resolved.search !== ''
      || resolved.hash !== ''
      || !resolved.pathname.endsWith('.tgz')
    ) {
      throw new Error(`npm dependency source is not allowed: ${path}`)
    }
  }
  const thirdPartyNotices = npmPackages.map(([path, pkg]) => ({
    package: path.replace(/^node_modules\//u, ''),
    version: pkg.version,
    license: pkg.license,
    resolved: pkg.resolved,
    integrity: pkg.integrity,
  })).sort((left, right) => left.package.localeCompare(right.package))
  if (thirdPartyNotices.some((notice) => (
    !notice.package || typeof notice.version !== 'string' || !notice.version
  ))) throw new Error('npm third-party notice inventory is incomplete')

  return {
    schema: 'origami2.dependency-policy.v1',
    policy: 'locked-integrity-and-license-allowlist',
    cargoLockSha256: digest(cargoLockPath),
    packageLockSha256: digest(packageLockPath),
    cargoPackages: cargoPackages.length,
    cargoRegistryPackages,
    npmPackages: npmPackages.length,
    npmIntegrity: 'sha512-required',
    npmLicenses: [...new Set(npmPackages.map(([, pkg]) => pkg.license))].sort(),
    licenseDatabase: {
      schema: 'origami2.lockfile-license-db.v1',
      source: 'apps/desktop/package-lock.json',
      sha256: digest(packageLockPath),
    },
    thirdPartyNotices,
    cargoSources: 'registry-checksum-required;git-forbidden',
    cargoLicenseDatabase: cargoLicenseDb,
    vulnerabilityAssessment: {
      status: 'ci-gated',
      npm: 'npm-audit-v2;node-24.11.1;audit-level-low',
      cargo: 'cargo-audit-0.22.2;rustsec-db-b5fc89b8be99e96f79194d8a6f11e9b4143b99f0;offline',
      rustsecAllowedWarnings,
      rustsecWarningLedger,
      rustsecReviewReport: buildRustsecReviewReport(),
      scope: 'package-lock.json;Cargo.lock',
    },
    result: 'pass',
  }
}

if (process.argv[1]?.endsWith('dependency_policy.mjs')) {
  const bytes = `${JSON.stringify(buildDependencyPolicy())}\n`
  if (process.argv[2]) writeFileSync(process.argv[2], bytes, 'utf8')
  else process.stdout.write(bytes)
}
