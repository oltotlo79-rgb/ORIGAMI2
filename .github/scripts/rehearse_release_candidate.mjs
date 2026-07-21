import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, join, resolve } from 'node:path'
import { copyFileSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'

import { authorizeRuntimeUpdate, parseRuntimeUpdateManifest } from '../../apps/desktop/src/lib/runtimeUpdateManifest.ts'
import { stageAuthorizedRuntimePayload } from '../../apps/desktop/src/lib/runtimeUpdatePayload.ts'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const version = '0.1.0'
const tag = `v${version}`
const temporary = mkdtempSync(join(tmpdir(), 'origami2-release-candidate-'))
const artifacts = join(temporary, 'artifacts')
const gnupg = join(temporary, 'gnupg')
const staging = join(temporary, 'staging')
mkdirSync(artifacts)
mkdirSync(gnupg, { mode: 0o700 })
mkdirSync(staging)

const fixedEnvironment = Object.freeze({
  LC_ALL: 'C', LANG: 'C', TZ: 'UTC', SOURCE_DATE_EPOCH: '1767225600',
})
const command = (program, args, { env = {}, ...options } = {}) => execFileSync(program, args, {
  cwd: root,
  env: { ...process.env, ...fixedEnvironment, GNUPGHOME: gnupg, ...env },
  stdio: 'pipe',
  ...options,
})
const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')
const mustFail = (operation, label) => {
  try { operation() } catch { return }
  throw new Error(`negative release fixture was accepted: ${label}`)
}

try {
  const cargoVersion = /^version = "([^"]+)"/mu.exec(readFileSync(join(root, 'Cargo.toml'), 'utf8'))?.[1]
  const tauriVersion = JSON.parse(readFileSync(join(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8')).version
  if (cargoVersion !== version || tauriVersion !== version) throw new Error('release tag and product versions diverged')
  const gpgVersion = command('gpg', ['--version'], { encoding: 'utf8' }).split('\n')[0]
  const opensslVersion = command('openssl', ['version'], { encoding: 'utf8' }).trim()
  if (!/^gpg \(GnuPG\) 2\.(?:2|4)\./u.test(gpgVersion)) throw new Error(`unaudited GPG version: ${gpgVersion}`)
  if (!/^OpenSSL 3\./u.test(opensslVersion)) throw new Error(`unaudited OpenSSL version: ${opensslVersion}`)

  command('gpg', ['--batch', '--pinentry-mode', 'loopback', '--passphrase', '', '--quick-generate-key', 'ORIGAMI2 Ephemeral Release <release-fixture@invalid.example>', 'rsa2048', 'sign', '1d'])
  const repository = join(temporary, 'repository')
  mkdirSync(repository)
  command('git', ['init', '--initial-branch=main'], { cwd: repository })
  command('git', ['config', 'user.name', 'ORIGAMI2 release rehearsal'], { cwd: repository })
  command('git', ['config', 'user.email', 'release-fixture@invalid.example'], { cwd: repository })
  command('git', ['config', 'user.signingkey', 'release-fixture@invalid.example'], { cwd: repository })
  writeFileSync(join(repository, 'VERSION'), `${version}\n`)
  command('git', ['add', 'VERSION'], { cwd: repository })
  command('git', ['commit', '-m', 'ephemeral release candidate'], { cwd: repository })
  command('git', ['tag', '-s', tag, '-m', 'ephemeral signed release candidate'], { cwd: repository })
  command('git', ['tag', '-v', tag], { cwd: repository })
  command('git', ['tag', `${tag}-unsigned`], { cwd: repository })
  mustFail(() => command('git', ['tag', '-v', `${tag}-unsigned`], { cwd: repository }), 'unsigned tag')

  const wrongGnupg = join(temporary, 'wrong-gnupg')
  mkdirSync(wrongGnupg, { mode: 0o700 })
  command('gpg', ['--batch', '--pinentry-mode', 'loopback', '--passphrase', '', '--quick-generate-key', 'ORIGAMI2 Wrong Key <wrong-key@invalid.example>', 'rsa2048', 'sign', '1d'], { env: { GNUPGHOME: wrongGnupg } })
  mustFail(() => command('git', ['tag', '-v', tag], { cwd: repository, env: { GNUPGHOME: wrongGnupg } }), 'wrong GPG keyring')

  const key = join(temporary, 'os-signing.key.pem')
  const certificate = join(temporary, 'os-signing.cert.pem')
  const wrongKey = join(temporary, 'wrong-os-signing.key.pem')
  const wrongCertificate = join(temporary, 'wrong-os-signing.cert.pem')
  command('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', '/CN=ORIGAMI2 Ephemeral OS Signing Fixture', '-addext', 'extendedKeyUsage=codeSigning', '-keyout', key, '-out', certificate])
  command('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', '/CN=ORIGAMI2 Wrong OS Signing Fixture', '-addext', 'extendedKeyUsage=codeSigning', '-keyout', wrongKey, '-out', wrongCertificate])
  mustFail(() => command('openssl', ['x509', '-in', certificate, '-checkend', '90000', '-noout']), 'expired certificate policy horizon')

  const platformPayloads = new Map([
    ['windows-x64', [`ORIGAMI2-v${version}-windows-x64-portable.zip`, `ORIGAMI2-v${version}-windows-x64-setup.exe`]],
    ['macos-arm64', [`ORIGAMI2-v${version}-macos-arm64-app.tar.gz`]],
  ])
  const manifests = new Map()
  for (const [platform, payloadNames] of platformPayloads) {
    for (const name of payloadNames) writeFileSync(join(artifacts, name), `signed release candidate fixture:${name}\n`)
    const sbomName = `ORIGAMI2-v${version}-${platform}.cdx.json`
    writeFileSync(join(artifacts, sbomName), `${JSON.stringify({
      bomFormat: 'CycloneDX', specVersion: '1.6', version: 1,
      metadata: { component: { type: 'application', name: 'ORIGAMI2', version } }, components: [],
    })}\n`)
    for (const name of [...payloadNames, sbomName]) {
      command('openssl', ['cms', '-sign', '-binary', '-in', join(artifacts, name), '-signer', certificate, '-inkey', key, '-outform', 'DER', '-out', `${join(artifacts, name)}.os-signature`, '-nosmimecap', '-md', 'sha256'])
    }
    const firstPayload = join(artifacts, payloadNames[0])
    mustFail(() => command('openssl', ['cms', '-verify', '-binary', '-inform', 'DER', '-in', `${firstPayload}.os-signature`, '-content', firstPayload, '-nointern', '-certfile', wrongCertificate, '-noverify', '-out', join(temporary, 'wrong-key-output.bin')]), `wrong OS key ${platform}`)
    command(process.execPath, [join(root, '.github/scripts/write_update_manifest.mjs'), artifacts], {
      env: { ...process.env, GNUPGHOME: gnupg, PLATFORM: platform, VERSION: version, SIGNATURE_POLICY: 'platform-signed' },
    })
    const manifestName = `ORIGAMI2-v${version}-${platform}.update.json`
    const manifestBody = readFileSync(join(artifacts, manifestName), 'utf8')
    const manifest = parseRuntimeUpdateManifest(manifestBody, platform)
    if (!manifest) throw new Error(`runtime parser rejected ${platform} release candidate`)
    const oppositePlatform = platform === 'windows-x64' ? 'macos-arm64' : 'windows-x64'
    if (parseRuntimeUpdateManifest(manifestBody, oppositePlatform) !== null) throw new Error('cross-platform manifest swap accepted')
    const prereleaseBody = JSON.stringify({ ...JSON.parse(manifestBody), version: `${version}-rc.1` })
    if (parseRuntimeUpdateManifest(prereleaseBody, platform) !== null) throw new Error('prerelease manifest accepted')
    const authorization = await authorizeRuntimeUpdate(
      { async requestManifest() { return manifestBody } },
      '0.0.0',
      platform,
    )
    if (authorization.kind !== 'authorized') throw new Error(`runtime authorization rejected ${platform}`)
    const rollback = await authorizeRuntimeUpdate({ async requestManifest() { return manifestBody } }, version, platform)
    if (rollback.kind !== 'rejected' || rollback.reason !== 'rollback') throw new Error('rollback manifest accepted')
    manifests.set(platform, authorization.authorization)
  }

  const releaseNames = [...platformPayloads.values()].flatMap((names) => names)
  releaseNames.push(
    `ORIGAMI2-v${version}-windows-x64.cdx.json`, `ORIGAMI2-v${version}-windows-x64.update.json`,
    `ORIGAMI2-v${version}-macos-arm64.cdx.json`, `ORIGAMI2-v${version}-macos-arm64.update.json`,
  )
  const checksums = releaseNames.sort().map((name) => `${sha256(join(artifacts, name))}  ${name}`).join('\n')
  const checksumsPath = join(artifacts, 'SHA256SUMS.txt')
  writeFileSync(checksumsPath, `${checksums}\n`)
  const provenancePath = join(artifacts, 'release-candidate.provenance.json')
  writeFileSync(provenancePath, `${JSON.stringify({
    _type: 'https://in-toto.io/Statement/v1',
    subject: releaseNames.map((name) => ({ name, digest: { sha256: sha256(join(artifacts, name)) } })),
    predicateType: 'https://slsa.dev/provenance/v1',
    predicate: { buildDefinition: { buildType: 'https://origami2.invalid/release-candidate-rehearsal/v1', externalParameters: { tag, version, locale: 'C', timezone: 'UTC', sourceDateEpoch: fixedEnvironment.SOURCE_DATE_EPOCH } }, runDetails: { builder: { id: 'local-ephemeral-ci-fixture' }, metadata: { gpgVersion, opensslVersion } } },
  })}\n`)
  for (const path of [checksumsPath, provenancePath]) {
    command('gpg', ['--batch', '--yes', '--armor', '--detach-sign', path])
    command('gpg', ['--batch', '--verify', `${path}.asc`, path])
  }
  for (const [path, label] of [[checksumsPath, 'checksum tamper'], [provenancePath, 'provenance tamper']]) {
    const tampered = `${path}.tampered`
    copyFileSync(path, tampered)
    writeFileSync(tampered, '\n', { flag: 'a' })
    mustFail(() => command('gpg', ['--batch', '--verify', `${path}.asc`, tampered]), label)
  }

  const windowsAuthorization = manifests.get('windows-x64')
  const windowsSbom = windowsAuthorization.assets.find(({ name }) => name.endsWith('.cdx.json'))
  const sbomTamper = await stageAuthorizedRuntimePayload(windowsAuthorization, windowsSbom.name, {
    transport: { async requestPayload() { return (async function * () { yield Buffer.from('tampered SBOM') })() } },
    signatureVerifier: { async verifyPlatformSignature() { return true } },
    staging: { async begin() { return { async write() {}, async commit() {}, async rollback() {} } } },
  })
  if (sbomTamper.kind !== 'rejected' || sbomTamper.reason !== 'hash_mismatch') throw new Error('SBOM tamper accepted')

  for (const [platform, manifest] of manifests) {
    for (const asset of manifest.assets.filter(({ name }) => !name.endsWith('.cdx.json'))) {
      const source = join(artifacts, asset.name)
      const result = await stageAuthorizedRuntimePayload(manifest, asset.name, {
        transport: { async requestPayload() { return (async function * () { yield readFileSync(source) })() } },
        signatureVerifier: { async verifyPlatformSignature() {
          const output = join(temporary, 'verified-payload.bin')
          command('openssl', ['cms', '-verify', '-binary', '-inform', 'DER', '-in', `${source}.os-signature`, '-content', source, '-certfile', certificate, '-noverify', '-out', output])
          return sha256(output) === sha256(source)
        } },
        staging: { async begin() {
          const target = join(staging, platform, asset.name)
          mkdirSync(dirname(target), { recursive: true })
          let chunks = []
          return { async write(chunk) { chunks.push(Buffer.from(chunk)) }, async commit() { writeFileSync(target, Buffer.concat(chunks)); chunks = [] }, async rollback() { chunks = [] } }
        } },
      })
      if (result.kind !== 'staged' || sha256(join(staging, platform, asset.name)) !== asset.sha256) {
        throw new Error(`runtime staging failed for ${asset.name}`)
      }
    }
  }

  const symlinkTarget = join(temporary, 'outside-staging')
  mkdirSync(symlinkTarget)
  const symlinkPlatform = join(staging, 'symlink-platform')
  symlinkSync(symlinkTarget, symlinkPlatform, 'dir')
  const symlinkAsset = windowsAuthorization.assets.find(({ name }) => name.endsWith('-setup.exe'))
  const symlinkSource = join(artifacts, symlinkAsset.name)
  const symlinkResult = await stageAuthorizedRuntimePayload(windowsAuthorization, symlinkAsset.name, {
    transport: { async requestPayload() { return (async function * () { yield readFileSync(symlinkSource) })() } },
    signatureVerifier: { async verifyPlatformSignature() { return true } },
    staging: { async begin() {
      if (lstatSync(symlinkPlatform).isSymbolicLink()) throw new Error('symlink staging root')
      throw new Error('unreachable')
    } },
  })
  if (symlinkResult.kind !== 'rejected' || symlinkResult.reason !== 'storage') throw new Error('staging symlink accepted')
  process.stdout.write('network-free signed release candidate rehearsal passed: tag, SBOM, checksums, provenance, manifests, parser, staging\n')
} finally {
  rmSync(temporary, { recursive: true, force: true })
}
