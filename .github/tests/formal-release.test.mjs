import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import { validateReleaseArchiveEntries } from '../scripts/release_archive_contract.mjs'
import { buildDependencyPolicy } from '../scripts/dependency_policy.mjs'

const root = resolve(import.meta.dirname, '..', '..')

test('release workflow keeps publication permissions out of build jobs', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const validator = readFileSync(join(root, '.github/scripts/validate_formal_release.mjs'), 'utf8')
  assert.match(workflow, /options: \[dry-run, prerelease, stable, promote\]/u)
  assert.match(validator, /'git', \['verify-tag', tag\]/u)
  assert.match(workflow, /permissions:\s*\n\s+contents: read/u)
  assert.match(workflow, /attest-build-provenance@43d14bc2/u)
  assert.match(workflow, /releases\/\$release_id[\s\S]*prerelease=false/u)
  assert.doesNotMatch(workflow, /pull_request:/u)
})

test('publication binds generated notes tag and immutable remote commit', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  assert.match(workflow, /commit: \$\{\{ steps\.contract\.outputs\.commit \}\}/u)
  assert.match(publish, /commits\/\$RELEASE_TAG.*--jq \.sha/u)
  assert.match(publish, /test "\$remote_commit" = "\$RELEASE_COMMIT"/u)
  assert.match(publish, /releases\/generate-notes/u)
  assert.match(publish, /target_commitish="\$RELEASE_COMMIT"/u)
  assert.match(publish, /\.name "\$notes_json"\)" = "\$RELEASE_TAG"/u)
  assert.match(publish, /target_commitish="\$RELEASE_COMMIT"/u)
  assert.match(publish, /body=@"\$notes_file"/u)
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
  assert.match(publish, /--hostname uploads\.github\.com/u)
  assert.match(publish, /verify_remote_release_assets\.mjs/u)
  assert.match(publish, /-F draft=false/u)
  assert.match(publish, /gh api --method DELETE "repos\/\$GH_REPO\/releases\/\$created_release_id"/u)
  assert.match(publish, /\.target_commitish.*= "\$RELEASE_COMMIT"/u)
  assert.match(publish, /\.name "\$state"\)" = "\$transaction_name"/u)
  assert.ok(publish.lastIndexOf('verify_remote_release_assets.mjs') < publish.indexOf('-F draft=false'))
  assert.match(publish, /releases\/\$created_release_id\/assets/u)
  assert.match(publish, /releases\/tags\/\$RELEASE_TAG" --jq \.id\)" = "\$created_release_id"/u)
  assert.match(publish, /commits\/\$RELEASE_TAG" --jq \.sha\)" = "\$RELEASE_COMMIT"/u)
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
  assert.match(workflow, /! grep -Eiq '\^link:.*rel="next"'/u)
  assert.match(workflow, /--proto-redir '=https'/u)
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
  assert.match(publish, /unzip -tqq/u)
  assert.match(publish, /entry_count.*-le 16/u)
  assert.match(publish, /archive_bytes \* 200 \+ 1048576/u)
  assert.match(publish, /find release -type l/u)

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
    const output = execFileSync('node', [verifier, path], { encoding: 'utf8' })
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
        () => execFileSync('node', [verifier, path], { stdio: 'pipe' }),
        /workflow artifact/u,
      )
    }
    writeFileSync(path, ' '.repeat(1_048_577))
    assert.throws(
      () => execFileSync('node', [verifier, path], { stdio: 'pipe' }),
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
  assert.match(workflow, /release\/\*\.update\.json/u)
  assert.match(workflow, /\.update\.json" \\/u)
  assert.doesNotMatch(manifestWriter, /https?:|url|endpoint/iu)
  assert.match(runtimeContract, /delivery: 'release_page_only'/u)
  assert.match(runtimeContract, /runtimeUpdaterAvailable: false/u)
})

test('provenance subjects cover the complete verified nine-asset set', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const publish = workflow.slice(workflow.indexOf('  publish:'), workflow.indexOf('  promote:'))
  const promote = workflow.slice(workflow.indexOf('  promote:'))
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
    assert.ok(promote.includes(name), name)
  }
  assert.equal(promote.match(/gh attestation verify "\$asset"/gu)?.length, 1)
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
  assert.match(build, /trap cleanup_signing_material EXIT/u)
  assert.match(build, /trap 'rm -f -- "\$key" "\$archive"' EXIT/u)
  assert.match(build, /::add-mask::\$SIGNING_IDENTITY/u)
  assert.match(build, /::add-mask::\$APPLE_NOTARY_KEY_ID/u)
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

test('CI always runs release contracts with read-only short-lived evidence', () => {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const checkoutCount = workflow.match(/actions\/checkout@/gu)?.length ?? 0
  const nonPersistentCount = workflow.match(/persist-credentials: false/gu)?.length ?? 0
  assert.equal(checkoutCount, 5)
  assert.equal(nonPersistentCount, checkoutCount)
  assert.equal(workflow.match(/timeout-minutes:/gu)?.length ?? 0, 5)
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

test('promotion reuses and verifies the complete prerelease asset set', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const promote = workflow.slice(workflow.indexOf('  promote:'))
  assert.match(promote, /gh release download "\$RELEASE_TAG"/u)
  assert.match(promote, /verify_merged_release_set\.mjs release/u)
  assert.match(promote, /gh attestation verify "\$asset"/u)
  assert.match(promote, /\.prerelease "\$before"\)" = true/u)
  assert.match(promote, /cmp "\$RUNNER_TEMP\/assets-before\.json"/u)
  assert.match(promote, /releases\/tags\/\$RELEASE_TAG" --jq \.id\)" = "\$release_id"/u)
  assert.match(promote, /commits\/\$RELEASE_TAG" --jq \.sha\)" = "\$RELEASE_COMMIT"/u)
  assert.match(promote, /patch_status=0/u)
  assert.match(promote, /releases\/\$release_id/u)
  assert.doesNotMatch(promote, /tauri build|tauri bundle|cargo build|npm run build/u)
  assert.ok(
    promote.indexOf('gh attestation verify') <
      promote.indexOf('gh api --method PATCH'),
  )
})

test('publication and promotion share the exact merged release verifier', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const mergedVerifier = readFileSync(
    join(root, '.github/scripts/verify_merged_release_set.mjs'),
    'utf8',
  )
  assert.equal(workflow.match(/verify_merged_release_set\.mjs release/gu)?.length, 2)
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
          env: { ...process.env, RELEASE_VERSION: '0.1.0' },
        },
      ),
      /merged release asset set mismatch/u,
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
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
  const output = execFileSync('node', ['.github/scripts/validate_formal_release.mjs'], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, REQUESTED_MODE: 'dry-run', REQUESTED_TAG: '' },
  })
  assert.match(output, /mode=dry-run/u)
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
            EXECUTED_TEST_COUNT: '28',
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
          RELEASE_VERSION: version,
          RELEASE_COMMIT: 'a'.repeat(40),
        },
      },
    )
    assert.match(output, /verified merged release set v0\.1\.0/u)
    assert.equal(readdirSync(directory).length, 9)
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
        EXECUTED_TEST_COUNT: '28',
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
      executedTestCount: 28,
      executedSuites: ['formal-release-contract'],
    }))

    writeFileSync(path, JSON.stringify({
      bomFormat: 'CycloneDX',
      components: [{ purl: 'duplicate' }, { purl: 'duplicate' }],
    }))
    assert.throws(bind, /duplicate CycloneDX purl/u)
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
    status: 'not-performed',
    reason: 'external-vulnerability-database-not-queried',
    scope: 'this-release-policy-evidence',
  })
  assert.ok(policy.cargoRegistryPackages > 0 && policy.cargoRegistryPackages <= 10000)
  assert.ok(policy.npmPackages > 0 && policy.npmPackages <= 10000)
  assert.match(policy.cargoLockSha256, /^[0-9a-f]{64}$/u)
  assert.match(policy.packageLockSha256, /^[0-9a-f]{64}$/u)
  assert.deepEqual(policy.npmLicenses, [...policy.npmLicenses].sort())
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  assert.ok(
    workflow.indexOf('Verify locked dependency integrity and license policy')
      < workflow.indexOf('Bind SBOM to source locks, version, commit, and toolchains'),
  )
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
    /publishable release mode requires platform signatures/u,
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
