import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

test('runtime updater binds required Windows and macOS tools paths and signing contracts', async () => {
  const [native, release, windowsVerifier] = await Promise.all([
    readFile(new URL('../src-tauri/src/runtime_update.rs', import.meta.url), 'utf8'),
    readFile(new URL('../../../.github/workflows/release.yml', import.meta.url), 'utf8'),
    readFile(new URL('../../../.github/scripts/verify_windows_signing_identity.ps1', import.meta.url), 'utf8'),
  ])
  assert.match(native, /System32\/curl\.exe/u)
  assert.match(native, /WindowsPowerShell\/v1\.0\/powershell\.exe/u)
  assert.match(native, /Get-AuthenticodeSignature/u)
  assert.match(native, /TimeStamperCertificate/u)
  assert.match(native, /\/usr\/bin\/curl/u)
  assert.match(native, /\/usr\/bin\/tar/u)
  assert.match(native, /\/usr\/bin\/codesign/u)
  assert.match(native, /\/usr\/bin\/open/u)
  assert.match(native, /LOCALAPPDATA/u)
  assert.match(native, /Library\/Caches\/ORIGAMI2\/runtime-update-v1/u)
  assert.match(native, /FILE_FLAG_OPEN_REPARSE_POINT/u)
  assert.match(native, /libc::O_NOFOLLOW/u)
  assert.doesNotMatch(native, /Command::new\("(?:sh|bash|cmd)"\)/u)
  assert.match(release, /codesign --force --deep --options runtime/u)
  assert.match(windowsVerifier, /Get-AuthenticodeSignature/u)
})
