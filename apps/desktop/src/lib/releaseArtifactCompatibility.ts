export const RELEASE_ARTIFACT_CONTRACT_SCHEMA =
  'origami2.release-artifact-compatibility.v1' as const

export type ReleasePlatform = 'windows-x64' | 'macos-arm64'

export type ReleaseArtifactPlan = Readonly<{
  schema: typeof RELEASE_ARTIFACT_CONTRACT_SCHEMA
  version: string
  platform: ReleasePlatform
  payloadName: string
  supplementalPayloadName: string | null
  checksumManifestName: string
  sbomName: string
  updateManifestName: string
  signatureVerification: 'authenticode' | 'apple_codesign'
  provenanceAttestationRequired: true
  userConfirmationRequired: true
}>

const MAX_ASSET_COUNT = 32
const MAX_ASSET_NAME_CODE_UNITS = 180

/**
 * Selects names only from GitHub Releases metadata. It deliberately retains no
 * download URL and performs no network request, download, signature check, or
 * installation. Those checks remain on the reviewed release page and require
 * an explicit user decision.
 */
export function selectReleaseArtifactPlan(
  version: unknown,
  platform: unknown,
  assetNames: unknown,
): ReleaseArtifactPlan | null {
  if (!isStableVersion(version) || !isPlatform(platform)) return null
  const admittedNames = admitAssetNames(assetNames)
  if (!admittedNames) return null

  const prefix = `ORIGAMI2-v${version}-${platform}`
  const payloadName = platform === 'windows-x64'
    ? `${prefix}-setup.exe`
    : `${prefix}-app.tar.gz`
  const supplementalPayloadName = platform === 'windows-x64'
    ? `${prefix}-portable.zip`
    : null
  const checksumManifestName = `SHA256SUMS-${platform}.txt`
  const sbomName = `${prefix}.cdx.json`
  const updateManifestName = `${prefix}.update.json`
  const required = [
    payloadName,
    checksumManifestName,
    sbomName,
    updateManifestName,
    ...(supplementalPayloadName ? [supplementalPayloadName] : []),
  ]
  const completeReleaseSet = [
    `ORIGAMI2-v${version}-windows-x64-setup.exe`,
    `ORIGAMI2-v${version}-windows-x64-portable.zip`,
    `ORIGAMI2-v${version}-windows-x64.cdx.json`,
    `ORIGAMI2-v${version}-windows-x64.update.json`,
    `SHA256SUMS-windows-x64.txt`,
    `ORIGAMI2-v${version}-macos-arm64-app.tar.gz`,
    `ORIGAMI2-v${version}-macos-arm64.cdx.json`,
    `ORIGAMI2-v${version}-macos-arm64.update.json`,
    `SHA256SUMS-macos-arm64.txt`,
  ]
  if (
    admittedNames.size !== completeReleaseSet.length
    || completeReleaseSet.some((name) => !admittedNames.has(name))
    || required.some((name) => !admittedNames.has(name))
  ) return null

  return Object.freeze({
    schema: RELEASE_ARTIFACT_CONTRACT_SCHEMA,
    version,
    platform,
    payloadName,
    supplementalPayloadName,
    checksumManifestName,
    sbomName,
    updateManifestName,
    signatureVerification: platform === 'windows-x64'
      ? 'authenticode'
      : 'apple_codesign',
    provenanceAttestationRequired: true,
    userConfirmationRequired: true,
  })
}

function admitAssetNames(value: unknown): ReadonlySet<string> | null {
  if (!Array.isArray(value) || value.length > MAX_ASSET_COUNT) return null
  const names = new Set<string>()
  for (const name of value) {
    if (
      typeof name !== 'string'
      || name.length === 0
      || name.length > MAX_ASSET_NAME_CODE_UNITS
      || name.includes('/')
      || name.includes('\\')
      || names.has(name)
    ) return null
    names.add(name)
  }
  return names
}

function isPlatform(value: unknown): value is ReleasePlatform {
  return value === 'windows-x64' || value === 'macos-arm64'
}

function isStableVersion(value: unknown): value is string {
  return typeof value === 'string'
    && /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(value)
}
