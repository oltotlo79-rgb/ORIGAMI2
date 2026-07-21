import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import test from 'node:test'

import {
  MAX_GITHUB_RELEASE_RESPONSE_BYTES,
  ORIGAMI2_GITHUB_RELEASES_API_URL,
  ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX,
  UPDATE_CHECK_TIMEOUT_MS,
  compareSemanticVersions,
  createGitHubReleasesFetchTransport,
  createUpdateCheckClient,
  parseGitHubLatestReleaseResponse,
  parseGitHubLatestReleaseResponseJson,
  type UpdateCheckFetch,
  type UpdateCheckTimeoutClock,
  type UpdateCheckTransport,
  type UpdateCheckTransportResponse,
} from '../src/lib/githubReleaseUpdate.ts'
import {
  DEFAULT_UPDATE_CHECK_SETTINGS,
  DISABLED_UPDATE_CHECK_SETTINGS,
} from '../src/lib/updateCheckSettings.ts'

test('the network and release-page authorities are fixed to the official repository', () => {
  assert.equal(
    ORIGAMI2_GITHUB_RELEASES_API_URL,
    'https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/latest',
  )
  assert.equal(
    ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX,
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/',
  )
  assert.equal(UPDATE_CHECK_TIMEOUT_MS, 10_000)
  assert.equal(MAX_GITHUB_RELEASE_RESPONSE_BYTES, 128 * 1024)
})

test('the parser retains only a stable version and its official release page', () => {
  const parsed = parseGitHubLatestReleaseResponse(release({
    body: 'Private project path C:\\Users\\alice\\secret.ori',
    author: { login: 'arbitrary-author' },
    assets: Array.from({ length: 9 }, () => ({
      browser_download_url: 'https://evil.example/payload.exe',
      signature: 'forged-native-updater-signature',
    })),
    tarball_url: 'https://evil.example/archive',
    zipball_url: 'https://evil.example/archive',
  }))

  assert.deepEqual(parsed, {
    version: '1.2.3',
    releasePageUrl:
      'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
  })
  assert.equal(Object.isFrozen(parsed), true)
  assert.deepEqual(Object.keys(parsed ?? {}), [
    'version',
    'releasePageUrl',
  ])
  assert.doesNotMatch(JSON.stringify(parsed), /alice|evil|payload|signature/iu)
})

test('draft prerelease inconsistent and malformed versions are rejected', () => {
  for (const candidate of [
    release({ draft: true }),
    release({ prerelease: true }),
    release({
      tag_name: 'v1.2.3-alpha.1',
      html_url:
        'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3-alpha.1',
    }),
    release({ tag_name: 'latest' }),
    release({ tag_name: 'v01.2.3' }),
    release({ tag_name: 'v1.2' }),
    release({ tag_name: 'V1.2.3' }),
    release({ tag_name: 'v1.2.3/../../evil' }),
    release({ draft: 'false' }),
    release({ prerelease: 0 }),
    release({ name: 'QUARANTINED v1.2.3' }),
    release({ name: 'ORIGAMI2 v9.9.9' }),
    release({ body: '## QUARANTINED RELEASE\n\nDo not install.' }),
    release({ body: '<!-- origami2-release-owner-sha256:abc123 -->' }),
    release({ body: 'x'.repeat(100_001) }),
    release({ assets: [] }),
    release({ assets: Array.from({ length: 10 }, () => ({})) }),
    release({ tag_name: 'ｖ1.2.3' }),
    release({ name: 'ＯＲＩＧＡＭＩ２ v1.2.3' }),
  ]) {
    assert.equal(parseGitHubLatestReleaseResponse(candidate), null)
  }
})

test('native and frontend dependency surfaces contain no automatic updater', () => {
  const desktop = resolve(import.meta.dirname, '..')
  for (const path of [
    resolve(desktop, 'package.json'),
    resolve(desktop, 'src-tauri', 'Cargo.toml'),
    resolve(desktop, 'src-tauri', 'tauri.conf.json'),
  ]) {
    assert.doesNotMatch(readFileSync(path, 'utf8'), /tauri-plugin-updater|@tauri-apps\/plugin-updater|"updater"/iu)
  }
})

test('release links reject every authority path and tag substitution', () => {
  for (const html_url of [
    'http://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    'https://evil.example/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    'https://github.com.evil.example/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    'https://alice@github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    'https://github.com:444/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    'https://github.com/other/ORIGAMI2/releases/tag/v1.2.3',
    'https://github.com/oltotlo79-rgb/other/releases/tag/v1.2.3',
    'https://github.com/oltotlo79-rgb/origami2/releases/tag/v1.2.3',
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v9.9.9',
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3/extra',
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3?download=1',
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3#notes',
    `https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/${
      'x'.repeat(600)
    }`,
  ]) {
    assert.equal(
      parseGitHubLatestReleaseResponse(release({ html_url })),
      null,
      html_url,
    )
  }
})

test('the release boundary rejects accessors proxies symbols and excessive fields', () => {
  let getterRead = false
  const accessor = release()
  Object.defineProperty(accessor, 'tag_name', {
    enumerable: true,
    get() {
      getterRead = true
      return 'v1.2.3'
    },
  })
  assert.equal(parseGitHubLatestReleaseResponse(accessor), null)
  assert.equal(getterRead, false)

  assert.equal(
    parseGitHubLatestReleaseResponse(new Proxy({}, {
      getPrototypeOf() {
        throw new Error('C:\\Users\\alice\\private.ori')
      },
    })),
    null,
  )
  assert.equal(
    parseGitHubLatestReleaseResponse(Object.assign(
      release(),
      { [Symbol('private')]: 'secret' },
    )),
    null,
  )
  assert.equal(
    parseGitHubLatestReleaseResponse(
      Object.assign(
        release(),
        Object.fromEntries(
          Array.from({ length: 125 }, (_, index) => [`extra_${index}`, index]),
        ),
      ),
    ),
    null,
  )

  const acceptedLimit = Object.assign(
    release(),
    Object.fromEntries(
      Array.from({ length: 121 }, (_, index) => [`extra_${index}`, index]),
    ),
  )
  assert.ok(parseGitHubLatestReleaseResponse(acceptedLimit))
})

test('JSON parsing is bounded by UTF-8 bytes before retaining fields', () => {
  const valid = JSON.stringify(release())
  assert.deepEqual(
    parseGitHubLatestReleaseResponseJson(valid),
    parseGitHubLatestReleaseResponse(release()),
  )
  for (const value of [
    null,
    '',
    '{',
    '[]',
    '{}',
    'x'.repeat(MAX_GITHUB_RELEASE_RESPONSE_BYTES + 1),
    JSON.stringify({
      filler: 'あ'.repeat(
        Math.floor(MAX_GITHUB_RELEASE_RESPONSE_BYTES / 3),
      ),
    }),
  ]) {
    assert.equal(parseGitHubLatestReleaseResponseJson(value), null)
  }
  for (const key of ['name', 'body', 'assets']) {
    const validRecord = valid.slice(1, -1)
    assert.equal(
      parseGitHubLatestReleaseResponseJson(`{"${key}":null,${validRecord}}`),
      null,
    )
  }
  for (const hostile of [
    `{"n\\u0061me":null,${valid.slice(1)}`,
    `{"b\\u006fdy":null,${valid.slice(1)}`,
    `{"__proto__":{},${valid.slice(1)}`,
    `{"safe":{"constructor":{}},${valid.slice(1)}`,
    `${'['.repeat(33)}null${']'.repeat(33)}`,
    JSON.stringify({ ...release(), extra: Array.from({ length: 4_097 }, () => 0) }),
    `{"\\ud83d\\ude00":1,"😀":2,${valid.slice(1)}`,
  ]) assert.equal(parseGitHubLatestReleaseResponseJson(hostile), null)
})

test('SemVer precedence follows the complete stable and prerelease ordering', () => {
  const ordered = [
    '1.0.0-alpha',
    '1.0.0-alpha.1',
    '1.0.0-alpha.beta',
    '1.0.0-beta',
    '1.0.0-beta.2',
    '1.0.0-beta.11',
    '1.0.0-rc.1',
    '1.0.0',
    '1.0.1',
    '1.1.0',
    '2.0.0',
  ]
  for (let index = 1; index < ordered.length; index += 1) {
    const previous = ordered[index - 1]
    const current = ordered[index]
    assert.equal(compareSemanticVersions(previous, current), -1)
    assert.equal(compareSemanticVersions(current, previous), 1)
  }
  assert.equal(compareSemanticVersions('v1.2.3', '1.2.3'), 0)
  assert.equal(
    compareSemanticVersions('1.2.3+windows.1', '1.2.3+source.9'),
    0,
  )
  assert.equal(
    compareSemanticVersions(
      `1.0.${'9'.repeat(40)}`,
      `1.0.1${'0'.repeat(40)}`,
    ),
    -1,
  )
})

test('malformed or excessive semantic versions never compare', () => {
  for (const value of [
    null,
    '',
    'v',
    '1',
    '1.2',
    '1.2.3.4',
    '01.2.3',
    '1.02.3',
    '1.2.03',
    '1.2.3-01',
    '1.2.3-',
    '1.2.3+',
    '1.2.3-alpha..1',
    '1.2.3_alpha',
    '1.2.3-α',
    'V1.2.3',
    ' 1.2.3',
    '1.2.3 ',
    '1.2.3-'.concat(
      Array.from({ length: 33 }, () => 'a').join('.'),
    ),
    '1'.repeat(129),
  ]) {
    assert.equal(compareSemanticVersions(value, '1.2.3'), null)
  }
})

test('disabled malformed settings and invalid local versions make no request', async () => {
  let calls = 0
  const client = createUpdateCheckClient({
    requestLatestRelease() {
      calls += 1
      return response()
    },
  })

  assert.deepEqual(
    await client.checkNow('1.0.0', DISABLED_UPDATE_CHECK_SETTINGS),
    { kind: 'disabled' },
  )
  assert.deepEqual(
    await client.checkNow('1.0.0', {
      enabled: true,
      surprise: true,
    }),
    { kind: 'unavailable', reason: 'invalid_settings' },
  )
  assert.deepEqual(
    await client.checkNow('private-project.ori', DEFAULT_UPDATE_CHECK_SETTINGS),
    { kind: 'unavailable', reason: 'invalid_current_version' },
  )
  assert.equal(calls, 0)
})

test('manual checks compare versions without sending local inputs', async () => {
  const argumentLists: unknown[][] = []
  const transport: UpdateCheckTransport = {
    requestLatestRelease(...arguments_: unknown[]) {
      argumentLists.push(arguments_)
      return response()
    },
  }
  const client = createUpdateCheckClient(transport)
  assert.deepEqual(Object.keys(client), ['checkNow'])

  const available = await client.checkNow(
    '1.2.2+private.local',
    DEFAULT_UPDATE_CHECK_SETTINGS,
  )
  assert.deepEqual(available, {
    kind: 'update_available',
    currentVersion: '1.2.2+private.local',
    latestVersion: '1.2.3',
    releasePageUrl:
      'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
  })
  assert.equal(Object.isFrozen(available), true)
  assert.deepEqual(argumentLists, [[]])

  assert.deepEqual(
    await client.checkNow('1.2.3+local', DEFAULT_UPDATE_CHECK_SETTINGS),
    {
      kind: 'up_to_date',
      currentVersion: '1.2.3+local',
      latestVersion: '1.2.3',
    },
  )
  assert.deepEqual(
    await client.checkNow('2.0.0', DEFAULT_UPDATE_CHECK_SETTINGS),
    {
      kind: 'up_to_date',
      currentVersion: '2.0.0',
      latestVersion: '1.2.3',
    },
  )
  assert.deepEqual(argumentLists, [[], [], []])
})

test('HTTP status content type schema body size and failures use fixed reasons', async () => {
  const cases: Array<[unknown, string]> = [
    [response({ status: 404, contentType: 'application/json', body: '{}' }),
      'no_published_release'],
    [response({ status: 403, contentType: 'application/json', body: '{}' }),
      'service_unavailable'],
    [response({ status: 429, contentType: 'application/json', body: '{}' }),
      'service_unavailable'],
    [response({ status: 500, contentType: 'application/json', body: '{}' }),
      'service_unavailable'],
    [response({ status: 200, contentType: 'text/html', body: '{}' }),
      'invalid_response'],
    [response({ status: 200, contentType: null, body: '{}' }),
      'invalid_response'],
    [response({ status: 200, contentType: 'application/json', body: '{}' }),
      'invalid_response'],
    [response({ finalUrl: 'https://evil.example/releases/latest' }),
      'invalid_response'],
    [response({ redirected: true }), 'invalid_response'],
    [{ status: 200, contentType: 'application/json', body: validBody(), extra: 1 },
      'invalid_response'],
    [response({ status: 99, contentType: 'application/json', body: validBody() }),
      'invalid_response'],
    [response({ status: 200, contentType: 'application/json', body:
      'x'.repeat(MAX_GITHUB_RELEASE_RESPONSE_BYTES + 1) }),
    'response_too_large'],
    [response({ status: 200, contentType: 'application/json', body: JSON.stringify({
      filler: 'あ'.repeat(Math.floor(MAX_GITHUB_RELEASE_RESPONSE_BYTES / 3)),
    }) }), 'response_too_large'],
  ]
  for (const [transportValue, reason] of cases) {
    const client = createUpdateCheckClient({
      requestLatestRelease: () => transportValue,
    })
    assert.deepEqual(
      await client.checkNow('1.0.0', DEFAULT_UPDATE_CHECK_SETTINGS),
      { kind: 'unavailable', reason },
    )
  }

  const privatePath = String.raw`C:\Users\alice\private-project.ori`
  const failed = createUpdateCheckClient({
    requestLatestRelease() {
      throw new Error(privatePath)
    },
  })
  const result = await failed.checkNow(
    '1.0.0',
    DEFAULT_UPDATE_CHECK_SETTINGS,
  )
  assert.deepEqual(result, {
    kind: 'unavailable',
    reason: 'network_unavailable',
  })
  assert.doesNotMatch(JSON.stringify(result), /alice|private-project/iu)
})

test('the fetch transport emits one bounded anonymous GET only when requested', async () => {
  const requests: Array<Readonly<{
    input: string
    init: RequestInit
  }>> = []
  const targetClock = clock()
  const fetch: UpdateCheckFetch = async (input, init) => {
    requests.push({ input, init })
    return responseAtApi(validBody(), {
      status: 200,
      headers: {
        'content-type': 'application/json; charset=utf-8',
      },
    })
  }
  const transport = createGitHubReleasesFetchTransport({
    fetch,
    clock: targetClock.value,
  })
  const client = createUpdateCheckClient(transport)

  assert.equal(requests.length, 0)
  assert.deepEqual(Object.keys(transport), ['requestLatestRelease'])
  assert.equal(
    (await client.checkNow(
      '1.0.0',
      DEFAULT_UPDATE_CHECK_SETTINGS,
    )).kind,
    'update_available',
  )
  assert.equal(requests.length, 1)
  assert.equal(requests[0]?.input, ORIGAMI2_GITHUB_RELEASES_API_URL)
  const init = requests[0]?.init
  assert.ok(init)
  const headers = init.headers as Readonly<Record<string, string>>
  assert.equal(init?.method, 'GET')
  assert.equal(init?.body, null)
  assert.equal(init?.cache, 'no-store')
  assert.equal(init?.credentials, 'omit')
  assert.equal(init?.redirect, 'error')
  assert.equal(init?.referrerPolicy, 'no-referrer')
  assert.equal(
    headers.Accept,
    'application/vnd.github+json',
  )
  assert.equal(
    headers.Authorization,
    undefined,
  )
  assert.equal(targetClock.delays[0], UPDATE_CHECK_TIMEOUT_MS)
  assert.deepEqual(targetClock.cleared, [targetClock.handle])

  const serializedRequest = JSON.stringify({
    input: requests[0]?.input,
    method: init?.method,
    headers: init?.headers,
    body: init?.body,
  })
  assert.doesNotMatch(
    serializedRequest,
    /private|project|usage|locale|1\.0\.0/iu,
  )
})

test('declared and streamed response limits stop oversized release data', async () => {
  const declaredClient = createUpdateCheckClient(
    createGitHubReleasesFetchTransport({
      fetch: async () => responseAtApi('{}', {
        status: 200,
        headers: {
          'content-type': 'application/json',
          'content-length': String(
            MAX_GITHUB_RELEASE_RESPONSE_BYTES + 1,
          ),
        },
      }),
      clock: clock().value,
    }),
  )
  assert.deepEqual(
    await declaredClient.checkNow(
      '1.0.0',
      DEFAULT_UPDATE_CHECK_SETTINGS,
    ),
    { kind: 'unavailable', reason: 'response_too_large' },
  )

  const streamedClient = createUpdateCheckClient(
    createGitHubReleasesFetchTransport({
      fetch: async () => responseAtApi(
        'x'.repeat(MAX_GITHUB_RELEASE_RESPONSE_BYTES + 1),
        {
          status: 200,
          headers: { 'content-type': 'application/json' },
        },
      ),
      clock: clock().value,
    }),
  )
  assert.deepEqual(
    await streamedClient.checkNow(
      '1.0.0',
      DEFAULT_UPDATE_CHECK_SETTINGS,
    ),
    { kind: 'unavailable', reason: 'response_too_large' },
  )
})

test('the fixed timeout aborts a stalled request without exposing its failure', async () => {
  let fireTimeout: (() => void) | null = null
  let cleared = false
  const timeoutClock: UpdateCheckTimeoutClock = {
    setTimeout(callback, delayMs) {
      assert.equal(delayMs, UPDATE_CHECK_TIMEOUT_MS)
      fireTimeout = callback
      return 7
    },
    clearTimeout(handle) {
      assert.equal(handle, 7)
      cleared = true
    },
  }
  const fetch: UpdateCheckFetch = (_input, init) => new Promise(
    (_resolve, reject) => {
      init.signal?.addEventListener('abort', () => {
        reject(new Error('C:\\Users\\alice\\private-project.ori'))
      })
    },
  )
  const client = createUpdateCheckClient(
    createGitHubReleasesFetchTransport({
      fetch,
      clock: timeoutClock,
    }),
  )

  const pending = client.checkNow(
    '1.0.0',
    DEFAULT_UPDATE_CHECK_SETTINGS,
  )
  await Promise.resolve()
  assert.ok(fireTimeout)
  fireTimeout()
  const result = await pending
  assert.deepEqual(result, {
    kind: 'unavailable',
    reason: 'network_unavailable',
  })
  assert.equal(cleared, true)
  assert.doesNotMatch(JSON.stringify(result), /alice|private-project/iu)
})

test('manual abort detaches the old request and a retry publishes only the new response', async () => {
  let calls = 0
  const fetch: UpdateCheckFetch = (_input, init) => {
    calls += 1
    if (calls === 1) return new Promise((_resolve, reject) => {
      init.signal?.addEventListener('abort', () => reject(new Error(
        'C:\\Users\\private\\old-response.json',
      )), { once: true })
    })
    return Promise.resolve(responseAtApi(validBody(), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }))
  }
  const client = createUpdateCheckClient(createGitHubReleasesFetchTransport({
    fetch,
    clock: clock().value,
  }))
  const controller = new AbortController()
  const old = client.checkNow('1.0.0', DEFAULT_UPDATE_CHECK_SETTINGS, controller.signal)
  await Promise.resolve()
  controller.abort()
  assert.deepEqual(await old, { kind: 'unavailable', reason: 'network_unavailable' })
  assert.deepEqual(await client.checkNow('1.0.0', DEFAULT_UPDATE_CHECK_SETTINGS), {
    kind: 'update_available',
    currentVersion: '1.0.0',
    latestVersion: '1.2.3',
    releasePageUrl: ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX + 'v1.2.3',
  })
  assert.equal(calls, 2)
})

function release(
  overrides: Readonly<Record<PropertyKey, unknown>> = {},
): Record<PropertyKey, unknown> {
  return {
    tag_name: 'v1.2.3',
    html_url:
      'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    draft: false,
    prerelease: false,
    name: 'ORIGAMI2 v1.2.3',
    body: 'Canonical generated release notes.',
    assets: Array.from({ length: 9 }, (_, id) => ({ id: id + 1 })),
    ...overrides,
  }
}

function validBody() {
  return JSON.stringify(release())
}

function response(
  overrides: Partial<UpdateCheckTransportResponse> = {},
): UpdateCheckTransportResponse {
  return {
    status: 200,
    contentType: 'application/vnd.github+json; charset=utf-8',
    body: validBody(),
    finalUrl: ORIGAMI2_GITHUB_RELEASES_API_URL,
    redirected: false,
    ...overrides,
  }
}

function responseAtApi(body: BodyInit, init: ResponseInit): Response {
  const value = new Response(body, init)
  Object.defineProperties(value, {
    url: { value: ORIGAMI2_GITHUB_RELEASES_API_URL },
    redirected: { value: false },
  })
  return value
}

function clock(): {
  value: UpdateCheckTimeoutClock
  handle: Readonly<{ id: 1 }>
  delays: number[]
  cleared: unknown[]
} {
  const target = {
    handle: Object.freeze({ id: 1 as const }),
    delays: [] as number[],
    cleared: [] as unknown[],
    value: null as unknown as UpdateCheckTimeoutClock,
  }
  target.value = {
    setTimeout(_callback, delayMs) {
      target.delays.push(delayMs)
      return target.handle
    },
    clearTimeout(handle) {
      target.cleared.push(handle)
    },
  }
  return target
}
