#!/usr/bin/env bash
set -euo pipefail

bundle="${1:-}"
expected_identity="${APPLE_SIGNING_IDENTITY:-}"

[[ -d "$bundle" ]] || { echo 'macOS signing verification requires an application bundle.' >&2; exit 1; }
[[ "$expected_identity" =~ ^Developer\ ID\ Application:\ [[:print:]]{1,160}\ \([A-Z0-9]{10}\)$ ]] || {
  echo 'macOS signing identity has an invalid shape.' >&2
  exit 1
}

details="$(codesign --display --verbose=4 "$bundle" 2>&1)" || {
  echo 'macOS signing metadata could not be read.' >&2
  exit 1
}
grep -Fqx "Authority=$expected_identity" <<<"$details" || {
  echo 'macOS bundle leaf signing authority does not match the configured identity.' >&2
  exit 1
}
grep -Eq '^TeamIdentifier=[A-Z0-9]{10}$' <<<"$details" || {
  echo 'macOS bundle TeamIdentifier is missing or invalid.' >&2
  exit 1
}
grep -Eq '^CodeDirectory .*flags=.*runtime' <<<"$details" || {
  echo 'macOS bundle is missing the hardened runtime flag.' >&2
  exit 1
}
codesign --verify --deep --strict --verbose=2 "$bundle"

echo 'verified macOS bundle signing identity, team binding, and hardened runtime'
