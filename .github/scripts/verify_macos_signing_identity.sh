#!/usr/bin/env bash
set -euo pipefail

bundle="${1:-}"
expected_identity="${APPLE_SIGNING_IDENTITY:-}"
run_started_at="${RELEASE_RUN_STARTED_AT:-}"

[[ -d "$bundle" ]] || { echo 'macOS signing verification requires an application bundle.' >&2; exit 1; }
[[ "$expected_identity" =~ ^Developer\ ID\ Application:\ [[:print:]]{1,160}\ \([A-Z0-9]{10}\)$ ]] || {
  echo 'macOS signing identity has an invalid shape.' >&2
  exit 1
}
[[ "$run_started_at" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || {
  echo 'GitHub release run timestamp is invalid.' >&2
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
timestamp_text="$(sed -n 's/^Timestamp=//p' <<<"$details")"
[[ -n "$timestamp_text" && "$(grep -c '^Timestamp=' <<<"$details")" -eq 1 ]] || {
  echo 'macOS signing timestamp evidence is missing or ambiguous.' >&2
  exit 1
}
timestamp_epoch="$(LC_ALL=C date -j -f '%b %d, %Y at %H:%M:%S %p' "$timestamp_text" '+%s' 2>/dev/null)" || {
  echo 'macOS signing timestamp evidence is invalid.' >&2
  exit 1
}
run_started_epoch="$(LC_ALL=C date -j -u -f '%Y-%m-%dT%H:%M:%SZ' "$run_started_at" '+%s' 2>/dev/null)" || {
  echo 'GitHub release run timestamp cannot be parsed.' >&2
  exit 1
}
(( timestamp_epoch >= run_started_epoch - 300 && timestamp_epoch <= run_started_epoch + 3900 )) || {
  echo 'macOS signing timestamp is outside the release build window.' >&2
  exit 1
}
codesign --verify --deep --strict --verbose=2 "$bundle"

echo 'verified macOS bundle signing identity, team binding, and hardened runtime'
