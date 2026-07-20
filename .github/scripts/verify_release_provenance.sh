#!/usr/bin/env bash
set -euo pipefail

directory="${1:-}"
version="${2:-}"
repository="${3:-}"
[[ -d "$directory" ]] || { echo 'release provenance directory is invalid' >&2; exit 1; }
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || {
  echo 'release provenance version is invalid' >&2; exit 1;
}
[[ "$repository" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || {
  echo 'release provenance repository is invalid' >&2; exit 1;
}

assets=(
  "ORIGAMI2-v${version}-windows-x64-setup.exe"
  "ORIGAMI2-v${version}-windows-x64-portable.zip"
  "ORIGAMI2-v${version}-windows-x64.cdx.json"
  "ORIGAMI2-v${version}-windows-x64.update.json"
  "ORIGAMI2-v${version}-macos-arm64-app.tar.gz"
  "ORIGAMI2-v${version}-macos-arm64.cdx.json"
  "ORIGAMI2-v${version}-macos-arm64.update.json"
  'SHA256SUMS-windows-x64.txt'
  'SHA256SUMS-macos-arm64.txt'
)
[[ "$(find "$directory" -maxdepth 1 -type f | wc -l | tr -d ' ')" -eq "${#assets[@]}" ]] || {
  echo 'release provenance asset set is incomplete or contains extras' >&2; exit 1;
}
for name in "${assets[@]}"; do
  file="$directory/$name"
  [[ -f "$file" && ! -L "$file" ]] || { echo 'release provenance asset is missing' >&2; exit 1; }
  verified=false
  for attempt in 1 2 3; do
    if gh attestation verify "$file" --repo "$repository"; then
      verified=true
      break
    fi
    [[ "$attempt" -eq 3 ]] || sleep 5
  done
  [[ "$verified" == true ]] || { echo 'release provenance verification failed' >&2; exit 1; }
done

echo 'verified provenance for the complete nine-asset release set'
