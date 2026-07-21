import {
  isUpdateCheckSettingsSnapshot,
  type UpdateCheckSettingsSnapshot,
} from './updateCheckSettings.ts'

export const ORIGAMI2_GITHUB_RELEASES_API_URL =
  'https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/latest'
export const ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX =
  'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/'
export const UPDATE_CHECK_TIMEOUT_MS = 10_000
export const MAX_GITHUB_RELEASE_RESPONSE_BYTES = 128 * 1024

const MAX_SEMANTIC_VERSION_CODE_UNITS = 128
const MAX_SEMANTIC_VERSION_IDENTIFIERS = 32
const MAX_RELEASE_URL_CODE_UNITS = 512
const MAX_RELEASE_ROOT_FIELDS = 128
const MAX_CONTENT_TYPE_CODE_UNITS = 128
const UTF8_ENCODER = new TextEncoder()

type ParsedSemanticVersion = Readonly<{
  canonical: string
  core: readonly [string, string, string]
  prerelease: readonly string[]
}>

export type GitHubLatestRelease = Readonly<{
  version: string
  releasePageUrl: string
}>

export type UpdateCheckTransportResponse = Readonly<{
  status: number
  contentType: string | null
  body: string
  finalUrl: string
  redirected: boolean
}>

export type UpdateCheckTransport = Readonly<{
  requestLatestRelease: () => unknown
}>

export type UpdateCheckUnavailableReason =
  | 'invalid_settings'
  | 'invalid_current_version'
  | 'network_unavailable'
  | 'no_published_release'
  | 'service_unavailable'
  | 'response_too_large'
  | 'invalid_response'

export type UpdateCheckResult =
  | Readonly<{ kind: 'disabled' }>
  | Readonly<{
    kind: 'up_to_date'
    currentVersion: string
    latestVersion: string
  }>
  | Readonly<{
    kind: 'update_available'
    currentVersion: string
    latestVersion: string
    releasePageUrl: string
  }>
  | Readonly<{
    kind: 'unavailable'
    reason: UpdateCheckUnavailableReason
  }>

export type UpdateCheckClient = Readonly<{
  /**
   * Performs exactly one user-initiated check. The client has no scheduler,
   * startup hook, downloader, installer, telemetry, or project-data input.
   */
  checkNow: (
    currentVersion: unknown,
    settings: unknown,
  ) => Promise<UpdateCheckResult>
}>

export type UpdateCheckFetch = (
  input: string,
  init: RequestInit,
) => Promise<Response>

export type UpdateCheckTimeoutClock = Readonly<{
  setTimeout: (callback: () => void, delayMs: number) => unknown
  clearTimeout: (handle: unknown) => void
}>

export type GitHubReleasesFetchTransportOptions = Readonly<{
  fetch?: UpdateCheckFetch
  clock?: UpdateCheckTimeoutClock
}>

class BoundedUpdateCheckTransportError extends Error {
  readonly reason: 'network_unavailable' | 'response_too_large'

  constructor(reason: 'network_unavailable' | 'response_too_large') {
    super('update check transport unavailable')
    this.name = 'BoundedUpdateCheckTransportError'
    this.reason = reason
  }
}

/**
 * Creates the production HTTP boundary. The caller cannot supply an owner,
 * repository, URL, request body, credentials, authorization token, referrer,
 * or redirect target.
 */
export function createGitHubReleasesFetchTransport(
  options: GitHubReleasesFetchTransportOptions = {},
): UpdateCheckTransport {
  const fetchRequest: UpdateCheckFetch = options.fetch
    ?? ((input, init) => globalThis.fetch(input, init))
  const clock = options.clock ?? defaultTimeoutClock

  return Object.freeze({
    async requestLatestRelease() {
      const controller = new AbortController()
      let timeoutHandle: unknown
      try {
        timeoutHandle = clock.setTimeout(
          () => controller.abort(),
          UPDATE_CHECK_TIMEOUT_MS,
        )
      } catch {
        throw new BoundedUpdateCheckTransportError('network_unavailable')
      }

      try {
        let response: Response
        try {
          response = await fetchRequest(
            ORIGAMI2_GITHUB_RELEASES_API_URL,
            {
              method: 'GET',
              headers: Object.freeze({
                Accept: 'application/vnd.github+json',
              }),
              body: null,
              cache: 'no-store',
              credentials: 'omit',
              redirect: 'error',
              referrerPolicy: 'no-referrer',
              signal: controller.signal,
            },
          )
        } catch {
          throw new BoundedUpdateCheckTransportError('network_unavailable')
        }

        return Object.freeze({
          status: response.status,
          contentType: response.headers.get('content-type'),
          body: await readBoundedResponseBody(response),
          finalUrl: response.url,
          redirected: response.redirected,
        })
      } catch (error) {
        if (isBoundedTransportError(error)) throw error
        throw new BoundedUpdateCheckTransportError('network_unavailable')
      } finally {
        try {
          clock.clearTimeout(timeoutHandle)
        } catch {
          // The completed request remains bounded by its settled promise.
        }
      }
    },
  })
}

/**
 * Builds a manual-only update client. Neither the installed version nor the
 * setting is passed to the transport; the request therefore cannot contain
 * project state, usage state, locale, file paths, or the installed version.
 */
export function createUpdateCheckClient(
  transport: UpdateCheckTransport,
): UpdateCheckClient {
  return Object.freeze({
    async checkNow(
      currentVersionValue: unknown,
      settingsValue: unknown,
    ): Promise<UpdateCheckResult> {
      if (!isUpdateCheckSettingsSnapshot(settingsValue)) {
        return unavailable('invalid_settings')
      }
      const settings: UpdateCheckSettingsSnapshot = settingsValue
      if (!settings.enabled) return DISABLED_RESULT

      const currentVersion = parseSemanticVersion(currentVersionValue)
      if (!currentVersion) return unavailable('invalid_current_version')

      let rawResponse: unknown
      try {
        rawResponse = await transport.requestLatestRelease()
      } catch (error) {
        return unavailable(
          isBoundedTransportError(error)
            ? error.reason
            : 'network_unavailable',
        )
      }

      const response = parseTransportResponse(rawResponse)
      if (response.kind === 'error') return unavailable(response.reason)

      const release = parseGitHubLatestReleaseResponseJson(response.body)
      if (!release) return unavailable('invalid_response')
      const latestVersion = parseSemanticVersion(release.version)
      if (!latestVersion) return unavailable('invalid_response')

      const comparison = compareParsedSemanticVersions(
        currentVersion,
        latestVersion,
      )
      if (comparison >= 0) {
        return Object.freeze({
          kind: 'up_to_date',
          currentVersion: currentVersion.canonical,
          latestVersion: latestVersion.canonical,
        })
      }
      return Object.freeze({
        kind: 'update_available',
        currentVersion: currentVersion.canonical,
        latestVersion: latestVersion.canonical,
        releasePageUrl: release.releasePageUrl,
      })
    },
  })
}

export function parseGitHubLatestReleaseResponseJson(
  body: unknown,
): GitHubLatestRelease | null {
  const bodyStatus = boundedJsonBody(body)
  if (bodyStatus.kind !== 'ready') return null
  try {
    const parsed: unknown = JSON.parse(bodyStatus.body)
    return parseGitHubLatestReleaseResponse(parsed)
  } catch {
    return null
  }
}

/**
 * Admits only the public GitHub fields required by update checking.
 * Release notes, asset URLs, download URLs, author data, and arbitrary fields
 * are never copied into the trusted DTO.
 */
export function parseGitHubLatestReleaseResponse(
  value: unknown,
): GitHubLatestRelease | null {
  try {
    const fields = selectedDataRecord(
      value,
      ['tag_name', 'html_url', 'name', 'body', 'draft', 'prerelease'],
      MAX_RELEASE_ROOT_FIELDS,
    )
    if (
      !fields
      || fields.draft !== false
      || fields.prerelease !== false
      || typeof fields.tag_name !== 'string'
      || typeof fields.html_url !== 'string'
      || typeof fields.name !== 'string'
      || typeof fields.body !== 'string'
      || fields.html_url.length > MAX_RELEASE_URL_CODE_UNITS
    ) return null

    const version = parseSemanticVersion(fields.tag_name)
    if (!version || version.prerelease.length !== 0) return null
    if (
      fields.name !== `ORIGAMI2 ${fields.tag_name}`
      || fields.body.startsWith('## QUARANTINED RELEASE')
      || fields.body.includes('origami2-release-owner-sha256:')
    ) return null
    const releasePageUrl = trustedReleasePageUrl(
      fields.html_url,
      fields.tag_name,
    )
    if (!releasePageUrl) return null
    return Object.freeze({
      version: version.canonical,
      releasePageUrl,
    })
  } catch {
    return null
  }
}

/**
 * Compares SemVer 2.0 values, accepting an optional conventional lowercase
 * `v` prefix. Build metadata is ignored. Returns null for malformed input.
 */
export function compareSemanticVersions(
  left: unknown,
  right: unknown,
): -1 | 0 | 1 | null {
  const parsedLeft = parseSemanticVersion(left)
  const parsedRight = parseSemanticVersion(right)
  return parsedLeft && parsedRight
    ? compareParsedSemanticVersions(parsedLeft, parsedRight)
    : null
}

function parseTransportResponse(
  value: unknown,
):
  | Readonly<{ kind: 'ready'; body: string }>
  | Readonly<{
    kind: 'error'
    reason:
      | 'no_published_release'
      | 'service_unavailable'
      | 'response_too_large'
      | 'invalid_response'
  }> {
  try {
    const record = exactDataRecord(value, [
      'status',
      'contentType',
      'body',
      'finalUrl',
      'redirected',
    ])
    if (
      !record
      || typeof record.status !== 'number'
      || !Number.isInteger(record.status)
      || record.status < 100
      || record.status > 599
    ) return Object.freeze({ kind: 'error', reason: 'invalid_response' })
    if (
      record.finalUrl !== ORIGAMI2_GITHUB_RELEASES_API_URL
      || record.redirected !== false
    ) return Object.freeze({ kind: 'error', reason: 'invalid_response' })
    if (record.status === 404) {
      return Object.freeze({
        kind: 'error',
        reason: 'no_published_release',
      })
    }
    if (record.status !== 200) {
      return Object.freeze({
        kind: 'error',
        reason: 'service_unavailable',
      })
    }
    if (
      !isTrustedJsonContentType(record.contentType)
      || typeof record.body !== 'string'
    ) return Object.freeze({ kind: 'error', reason: 'invalid_response' })

    const body = boundedJsonBody(record.body)
    if (body.kind === 'too_large') {
      return Object.freeze({
        kind: 'error',
        reason: 'response_too_large',
      })
    }
    if (body.kind !== 'ready') {
      return Object.freeze({ kind: 'error', reason: 'invalid_response' })
    }
    return Object.freeze({ kind: 'ready', body: body.body })
  } catch {
    return Object.freeze({ kind: 'error', reason: 'invalid_response' })
  }
}

function boundedJsonBody(
  value: unknown,
):
  | Readonly<{ kind: 'ready'; body: string }>
  | Readonly<{ kind: 'invalid' }>
  | Readonly<{ kind: 'too_large' }> {
  if (typeof value !== 'string' || value.length === 0) {
    return Object.freeze({ kind: 'invalid' })
  }
  if (value.length > MAX_GITHUB_RELEASE_RESPONSE_BYTES) {
    return Object.freeze({ kind: 'too_large' })
  }
  let byteLength: number
  try {
    byteLength = UTF8_ENCODER.encode(value).byteLength
  } catch {
    return Object.freeze({ kind: 'invalid' })
  }
  return byteLength <= MAX_GITHUB_RELEASE_RESPONSE_BYTES
    ? Object.freeze({ kind: 'ready', body: value })
    : Object.freeze({ kind: 'too_large' })
}

function parseSemanticVersion(value: unknown): ParsedSemanticVersion | null {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > MAX_SEMANTIC_VERSION_CODE_UNITS
  ) return null
  const source = value.startsWith('v') ? value.slice(1) : value
  const match = /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/u
    .exec(source)
  if (!match) return null
  const major = match[1]
  const minor = match[2]
  const patch = match[3]
  if (major === undefined || minor === undefined || patch === undefined) {
    return null
  }
  const prerelease = match[4]?.split('.') ?? []
  const build = match[5]?.split('.') ?? []
  if (
    prerelease.length > MAX_SEMANTIC_VERSION_IDENTIFIERS
    || build.length > MAX_SEMANTIC_VERSION_IDENTIFIERS
    || prerelease.some((identifier) =>
      isNumericIdentifier(identifier)
      && identifier.length > 1
      && identifier.startsWith('0'))
  ) return null

  const canonical = `${major}.${minor}.${patch}${
    prerelease.length > 0 ? `-${prerelease.join('.')}` : ''
  }${build.length > 0 ? `+${build.join('.')}` : ''}`
  const core: readonly [string, string, string] = Object.freeze([
    major,
    minor,
    patch,
  ])
  return Object.freeze({
    canonical,
    core,
    prerelease: Object.freeze(prerelease),
  })
}

function compareParsedSemanticVersions(
  left: ParsedSemanticVersion,
  right: ParsedSemanticVersion,
): -1 | 0 | 1 {
  for (let index = 0; index < 3; index += 1) {
    const comparison = compareNumericIdentifiers(
      left.core[index] ?? '0',
      right.core[index] ?? '0',
    )
    if (comparison !== 0) return comparison
  }

  if (
    left.prerelease.length === 0
    || right.prerelease.length === 0
  ) {
    if (left.prerelease.length === right.prerelease.length) return 0
    return left.prerelease.length === 0 ? 1 : -1
  }

  const commonLength = Math.min(
    left.prerelease.length,
    right.prerelease.length,
  )
  for (let index = 0; index < commonLength; index += 1) {
    const leftIdentifier = left.prerelease[index] ?? ''
    const rightIdentifier = right.prerelease[index] ?? ''
    if (leftIdentifier === rightIdentifier) continue
    const leftNumeric = isNumericIdentifier(leftIdentifier)
    const rightNumeric = isNumericIdentifier(rightIdentifier)
    if (leftNumeric && rightNumeric) {
      return compareNumericIdentifiers(leftIdentifier, rightIdentifier)
    }
    if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1
    return leftIdentifier < rightIdentifier ? -1 : 1
  }
  return left.prerelease.length === right.prerelease.length
    ? 0
    : left.prerelease.length < right.prerelease.length ? -1 : 1
}

function compareNumericIdentifiers(
  left: string,
  right: string,
): -1 | 0 | 1 {
  if (left.length !== right.length) return left.length < right.length ? -1 : 1
  if (left === right) return 0
  return left < right ? -1 : 1
}

function isNumericIdentifier(value: string) {
  return /^[0-9]+$/u.test(value)
}

function trustedReleasePageUrl(
  value: string,
  expectedTagName: string,
): string | null {
  try {
    const parsed = new URL(value)
    const expectedPathPrefix =
      '/oltotlo79-rgb/ORIGAMI2/releases/tag/'
    if (
      parsed.protocol !== 'https:'
      || parsed.hostname !== 'github.com'
      || parsed.port !== ''
      || parsed.username !== ''
      || parsed.password !== ''
      || parsed.search !== ''
      || parsed.hash !== ''
      || !parsed.pathname.startsWith(expectedPathPrefix)
    ) return null
    const encodedTag = parsed.pathname.slice(expectedPathPrefix.length)
    if (
      encodedTag.length === 0
      || encodedTag.includes('/')
      || decodeURIComponent(encodedTag) !== expectedTagName
    ) return null
    return `${ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX}${
      encodeURIComponent(expectedTagName)
    }`
  } catch {
    return null
  }
}

function isTrustedJsonContentType(value: unknown): boolean {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > MAX_CONTENT_TYPE_CODE_UNITS
  ) return false
  const [mediaType, ...parameters] = value
    .toLowerCase()
    .split(';')
    .map((part) => part.trim())
  if (
    mediaType !== 'application/json'
    && mediaType !== 'application/vnd.github+json'
  ) return false
  return parameters.every((parameter) => parameter === 'charset=utf-8')
}

async function readBoundedResponseBody(response: Response): Promise<string> {
  const contentLength = response.headers.get('content-length')
  if (contentLength !== null) {
    if (!/^(0|[1-9][0-9]*)$/u.test(contentLength)) {
      throw new BoundedUpdateCheckTransportError('network_unavailable')
    }
    if (compareDecimalToLimit(
      contentLength,
      MAX_GITHUB_RELEASE_RESPONSE_BYTES,
    ) > 0) {
      throw new BoundedUpdateCheckTransportError('response_too_large')
    }
  }

  const reader = response.body?.getReader()
  if (!reader) return ''
  const decoder = new TextDecoder('utf-8', { fatal: true })
  const chunks: string[] = []
  let bytesRead = 0
  try {
    while (true) {
      const chunk = await reader.read()
      if (chunk.done) break
      if (!(chunk.value instanceof Uint8Array)) {
        throw new BoundedUpdateCheckTransportError('network_unavailable')
      }
      bytesRead += chunk.value.byteLength
      if (bytesRead > MAX_GITHUB_RELEASE_RESPONSE_BYTES) {
        void reader.cancel().catch(() => undefined)
        throw new BoundedUpdateCheckTransportError('response_too_large')
      }
      chunks.push(decoder.decode(chunk.value, { stream: true }))
    }
    chunks.push(decoder.decode())
    return chunks.join('')
  } catch (error) {
    if (isBoundedTransportError(error)) throw error
    throw new BoundedUpdateCheckTransportError('network_unavailable')
  } finally {
    try {
      reader.releaseLock()
    } catch {
      // Settled ownership does not depend on a hostile stream implementation.
    }
  }
}

function compareDecimalToLimit(
  decimal: string,
  limit: number,
): -1 | 0 | 1 {
  const right = String(limit)
  if (decimal.length !== right.length) return decimal.length < right.length ? -1 : 1
  if (decimal === right) return 0
  return decimal < right ? -1 : 1
}

function isBoundedTransportError(
  value: unknown,
): value is BoundedUpdateCheckTransportError {
  try {
    return value instanceof BoundedUpdateCheckTransportError
      && (
        value.reason === 'network_unavailable'
        || value.reason === 'response_too_large'
      )
  } catch {
    return false
  }
}

function unavailable(
  reason: UpdateCheckUnavailableReason,
): UpdateCheckResult {
  return Object.freeze({ kind: 'unavailable', reason })
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  if (
    value === null
    || typeof value !== 'object'
    || Array.isArray(value)
  ) return null
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const actualKeys = Reflect.ownKeys(descriptors)
  if (
    actualKeys.length !== keys.length
    || actualKeys.some((key) => typeof key !== 'string')
    || keys.some((key) => !Object.hasOwn(descriptors, key))
  ) return null

  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of keys) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot as Readonly<Record<Keys[number], unknown>>
}

function selectedDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
  maximumFields: number,
): Readonly<Record<Keys[number], unknown>> | null {
  if (
    value === null
    || typeof value !== 'object'
    || Array.isArray(value)
  ) return null
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const ownKeys = Reflect.ownKeys(value)
  if (
    ownKeys.length > maximumFields
    || ownKeys.some((key) => typeof key !== 'string')
  ) return null

  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot as Readonly<Record<Keys[number], unknown>>
}

const DISABLED_RESULT: UpdateCheckResult = Object.freeze({
  kind: 'disabled',
})

const defaultTimeoutClock: UpdateCheckTimeoutClock = Object.freeze({
  setTimeout(callback, delayMs) {
    return globalThis.setTimeout(callback, delayMs)
  },
  clearTimeout(handle) {
    globalThis.clearTimeout(
      handle as ReturnType<typeof globalThis.setTimeout>,
    )
  },
})
