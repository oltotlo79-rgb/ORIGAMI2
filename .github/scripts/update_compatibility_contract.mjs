const PLATFORM_TARGETS = Object.freeze({
  'windows-x64': 'x86_64-pc-windows-msvc',
  'macos-arm64': 'aarch64-apple-darwin',
})
const V1_KEYS = ['schema', 'version', 'platform', 'signaturePolicy', 'assets']
const LEGACY_KEYS = ['version', 'platform', 'target', 'unsigned', 'files']

export function compareStableVersions(left, right) {
  const a = parseVersion(left)
  const b = parseVersion(right)
  if (!a || !b) throw new Error('update version is malformed')
  for (let index = 0; index < 3; index += 1) {
    if (a[index] !== b[index]) return a[index] < b[index] ? -1 : 1
  }
  return 0
}

export function normalizeUpdateFeed(value) {
  const record = exactRecord(value)
  if (!record) throw new Error('update feed must be a plain object')
  if (record.schema === 'origami2.update-manifest.v1') return validateV1(record)
  if (Object.hasOwn(record, 'schema')) throw new Error('unsupported update feed schema')
  requireExactKeys(record, LEGACY_KEYS)
  if (record.unsigned !== true) throw new Error('legacy signature metadata is invalid')
  if (record.target !== PLATFORM_TARGETS[record.platform]) {
    throw new Error('legacy platform/architecture mapping is invalid')
  }
  return validateV1({
    schema: 'origami2.update-manifest.v1', version: record.version,
    platform: record.platform, signaturePolicy: 'unsigned-dry-run',
    assets: record.files,
  })
}

export function validateUpgrade(previousValue, nextValue) {
  const previous = normalizeUpdateFeed(previousValue)
  const next = normalizeUpdateFeed(nextValue)
  if (previous.platform !== next.platform) throw new Error('update platform changed')
  if (compareStableVersions(previous.version, next.version) >= 0) {
    throw new Error('update rollback or same-version replacement rejected')
  }
  return Object.freeze({ previous, next, targetTriple: PLATFORM_TARGETS[next.platform] })
}

function validateV1(record) {
  requireExactKeys(record, V1_KEYS)
  const version = parseVersion(record.version)
  if (!version) throw new Error('update version is malformed')
  if (!Object.hasOwn(PLATFORM_TARGETS, record.platform)) throw new Error('update platform is unsupported')
  if (!['platform-signed', 'unsigned-dry-run'].includes(record.signaturePolicy)) {
    throw new Error('update signature metadata is invalid')
  }
  if (!Array.isArray(record.assets) || record.assets.length < 2 || record.assets.length > 3) {
    throw new Error('update assets are invalid')
  }
  const names = new Set()
  const prefix = `ORIGAMI2-v${record.version}-${record.platform}`
  const assets = record.assets.map((candidate) => {
    const asset = exactRecord(candidate)
    if (!asset) throw new Error('update asset is malformed')
    requireExactKeys(asset, ['name', 'sha256'])
    if (typeof asset.name !== 'string' || !asset.name.startsWith(`${prefix}-`) || asset.name.includes('/') || asset.name.includes('\\') || names.has(asset.name)) {
      throw new Error('update asset name is unsafe or inconsistent')
    }
    if (typeof asset.sha256 !== 'string' || !/^[0-9a-f]{64}$/u.test(asset.sha256)) {
      throw new Error('update checksum metadata is invalid')
    }
    names.add(asset.name)
    return Object.freeze({ name: asset.name, sha256: asset.sha256 })
  })
  return Object.freeze({
    schema: 'origami2.update-manifest.v1', version: record.version,
    platform: record.platform, signaturePolicy: record.signaturePolicy,
    assets: Object.freeze(assets),
  })
}

function exactRecord(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null ? value : null
}

function requireExactKeys(record, expected) {
  const keys = Object.keys(record).sort()
  const wanted = [...expected].sort()
  if (keys.length !== wanted.length || keys.some((key, index) => key !== wanted[index])) {
    throw new Error('update feed fields are malformed')
  }
}

function parseVersion(value) {
  const match = /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.exec(value ?? '')
  return match ? match.slice(1).map((part) => BigInt(part)) : null
}
