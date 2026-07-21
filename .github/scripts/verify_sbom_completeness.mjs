import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { buildDependencyPolicy } from './dependency_policy.mjs'

const path = resolve(process.argv[2] ?? '')
const sbom = JSON.parse(readFileSync(path, 'utf8'))
if (sbom.bomFormat !== 'CycloneDX' || !Array.isArray(sbom.components)) throw new Error('invalid CycloneDX SBOM')
const actual = new Set(sbom.components.map((component) => {
  const name = typeof component?.group === 'string' && component.group.length > 0
    ? `${component.group}/${component.name}`
    : component?.name
  return `${name}@${component?.version}`
}))
const policy = buildDependencyPolicy()
const required = [
  ...policy.thirdPartyNotices.map(({ package: name, version }) => `${name}@${version}`),
  ...policy.cargoLicenseDatabase.packages.map(({ package: name }) => name),
]
const missing = [...new Set(required)].filter((identity) => !actual.has(identity))
if (missing.length > 0) throw new Error(`CycloneDX SBOM omits locked dependencies: ${missing.slice(0, 20).join(', ')}`)
if (new Set(sbom.components.map((component) => component?.['bom-ref']).filter(Boolean)).size
  !== sbom.components.map((component) => component?.['bom-ref']).filter(Boolean).length) {
  throw new Error('CycloneDX SBOM contains duplicate bom-ref values')
}
process.stdout.write(`CycloneDX SBOM covers ${new Set(required).size} locked dependency identities\n`)
