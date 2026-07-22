import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { chmodSync, copyFileSync, linkSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import { validateReleaseArchiveEntries } from '../scripts/release_archive_contract.mjs'
import { buildDependencyPolicy } from '../scripts/dependency_policy.mjs'

const root = resolve(import.meta.dirname, '..', '..')
const ciArtifactFixture = {
  artifactId: '7', name: 'rustsec-warning-review', digest: `sha256:${'c'.repeat(64)}`,
  archiveSha256: 'c'.repeat(64), size: 128,
  reportSha256: 'd'.repeat(64),
  createdAt: '2026-07-20T00:00:00.000Z', expiresAt: '2026-07-27T00:00:00.000Z',
  workflowRunId: '67890', runAttempt: 1, checkSuiteId: '24680',
}
const ciArtifactsFixture = [
  { artifactId: '8', name: 'ORIGAMI2-macos-app-67890', digest: `sha256:${'a'.repeat(64)}`, size: 256 },
  { artifactId: '9', name: 'ORIGAMI2-windows-nsis-67890', digest: `sha256:${'b'.repeat(64)}`, size: 512 },
  { artifactId: '7', name: 'rustsec-warning-review', digest: ciArtifactFixture.digest, size: ciArtifactFixture.size },
  { artifactId: '10', name: 'sample-viewer-runtime-log', digest: `sha256:${'e'.repeat(64)}`, size: 64 },
].map((artifact) => ({
  ...artifact,
  createdAt: ciArtifactFixture.createdAt,
  expiresAt: ciArtifactFixture.expiresAt,
}))
function fixtureCrc32(bytes) {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1))
  }
  return (crc ^ 0xffffffff) >>> 0
}
function singleEntryZip(name, bytes) {
  const filename = Buffer.from(name)
  const crc = fixtureCrc32(bytes)
  const local = Buffer.alloc(30 + filename.length)
  local.writeUInt32LE(0x04034b50, 0); local.writeUInt16LE(20, 4)
  local.writeUInt32LE(crc, 14); local.writeUInt32LE(bytes.length, 18); local.writeUInt32LE(bytes.length, 22)
  local.writeUInt16LE(filename.length, 26); filename.copy(local, 30)
  const central = Buffer.alloc(46 + filename.length)
  central.writeUInt32LE(0x02014b50, 0); central.writeUInt16LE(20, 4); central.writeUInt16LE(20, 6)
  central.writeUInt32LE(crc, 16); central.writeUInt32LE(bytes.length, 20); central.writeUInt32LE(bytes.length, 24)
  central.writeUInt16LE(filename.length, 28); filename.copy(central, 46)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(1, 8); end.writeUInt16LE(1, 10)
  end.writeUInt32LE(central.length, 12); end.writeUInt32LE(local.length + bytes.length, 16)
  return Buffer.concat([local, bytes, central, end])
}

test('PowerShell and bash release helpers anchor repository paths to their script', () => {
  const packageScript = readFileSync(
    join(root, '.github/scripts/package_formal_release.ps1'),
    'utf8',
  )
  const macosVerifier = readFileSync(
    join(root, '.github/scripts/verify_macos_bundle.sh'),
    'utf8',
  )
  assert.match(packageScript, /Join-Path \$PSScriptRoot '\.\.\\\.\.'/u)
  assert.doesNotMatch(packageScript, /\$env:GITHUB_WORKSPACE|node \.github/u)
  assert.match(macosVerifier, /BASH_SOURCE\[0\]/u)
  assert.match(macosVerifier, /repository_root\/target\/release\/bundle\/macos/u)

  const temporaryRoot = mkdtempSync(join(tmpdir(), 'origami2-release-cwd-'))
  try {
    const results = ['root', 'apps', 'external'].map((caller) => {
      const fixtureRoot = join(temporaryRoot, `fixture-${caller}`)
      const scripts = join(fixtureRoot, '.github', 'scripts')
      const release = join(fixtureRoot, 'target', 'release')
      const output = join(fixtureRoot, 'target', 'formal-release')
      mkdirSync(join(release, 'bundle', 'nsis'), { recursive: true })
      mkdirSync(join(release, 'fonts'), { recursive: true })
      mkdirSync(join(release, 'licenses'), { recursive: true })
      mkdirSync(scripts, { recursive: true })
      mkdirSync(output, { recursive: true })
      copyFileSync(
        join(root, '.github/scripts/package_formal_release.ps1'),
        join(scripts, 'package_formal_release.ps1'),
      )
      copyFileSync(
        join(root, '.github/scripts/write_update_manifest.mjs'),
        join(scripts, 'write_update_manifest.mjs'),
      )
      writeFileSync(join(release, 'bundle', 'nsis', 'installer.exe'), 'installer')
      writeFileSync(join(release, 'origami2-desktop.exe'), 'desktop')
      writeFileSync(join(release, 'fonts', 'font.ttf'), 'font')
      writeFileSync(join(release, 'licenses', 'license.txt'), 'license')
      writeFileSync(
        join(output, 'ORIGAMI2-v1.2.3-windows-x64.cdx.json'),
        '{"bomFormat":"CycloneDX"}\n',
      )
      const cwd = caller === 'root'
        ? fixtureRoot
        : caller === 'apps'
          ? join(fixtureRoot, 'apps', 'desktop')
          : join(temporaryRoot, 'unrelated-caller')
      mkdirSync(cwd, { recursive: true })
      execFileSync(
        process.platform === 'win32' ? 'powershell.exe' : 'pwsh',
        ['-NoProfile', '-File', join(scripts, 'package_formal_release.ps1')],
        {
          cwd,
          env: {
            ...process.env,
            GITHUB_WORKSPACE: join(temporaryRoot, 'wrong-workspace'),
            PLATFORM: 'windows-x64',
            VERSION: '1.2.3',
            SIGNATURE_POLICY: 'unsigned-dry-run',
          },
          stdio: 'pipe',
        },
      )
      const manifest = JSON.parse(readFileSync(
        join(output, 'ORIGAMI2-v1.2.3-windows-x64.update.json'),
        'utf8',
      ))
      return {
        files: readdirSync(output).sort(),
        assets: manifest.assets.map(({ name }) => name),
      }
    })
    assert.deepEqual(results[1], results[0])
    assert.deepEqual(results[2], results[0])
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true })
  }
})

test('release workflow keeps publication permissions out of build jobs', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const validator = readFileSync(join(root, '.github/scripts/validate_formal_release.mjs'), 'utf8')
  assert.match(workflow, /options: \[dry-run, prerelease, stable, promote\]/u)
  assert.match(validator, /execFileSync\('git', args,[\s\S]*cwd: repositoryRoot/u)
  assert.match(validator, /git\(\['verify-tag', tag\]/u)
  assert.match(workflow, /permissions:\s*\n\s+contents: read/u)
  assert.match(workflow, /attest-build-provenance@43d14bc2/u)
  assert.match(workflow, /releases\/\$release_id[\s\S]*prerelease=false/u)
  assert.doesNotMatch(workflow, /pull_request:/u)
})

test('publication binds generated notes tag and immutable remote commit', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  assert.match(workflow, /commit: \$\{\{ steps\.contract\.outputs\.commit \}\}/u)
  assert.equal(workflow.match(/ref: \$\{\{ needs\.validate\.outputs\.commit \}\}/gu)?.length ?? 0, 3)
  assert.doesNotMatch(publish, /ref: \$\{\{ needs\.validate\.outputs\.tag/u)
  assert.match(publish, /commits\/\$RELEASE_TAG.*--jq \.sha/u)
  assert.match(publish, /test "\$remote_commit" = "\$RELEASE_COMMIT"/u)
  assert.match(publish, /releases\/generate-notes/u)
  assert.match(publish, /target_commitish="\$RELEASE_COMMIT"/u)
  assert.match(publish, /\.name "\$notes_json"\)" = "\$RELEASE_TAG"/u)
  assert.match(publish, /target_commitish="\$RELEASE_COMMIT"/u)
  assert.match(publish, /body=@"\$notes_file"/u)
  assert.match(publish, /select\(length > 0 and length <= 100000\)/u)
  assert.match(publish, /test -s "\$notes_file"/u)
  assert.doesNotMatch(publish, /--generate-notes|--clobber|gh release delete|gh release upload/u)
  assert.ok(
    publish.indexOf('! gh release view') < publish.indexOf('created="$RUNNER_TEMP'),
  )
})

test('release publication uses a bounded rollback-safe draft transaction', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  assert.match(publish, /trap cleanup_partial_draft EXIT/u)
  assert.match(publish, /transaction_name="ORIGAMI2 draft transaction \$GITHUB_RUN_ID-\$GITHUB_RUN_ATTEMPT"/u)
  assert.match(publish, /randomBytes\(32\).*toString\('hex'\)/u)
  assert.match(publish, /\[\[ "\$ownership_token" =~ \^\[0-9a-f\]\{64\}\$ \]\]/u)
  assert.match(publish, /echo "::add-mask::\$ownership_token"/u)
  assert.match(publish, /ownership_commitment="\$\(printf '%s' "\$ownership_token" \| sha256sum/u)
  assert.match(publish, /unset ownership_token/u)
  assert.match(publish, /ownership_marker="<!-- origami2-release-owner-sha256:\$ownership_commitment -->"/u)
  assert.doesNotMatch(publish, /origami2-release-owner:\$ownership_token/u)
  assert.match(publish, /select\(length > 0 and length <= 100000\)/u)
  assert.match(publish, /stat -c '%s' "\$notes_file"\)" -le 100128/u)
  assert.match(publish, /cp -- "\$notes_file" "\$public_notes_file"/u)
  assert.match(publish, /-F body=@"\$public_notes_file"/u)
  assert.match(publish, /--rawfile body "\$public_notes_file" '\.body == \$body'/u)
  assert.match(publish, /verify_owned_draft\(\)/u)
  assert.match(publish, /created_release_id="\$\(jq -er '\.id \| numbers \| select\(\. > 0\)' "\$created" \|\| true\)"/u)
  assert.match(publish, /\.id == \$id and \.name == \$name and \.tag_name == \$tag/u)
  assert.match(publish, /endswith\("\\n\\n" \+ \$marker \+ "\\n"\)/u)
  assert.match(publish, /--hostname uploads\.github\.com/u)
  assert.match(publish, /verify_remote_release_assets\.mjs/u)
  assert.match(publish, /jq '\.assets \| length' "\$created"\)" -eq 0/u)
  assert.match(publish, /verified-release\.sha256[\s\S]*sha256sum --check --strict "\$RUNNER_TEMP\/verified-release\.sha256"/u)
  assert.match(publish, /-F draft=false/u)
  assert.match(publish, /gh api --method DELETE "repos\/\$GH_REPO\/releases\/\$created_release_id"/u)
  assert.match(publish, /for rollback_attempt in 1 2 3/u)
  assert.match(publish, /test "\$rollback_complete" = true/u)
  assert.match(publish, /if \[ "\$status" -ne 0 \] && \[ "\$release_published" = true \]/u)
  assert.match(publish, /-f name="QUARANTINED \$RELEASE_TAG" -F draft=false -F prerelease=true/u)
  assert.match(publish, /Automated post-publication verification failed\. Do not install these assets\./u)
  assert.match(publish, /post-publication-verification-failure/u)
  assert.match(publish, /quarantineVerified:\$quarantined/u)
  assert.match(publish, />> "\$GITHUB_STEP_SUMMARY"/u)
  assert.match(publish, /test "\$quarantine_verified" = true/u)
  assert.doesNotMatch(publish, /QUARANTINED \$RELEASE_TAG[^\n]*draft=true/u)
  assert.match(publish, /quarantine_state"\)" = "\$asset_identity"/u)
  assert.match(publish, /--header "If-Match: \$quarantine_etag"/u)
  assert.match(publish, /stat -c '%s' "\$quarantine_body"\)" -le 100512/u)
  assert.match(publish, /name "\$quarantine_state"\)" = "QUARANTINED \$RELEASE_TAG"[\s\S]*quarantine_verified=true[\s\S]*elif timeout 60s gh api --method PATCH/u)
  assert.match(publish, /chmod 0600 "\$quarantine_evidence"/u)
  assert.doesNotMatch(publish, /quarantine_evidence[\s\S]{0,300}\$GH_TOKEN/u)
  assert.match(publish, /previous_latest_id=none/u)
  assert.match(publish, /releases\/latest/u)
  assert.match(publish, /test "\$latest_after_status" = 404/u)
  assert.match(publish, /test "\$\(jq -r \.id "\$latest_after"\)" = "\$previous_latest_id"/u)
  assert.match(publish, /test "\$\(jq -r \.prerelease "\$latest_after"\)" = false/u)
  assert.match(publish, /test "\$\(jq -r '\.id \/\/ empty' "\$latest_after"\)" != "\$created_release_id"/u)
  assert.ok(publish.indexOf('release_published=true') > publish.indexOf('-F draft=false'))
  assert.match(publish, /\.target_commitish.*= "\$RELEASE_COMMIT"/u)
  assert.ok(publish.indexOf('verify_remote_release_assets.mjs') < publish.indexOf('-F draft=false'))
  assert.ok(publish.lastIndexOf('verify_remote_release_assets.mjs') > publish.indexOf('-F draft=false'))
  assert.match(publish, /releases\/\$created_release_id\/assets/u)
  assert.ok((publish.match(/verify_owned_draft "\$created_release_id"/gu)?.length ?? 0) >= 4)
  assert.match(publish, /releases\/tags\/\$RELEASE_TAG" --jq \.id\)" = "\$created_release_id"/u)
  assert.match(publish, /commits\/\$RELEASE_TAG" --jq \.sha\)" = "\$RELEASE_COMMIT"/u)
  assert.match(publish, /chmod 0555 release/u)
  assert.match(publish, /release-staging\.sha256/u)
  assert.match(publish, /exec \{asset_fd\}<"\$asset"/u)
  assert.match(publish, /--input "\/proc\/\$\$\/fd\/\$asset_fd"/u)
  assert.ok(publish.lastIndexOf('release-staging.sha256') > publish.indexOf('verify_remote_release_assets.mjs'))
  const remoteVerifier = readFileSync(join(root, '.github/scripts/verify_remote_release_assets.mjs'), 'utf8')
  assert.match(remoteVerifier, /local release asset names are non-canonical/u)
  assert.match(remoteVerifier, /SHA256SUMS-windows-x64\.txt/u)
  assert.match(publish, /publish_verified=false/u)
  assert.match(publish, /for attempt in 1 2 3/u)
  assert.match(publish, /asset_identity="\$\(jq -cer/u)
  assert.match(publish, /\{id, name, size, digest, updated_at\}/u)
  assert.match(publish, /\(\[\.\[\]\.id\] \| unique \| length\) == 9/u)
  assert.match(publish, /Cache-Control: no-cache/u)
  assert.match(publish, /verify_release_api_headers\(\)/u)
  assert.match(publish, /verify_release_response_headers\.mjs "\$response_headers" "\$response_status"/u)
  assert.match(publish, /\[\[ "\$GH_REPO" =~ \^\[A-Za-z0-9_.-\]\{1,100\}\/\[A-Za-z0-9_.-\]\{1,100\}\$ \]\]/u)
  assert.match(publish, /\[\[ "\$RELEASE_TAG" =~ \^v\(0\|\[1-9\]\[0-9\]\*\)/u)
  assert.match(publish, /test "\$published" != "\$published_headers"/u)
  assert.match(publish, /test "\$tagged_published" != "\$tagged_headers"/u)
  assert.match(publish, /--connect-timeout 15 --max-time 60/u)
  assert.match(publish, /--write-out '%\{http_code\}'/u)
  assert.match(publish, /test "\$current_identity" = "\$asset_identity"/u)
  assert.match(publish, /consecutive_verified=\$\(\(consecutive_verified \+ 1\)\)/u)
  assert.match(publish, /if \[ "\$consecutive_verified" -eq 2 \]/u)
  assert.match(publish, /releases\/tags\/\$RELEASE_TAG/u)
  assert.match(publish, /test "\$tagged_identity" = "\$asset_identity"/u)
  assert.match(publish, /test "\$publish_verified" = true/u)
  assert.ok(publish.lastIndexOf("created_release_id=''") > publish.indexOf('test "$publish_verified" = true'))
  assert.ok(publish.lastIndexOf('draft_created=false') > publish.indexOf('test "$publish_verified" = true'))
})

test('release API response headers reject duplicate folded and provisional metadata', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-release-headers-'))
  const fixture = join(directory, 'headers.txt')
  const verifier = join(root, '.github/scripts/verify_release_response_headers.mjs')
  const valid = [
    'HTTP/2 200', 'content-type: application/vnd.github+json; charset=utf-8',
    'etag: "abc123"', 'cache-control: private, max-age=0, s-maxage=0',
    'x-ratelimit-remaining: 42', '', '',
  ].join('\r\n')
  const verify = (value, status = '200') => {
    writeFileSync(fixture, value)
    execFileSync(process.execPath, [verifier, fixture, status])
  }
  try {
    verify(valid)
    for (const hostile of [
      valid.replace('etag: "abc123"', 'etag: "abc123"\r\netag: "def456"'),
      valid.replace('content-type:', 'content-type: application/json\r\ncontent-type:'),
      valid.replace('cache-control:', 'cache-control: private\r\ncache-control:'),
      valid.replace('etag:', ' etag:'),
      valid.replace('etag: "abc123"', 'etag: "abc\u0000def"'),
      'HTTP/1.1 100 Continue\r\n\r\n' + valid,
      'HTTP/1.1 200 Connection established\r\n\r\n' + valid,
      '\uFEFF' + valid,
      valid + '{"body":"header confusion"}',
    ]) assert.throws(() => verify(hostile))
    assert.throws(() => verify(valid, '304'))
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('all workflow actions are immutable SHA-pinned with bounded release jobs', () => {
  const workflowNames = ['ci.yml', 'release.yml', 'release-windows.yml']
  for (const name of workflowNames) {
    const workflow = readFileSync(join(root, '.github/workflows', name), 'utf8')
    const references = [...workflow.matchAll(/uses:\s*([^\s@]+)@([^\s#]+)/gu)]
    assert.ok(references.length > 0)
    for (const [, action, revision] of references) {
      assert.match(revision, /^[0-9a-f]{40}$/u, `${name}: ${action}@${revision}`)
    }
    const checkoutCount = references.filter(([, action]) =>
      action === 'actions/checkout').length
    assert.equal(
      workflow.match(/persist-credentials: false/gu)?.length ?? 0,
      checkoutCount,
      `${name}: every checkout must discard credentials`,
    )
  }

  const release = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  assert.equal(release.match(/timeout-minutes:/gu)?.length ?? 0, 4)
  assert.match(
    release,
    /name: formal-release-\$\{\{ matrix\.platform \}\}[\s\S]*retention-days: 1/u,
  )
  const build = release.slice(release.indexOf('  build:'), release.indexOf('  publish:'))
  assert.doesNotMatch(build, /contents: write|id-token: write|attestations: write/u)
})

test('release build toolchains are exact versions rather than moving channels', () => {
  for (const name of ['release.yml', 'release-windows.yml']) {
    const workflow = readFileSync(join(root, '.github/workflows', name), 'utf8')
    assert.match(workflow, /toolchain: 1\.90\.0/u, name)
    assert.match(workflow, /node-version: 24\.11\.1/u, name)
    assert.doesNotMatch(workflow, /toolchain: stable|node-version: 24\s*$/mu, name)
    assert.doesNotMatch(workflow, /Swatinem\/rust-cache|cache:\s*npm|cache-dependency-path:/u, name)
  }
})

test('legacy Windows release audit dependencies are hash pinned and cache free', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release-windows.yml'), 'utf8')
  const requirements = readFileSync(join(root, '.github/release-audit-requirements.txt'), 'utf8')
  assert.match(workflow, /python-version: "3\.12\.10"/u)
  assert.match(workflow, /--require-hashes --no-deps -r \.github\/release-audit-requirements\.txt/u)
  assert.match(workflow, /ref: \$\{\{ needs\.validate-test-build\.outputs\.commit_sha \}\}/u)
  assert.equal(requirements.match(/--hash=sha256:[0-9a-f]{64}/gu)?.length ?? 0, 3)
  assert.doesNotMatch(workflow, /Swatinem\/rust-cache|cache:\s*npm/u)
})

test('release-gating CI uses exact toolchains and digest-verified external tools', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  assert.doesNotMatch(workflow, /toolchain: stable|node-version: 24\s*$|python-version: "3\.12"\s*$/mu)
  assert.equal(workflow.match(/toolchain: 1\.90\.0/gu)?.length ?? 0, 5)
  assert.equal(workflow.match(/node-version: 24\.11\.1/gu)?.length ?? 0, 4)
  assert.match(workflow, /python-version: "3\.12\.10"/u)
  assert.match(workflow, /blender-4\.5\.11-linux-x64\.tar\.xz[\s\S]*sha256sum --check --strict/u)
  assert.match(workflow, /PrusaSlicer-2\.9\.6\.zip[\s\S]*Get-FileHash[\s\S]*checksum mismatch/u)
  assert.match(workflow, /--require-hashes --no-deps -r \.github\/release-audit-requirements\.txt/u)
  assert.match(workflow, /npm audit --package-lock-only --audit-level=low/u)
  assert.match(workflow, /cargo install cargo-audit --version 0\.22\.2 --locked/u)
  assert.match(workflow, /checkout --detach b5fc89b8be99e96f79194d8a6f11e9b4143b99f0/u)
  assert.match(workflow, /cargo audit --db "\$database" --no-fetch --deny warnings/u)
  assert.match(workflow, /verify_rustsec_warning_ledger\.mjs "\$audit_report" "\$review_report" > "\$allowed_warnings_file"/u)
  assert.match(workflow, /name: rustsec-warning-review[\s\S]*retention-days: 7/u)
  assert.match(workflow, /cargo audit --db "\$database" --no-fetch --json/u)
  assert.doesNotMatch(workflow, /npx (?!--no-install)/u)
  for (const command of workflow.matchAll(/cargo test[^\r\n]*/gu)) {
    assert.match(command[0], /--locked/u)
  }
})

test('formal builds cannot resolve undeclared npm or Cargo inputs', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  assert.match(workflow, /npm ci[\s\S]*cargo metadata --locked --no-deps --format-version 1/u)
  assert.equal(workflow.match(/npx --no-install tauri/gu)?.length ?? 0, 2)
  assert.doesNotMatch(workflow, /npx tauri|npm install/u)
  assert.ok(workflow.indexOf('dependency_policy.mjs') < workflow.indexOf('Build Windows portable executable'))
})

test('all direct and nested action runtimes match the audited Node.js 24 inventory', () => {
  const workflowNames = ['ci.yml', 'release.yml', 'release-windows.yml']
  const used = new Set()
  for (const name of workflowNames) {
    const workflow = readFileSync(join(root, '.github/workflows', name), 'utf8')
    for (const match of workflow.matchAll(/uses:\s*([^\s@]+@[0-9a-f]{40})/gu)) {
      used.add(match[1])
    }
  }
  const contract = JSON.parse(readFileSync(
    join(root, '.github/action-runtime-contract.json'),
    'utf8',
  ))
  assert.equal(contract.schema, 'origami2.github-action-runtime-contract.v1')
  assert.deepEqual([...used].sort(), Object.keys(contract.direct).sort())
  for (const [reference, runtime] of [
    ...Object.entries(contract.direct),
    ...Object.entries(contract.nested),
  ]) {
    assert.match(reference, /@[0-9a-f]{40}$/u)
    assert.ok(runtime === 'node24' || runtime === 'composite')
    assert.notEqual(runtime, 'node20')
  }
  assert.equal(
    contract.nested['actions/attest@daf44fb950173508f38bd2406030372c1d1162b1'],
    'node24',
  )
})

test('publication verifies current-run artifact archive digests before extraction', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = join(root, '.github/scripts/verify_workflow_artifact_metadata.mjs')
  assert.match(workflow, /actions: read/u)
  assert.match(workflow, /actions\/runs\/\$GITHUB_RUN_ID\/artifacts/u)
  assert.match(workflow, /actions\/artifacts\/\$artifact_id\/zip/u)
  assert.match(workflow, /actual_digest.*expected_digest/u)
  assert.match(workflow, /--max-redirs 0 --max-filesize 1048576/u)
  assert.match(workflow, /! grep -Eiq '\^link:' "\$metadata_headers"/u)
  assert.match(workflow, /--proto-redir '=https'/u)
  assert.match(workflow, /url_effective/u)
  assert.match(workflow, /\.blob\\\.core\\\.windows\\\.net/u)
  assert.match(workflow, /--max-filesize 2147483648/u)
  assert.match(workflow, /content-type: \(application\/zip\|application\/octet-stream\)/u)
  assert.match(workflow, /name: Remove temporary artifact transport files[\s\S]*if: always\(\)/u)
  assert.ok(
    workflow.indexOf('Verify immutable workflow artifact archive digests') <
      workflow.indexOf('Extract only the digest-verified artifact archives'),
  )
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  assert.doesNotMatch(publish, /actions\/download-artifact@/u)
  assert.match(publish, /unzip -Z1/u)
  assert.match(publish, /sort -u "\$entries"/u)
  assert.match(publish, /tolower\(\$0\).*sort -u/su)
  assert.match(publish, /content_length.*stat -c '%s'/su)
  assert.match(publish, /sha256sum --check --strict "\$archive\.sha256"/u)
  assert.match(publish, /find release -type f -links \+1/u)
  assert.match(publish, /release_root="\$\(realpath -e release\)"/u)
  assert.match(publish, /test "\$\(dirname "\$resolved"\)" = "\$release_root"/u)
  assert.match(publish, /unzip -tqq/u)
  assert.match(publish, /entry_count.*-le 16/u)
  assert.match(publish, /archive_bytes \* 200 \+ 1048576/u)
  assert.match(publish, /find release -mindepth 1 -maxdepth 1 ! -type f/u)

  const directory = mkdtempSync(join(tmpdir(), 'origami2-artifact-metadata-'))
  try {
    const valid = {
      total_count: 2,
      artifacts: [
        artifact(2, 'formal-release-windows-x64', 'a'),
        artifact(1, 'formal-release-macos-arm64', 'b'),
      ],
    }
    const path = join(directory, 'metadata.json')
    writeFileSync(path, JSON.stringify(valid))
    const verifierEnv = {
      ...process.env,
      GITHUB_RUN_ID: '12345',
      RELEASE_COMMIT: 'a'.repeat(40),
    }
    const output = execFileSync('node', [verifier, path], { encoding: 'utf8', env: verifierEnv })
    assert.match(output, /^formal-release-macos-arm64\t1\tb{64}$/mu)
    for (const invalid of [
      { ...valid, total_count: 3 },
      { ...valid, artifacts: [valid.artifacts[0], valid.artifacts[0]] },
      { ...valid, artifacts: [valid.artifacts[0]] },
      { ...valid, artifacts: [artifact(1, 'formal-release-macos-arm64', 'z'), valid.artifacts[0]] },
      { ...valid, artifacts: [{ ...valid.artifacts[1], expired: true }, valid.artifacts[0]] },
      { ...valid, artifacts: [{ ...valid.artifacts[1], size_in_bytes: 0 }, valid.artifacts[0]] },
      { ...valid, artifacts: [{ ...valid.artifacts[1], size_in_bytes: 2_147_483_649 }, valid.artifacts[0]] },
    ]) {
      writeFileSync(path, JSON.stringify(invalid))
      assert.throws(
        () => execFileSync('node', [verifier, path], { stdio: 'pipe', env: verifierEnv }),
        /workflow artifact/u,
      )
    }
    writeFileSync(path, ' '.repeat(1_048_577))
    assert.throws(
      () => execFileSync('node', [verifier, path], { stdio: 'pipe', env: verifierEnv }),
      /metadata size is invalid/u,
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

function artifact(id, name, digestCharacter) {
  return {
    id,
    name,
    expired: false,
    size_in_bytes: 1024,
    digest: `sha256:${digestCharacter.repeat(64)}`,
    workflow_run: { id: 12345, head_sha: 'a'.repeat(40) },
    created_at: '2026-07-21T00:00:00Z',
    expires_at: '2026-07-22T00:00:00Z',
  }
}

test('formal manifest remains an attested manual-review artifact, not an updater endpoint', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const manifestWriter = readFileSync(
    join(root, '.github/scripts/write_update_manifest.mjs'),
    'utf8',
  )
  const runtimeContract = readFileSync(
    join(root, 'apps/desktop/src/lib/releaseArtifactCompatibility.ts'),
    'utf8',
  )
  const provenanceVerifier = readFileSync(
    join(root, '.github/scripts/verify_release_provenance.sh'),
    'utf8',
  )
  assert.match(workflow, /release\/\*\.update\.json/u)
  assert.match(provenanceVerifier, /\.update\.json"/u)
  assert.doesNotMatch(manifestWriter, /https?:|url|endpoint/iu)
  assert.match(runtimeContract, /delivery: 'release_page_only'/u)
  assert.match(runtimeContract, /runtimeUpdaterAvailable: false/u)
})

test('provenance subjects cover the complete verified nine-asset set', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  const promote = workflow.slice(workflow.indexOf('  promote:'))
  const provenanceVerifier = readFileSync(
    join(root, '.github/scripts/verify_release_provenance.sh'),
    'utf8',
  )
  for (const pattern of [
    'release/*.exe',
    'release/*.zip',
    'release/*.tar.gz',
    'release/*.cdx.json',
    'release/*.update.json',
    'release/SHA256SUMS-*.txt',
  ]) {
    assert.ok(publish.includes(pattern), pattern)
  }
  for (const name of [
    'windows-x64-setup.exe',
    'windows-x64-portable.zip',
    'windows-x64.cdx.json',
    'windows-x64.update.json',
    'macos-arm64-app.tar.gz',
    'macos-arm64.cdx.json',
    'macos-arm64.update.json',
    'SHA256SUMS-windows-x64.txt',
    'SHA256SUMS-macos-arm64.txt',
  ]) {
    assert.ok(provenanceVerifier.includes(name), name)
  }
  assert.match(promote, /verify_release_provenance\.sh/u)
  assert.equal(provenanceVerifier.match(/gh attestation verify "\$file"/gu)?.length, 1)
})

test('publication verifies all newly attested assets before creating a release', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  const helper = readFileSync(
    join(root, '.github/scripts/verify_release_provenance.sh'),
    'utf8',
  )
  assert.ok(
    publish.indexOf('actions/attest-build-provenance@')
      < publish.indexOf('verify_release_provenance.sh'),
  )
  assert.ok(
    publish.indexOf('verify_release_provenance.sh')
      < publish.indexOf('Publish immutable tagged release'),
  )
  assert.equal(workflow.match(/verify_release_provenance\.sh/gu)?.length ?? 0, 2)
  assert.match(helper, /\[\[ "\$\(find "\$directory" -maxdepth 1 -type f \| wc -l \| tr -d ' '\)" -eq "\$\{#assets\[@\]\}" \]\]/u)
  assert.equal(helper.match(/^  "?ORIGAMI2-v|^  'SHA256SUMS-/gmu)?.length ?? 0, 9)
  assert.match(helper, /gh attestation verify "\$file" --repo "\$repository"/u)
})

test('signing secrets are approval-gated masked cleaned and absent from fork CI', () => {
  const release = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const ci = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const build = release.slice(release.indexOf('  build:'), release.indexOf('  publish:'))
  assert.match(
    build,
    /environment: \$\{\{ needs\.validate\.outputs\.mode != 'dry-run' && 'formal-release-signing' \|\| '' \}\}/u,
  )
  assert.match(release, /environment: formal-release/u)
  assert.doesNotMatch(ci, /\$\{\{ secrets\./u)
  assert.doesNotMatch(release, /pull_request:/u)
  assert.match(build, /Remove-Item -LiteralPath \$certificate -Force/u)
  assert.match(build, /security delete-keychain "\$keychain"/u)
  assert.match(build, /trap 'cleanup_signing_material \$\?' EXIT/u)
  assert.match(build, /trap 'cleanup_notary_material \$\?' EXIT/u)
  assert.match(build, /::add-mask::\$SIGNING_IDENTITY/u)
  assert.match(build, /::add-mask::\$env:CERTIFICATE_PASSWORD/u)
  assert.match(build, /::add-mask::\$APPLE_NOTARY_KEY_BASE64/u)
  assert.match(build, /::add-mask::\$APPLE_NOTARY_KEY_ID/u)
  assert.match(build, /Cleanup Windows signing material after every outcome[\s\S]*if: always\(\)/u)
  assert.match(build, /Cleanup Apple signing and notarization material after every outcome[\s\S]*if: always\(\)/u)
  assert.match(build, /trap 'cleanup_signing_material 143' TERM/u)
  assert.match(build, /trap 'cleanup_notary_material 143' TERM/u)
  assert.doesNotMatch(build, /Remove-Item[^\n]*SilentlyContinue/u)
  assert.doesNotMatch(build, /signtool sign[^\n]*\/p/u)
  assert.match(build, /Import-PfxCertificate[\s\S]*-CertStoreLocation \$storePath/u)
  assert.match(build, /ulimit -c 0/u)
  assert.doesNotMatch(release, /GITHUB_(?:ENV|OUTPUT)[^\n]*(?:TOKEN|PASSWORD|CERTIFICATE|SIGNING|NOTARY)/iu)
  assert.doesNotMatch(release, /actions\/cache@[\s\S]{0,500}(?:\.pfx|\.p12|\.p8|keychain|signature-verification)/iu)
  assert.match(build, /isolated signing store collision/u)
  assert.match(build, /imported certificate identity mismatch/u)
  assert.doesNotMatch(build, /Cert:\\(?:LocalMachine|CurrentUser)\\(?:Root|CA|AuthRoot)/u)
  assert.doesNotMatch(build, /notarytool store-credentials|--keychain-profile/u)
  assert.match(build, /origami2-keychain-search-list\.bin/u)
  assert.match(build, /security list-keychains -d user -s "\$\{original_keychains\[@\]\}"/u)
  const secretReferences = build.match(/\$\{\{ secrets\.[A-Z0-9_]+ \}\}/gu) ?? []
  assert.equal(secretReferences.length, 10)
  for (const step of [
    'Sign Windows portable executable',
    'Sign Windows installer',
    'Sign macOS application',
    'Notarize and staple macOS application',
  ]) {
    const offset = build.indexOf(`- name: ${step}`)
    assert.ok(offset >= 0)
    assert.match(
      build.slice(offset, offset + 300),
      /needs\.validate\.outputs\.mode != 'dry-run'/u,
    )
  }
})

test('macOS signing binds the bundle to the configured leaf identity and hardened runtime', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = readFileSync(
    join(root, '.github/scripts/verify_macos_signing_identity.sh'),
    'utf8',
  )
  assert.match(workflow, /APPLE_SIGNING_IDENTITY="\$SIGNING_IDENTITY"[\s\\]*bash \.\.\/\.\.\/\.github\/scripts\/verify_macos_signing_identity\.sh/u)
  assert.match(verifier, /Developer\\ ID\\ Application/u)
  assert.match(verifier, /grep -Fqx "Authority=\$expected_identity"/u)
  assert.match(verifier, /TeamIdentifier=\[A-Z0-9\]\{10\}/u)
  assert.match(verifier, /flags=\.\*runtime/u)
  assert.match(verifier, /timestamp_epoch >= run_started_epoch - 300/u)
  assert.match(workflow, /notarytool submit[\s\S]*--output-format json[\s\S]*jq -r \.status/u)
  assert.match(verifier, /codesign --verify --deep --strict/u)
  assert.doesNotMatch(verifier, /echo[^\n]*expected_identity/u)
})

test('Windows signing binds each asset to the configured PFX chain and RFC 3161 timestamp', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = readFileSync(
    join(root, '.github/scripts/verify_windows_signing_identity.ps1'),
    'utf8',
  )
  assert.equal(
    workflow.match(/verify_windows_signing_identity\.ps1/gu)?.length ?? 0,
    2,
  )
  assert.match(verifier, /Get-PfxData -FilePath \$Certificate -Password \$password/u)
  assert.match(verifier, /SignerCertificate\.Thumbprint -cne \$leaf\.Thumbprint/u)
  assert.match(verifier, /1\.3\.6\.1\.5\.5\.7\.3\.3/u)
  assert.match(verifier, /1\.3\.6\.1\.5\.5\.7\.3\.8/u)
  assert.match(verifier, /signtool verify \/v \/pa \/all \/tw/u)
  assert.match(verifier, /windows-signature-verification\.log/u)
  assert.match(verifier, /X509RevocationMode\]::Online/u)
  assert.match(verifier, /X509RevocationFlag\]::EntireChain/u)
  assert.match(verifier, /UrlRetrievalTimeout = \[TimeSpan\]::FromSeconds\(30\)/u)
  assert.match(verifier, /SignatureAlgorithm\.Value/u)
  assert.match(verifier, /KeySize -lt 2048/u)
  assert.match(verifier, /timestamp -lt \$runStartedAt\.AddMinutes\(-5\)/u)
  assert.match(workflow, /actions\/runs\/\$GITHUB_RUN_ID[\s\S]*run_started_at/u)
  assert.doesNotMatch(verifier, /Write-Output[^\n]*(?:Thumbprint|Subject|passwordText)/u)
})

test('CI always runs release contracts with read-only short-lived evidence', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const checkoutCount = workflow.match(/actions\/checkout@/gu)?.length ?? 0
  const nonPersistentCount = workflow.match(/persist-credentials: false/gu)?.length ?? 0
  assert.equal(checkoutCount, 6)
  assert.equal(nonPersistentCount, checkoutCount)
  assert.equal(workflow.match(/timeout-minutes:/gu)?.length ?? 0, 6)
  assert.match(workflow, /cancel-in-progress: true/u)
  assert.match(workflow, /permissions:\s*\n\s+contents: read/u)
  assert.match(workflow, /npm test/u)
  assert.match(
    workflow,
    /node --test \.\.\/\.\.\/\.github\/tests\/formal-release\.test\.mjs/u,
  )
  const uploadBlocks = workflow.match(
    /uses: actions\/upload-artifact@[\s\S]*?(?=\n\s{6}- |\n\s{2}[a-z-]+:|\s*$)/gu,
  ) ?? []
  assert.ok(uploadBlocks.length >= 6)
  for (const block of uploadBlocks) {
    assert.match(block, /retention-days: 7/u)
  }
})

test('CI cache action is pinned to the verified Node.js 24 release', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const pinned = 'actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9'
  assert.equal(workflow.split(pinned).length - 1, 2)
  assert.doesNotMatch(workflow, /actions\/cache@0057852bfaa89/u)
  assert.match(workflow, /# v6\.1\.0 \(Node\.js 24\)/u)
})

test('CI retains bounded browser accessibility evidence only on failure', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const smoke = readFileSync(
    join(root, 'apps/desktop/scripts/accessibility-browser-smoke.mjs'),
    'utf8',
  )
  assert.match(workflow, /id: accessibility-browser/u)
  assert.match(
    workflow,
    /if: failure\(\) && steps\.accessibility-browser\.outcome == 'failure'/u,
  )
  assert.match(workflow, /name: accessibility-browser-failure/u)
  assert.match(workflow, /if-no-files-found: error[\s\S]*retention-days: 7/u)
  assert.match(smoke, /origami2\.accessibility-failure\.v1/u)
  assert.match(smoke, /serverOutput\.slice\(-16_000\)/u)
  assert.match(smoke, /fullPage: true/u)
})

test('macOS CI rejects bounded adversarial bundle fixtures', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const verifier = readFileSync(
    join(root, '.github/scripts/verify_macos_bundle.sh'),
    'utf8',
  )
  const adversarial = readFileSync(
    join(root, '.github/tests/macos_bundle_adversarial_contract.sh'),
    'utf8',
  )
  assert.match(workflow, /macos_bundle_adversarial_contract\.sh/u)
  assert.match(verifier, /-type l -print \| sed/u)
  assert.match(verifier, /-links \+1/u)
  assert.match(verifier, /536870912/u)
  assert.match(verifier, /1048576/u)
  assert.match(verifier, /Contents\/MacOS must contain exactly/u)
  for (const fixture of [
    'symbolic-link',
    'hard-link',
    'extra-executable',
    'oversized-file',
    'wrong-version',
  ]) {
    assert.match(adversarial, new RegExp(fixture, 'u'))
  }
})

test('Windows CI rejects bounded adversarial bundle fixtures', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const verifier = readFileSync(join(root, 'scripts/verify_windows_bundle.ps1'), 'utf8')
  const adversarial = readFileSync(
    join(root, '.github/tests/windows_bundle_adversarial_contract.ps1'),
    'utf8',
  )
  assert.match(workflow, /windows_bundle_adversarial_contract\.ps1/u)
  assert.match(verifier, /FileAttributes\]::ReparsePoint/u)
  assert.match(verifier, /fsutil\.exe hardlink list/u)
  assert.match(verifier, /536870912/u)
  assert.match(verifier, /100000-file audit bound/u)
  assert.match(verifier, /Portable and embedded Windows executable payloads differ/u)
  for (const fixture of [
    'extra-dll',
    'hardlink-installer',
    'reparse-installer',
    'oversized-installer',
    'wrong-version',
    'substituted-portable',
  ]) {
    assert.match(adversarial, new RegExp(fixture, 'u'))
  }
})

test('Windows CI executes native recovery close and diagnostics persistence contracts', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const start = workflow.indexOf('Verify Windows-native recovery close and diagnostics persistence')
  const end = workflow.indexOf('Remove stale Windows bundle outputs', start)
  assert.ok(start > 0 && end > start)
  const step = workflow.slice(start, end)
  assert.match(step, /cargo test --locked -p origami2-desktop --lib recovery::tests -- --test-threads=1/u)
  assert.match(step, /cargo test --locked -p origami2-desktop --lib diagnostics::tests -- --test-threads=1/u)
  assert.match(step, /Windows recovery and close contract failed/u)
  assert.match(step, /Windows diagnostics persistence contract failed/u)
  assert.doesNotMatch(step, /continue-on-error|\|\| true/u)
})

test('CI requires the production C6 dyadic browser and exact native lifecycle', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const desktopPackage = JSON.parse(readFileSync(join(root, 'apps/desktop/package.json'), 'utf8'))
  const nativeRead = readFileSync(
    join(root, 'apps/desktop/src-tauri/src/stacked_fold_read.rs'),
    'utf8',
  )
  const browserRead = readFileSync(
    join(root, 'apps/desktop/scripts/dyadic-panel-browser-e2e.mjs'),
    'utf8',
  )
  assert.equal(
    desktopPackage.scripts['test:dyadic-panel-browser'],
    'node scripts/dyadic-panel-browser-e2e.mjs',
  )
  assert.equal(
    workflow.match(/npm run test:dyadic-panel-browser/gu)?.length,
    1,
    'the C6 production Panel browser harness must be one required frontend gate',
  )
  const exactFilter = 'stacked_fold_read::tests::even_cycle_exact_schedules_are_admitted_by_strict_dyadic_read'
  assert.equal(
    workflow.match(new RegExp(exactFilter, 'gu'))?.length,
    1,
    'one exact Rust command must cover every bounded even-cycle family without duplicate jobs',
  )
  assert.match(workflow, new RegExp(`cargo test --locked -p origami2-desktop --lib\\s+${exactFilter}\\s+-- --exact --test-threads=1`, 'u'))
  for (const fixtureTest of [
    'concave_boundary_strict_dyadic_read_fails_closed_without_mutation_authority',
    'cut_boundary_strict_dyadic_read_fails_closed_without_mutation_authority',
    'hole_boundary_strict_dyadic_read_fails_closed_without_mutation_authority',
    'open_cut_seam_strict_dyadic_preflight_is_unsupported_no_op',
    'nonfinite_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'degenerate_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'missing_boundary_vertex_strict_dyadic_preflight_is_unsupported_no_op',
    'duplicate_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'self_intersecting_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'zero_length_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'missing_pose_capability_strict_dyadic_read_returns_unsupported_dto',
    'tree_pose_capability_strict_dyadic_read_returns_unsupported_dto',
  ]) {
    const fixtureFilter = `stacked_fold_read::tests::${fixtureTest}`
    assert.equal(workflow.match(new RegExp(fixtureFilter, 'gu'))?.length, 1)
    assert.match(workflow, new RegExp(`cargo test --locked -p origami2-desktop --lib\\s+${fixtureFilter}\\s+-- --exact --test-threads=1`, 'u'))
    assert.match(nativeRead, new RegExp(`fn ${fixtureTest}\\(\\)`, 'u'))
  }
  for (const scenario of ['concave', 'cut', 'hole', 'seam', 'duplicate-boundary', 'self-intersection', 'zero-length', 'missing-capability', 'tree-capability']) {
    assert.match(browserRead, new RegExp(`'${scenario}'`, 'u'))
  }
  assert.match(browserRead, /reason unsupported_geometry/u)
  assert.match(browserRead, /openScenario\('no-path', 6\)/u)
  assert.match(browserRead, /reason no_certified_path/u)
  for (const fixture of ['octagonal-c8', 'radial-c16', 'cactus-c32', 'cactus-c64']) {
    assert.equal(nativeRead.match(new RegExp(`"${fixture}"`, 'gu'))?.length, 1)
  }
  assert.match(nativeRead, /dyadic_request_hinge_counts_are_bounded_v1\(64, Some\(64\)\)/u)
  assert.match(nativeRead, /!dyadic_request_hinge_counts_are_bounded_v1\(65, Some\(64\)\)/u)
  assert.match(nativeRead, /!dyadic_request_hinge_counts_are_bounded_v1\(64, Some\(65\)\)/u)
})

test('CI requires one complete named-technique instruction export browser gate', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const desktopPackage = JSON.parse(readFileSync(join(root, 'apps/desktop/package.json'), 'utf8'))
  const browserHarness = readFileSync(
    join(root, 'apps/desktop/scripts/miura-instruction-export-browser-e2e.mjs'),
    'utf8',
  )
  assert.equal(
    desktopPackage.scripts['test:miura-instruction-export-browser'],
    'node scripts/miura-instruction-export-browser-e2e.mjs',
  )
  assert.equal(
    workflow.match(/npm run test:miura-instruction-export-browser/gu)?.length,
    1,
    'all named techniques must run once in the required frontend job',
  )
  assert.doesNotMatch(
    workflow.match(/- name: Verify all named technique instruction exports[\s\S]*?run: npm run test:miura-instruction-export-browser/u)?.[0] ?? '',
    /continue-on-error|\|\| true/u,
  )
  for (const marker of [
    'miura', 'inside_reverse_fold', 'outside_reverse_fold', 'sink_fold', 'accordion_fold',
    'layer_selective', 'book_fold', 'squash_fold', 'petal_fold', 'crimp_fold',
    'mountain_fold', 'valley_fold',
  ]) {
    assert.match(browserHarness, new RegExp(`['"]${marker}['"]`, 'u'))
  }
})

test('CI requires exactly one full-App instruction export routing browser gate', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const desktopPackage = JSON.parse(readFileSync(join(root, 'apps/desktop/package.json'), 'utf8'))
  const browserHarness = readFileSync(
    join(root, 'apps/desktop/scripts/app-instruction-export-browser-e2e.mjs'),
    'utf8',
  )
  assert.equal(
    desktopPackage.scripts['test:app-instruction-export-browser'],
    'node scripts/app-instruction-export-browser-e2e.mjs',
  )
  assert.equal(workflow.match(/npm run test:app-instruction-export-browser/gu)?.length, 1)
  assert.doesNotMatch(
    workflow.match(/- name: Verify full App instruction export routing[\s\S]*?run: npm run test:app-instruction-export-browser/u)?.[0] ?? '',
    /continue-on-error|\|\| true/u,
  )
  for (const marker of [
    "setSaveMode('failure')", "for (const mode of ['stale', 'tamper'])",
    'preview_instruction_export:pdf', 'preview_instruction_export:svg_zip',
    'save_instruction_export', 'cancel_instruction_export',
  ]) assert.match(browserHarness, new RegExp(marker.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&'), 'u'))
})

test('CI requires one native export gate for all eight real technique compilers', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const instructionExport = readFileSync(
    join(root, 'apps/desktop/src-tauri/src/instruction_export.rs'),
    'utf8',
  )
  assert.equal(
    workflow.match(/instruction_export::tests::compiled_/gu)?.length,
    1,
    'the eight real compilers must share one required Rust invocation',
  )
  assert.match(
    workflow,
    /cargo test --locked -p origami2-desktop --lib \\\s+instruction_export::tests::compiled_ \\\s+-- --test-threads=1/u,
  )
  const compilerMarkers = [
    'book_fold', 'basic_fold', 'reverse_fold', 'sink_fold', 'squash_fold', 'crimp_fold',
    'layer_selective', 'accordion_fold',
  ]
  const gate = workflow.match(
    /- name: Verify all real named-technique compilers reach native exports[\s\S]*?-- --test-threads=1/u,
  )?.[0] ?? ''
  assert.doesNotMatch(gate, /continue-on-error|\|\| true/u)
  for (const marker of compilerMarkers) {
    assert.match(gate, new RegExp(`(?:coverage=|,)${marker}(?:,|'|$)`, 'u'))
    assert.match(instructionExport, new RegExp(`compile_certified_${marker}_timeline_v1`, 'u'))
  }
})

test('promotion reuses and verifies the complete prerelease asset set', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const promote = workflow.slice(workflow.indexOf('  promote:'))
  assert.match(promote, /gh release download "\$RELEASE_TAG"/u)
  assert.match(promote, /verify_merged_release_set\.mjs release/u)
  assert.match(promote, /verify_release_provenance\.sh/u)
  assert.match(promote, /before_prerelease="\$\(jq -r \.prerelease "\$before"\)"/u)
  assert.match(promote, /test "\$before_prerelease" = true \|\| test "\$before_prerelease" = false/u)
  assert.match(promote, /cmp "\$RUNNER_TEMP\/assets-before\.json"/u)
  assert.match(promote, /releases\/tags\/\$RELEASE_TAG" --jq \.id\)" = "\$release_id"/u)
  assert.match(promote, /commits\/\$RELEASE_TAG" --jq \.sha\)" = "\$RELEASE_COMMIT"/u)
  assert.match(promote, /patch_status=0/u)
  assert.match(promote, /if \[ "\$before_prerelease" = true \]; then/u)
  assert.match(promote, /for attempt in 1 2 3/u)
  assert.match(promote, /test "\$final_verified" = true/u)
  assert.match(promote, /releases\/\$release_id/u)
  assert.doesNotMatch(promote, /tauri build|tauri bundle|cargo build|npm run build/u)
  assert.ok(
    promote.indexOf('verify_release_provenance.sh') <
      promote.indexOf('gh api --method PATCH'),
  )
  assert.ok(
    promote.lastIndexOf('releases/tags/$RELEASE_TAG')
      > promote.indexOf('gh api --method PATCH'),
  )
  assert.ok(
    promote.lastIndexOf('commits/$RELEASE_TAG')
      > promote.indexOf('gh api --method PATCH'),
  )
})

test('promotion is retry-safe and accepts only an ID-bound fully verified final state', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const promote = workflow.slice(workflow.indexOf('  promote:'))
  assert.match(workflow, /group: formal-release-\$\{\{ inputs\.tag \|\| github\.ref_name \|\| github\.sha \}\}/u)
  assert.match(workflow, /cancel-in-progress: false/u)
  assert.match(promote, /before_prerelease.*true.*false/su)
  assert.match(promote, /if \[ "\$before_prerelease" = true \]; then[\s\S]*--method PATCH/u)
  assert.match(promote, /patch_status=0[\s\S]*final_verified=false[\s\S]*for attempt in 1 2 3/u)
  assert.match(promote, /\.id "\$after"\)" = "\$release_id"/u)
  assert.match(promote, /\.prerelease "\$after"\)" = false/u)
  assert.match(promote, /cmp "\$RUNNER_TEMP\/assets-before\.json" "\$RUNNER_TEMP\/assets-after\.json"/u)
})

test('publication and promotion share the exact merged release verifier', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const mergedVerifier = readFileSync(
    join(root, '.github/scripts/verify_merged_release_set.mjs'),
    'utf8',
  )
  assert.equal(workflow.match(/verify_merged_release_set\.mjs release/gu)?.length, 3)
  assert.match(mergedVerifier, /merged release asset set mismatch/u)
  assert.match(mergedVerifier, /verify_formal_release\.mjs/u)
  assert.match(mergedVerifier, /REQUIRE_SIGNATURE: 'false'/u)
  assert.match(mergedVerifier, /finally[\s\S]*rmSync/u)
  assert.ok(
    workflow.indexOf('verify_merged_release_set.mjs release') <
      workflow.indexOf('attest-build-provenance'),
  )

  const directory = mkdtempSync(join(tmpdir(), 'origami2-merged-release-'))
  try {
    writeFileSync(join(directory, 'unexpected-asset'), 'unexpected')
    assert.throws(
      () => execFileSync(
        'node',
        ['.github/scripts/verify_merged_release_set.mjs', directory],
        {
          cwd: root,
          stdio: 'pipe',
          env: {
            ...process.env,
            RELEASE_VERSION: '0.1.0',
            EXPECTED_SIGNATURE_POLICY: 'unsigned-dry-run',
          },
        },
      ),
      /merged release asset set mismatch/u,
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('merged release verification binds update manifests to signed payload policy', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const mergedVerifier = readFileSync(
    join(root, '.github/scripts/verify_merged_release_set.mjs'),
    'utf8',
  )
  const platformVerifier = readFileSync(
    join(root, '.github/scripts/verify_formal_release.mjs'),
    'utf8',
  )
  assert.match(mergedVerifier, /REQUIRE_SIGNATURE: 'false'/u)
  assert.match(mergedVerifier, /EXPECTED_SIGNATURE_POLICY: expectedSignaturePolicy/u)
  assert.equal(workflow.match(/EXPECTED_SIGNATURE_POLICY: platform-signed/gu)?.length ?? 0, 2)
  assert.match(platformVerifier, /const expectedSignaturePolicy = process\.env\.EXPECTED_SIGNATURE_POLICY/u)
  assert.match(platformVerifier, /signaturePolicy: expectedSignaturePolicy/u)
  assert.match(platformVerifier, /buildMode: expectedSignaturePolicy === 'platform-signed'/u)
  assert.match(platformVerifier, /EXPECTED_SIGNATURE_POLICY is invalid/u)
  assert.match(platformVerifier, /release mode and signature policy are inconsistent/u)
  assert.match(workflow, /REQUIRE_SIGNATURE:.*mode != 'dry-run'[\s\S]*EXPECTED_SIGNATURE_POLICY:.*mode != 'dry-run'/u)
})

test('CI attempt and suite evidence is transitively bound to every release integrity layer', () => {
  const artifactVerifier = readFileSync(
    join(root, '.github/scripts/verify_formal_release.mjs'),
    'utf8',
  )
  const manifestWriter = readFileSync(
    join(root, '.github/scripts/write_update_manifest.mjs'),
    'utf8',
  )
  const provenanceVerifier = readFileSync(
    join(root, '.github/scripts/verify_release_provenance.sh'),
    'utf8',
  )
  assert.match(artifactVerifier, /runAttempt: releaseEvidence\.ciChecks\.runAttempt/u)
  assert.match(artifactVerifier, /checkSuiteId: releaseEvidence\.ciChecks\.checkSuiteId/u)
  assert.match(artifactVerifier, /rustsecReviewArtifact/u)
  assert.match(artifactVerifier, /reportSha256/u)
  assert.match(artifactVerifier, /artifacts: releaseEvidence\.ciChecks\.artifacts/u)
  assert.match(artifactVerifier, /CycloneDX SBOM canonical release evidence mismatch/u)
  assert.match(manifestWriter, /`\$\{prefix\}\.cdx\.json`/u)
  assert.match(provenanceVerifier, /\.cdx\.json"/u)
  assert.match(provenanceVerifier, /\.update\.json"/u)
  assert.match(provenanceVerifier, /SHA256SUMS-/u)
})

test('CI and formal release share the strict macOS bundle contract', () => {
  const ciWorkflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = readFileSync(join(root, '.github/scripts/verify_macos_bundle.sh'), 'utf8')
  assert.match(ciWorkflow, /\.\/\.github\/scripts\/verify_macos_bundle\.sh/u)
  assert.match(ciWorkflow, /ORIGAMI2-macos-app\.tar\.gz/u)
  assert.match(releaseWorkflow, /verify_macos_bundle\.sh target\/release\/bundle\/macos\/ORIGAMI2\.app/u)
  assert.match(verifier, /CFBundleIdentifier/u)
  assert.match(verifier, /CFBundleShortVersionString/u)
  assert.match(verifier, /\[\[ -x "\$bundle\/Contents\/MacOS\/\$executable_name" \]\]/u)
  assert.match(verifier, /c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f/u)
  assert.match(releaseWorkflow, /xcrun notarytool submit/u)
  assert.match(releaseWorkflow, /xcrun stapler staple/u)
  assert.match(releaseWorkflow, /spctl --assess --type execute/u)
  assert.match(releaseWorkflow, /APPLE_NOTARY_KEY_BASE64/u)
})

test('CI and formal release share the strict Windows bundle contract', () => {
  const ciWorkflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = readFileSync(join(root, 'scripts/verify_windows_bundle.ps1'), 'utf8')
  assert.match(ciWorkflow, /verify_windows_bundle\.ps1[\s\S]*-ExpectedSignatureStatus NotSigned/u)
  assert.match(releaseWorkflow, /verify_windows_bundle\.ps1[\s\S]*-ExpectedVersion \$env:RELEASE_VERSION/u)
  assert.match(releaseWorkflow, /SIGNATURE_STATUS:.*Valid.*NotSigned/u)
  assert.match(verifier, /GetVersionInfo/u)
  assert.match(verifier, /Get-AuthenticodeSignature/u)
  assert.match(verifier, /TimeStamperCertificate/u)
  assert.match(verifier, /RFC 3161 timestamp/u)
  assert.match(verifier, /Embedded Windows executable/u)
  assert.match(verifier, /NotoSansJP-Variable\.ttf/u)
  assert.match(verifier, /NotoSansJP-OFL\.txt/u)
})

test('dry-run validates without a tag or GitHub mutation', () => {
  const temporary = mkdtempSync(join(tmpdir(), 'origami2-caller-cwd-'))
  try {
    const outputs = [root, join(root, 'apps', 'desktop'), temporary].map((cwd) =>
      execFileSync('node', [join(root, '.github/scripts/validate_formal_release.mjs')], {
        cwd,
        encoding: 'utf8',
        env: {
          ...process.env,
          GITHUB_OUTPUT: undefined,
          REQUESTED_MODE: 'dry-run',
          REQUESTED_TAG: '',
        },
      }))
    assert.equal(new Set(outputs).size, 1)
    assert.match(outputs[0], /mode=dry-run/u)
  } finally {
    rmSync(temporary, { recursive: true, force: true })
  }
  assert.throws(
    () => execFileSync('node', ['.github/scripts/validate_formal_release.mjs'], {
      cwd: root,
      stdio: 'pipe',
      env: { ...process.env, REQUESTED_MODE: 'dry-run', REQUESTED_TAG: 'v0.1.0' },
    }),
    /dry-run must not select a release tag/u,
  )
})

test('update manifest generator emits canonical version and digest bindings', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-update-manifest-'))
  try {
    const prefix = 'ORIGAMI2-v0.1.0-macos-arm64'
    const payloads = {
      [`${prefix}-app.tar.gz`]: 'application',
      [`${prefix}.cdx.json`]: JSON.stringify({ bomFormat: 'CycloneDX', components: [] }),
    }
    for (const [name, value] of Object.entries(payloads)) {
      writeFileSync(join(directory, name), value)
    }
    execFileSync(
      'node',
      ['.github/scripts/write_update_manifest.mjs', directory],
      {
        cwd: root,
        env: {
          ...process.env,
          PLATFORM: 'macos-arm64',
          VERSION: '0.1.0',
          SIGNATURE_POLICY: 'unsigned-dry-run',
        },
      },
    )
    const bytes = readFileSync(join(directory, `${prefix}.update.json`), 'utf8')
    const parsed = JSON.parse(bytes)
    assert.equal(bytes, `${JSON.stringify(parsed)}\n`)
    assert.deepEqual(parsed, {
      schema: 'origami2.update-manifest.v1',
      version: '0.1.0',
      platform: 'macos-arm64',
      signaturePolicy: 'unsigned-dry-run',
      assets: Object.entries(payloads).sort(([left], [right]) =>
        left.localeCompare(right)).map(([name, value]) => ({
        name,
        sha256: createHash('sha256').update(value).digest('hex'),
      })),
    })
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('release helpers reject hostile shell inputs without reflecting secret values', () => {
  const rejectedStderr = (command, args, env) => {
    try {
      execFileSync(command, args, { cwd: root, env: { ...process.env, ...env }, stdio: 'pipe' })
      assert.fail('hostile release input was accepted')
    } catch (error) {
      return `${error.stdout ?? ''}${error.stderr ?? ''}`
    }
  }
  const secret = 'DO_NOT_LOG_release_secret_7f3a'
  for (const hostileMode of [`stable\n${secret}`, `--${secret}`, `*${secret}`]) {
    const stderr = rejectedStderr(
      'node',
      ['.github/scripts/validate_formal_release.mjs'],
      { REQUESTED_MODE: hostileMode, REQUESTED_TAG: '' },
    )
    assert.match(stderr, /unsupported release mode/u)
    assert.doesNotMatch(stderr, new RegExp(secret, 'u'))
  }
  for (const hostileTag of [`v0.1.0\n${secret}`, `--${secret}`, `v0.1.*${secret}`]) {
    const stderr = rejectedStderr(
      'node',
      ['.github/scripts/validate_formal_release.mjs'],
      { REQUESTED_MODE: 'stable', REQUESTED_TAG: hostileTag },
    )
    assert.match(stderr, /release tag does not match application version/u)
    assert.doesNotMatch(stderr, new RegExp(secret, 'u'))
  }
  for (const [name, value] of [
    ['VERSION', `0.1.0\n${secret}`],
    ['PLATFORM', `--${secret}`],
    ['PLATFORM', `*${secret}`],
  ]) {
    const stderr = rejectedStderr(
      process.platform === 'win32' ? 'powershell.exe' : 'pwsh',
      ['-NoProfile', '-File', '.github/scripts/package_formal_release.ps1'],
      {
        VERSION: '0.1.0',
        PLATFORM: 'windows-x64',
        [name]: value,
      },
    )
    assert.match(stderr, /Release (?:version|platform)/u)
    assert.doesNotMatch(stderr, new RegExp(secret, 'u'))
  }
  for (const hostilePath of [`--${secret}`, `*${secret}`, `bad\n${secret}`]) {
    const stderr = rejectedStderr(
      'node',
      ['.github/scripts/write_update_manifest.mjs', hostilePath],
      {
        VERSION: '0.1.0',
        PLATFORM: 'windows-x64',
        SIGNATURE_POLICY: 'unsigned-dry-run',
      },
    )
    assert.match(stderr, /invalid update manifest directory path/u)
    assert.doesNotMatch(stderr, new RegExp(secret, 'u'))
  }
  const sbomDirectory = mkdtempSync(join(tmpdir(), 'origami2-hostile-identity-'))
  try {
    const sbomPath = join(sbomDirectory, 'fixture.cdx.json')
    for (const [name, value] of [
      ['RUSTC_VERSION', `rustc 1.90.0\n${secret}`],
      ['NODE_VERSION', `v24.0.0*${secret}`],
      ['TARGET_TRIPLE', `--${secret}`],
    ]) {
      writeFileSync(sbomPath, JSON.stringify({ bomFormat: 'CycloneDX', components: [] }))
      const stderr = rejectedStderr(
        'node',
        ['.github/scripts/bind_release_sbom.mjs', sbomPath],
        {
          VERSION: '0.1.0',
          PLATFORM: 'windows-x64',
          RELEASE_COMMIT: 'a'.repeat(40),
          EXPECTED_SIGNATURE_POLICY: 'unsigned-dry-run',
          RUSTC_VERSION: 'rustc 1.90.0 (fixture)',
          NODE_VERSION: 'v24.0.0',
          BUILD_MODE: 'unsigned-dry-run',
          TARGET_TRIPLE: 'x86_64-pc-windows-msvc',
          RELEASE_RUN_ID: '12345',
          RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:00Z',
          SOURCE_COMMIT_AUTHORED_AT: '2026-07-18T00:00:00Z',
          SOURCE_COMMIT_COMMITTED_AT: '2026-07-19T00:00:00Z',
          RELEASE_TAG_CREATED_AT: '',
          EXECUTED_TEST_COUNT: '33',
          CI_CHECK_EVIDENCE_JSON: JSON.stringify({
            schema: 'origami2.ci-check-evidence.v1',
            sourceCommit: 'a'.repeat(40),
          }),
          [name]: value,
        },
      )
      assert.match(stderr, /invalid (?:rustc version|Node\.js version|build target triple)/u)
      assert.doesNotMatch(stderr, new RegExp(secret, 'u'))
    }
  } finally {
    rmSync(sbomDirectory, { recursive: true, force: true })
  }
})

test('credential-free dry-run fixture proves the complete nine-asset handoff', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-formal-dry-run-'))
  try {
    const version = '0.1.0'
    const platformPayloads = new Map([
      ['windows-x64', {
        [`ORIGAMI2-v${version}-windows-x64-setup.exe`]: 'unsigned installer fixture',
        [`ORIGAMI2-v${version}-windows-x64-portable.zip`]: 'portable fixture',
        [`ORIGAMI2-v${version}-windows-x64.cdx.json`]: JSON.stringify({
          bomFormat: 'CycloneDX', components: [],
        }),
      }],
      ['macos-arm64', {
        [`ORIGAMI2-v${version}-macos-arm64-app.tar.gz`]: 'application fixture',
        [`ORIGAMI2-v${version}-macos-arm64.cdx.json`]: JSON.stringify({
          bomFormat: 'CycloneDX', components: [],
        }),
      }],
    ])
    for (const [platform, payloads] of platformPayloads) {
      for (const [name, bytes] of Object.entries(payloads)) {
        writeFileSync(join(directory, name), bytes)
      }
      execFileSync(
        'node',
        [
          '.github/scripts/bind_release_sbom.mjs',
          join(directory, `ORIGAMI2-v${version}-${platform}.cdx.json`),
        ],
        {
          cwd: root,
          env: {
            ...process.env,
            VERSION: version,
            PLATFORM: platform,
            RELEASE_COMMIT: 'a'.repeat(40),
            RUSTC_VERSION: 'rustc 1.90.0 (fixture)',
            NODE_VERSION: 'v24.0.0',
            BUILD_MODE: 'unsigned-dry-run',
            TARGET_TRIPLE: platform === 'windows-x64'
              ? 'x86_64-pc-windows-msvc'
              : 'aarch64-apple-darwin',
            RELEASE_RUN_ID: '12345',
            RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:00Z',
            SOURCE_COMMIT_AUTHORED_AT: '2026-07-18T00:00:00Z',
            SOURCE_COMMIT_COMMITTED_AT: '2026-07-19T00:00:00Z',
            RELEASE_TAG_CREATED_AT: '',
            EXECUTED_TEST_COUNT: '28',
            CI_CHECK_EVIDENCE_JSON: JSON.stringify({
              schema: 'origami2.ci-check-evidence.v1',
              sourceCommit: 'a'.repeat(40),
              workflow: '.github/workflows/ci.yml',
              workflowRunId: '67890',
              runAttempt: 1,
              checkSuiteId: '24680',
              checks: [{ name: 'test', conclusion: 'success' }],
              artifacts: ciArtifactsFixture,
              rustsecReviewArtifact: ciArtifactFixture,
            }),
          },
        },
      )
      execFileSync(
        'node',
        ['.github/scripts/write_update_manifest.mjs', directory],
        {
          cwd: root,
          env: {
            ...process.env,
            PLATFORM: platform,
            VERSION: version,
            SIGNATURE_POLICY: 'unsigned-dry-run',
          },
        },
      )
      const names = [
        ...Object.keys(payloads),
        `ORIGAMI2-v${version}-${platform}.update.json`,
      ].sort()
      const checksums = names.map((name) =>
        `${createHash('sha256').update(readFileSync(join(directory, name))).digest('hex')}  ${name}`,
      )
      writeFileSync(
        join(directory, `SHA256SUMS-${platform}.txt`),
        `${checksums.join('\n')}\n`,
      )
    }
    const output = execFileSync(
      'node',
      ['.github/scripts/verify_merged_release_set.mjs', directory],
      {
        cwd: root,
        encoding: 'utf8',
        env: {
          ...process.env,
          EXPECTED_SIGNATURE_POLICY: 'unsigned-dry-run',
          RELEASE_VERSION: version,
          RELEASE_COMMIT: 'a'.repeat(40),
        },
      },
    )
    assert.match(output, /verified merged release set v0\.1\.0/u)
    const alternateCwdOutput = execFileSync(
      'node',
      [join(root, '.github/scripts/verify_merged_release_set.mjs'), directory],
      {
        cwd: directory,
        encoding: 'utf8',
        env: {
          ...process.env,
          EXPECTED_SIGNATURE_POLICY: 'unsigned-dry-run',
          RELEASE_VERSION: version,
          RELEASE_COMMIT: 'a'.repeat(40),
        },
      },
    )
    assert.match(alternateCwdOutput, /verified merged release set v0\.1\.0/u)
    assert.equal(readdirSync(directory).length, 9)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('CI dry-run rehearses ephemeral signed candidate through runtime staging', () => {
  const dryRun = readFileSync(join(root, '.github/scripts/run_release_dry_run.mjs'), 'utf8')
  const rehearsal = readFileSync(join(root, '.github/scripts/rehearse_release_candidate.mjs'), 'utf8')
  assert.match(dryRun, /rehearse_release_candidate\.mjs/u)
  for (const boundary of [
    'quick-generate-key', "git', ['tag', '-s'", 'extendedKeyUsage=codeSigning',
    "openssl', ['cms', '-sign'", 'CycloneDX', 'SHA256SUMS.txt',
    'slsa.dev/provenance/v1', 'write_update_manifest.mjs',
    'parseRuntimeUpdateManifest', 'stageAuthorizedRuntimePayload',
    'unsigned tag', 'wrong GPG keyring', 'wrong OS key',
    'expired certificate policy horizon', 'checksum tamper', 'provenance tamper',
    'SBOM tamper', 'cross-platform manifest swap', 'prerelease manifest',
    'rollback manifest', 'staging symlink', "LC_ALL: 'C'", "TZ: 'UTC'",
    'gpgVersion', 'opensslVersion',
  ]) assert.ok(rehearsal.includes(boundary), boundary)
  assert.doesNotMatch(rehearsal, /https?:\/\/(?!in-toto\.io|slsa\.dev|origami2\.invalid)/u)
})

test('SBOM completeness gate covers every locked npm and Cargo identity', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-sbom-completeness-'))
  try {
    const policy = buildDependencyPolicy()
    const components = [
      ...policy.thirdPartyNotices.map(({ package: name, version }) => {
        if (!name.startsWith('@')) return { name, version }
        const separator = name.indexOf('/')
        return { group: name.slice(0, separator), name: name.slice(separator + 1), version }
      }),
      ...policy.cargoLicenseDatabase.packages.map(({ package: identity }) => {
        const separator = identity.lastIndexOf('@')
        return { name: identity.slice(0, separator), version: identity.slice(separator + 1) }
      }),
    ].map((component, index) => ({ ...component, 'bom-ref': `dependency-${index}` }))
    const path = join(directory, 'complete.cdx.json')
    writeFileSync(path, JSON.stringify({ bomFormat: 'CycloneDX', components }))
    execFileSync(process.execPath, [join(root, '.github/scripts/verify_sbom_completeness.mjs'), path])
    writeFileSync(path, JSON.stringify({ bomFormat: 'CycloneDX', components: components.slice(1) }))
    assert.throws(
      () => execFileSync(process.execPath, [join(root, '.github/scripts/verify_sbom_completeness.mjs'), path]),
      /CycloneDX SBOM omits locked dependencies/u,
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('CycloneDX binding records exact locks commit version platform and toolchains', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-sbom-binding-'))
  try {
    const path = join(directory, 'sbom.json')
    const bind = () => execFileSync('node', ['.github/scripts/bind_release_sbom.mjs', path], {
      cwd: root,
      stdio: 'pipe',
      env: {
        ...process.env,
        VERSION: '0.1.0',
        PLATFORM: 'windows-x64',
        RELEASE_COMMIT: 'a'.repeat(40),
        RUSTC_VERSION: 'rustc 1.90.0 (fixture)',
        NODE_VERSION: 'v24.0.0',
        BUILD_MODE: 'unsigned-dry-run',
        TARGET_TRIPLE: 'x86_64-pc-windows-msvc',
        RELEASE_RUN_ID: '12345',
        RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:00Z',
        SOURCE_COMMIT_AUTHORED_AT: '2026-07-18T00:00:00Z',
        SOURCE_COMMIT_COMMITTED_AT: '2026-07-19T00:00:00Z',
        RELEASE_TAG_CREATED_AT: '',
        EXECUTED_TEST_COUNT: '28',
        CI_CHECK_EVIDENCE_JSON: JSON.stringify({
          schema: 'origami2.ci-check-evidence.v1',
          sourceCommit: 'a'.repeat(40),
          workflow: '.github/workflows/ci.yml',
          workflowRunId: '67890',
          runAttempt: 1,
          checkSuiteId: '24680',
          checks: [{ name: 'test', conclusion: 'success' }],
          artifacts: ciArtifactsFixture,
          rustsecReviewArtifact: ciArtifactFixture,
        }),
      },
    })
    writeFileSync(path, JSON.stringify({
      bomFormat: 'CycloneDX',
      components: [{ 'bom-ref': 'one', purl: 'pkg:cargo/one@1' }],
    }))
    bind()
    const sbom = JSON.parse(readFileSync(path, 'utf8'))
    assert.deepEqual(sbom.metadata.component, {
      type: 'application', name: 'ORIGAMI2', version: '0.1.0',
    })
    const properties = Object.fromEntries(
      sbom.metadata.properties.map(({ name, value }) => [name, value]),
    )
    assert.equal(properties['origami2.release.source-commit'], 'a'.repeat(40))
    assert.equal(properties['origami2.release.platform'], 'windows-x64')
    assert.equal(properties['origami2.build.rustc-version'], 'rustc 1.90.0 (fixture)')
    assert.equal(
      properties['origami2.build.cargo-lock-sha256'],
      createHash('sha256').update(readFileSync(join(root, 'Cargo.lock'))).digest('hex'),
    )
    assert.equal(properties['origami2.build.identity-json'], JSON.stringify({
      schema: 'origami2.build-identity.v1',
      sourceCommit: 'a'.repeat(40),
      version: '0.1.0',
      platform: 'windows-x64',
      cargoLockSha256: properties['origami2.build.cargo-lock-sha256'],
      packageLockSha256: properties['origami2.build.package-lock-sha256'],
      rustcVersion: 'rustc 1.90.0 (fixture)',
      nodeVersion: 'v24.0.0',
      buildMode: 'unsigned-dry-run',
      targetTriple: 'x86_64-pc-windows-msvc',
    }))
    assert.equal(
      properties['origami2.dependency.policy-json'],
      JSON.stringify(buildDependencyPolicy()),
    )
    assert.equal(properties['origami2.release.evidence-json'], JSON.stringify({
      schema: 'origami2.release-evidence.v1',
      sourceCommit: 'a'.repeat(40),
      ciRunId: '12345',
      runStartedAt: '2026-07-21T00:00:00Z',
      sourceCommitAuthoredAt: '2026-07-18T00:00:00Z',
      sourceCommitCommittedAt: '2026-07-19T00:00:00Z',
      releaseTagCreatedAt: null,
      executedTestCount: 28,
      executedSuites: ['formal-release-contract'],
      ciChecks: {
        schema: 'origami2.ci-check-evidence.v1',
        sourceCommit: 'a'.repeat(40),
        workflow: '.github/workflows/ci.yml',
        workflowRunId: '67890',
        runAttempt: 1,
        checkSuiteId: '24680',
        checks: [{ name: 'test', conclusion: 'success' }],
        artifacts: ciArtifactsFixture,
        rustsecReviewArtifact: ciArtifactFixture,
      },
      rustsecWarningReview: buildDependencyPolicy().vulnerabilityAssessment.rustsecReviewReport,
    }))
    const rootBoundBytes = readFileSync(path, 'utf8')
    execFileSync('node', [join(root, '.github/scripts/bind_release_sbom.mjs'), path], {
      cwd: directory,
      stdio: 'pipe',
      env: {
        ...process.env,
        VERSION: '0.1.0',
        PLATFORM: 'windows-x64',
        RELEASE_COMMIT: 'a'.repeat(40),
        RUSTC_VERSION: 'rustc 1.90.0 (fixture)',
        NODE_VERSION: 'v24.0.0',
        BUILD_MODE: 'unsigned-dry-run',
        TARGET_TRIPLE: 'x86_64-pc-windows-msvc',
        RELEASE_RUN_ID: '12345',
        RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:00Z',
        SOURCE_COMMIT_AUTHORED_AT: '2026-07-18T00:00:00Z',
        SOURCE_COMMIT_COMMITTED_AT: '2026-07-19T00:00:00Z',
        RELEASE_TAG_CREATED_AT: '',
        EXECUTED_TEST_COUNT: '28',
        CI_CHECK_EVIDENCE_JSON: JSON.stringify({
          schema: 'origami2.ci-check-evidence.v1',
          sourceCommit: 'a'.repeat(40),
          workflow: '.github/workflows/ci.yml',
          workflowRunId: '67890',
          runAttempt: 1,
          checkSuiteId: '24680',
          checks: [{ name: 'test', conclusion: 'success' }],
          artifacts: ciArtifactsFixture,
          rustsecReviewArtifact: ciArtifactFixture,
        }),
      },
    })
    assert.equal(readFileSync(path, 'utf8'), rootBoundBytes)

    writeFileSync(path, JSON.stringify({
      bomFormat: 'CycloneDX',
      components: [{ purl: 'duplicate' }, { purl: 'duplicate' }],
    }))
    assert.throws(bind, /duplicate CycloneDX purl/u)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('release chronology rejects missing offset future and replayed evidence at exact boundaries', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-chronology-'))
  const path = join(directory, 'sbom.json')
  const ciEvidence = (createdAt = '2026-07-20T00:00:00.000Z') => ({
    schema: 'origami2.ci-check-evidence.v1', sourceCommit: 'a'.repeat(40),
    workflow: '.github/workflows/ci.yml', workflowRunId: '67890', runAttempt: 1,
    checkSuiteId: '24680', checks: [{ name: 'test', conclusion: 'success' }],
    artifacts: ciArtifactsFixture.map((artifact) => ({
      ...artifact,
      createdAt,
      expiresAt: new Date(Date.parse(createdAt) + 7 * 86_400_000).toISOString(),
    })),
    rustsecReviewArtifact: ciArtifactFixture,
  })
  const run = (overrides = {}) => {
    writeFileSync(path, JSON.stringify({ bomFormat: 'CycloneDX', components: [] }))
    return execFileSync('node', ['.github/scripts/bind_release_sbom.mjs', path], {
      cwd: root, stdio: 'pipe',
      env: {
        ...process.env, VERSION: '0.1.0', PLATFORM: 'windows-x64',
        RELEASE_COMMIT: 'a'.repeat(40), RUSTC_VERSION: 'rustc 1.90.0 (fixture)',
        NODE_VERSION: 'v24.0.0', BUILD_MODE: 'unsigned-dry-run',
        TARGET_TRIPLE: 'x86_64-pc-windows-msvc', RELEASE_RUN_ID: '12345',
        RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:00Z',
        SOURCE_COMMIT_AUTHORED_AT: '2026-07-18T00:00:00Z',
        SOURCE_COMMIT_COMMITTED_AT: '2026-07-19T00:00:00Z', RELEASE_TAG_CREATED_AT: '',
        EXECUTED_TEST_COUNT: '28', CI_CHECK_EVIDENCE_JSON: JSON.stringify(ciEvidence()),
        ...overrides,
      },
    })
  }
  try {
    assert.throws(() => run({ RELEASE_RUN_STARTED_AT: '' }), /run start time/u)
    assert.throws(() => run({ RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:00.001Z' }), /run start time/u)
    assert.throws(() => run({ RELEASE_RUN_STARTED_AT: '2026-07-21T00:00:60Z' }), /run start time/u)
    assert.throws(() => run({ RELEASE_RUN_STARTED_AT: '2199-07-21T00:00:00Z' }), /run start time/u)
    assert.throws(() => run({ EXECUTED_TEST_COUNT: '9007199254740992' }), /test count/u)
    const duplicateKeyEvidence = JSON.stringify(ciEvidence()).replace(
      '{',
      '{"schema":"attacker-controlled",',
    )
    assert.throws(() => run({ CI_CHECK_EVIDENCE_JSON: duplicateKeyEvidence }), /CI check evidence is non-canonical/u)
    assert.throws(() => run({ SOURCE_COMMIT_AUTHORED_AT: '2026-07-18T00:00:00+00:00' }), /author time/u)
    assert.throws(() => run({ SOURCE_COMMIT_COMMITTED_AT: '2026-07-21T00:06:00Z' }), /chronology/u)
    assert.throws(() => run({ SOURCE_COMMIT_COMMITTED_AT: '2026-06-20T23:59:59Z' }), /chronology/u)
    assert.throws(() => run({
      SOURCE_COMMIT_COMMITTED_AT: '2026-07-20T00:10:00Z',
      CI_CHECK_EVIDENCE_JSON: JSON.stringify(ciEvidence('2026-07-20T00:04:59.000Z')),
    }), /chronology/u)
    assert.doesNotThrow(() => run({
      SOURCE_COMMIT_AUTHORED_AT: '2026-07-21T00:05:00Z',
      SOURCE_COMMIT_COMMITTED_AT: '2026-07-21T00:05:00Z',
      CI_CHECK_EVIDENCE_JSON: JSON.stringify(ciEvidence('2026-07-21T00:05:00.000Z')),
    }))
    const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
    assert.match(workflow, /git cat-file -t "refs\/tags\/\$RELEASE_TAG"\)" = tag/u)
    const verifier = readFileSync(join(root, '.github/scripts/verify_formal_release.mjs'), 'utf8')
    assert.match(verifier, /Object\.keys\(releaseEvidence\)\.sort\(\)/u)
    assert.match(verifier, /canonicalEvidenceTime/u)
    const binder = readFileSync(join(root, '.github/scripts/bind_release_sbom.mjs'), 'utf8')
    assert.match(binder, /isSymbolicLink\(\)/u)
    assert.match(binder, /file changed during binding/u)
    assert.match(binder, /bytes changed during binding/u)
    assert.match(binder, /writeSync\(sbomFd, output, 0, output\.length, 0\)/u)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('SBOM binder rejects shared and non-writable filesystem identities', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-sbom-filesystem-'))
  try {
    const source = join(directory, 'source.json')
    const hardlink = join(directory, 'hardlink.json')
    writeFileSync(source, '{"bomFormat":"CycloneDX","components":[]}')
    linkSync(source, hardlink)
    assert.throws(
      () => execFileSync('node', ['.github/scripts/bind_release_sbom.mjs', hardlink], { cwd: root, stdio: 'pipe' }),
      /exclusive non-sparse regular file/u,
    )
    if (process.platform !== 'win32') {
      rmSync(hardlink)
      chmodSync(source, 0o444)
      assert.throws(
        () => execFileSync('node', ['.github/scripts/bind_release_sbom.mjs', source], { cwd: root, stdio: 'pipe' }),
      )
      chmodSync(source, 0o644)
    }
    const binder = readFileSync(join(root, '.github/scripts/bind_release_sbom.mjs'), 'utf8')
    assert.match(binder, /pathStat\.blocks \* 512 < pathStat\.size/u)
    assert.match(binder, /fsyncSync\(sbomFd\)/u)
    assert.match(binder, /openedStat\.nlink !== 1/u)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('credential-free dependency policy bounds lock integrity and npm licenses', () => {
  const policy = buildDependencyPolicy()
  assert.equal(policy.schema, 'origami2.dependency-policy.v1')
  assert.equal(policy.result, 'pass')
  assert.equal(policy.cargoSources, 'registry-checksum-required;git-forbidden')
  assert.equal(policy.npmIntegrity, 'sha512-required')
  assert.deepEqual(policy.vulnerabilityAssessment, {
    status: 'ci-gated',
    npm: 'npm-audit-v2;node-24.11.1;audit-level-low',
    cargo: 'cargo-audit-0.22.2;rustsec-db-b5fc89b8be99e96f79194d8a6f11e9b4143b99f0;offline',
    rustsecAllowedWarnings: JSON.parse(readFileSync(
      join(root, '.github/rustsec-warning-ledger.json'),
      'utf8',
    )).entries.map(({ id }) => id),
    rustsecWarningLedger: JSON.parse(readFileSync(
      join(root, '.github/rustsec-warning-ledger.json'),
      'utf8',
    )),
    rustsecReviewReport: policy.vulnerabilityAssessment.rustsecReviewReport,
    scope: 'package-lock.json;Cargo.lock',
  })
  assert.ok(policy.cargoRegistryPackages > 0 && policy.cargoRegistryPackages <= 10000)
  assert.ok(policy.npmPackages > 0 && policy.npmPackages <= 10000)
  assert.match(policy.cargoLockSha256, /^[0-9a-f]{64}$/u)
  assert.match(policy.packageLockSha256, /^[0-9a-f]{64}$/u)
  assert.deepEqual(policy.npmLicenses, [...policy.npmLicenses].sort())
  assert.equal(policy.licenseDatabase.schema, 'origami2.lockfile-license-db.v1')
  assert.equal(policy.licenseDatabase.sha256, policy.packageLockSha256)
  assert.equal(policy.thirdPartyNotices.length, policy.npmPackages)
  assert.equal(policy.cargoLicenseDatabase.schema, 'origami2.cargo-license-db.v1')
  assert.equal(policy.cargoLicenseDatabase.cargoLockSha256, policy.cargoLockSha256)
  assert.equal(policy.cargoLicenseDatabase.packages.length, policy.cargoPackages)
  assert.ok(policy.cargoLicenseDatabase.packages.every(({ package: name, license }) => name && license))
  assert.ok(policy.cargoLicenseDatabase.packages.every(({ source, checksum }) => (
    (source === null && checksum === null)
    || (source === 'registry+https://github.com/rust-lang/crates.io-index'
      && /^[0-9a-f]{64}$/u.test(checksum))
  )))
  assert.deepEqual(
    policy.thirdPartyNotices.map(({ package: name }) => name),
    [...policy.thirdPartyNotices.map(({ package: name }) => name)].sort(),
  )
  assert.ok(policy.thirdPartyNotices.every((notice) => (
    notice.package
    && notice.version
    && policy.npmLicenses.includes(notice.license)
    && notice.resolved.startsWith('https://registry.npmjs.org/')
    && /^sha512-[A-Za-z0-9+/]+={0,2}$/u.test(notice.integrity)
  )))
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  assert.ok(
    workflow.indexOf('Verify lockfile content before release build')
      < workflow.indexOf('Build Windows portable executable'),
  )
  const policySource = readFileSync(join(root, '.github/scripts/dependency_policy.mjs'), 'utf8')
  assert.match(policySource, /npm package manifest and lockfile root are out of sync/u)
  assert.match(policySource, /canonicalRoot\(lockedRoot\) !== canonicalRoot\(packageManifest\)/u)
  assert.match(policySource, /Cargo source provenance mismatch/u)
  assert.match(policySource, /Cargo source provenance is not allowed/u)
  assert.match(policySource, /npm dependency source is missing or invalid/u)
  assert.match(policySource, /resolved\.hostname !== 'registry\.npmjs\.org'/u)
})

test('dependency policy is independent of the caller working directory', () => {
  const output = execFileSync('node', ['../../.github/scripts/dependency_policy.mjs'], {
    cwd: join(root, 'apps', 'desktop'),
    encoding: 'utf8',
  })
  assert.deepEqual(JSON.parse(output), buildDependencyPolicy())
})

test('release CI evidence rejects duplicate and incomplete check runs', () => {
  const verifierSource = readFileSync(join(root, '.github/scripts/verify_release_ci.mjs'), 'utf8')
  assert.match(verifierSource, /attempt <= 3/u)
  assert.match(verifierSource, /AbortSignal\.timeout\(30_000\)/u)
  assert.match(verifierSource, /retry-after/u)
  assert.match(verifierSource, /x-ratelimit-remaining/u)
  assert.match(verifierSource, /seconds > 30/u)
  assert.match(verifierSource, /unexpected 304 response/u)
  assert.match(verifierSource, /response identity changed during retry/u)
  assert.match(verifierSource, /accept-encoding': 'identity'/u)
  assert.match(verifierSource, /body is partial or oversized/u)
  const directory = mkdtempSync(join(tmpdir(), 'origami2-ci-evidence-'))
  try {
    const runsPath = join(directory, 'runs.json')
    const checksPath = join(directory, 'checks.json')
    const artifactsPath = join(directory, 'artifacts.json')
    const artifactArchivePath = join(directory, 'artifact.zip')
    const commit = 'b'.repeat(40)
    const reportBytes = Buffer.from(`${JSON.stringify(
      buildDependencyPolicy().vulnerabilityAssessment.rustsecReviewReport,
      null,
      2,
    )}\n`)
    const reportDigest = createHash('sha256').update(reportBytes).digest('hex')
    const artifactBytes = singleEntryZip('rustsec-warning-review.json', reportBytes)
    const artifactDigest = createHash('sha256').update(artifactBytes).digest('hex')
    writeFileSync(artifactArchivePath, artifactBytes)
    const successfulRun = (id = 42) => ({
      id,
      head_sha: commit,
      status: 'completed',
      conclusion: 'success',
      path: '.github/workflows/ci.yml',
      event: 'push',
      head_branch: 'main',
      run_attempt: 1,
      check_suite_id: 84,
      updated_at: new Date().toISOString(),
    })
    writeFileSync(runsPath, JSON.stringify({
      total_count: 1,
      workflow_runs: [successfulRun()],
    }))
    const verify = (extraEnv = {}) => execFileSync('node', ['.github/scripts/verify_release_ci.mjs'], {
      cwd: root,
      encoding: 'utf8',
      env: {
        ...process.env,
        RELEASE_COMMIT: commit,
        WORKFLOW_RUNS_FIXTURE: runsPath,
        CHECK_RUNS_FIXTURE: checksPath,
        ARTIFACTS_FIXTURE: artifactsPath,
        ARTIFACT_ARCHIVE_FIXTURE: artifactArchivePath,
        ...extraEnv,
      },
    })
    const check = (name, status = 'completed', conclusion = 'success') => ({
      name, status, conclusion,
      details_url: 'https://github.com/example/repo/actions/runs/42/job/1',
      app: { slug: 'github-actions' },
      check_suite: { id: 84 },
    })
    const requiredChecks = [
      'dependency-advisory-audit', 'frontend', 'macos-bundle',
      'rust (macos-latest)', 'rust (windows-latest)',
      'slicer-acceptance', 'windows-bundle',
    ]
    writeFileSync(checksPath, JSON.stringify({
      total_count: requiredChecks.length,
      check_runs: requiredChecks.map((name) => check(name)),
    }))
    const createdAt = new Date().toISOString()
    const expiresAt = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString()
    const artifactRecord = {
      id: 7, name: 'rustsec-warning-review', expired: false,
      size_in_bytes: artifactBytes.length, digest: `sha256:${artifactDigest}`,
      created_at: createdAt, expires_at: expiresAt,
      workflow_run: { id: 42, head_sha: commit },
    }
    const otherArtifacts = [
      { id: 8, name: 'ORIGAMI2-macos-app-42' },
      { id: 9, name: 'ORIGAMI2-windows-nsis-42' },
      { id: 10, name: 'sample-viewer-runtime-log' },
    ].map((entry) => ({
      ...entry, expired: false, size_in_bytes: 1, digest: `sha256:${'a'.repeat(64)}`,
      created_at: createdAt, expires_at: expiresAt,
      workflow_run: { id: 42, head_sha: commit },
    }))
    const writeArtifacts = (review = artifactRecord, additional = []) => writeFileSync(
      artifactsPath,
      JSON.stringify({
        total_count: 1 + otherArtifacts.length + additional.length,
        artifacts: [review, ...otherArtifacts, ...additional],
      }),
    )
    writeArtifacts()
    assert.deepEqual(JSON.parse(verify()), {
      schema: 'origami2.ci-check-evidence.v1',
      sourceCommit: commit,
      workflow: '.github/workflows/ci.yml',
      workflowRunId: '42',
      runAttempt: 1,
      checkSuiteId: '84',
      checks: requiredChecks.map((name) => ({ name, conclusion: 'success' })),
      artifacts: [
        { artifactId: '8', name: 'ORIGAMI2-macos-app-42', digest: `sha256:${'a'.repeat(64)}`, size: 1, createdAt, expiresAt },
        { artifactId: '9', name: 'ORIGAMI2-windows-nsis-42', digest: `sha256:${'a'.repeat(64)}`, size: 1, createdAt, expiresAt },
        { artifactId: '7', name: 'rustsec-warning-review', digest: `sha256:${artifactDigest}`, size: artifactBytes.length, createdAt, expiresAt },
        { artifactId: '10', name: 'sample-viewer-runtime-log', digest: `sha256:${'a'.repeat(64)}`, size: 1, createdAt, expiresAt },
      ],
      rustsecReviewArtifact: {
        artifactId: '7', name: 'rustsec-warning-review',
        digest: `sha256:${artifactDigest}`, archiveSha256: artifactDigest,
        reportSha256: reportDigest,
        size: artifactBytes.length, createdAt, expiresAt,
        workflowRunId: '42', runAttempt: 1, checkSuiteId: '84',
      },
    })
    assert.throws(() => verify({ API_LINK_FIXTURE: '<https://api.github.test/page=2>; rel="next"' }), /pagination is forbidden/u)
    writeFileSync(artifactsPath, JSON.stringify({
      total_count: 5,
      artifacts: [artifactRecord, ...otherArtifacts],
    }))
    assert.throws(verify, /artifact set is incomplete or outside bounds/u)
    writeArtifacts()
    writeFileSync(artifactArchivePath, Buffer.from('tampered'))
    assert.throws(verify, /artifact digest mismatch/u)
    writeFileSync(artifactArchivePath, artifactBytes)
    const surplusDeclared = Buffer.from(artifactBytes)
    surplusDeclared.writeUInt16LE(2, surplusDeclared.length - 14)
    surplusDeclared.writeUInt16LE(2, surplusDeclared.length - 12)
    const surplusDigest = createHash('sha256').update(surplusDeclared).digest('hex')
    writeFileSync(artifactArchivePath, surplusDeclared)
    writeArtifacts({
      ...artifactRecord, size_in_bytes: surplusDeclared.length, digest: `sha256:${surplusDigest}`,
    })
    assert.throws(verify, /ZIP entry set/u)
    const staleArchive = singleEntryZip('rustsec-warning-review.json', Buffer.from('{}\n'))
    const staleDigest = createHash('sha256').update(staleArchive).digest('hex')
    writeFileSync(artifactArchivePath, staleArchive)
    writeArtifacts({
      ...artifactRecord, size_in_bytes: staleArchive.length, digest: `sha256:${staleDigest}`,
    })
    assert.throws(verify, /non-canonical or stale/u)
    writeFileSync(artifactArchivePath, artifactBytes)
    writeArtifacts({ ...artifactRecord, expired: true })
    assert.throws(verify, /identity or retention/u)
    writeArtifacts(artifactRecord, [{
      ...otherArtifacts[0], id: 11, name: 'unknown-artifact',
    }])
    assert.throws(verify, /incomplete, duplicated, or unexpected/u)
    writeArtifacts(artifactRecord, [{ ...otherArtifacts[0], id: 11 }])
    assert.throws(verify, /incomplete, duplicated, or unexpected/u)
    otherArtifacts[0].expired = true
    writeArtifacts()
    assert.throws(verify, /identity or retention/u)
    otherArtifacts[0].expired = false
    writeArtifacts()
    writeFileSync(checksPath, JSON.stringify({
      total_count: requiredChecks.length - 1,
      check_runs: requiredChecks.slice(1).map((name) => check(name)),
    }))
    assert.throws(verify, /required check set/u)
    writeFileSync(runsPath, JSON.stringify({
      total_count: 1,
      workflow_runs: [{
        ...successfulRun(),
        updated_at: new Date(Date.now() - 15 * 24 * 60 * 60 * 1000).toISOString(),
      }],
    }))
    assert.throws(verify, /workflow run identity/u)
    writeFileSync(runsPath, JSON.stringify({ total_count: 1, workflow_runs: [successfulRun()] }))
    writeFileSync(checksPath, JSON.stringify({
      total_count: 2, check_runs: [check('test'), check('test')],
    }))
    assert.throws(verify, /duplicated/u)
    writeFileSync(checksPath, JSON.stringify({
      total_count: 1, check_runs: [check('test', 'in_progress', null)],
    }))
    assert.throws(verify, /incomplete or unsuccessful/u)
    writeFileSync(runsPath, JSON.stringify({
      total_count: 2,
      workflow_runs: [
        successfulRun(),
        successfulRun(43),
      ],
    }))
    assert.throws(verify, /exactly one successful/u)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('local artifact verifier accepts checksummed CycloneDX fixtures', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-release-contract-'))
  try {
    const prefix = 'ORIGAMI2-v0.1.0-windows-x64'
    const payloads = {
      [`${prefix}-setup.exe`]: 'installer',
      [`${prefix}-portable.zip`]: 'portable',
      [`${prefix}.cdx.json`]: JSON.stringify({ bomFormat: 'CycloneDX', components: [] }),
    }
    payloads[`${prefix}.update.json`] = `${JSON.stringify({
      schema: 'origami2.update-manifest.v1',
      version: '0.1.0',
      platform: 'windows-x64',
      signaturePolicy: 'unsigned-dry-run',
      assets: Object.entries(payloads).sort(([left], [right]) =>
        left.localeCompare(right)).map(([name, value]) => ({
        name,
        sha256: createHash('sha256').update(value).digest('hex'),
      })),
    })}\n`
    const checksums = []
    for (const [name, value] of Object.entries(payloads)) {
      writeFileSync(join(directory, name), value)
      checksums.push(`${createHash('sha256').update(value).digest('hex')}  ${name}`)
    }
    writeFileSync(
      join(directory, 'SHA256SUMS-windows-x64.txt'),
      `${checksums.sort((left, right) => left.slice(66).localeCompare(right.slice(66))).join('\n')}\n`,
    )
    const verifyOptions = {
      cwd: root,
      env: {
        ...process.env,
        RELEASE_PLATFORM: 'windows-x64',
        RELEASE_VERSION: '0.1.0',
        REQUIRE_SIGNATURE: 'false',
      },
    }
    execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', directory],
      verifyOptions,
    )

    const manifestName = `${prefix}.update.json`
    const tampered = JSON.parse(payloads[manifestName])
    tampered.assets[0].sha256 = '0'.repeat(64)
    const tamperedBytes = `${JSON.stringify(tampered)}\n`
    writeFileSync(join(directory, manifestName), tamperedBytes)
    const tamperedChecksums = checksums.map((line) =>
      line.endsWith(`  ${manifestName}`)
        ? `${createHash('sha256').update(tamperedBytes).digest('hex')}  ${manifestName}`
        : line,
    )
    writeFileSync(
      join(directory, 'SHA256SUMS-windows-x64.txt'),
      `${tamperedChecksums.join('\n')}\n`,
    )
    assert.throws(
      () => execFileSync(
        'node',
        ['.github/scripts/verify_formal_release.mjs', directory],
        { ...verifyOptions, stdio: 'pipe' },
      ),
      /digest binding failed/u,
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('local artifact verifier rejects non-canonical checksum manifests', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-release-contract-'))
  try {
    const prefix = 'ORIGAMI2-v0.1.0-windows-x64'
    const payloads = {
      [`${prefix}-setup.exe`]: 'installer',
      [`${prefix}-portable.zip`]: 'portable',
      [`${prefix}.cdx.json`]: JSON.stringify({ bomFormat: 'CycloneDX', components: [] }),
    }
    payloads[`${prefix}.update.json`] = `${JSON.stringify({
      schema: 'origami2.update-manifest.v1',
      version: '0.1.0',
      platform: 'windows-x64',
      signaturePolicy: 'unsigned-dry-run',
      assets: Object.entries(payloads).sort(([left], [right]) =>
        left.localeCompare(right)).map(([name, value]) => ({
        name,
        sha256: createHash('sha256').update(value).digest('hex'),
      })),
    })}\n`
    for (const [name, value] of Object.entries(payloads)) {
      writeFileSync(join(directory, name), value)
    }
    const entries = Object.entries(payloads).map(([name, value]) =>
      `${createHash('sha256').update(value).digest('hex')}  ${name}`,
    )
    const manifest = join(directory, 'SHA256SUMS-windows-x64.txt')
    const verify = () => execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', directory],
      {
        cwd: root,
        stdio: 'pipe',
        env: {
          ...process.env,
          RELEASE_PLATFORM: 'windows-x64',
          RELEASE_VERSION: '0.1.0',
          REQUIRE_SIGNATURE: 'false',
        },
      },
    )

    writeFileSync(manifest, `${entries.reverse().join('\n')}\n`)
    assert.throws(verify, /non-canonical/u)

    writeFileSync(manifest, `${entries.slice(0, 2).sort().join('\n')}\n`)
    assert.throws(verify, /incomplete/u)

    writeFileSync(manifest, `${[...entries, entries[0]].sort().join('\n')}\n`)
    assert.throws(verify, /non-canonical/u)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('local artifact verifier rejects unbounded platform and version input', () => {
  const verify = (platform, version) => () => execFileSync(
    'node',
    ['.github/scripts/verify_formal_release.mjs', root],
    {
      cwd: root,
      stdio: 'pipe',
      env: {
        ...process.env,
        RELEASE_PLATFORM: platform,
        RELEASE_VERSION: version,
        REQUIRE_SIGNATURE: 'false',
      },
    },
  )
  assert.throws(verify('linux-x64', '0.1.0'), /unsupported release platform/u)
  assert.throws(verify('windows-x64', '../0.1.0'), /invalid release version/u)
  assert.throws(verify('macos-arm64', '01.0.0'), /invalid release version/u)
})

test('local artifact verifier requires explicit signature policy and verifies packaged payloads', () => {
  const verifier = readFileSync(
    join(root, '.github/scripts/verify_formal_release.mjs'),
    'utf8',
  )
  assert.match(verifier, /origami2-desktop\.exe/u)
  assert.match(verifier, /origami2-macos-signature-/u)
  assert.match(verifier, /TimeStamperCertificate/u)
  assert.match(verifier, /signtool.*verify.*\/pa.*\/all/su)
  assert.match(verifier, /stapler', 'validate'/u)
  assert.match(verifier, /spctl', \['--assess'/u)
  assert.doesNotMatch(
    verifier,
    /target['"], ['"]release['"], ['"]bundle['"], ['"]macos/u,
  )
  assert.throws(
    () => execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', root],
      {
        cwd: root,
        stdio: 'pipe',
        env: {
          ...process.env,
          RELEASE_PLATFORM: 'windows-x64',
          RELEASE_VERSION: '0.1.0',
          REQUIRE_SIGNATURE: 'yes',
        },
      },
    ),
    /REQUIRE_SIGNATURE must be exactly true or false/u,
  )
  assert.throws(
    () => execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', root],
      {
        cwd: root,
        stdio: 'pipe',
        env: {
          ...process.env,
          RELEASE_PLATFORM: 'windows-x64',
          RELEASE_VERSION: '0.1.0',
          RELEASE_MODE: 'stable',
          REQUIRE_SIGNATURE: 'false',
        },
      },
    ),
    /release mode and signature policy are inconsistent/u,
  )
  assert.throws(
    () => execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', root],
      {
        cwd: root,
        stdio: 'pipe',
        env: {
          ...process.env,
          RELEASE_PLATFORM: 'windows-x64',
          RELEASE_VERSION: '1.2.3',
          RELEASE_MODE: 'dry-run',
          REQUIRE_SIGNATURE: 'true',
          EXPECTED_SIGNATURE_POLICY: 'platform-signed',
        },
      },
    ),
    /release mode and signature policy are inconsistent/u,
  )
})

test('release archive contract rejects traversal absolute and foreign-root entries', () => {
  assert.equal(
    validateReleaseArchiveEntries('windows-x64', [
      'origami2-desktop.exe',
      'fonts/NotoSansJP-Variable.ttf',
    ]),
    true,
  )
  assert.equal(
    validateReleaseArchiveEntries('macos-arm64', [
      'ORIGAMI2.app/',
      'ORIGAMI2.app/Contents/MacOS/origami2-desktop',
    ]),
    true,
  )
  for (const entries of [
    ['origami2-desktop.exe', '../outside'],
    ['origami2-desktop.exe', '/absolute'],
    ['origami2-desktop.exe', 'C:/absolute'],
    ['origami2-desktop.exe', 'fonts\\outside'],
    ['origami2-desktop.exe', 'fonts/./outside'],
    ['origami2-desktop.exe', 'fonts//outside'],
  ]) {
    assert.throws(
      () => validateReleaseArchiveEntries('windows-x64', entries),
      /unsafe path|traversal path/u,
    )
  }
  assert.throws(
    () => validateReleaseArchiveEntries('windows-x64', ['fonts/font.ttf']),
    /executable contract/u,
  )
  assert.throws(
    () => validateReleaseArchiveEntries(
      'windows-x64',
      ['origami2-desktop.exe', 'unexpected/file'],
    ),
    /unexpected root/u,
  )
  assert.throws(
    () => validateReleaseArchiveEntries(
      'windows-x64',
      ['origami2-desktop.exe', 'origami2-desktop.exe'],
    ),
    /duplicate entries/u,
  )
  assert.throws(
    () => validateReleaseArchiveEntries('macos-arm64', ['Other.app/file']),
    /unexpected root/u,
  )
})
