import { normalizeUpdateFeed, validateUpgrade } from './update_compatibility_contract.mjs'

const hash = 'a'.repeat(64)
const files = (version, platform) => [
  { name: `ORIGAMI2-v${version}-${platform}-payload.bin`, sha256: hash },
  { name: `ORIGAMI2-v${version}-${platform}-metadata.cdx.json`, sha256: 'b'.repeat(64) },
]
const legacy = (version, platform, target) => ({
  version, platform, target, unsigned: true, files: files(version, platform),
})
const current = (version, platform, signaturePolicy = 'unsigned-dry-run') => ({
  schema: 'origami2.update-manifest.v1', version, platform, signaturePolicy,
  assets: files(version, platform),
})
const reject = (callback, pattern) => {
  try { callback() } catch (error) {
    if (pattern.test(String(error))) return
    throw error
  }
  throw new Error(`fixture unexpectedly accepted: ${pattern}`)
}

const windowsLegacy = legacy('1.9.9', 'windows-x64', 'x86_64-pc-windows-msvc')
const migrated = normalizeUpdateFeed(windowsLegacy)
if (migrated.schema !== 'origami2.update-manifest.v1') throw new Error('legacy migration failed')
validateUpgrade(windowsLegacy, current('2.0.0', 'windows-x64'))
validateUpgrade(
  legacy('1.9.9', 'macos-arm64', 'aarch64-apple-darwin'),
  current('2.0.0', 'macos-arm64', 'platform-signed'),
)
reject(() => validateUpgrade(current('2.0.0', 'windows-x64'), current('1.9.9', 'windows-x64')), /rollback/u)
reject(() => validateUpgrade(current('2.0.0', 'windows-x64'), current('2.0.0', 'windows-x64')), /same-version/u)
reject(() => normalizeUpdateFeed(legacy('1.0.0', 'windows-x64', 'aarch64-apple-darwin')), /mapping/u)
reject(() => normalizeUpdateFeed(current('1.0.0', 'linux-x64')), /platform/u)
reject(() => normalizeUpdateFeed({ ...current('1.0.0', 'windows-x64'), extra: true }), /fields/u)
reject(() => normalizeUpdateFeed({ ...current('01.0.0', 'windows-x64') }), /version/u)
reject(() => normalizeUpdateFeed({ ...current('1.0.0', 'windows-x64'), signaturePolicy: 'none' }), /signature/u)
reject(() => normalizeUpdateFeed({ ...current('1.0.0', 'windows-x64'), assets: [{ ...files('1.0.0', 'windows-x64')[0], sha256: 'A'.repeat(64) }, files('1.0.0', 'windows-x64')[1]] }), /checksum/u)
reject(() => normalizeUpdateFeed({ ...current('1.0.0', 'windows-x64'), assets: [{ name: 'ORIGAMI2-v1.0.0-windows-x64-../evil', sha256: hash }, files('1.0.0', 'windows-x64')[1]] }), /unsafe/u)
for (const malformed of ['', '{', 'null', '[]']) {
  reject(() => normalizeUpdateFeed(JSON.parse(malformed)), /plain object|Unexpected|JSON/u)
}
console.log('update feed compatibility, migration, and rollback fixtures verified')
