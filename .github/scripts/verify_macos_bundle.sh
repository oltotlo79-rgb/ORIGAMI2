#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "$script_dir/../.." && pwd -P)"
bundle="${1:-$repository_root/target/release/bundle/macos/ORIGAMI2.app}"
expected_version="${2:-}"

if [[ ! -d "$bundle" ]]; then
  echo "macOS application bundle was not found: $bundle" >&2
  exit 1
fi

file_count="$(find "$bundle" -xdev -type f | wc -l | tr -d ' ')"
[[ "$file_count" -gt 0 && "$file_count" -le 100000 ]] || {
  echo "macOS bundle file count is outside the audited bound: $file_count" >&2
  exit 1
}
[[ -z "$(find "$bundle" -xdev -type l -print | sed -n '1p')" ]] || {
  echo "macOS bundle must not contain symbolic links." >&2
  exit 1
}
[[ -z "$(find "$bundle" -xdev -type f -links +1 -print | sed -n '1p')" ]] || {
  echo "macOS bundle must not contain hard-linked files." >&2
  exit 1
}
largest_file="$(find "$bundle" -xdev -type f -exec stat -f '%z' {} + | sort -nr | head -n 1)"
[[ "$largest_file" -le 536870912 ]] || {
  echo "macOS bundle contains an oversized file: $largest_file bytes." >&2
  exit 1
}
bundle_kib="$(du -sk "$bundle" | awk '{print $1}')"
[[ "$bundle_kib" -le 1048576 ]] || {
  echo "macOS bundle exceeds the 1 GiB audit bound." >&2
  exit 1
}

info_plist="$bundle/Contents/Info.plist"
[[ -f "$info_plist" ]] || { echo "Info.plist is missing." >&2; exit 1; }

read_plist() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$info_plist"
}

identifier="$(read_plist CFBundleIdentifier)"
version="$(read_plist CFBundleShortVersionString)"
executable_name="$(read_plist CFBundleExecutable)"

[[ "$identifier" == "dev.origami2.editor" ]] || {
  echo "Unexpected bundle identifier: $identifier" >&2
  exit 1
}
if [[ -n "$expected_version" && "$version" != "$expected_version" ]]; then
  echo "Bundle version $version does not match expected version $expected_version." >&2
  exit 1
fi
[[ "$executable_name" != */* && "$executable_name" != *\\* && -n "$executable_name" ]] || {
  echo "Unsafe CFBundleExecutable value: $executable_name" >&2
  exit 1
}
[[ -x "$bundle/Contents/MacOS/$executable_name" ]] || {
  echo "Bundle executable is missing or is not executable." >&2
  exit 1
}
executable_entry_count="$(find "$bundle/Contents/MacOS" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')"
only_executable="$(find "$bundle/Contents/MacOS" -mindepth 1 -maxdepth 1 -print | sed -n '1p')"
[[ "$executable_entry_count" -eq 1 && "$only_executable" == "$bundle/Contents/MacOS/$executable_name" ]] || {
  echo "Contents/MacOS must contain exactly the declared executable." >&2
  exit 1
}

font="$bundle/Contents/Resources/fonts/NotoSansJP-Variable.ttf"
license="$bundle/Contents/Resources/licenses/NotoSansJP-OFL.txt"
[[ -f "$font" ]] || { echo "Bundled Japanese font is missing." >&2; exit 1; }
[[ -f "$license" ]] || { echo "Bundled font license is missing." >&2; exit 1; }

font_digest="$(shasum -a 256 "$font" | awk '{print $1}')"
license_digest="$(shasum -a 256 "$license" | awk '{print $1}')"
[[ "$font_digest" == "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f" ]] || {
  echo "Bundled Japanese font checksum mismatch." >&2
  exit 1
}
[[ "$license_digest" == "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9" ]] || {
  echo "Bundled font license checksum mismatch." >&2
  exit 1
}

echo "verified macOS bundle: identifier=$identifier version=$version executable=$executable_name"
