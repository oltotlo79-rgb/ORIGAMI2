import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import { validateReleaseArchiveEntries } from '../scripts/release_archive_contract.mjs'

const root = resolve(import.meta.dirname, '..', '..')

test('release workflow keeps publication permissions out of build jobs', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const validator = readFileSync(join(root, '.github/scripts/validate_formal_release.mjs'), 'utf8')
  assert.match(workflow, /options: \[dry-run, prerelease, stable, promote\]/u)
  assert.match(validator, /'git', \['verify-tag', tag\]/u)
  assert.match(workflow, /permissions:\s*\n\s+contents: read/u)
  assert.match(workflow, /attest-build-provenance@43d14bc2/u)
  assert.match(workflow, /gh release edit "\$RELEASE_TAG".*--prerelease=false --latest/u)
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
  assert.doesNotMatch(promote, /tauri build|tauri bundle|cargo build|npm run build/u)
  assert.ok(
    promote.indexOf('gh attestation verify') <
      promote.indexOf('gh release edit'),
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
