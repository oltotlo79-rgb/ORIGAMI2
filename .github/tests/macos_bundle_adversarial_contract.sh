#!/usr/bin/env bash
set -euo pipefail

source_bundle="$1"
expected_version="$2"
verifier="$(cd "$(dirname "$0")/../scripts" && pwd)/verify_macos_bundle.sh"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/origami2-macos-adversarial.XXXXXX")"
trap 'rm -rf -- "$temporary_root"' EXIT
bundle="$temporary_root/ORIGAMI2.app"
cp -R "$source_bundle" "$bundle"

expect_rejection() {
  local label="$1"
  if "$verifier" "$bundle" "$expected_version" >/dev/null 2>&1; then
    echo "validator accepted adversarial fixture: $label" >&2
    exit 1
  fi
}

ln -s ../Info.plist "$bundle/Contents/Resources/linked.plist"
expect_rejection symbolic-link
rm "$bundle/Contents/Resources/linked.plist"

ln "$bundle/Contents/Resources/licenses/NotoSansJP-OFL.txt" \
  "$bundle/Contents/Resources/licenses/duplicate-license.txt"
expect_rejection hard-link
rm "$bundle/Contents/Resources/licenses/duplicate-license.txt"

cp "$bundle/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$bundle/Contents/Info.plist")" \
  "$bundle/Contents/MacOS/unexpected-helper"
expect_rejection extra-executable
rm "$bundle/Contents/MacOS/unexpected-helper"

mkfile -n 600m "$bundle/Contents/Resources/oversized.fixture"
expect_rejection oversized-file
rm "$bundle/Contents/Resources/oversized.fixture"

if "$verifier" "$bundle" '999.0.0' >/dev/null 2>&1; then
  echo "validator accepted adversarial fixture: wrong-version" >&2
  exit 1
fi
if "$verifier" "$bundle" "$expected_version" >/dev/null; then
  echo "macOS adversarial bundle contract passed"
fi
