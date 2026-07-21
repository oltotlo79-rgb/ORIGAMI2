import { compareSemanticVersions } from './githubReleaseUpdate.ts'
import type { ReleasePlatform } from './releaseArtifactCompatibility.ts'

const MAX_MANIFEST_BYTES = 16 * 1024
const SHA256 = /^[0-9a-f]{64}$/u

export type RuntimeUpdateAsset = Readonly<{ name: string; sha256: string }>
export type RuntimeUpdateAuthorization = Readonly<{
  version: string
  platform: ReleasePlatform
  signaturePolicy: 'platform-signed'
  assets: readonly RuntimeUpdateAsset[]
}>
export type RuntimeUpdateManifestTransport = Readonly<{
  requestManifest: (signal?: AbortSignal) => Promise<unknown>
}>
export type RuntimeUpdateManifestResult =
  | Readonly<{ kind: 'authorized'; authorization: RuntimeUpdateAuthorization }>
  | Readonly<{ kind: 'rejected'; reason: 'offline' | 'malformed' | 'rollback' }>

/**
 * Verifies release metadata before any payload downloader receives authority.
 * The transport supplies metadata only; this client retains no payload URL.
 */
export async function authorizeRuntimeUpdate(
  transport: RuntimeUpdateManifestTransport,
  currentVersion: unknown,
  expectedPlatform: ReleasePlatform,
  signal?: AbortSignal,
): Promise<RuntimeUpdateManifestResult> {
  let response: unknown
  try {
    response = await transport.requestManifest(signal)
  } catch {
    return Object.freeze({ kind: 'rejected', reason: 'offline' })
  }
  const authorization = parseRuntimeUpdateManifest(response, expectedPlatform)
  if (!authorization) return Object.freeze({ kind: 'rejected', reason: 'malformed' })
  const comparison = compareSemanticVersions(currentVersion, authorization.version)
  if (comparison === null) return Object.freeze({ kind: 'rejected', reason: 'malformed' })
  if (comparison >= 0) return Object.freeze({ kind: 'rejected', reason: 'rollback' })
  return Object.freeze({ kind: 'authorized', authorization })
}

export function parseRuntimeUpdateManifest(
  body: unknown,
  expectedPlatform: ReleasePlatform,
): RuntimeUpdateAuthorization | null {
  if (typeof body !== 'string' || body.length === 0 || body.length > MAX_MANIFEST_BYTES) return null
  let value: unknown
  try { value = JSON.parse(body) } catch { return null }
  if (!isExactRecord(value, ['schema', 'version', 'platform', 'signaturePolicy', 'assets'])) return null
  if (
    value.schema !== 'origami2.update-manifest.v1'
    || value.platform !== expectedPlatform
    || value.signaturePolicy !== 'platform-signed'
    || typeof value.version !== 'string'
    || !/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(value.version)
    || !Array.isArray(value.assets)
  ) return null
  const prefix = `ORIGAMI2-v${value.version}-${expectedPlatform}`
  const expectedNames = expectedPlatform === 'windows-x64'
    ? [`${prefix}-portable.zip`, `${prefix}-setup.exe`, `${prefix}.cdx.json`]
    : [`${prefix}-app.tar.gz`, `${prefix}.cdx.json`]
  if (value.assets.length !== expectedNames.length) return null
  const assets: RuntimeUpdateAsset[] = []
  for (const candidate of value.assets) {
    if (!isExactRecord(candidate, ['name', 'sha256'])
      || typeof candidate.name !== 'string'
      || typeof candidate.sha256 !== 'string'
      || !SHA256.test(candidate.sha256)) return null
    assets.push(Object.freeze({ name: candidate.name, sha256: candidate.sha256 }))
  }
  assets.sort((left, right) => left.name.localeCompare(right.name, 'en'))
  expectedNames.sort()
  if (assets.some((asset, index) => asset.name !== expectedNames[index])) return null
  return Object.freeze({
    version: value.version,
    platform: expectedPlatform,
    signaturePolicy: 'platform-signed',
    assets: Object.freeze(assets),
  })
}

function isExactRecord(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return false
  const actual = Object.keys(value).sort()
  const expected = [...keys].sort()
  return actual.length === expected.length && actual.every((key, index) => key === expected[index])
}
