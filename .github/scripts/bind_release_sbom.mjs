import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'

const path = process.argv[2]
const sbom = JSON.parse(readFileSync(path, 'utf8'))
const version = process.env.VERSION
const platform = process.env.PLATFORM
const commit = process.env.RELEASE_COMMIT
const rustc = process.env.RUSTC_VERSION
const node = process.env.NODE_VERSION
if (sbom.bomFormat !== 'CycloneDX' || !Array.isArray(sbom.components)) {
  throw new Error('invalid CycloneDX source SBOM')
}
if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(version ?? '')) {
  throw new Error('invalid SBOM release version')
}
if (!['windows-x64', 'macos-arm64'].includes(platform)) throw new Error('invalid SBOM platform')
if (!/^[0-9a-f]{40}$/u.test(commit ?? '')) throw new Error('invalid SBOM source commit')
if (!/^rustc [0-9]+\.[0-9]+\.[0-9]+/u.test(rustc ?? '')) throw new Error('invalid rustc version')
if (!/^v[0-9]+\.[0-9]+\.[0-9]+/u.test(node ?? '')) throw new Error('invalid Node.js version')

for (const key of ['bom-ref', 'purl']) {
  const values = sbom.components.map((component) => component?.[key]).filter(Boolean)
  if (new Set(values).size !== values.length) throw new Error(`duplicate CycloneDX ${key}`)
}
const digest = (file) => createHash('sha256').update(readFileSync(file)).digest('hex')
const properties = {
  'origami2.build.cargo-lock-sha256': digest('Cargo.lock'),
  'origami2.build.node-version': node,
  'origami2.build.package-lock-sha256': digest('apps/desktop/package-lock.json'),
  'origami2.build.rustc-version': rustc,
  'origami2.release.platform': platform,
  'origami2.release.source-commit': commit,
  'origami2.release.version': version,
}
sbom.metadata = {
  ...(sbom.metadata ?? {}),
  component: { type: 'application', name: 'ORIGAMI2', version },
  properties: Object.entries(properties).map(([name, value]) => ({ name, value })),
}
writeFileSync(path, `${JSON.stringify(sbom)}\n`, 'utf8')
