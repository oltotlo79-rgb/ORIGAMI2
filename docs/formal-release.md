# Formal GitHub Release

`.github/workflows/release.yml` is the cross-platform release path. It does not
publish from pull requests. Its default manual mode is `dry-run`, which builds,
packages, generates CycloneDX SBOMs and SHA-256 manifests, and verifies local
artifact contracts without requesting write or provenance permissions.

Publishing requires an existing annotated tag whose GPG/SSH signature passes
`git verify-tag`, whose commit is the checked-out commit, and whose `vX.Y.Z`
version matches `tauri.conf.json`.

## Modes

- `dry-run`: build and verify unsigned local workflow artifacts only.
- `prerelease`: build signed artifacts, attest provenance, and create a
  prerelease. A pushed canonical version tag uses this mode.
- `stable`: build signed artifacts and create a stable release directly.
- `promote`: verify the existing signed tag and change an existing prerelease
  to stable without rebuilding or replacing its assets.

The protected `formal-release` GitHub environment should require reviewer
approval. Only `publish` receives `contents: write`, `id-token: write`, and
`attestations: write`; `promote` receives only `contents: write`. Validation and
build jobs remain read-only and checkouts do not persist credentials.

## Required signing secrets

- `WINDOWS_CERTIFICATE_BASE64`
- `WINDOWS_CERTIFICATE_PASSWORD`
- `RELEASE_SIGNING_PUBLIC_KEY` (armored GPG public key trusted for release tags)
- `APPLE_CERTIFICATE_BASE64`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`

Windows publication signs and verifies both the portable executable and NSIS
installer. The portable ZIP includes the executable and required font/license
resources. macOS publication imports an ephemeral certificate, applies hardened
runtime signing to the `.app`, and verifies the resulting code signature.

Each platform artifact set contains a CycloneDX JSON SBOM and a SHA-256 manifest
covering every payload, including the SBOM. The publish job verifies both
manifests again, emits GitHub build-provenance attestations, and uses generated
release notes. It refuses to overwrite an existing release. Promotion preserves
the prerelease assets and notes.

Windows portable ZIP and macOS application tar.gz archives use the shared
deterministic archive writer. Entry order, timestamps, owners, modes, compression,
and symbolic-link rejection are fixed; the contract suite rebuilds both formats
after source metadata drift and requires byte-identical output.

Local contract tests run with:

```sh
node --test .github/tests/formal-release.test.mjs
```

They exercise dry-run validation and a synthetic checksummed artifact set
without contacting GitHub or creating a release.
