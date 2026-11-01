#!/usr/bin/env bash
# Layout checks for the macOS disk image packager. No network and no hdiutil.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=script/package-macos-dmg.sh
source "$root/script/package-macos-dmg.sh"

fail() {
  echo "dmg package test: $*" >&2
  exit 1
}

assert_eq() {
  local actual="$1"
  local expected="$2"
  local label="$3"
  if [[ "$actual" != "$expected" ]]; then
    fail "$label: got '$actual' expected '$expected'"
  fi
}

assert_eq "$(app_bundle_name)" "OwnTerm.app" "app name"
assert_eq "$(dmg_filename universal)" "ownterm-macos-universal.dmg" "universal dmg"
assert_eq "$(dmg_filename arm64)" "ownterm-macos-arm64.dmg" "arm64 dmg"
assert_eq "$(dmg_filename x86_64)" "ownterm-macos-x86_64.dmg" "x86_64 dmg"
assert_eq "$(package_mode 1 1 1)" "universal" "lipo success"
assert_eq "$(package_mode 1 1 0)" "split" "lipo unavailable"
assert_eq "$(package_mode 1 0 0)" "arm64" "arm64 only"
assert_eq "$(package_mode 0 1 0)" "x86_64" "x86_64 only"

if package_mode 0 0 0 >/dev/null 2>&1; then
  fail "package_mode accepted no binaries"
fi
if validate_version "not-a-version" >/dev/null 2>&1; then
  fail "validate_version accepted junk"
fi
validate_version "0.5.1"

if grep -E -n 'notarytool|codesign|altool' "$root/script/package-macos-dmg.sh"; then
  fail "packaging must not sign or notarize"
fi

release_yml="$root/.github/workflows/release.yml"
for needle in \
  "ownterm-macos-arm64" \
  "ownterm-macos-x86_64" \
  "ownterm-linux-x86_64" \
  "ownterm-windows-x86_64" \
  "script/package-macos-dmg.sh" \
  "ownterm-macos-universal.dmg" \
  "-p warp --bin ownterm" \
  "-p warp_tui --bin ownterm-tui" \
  "OWNTERM_WINDOWS_ICON" \
  "script/capture-ownterm.sh" \
  "SHA256SUMS"; do
  if ! grep -F -q -- "$needle" "$release_yml"; then
    fail "release.yml is missing $needle"
  fi
done

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

binary="$tmp/ownterm-bin"
printf 'ownterm-binary' >"$binary"

bash "$root/script/package-macos-dmg.sh" \
  --layout-only \
  --x86_64 "$binary" \
  --version 0.5.1 \
  --icon "$root/app/channels/oss/icon/no-padding/512x512.png" \
  --out-dir "$tmp/plain"
app="$tmp/plain/OwnTerm.app"
if [[ ! -x "$app/Contents/MacOS/ownterm" ]]; then
  fail "layout did not install the ownterm executable"
fi
if ! cmp -s "$binary" "$app/Contents/MacOS/ownterm"; then
  fail "bundled executable does not match the input binary"
fi
plist="$app/Contents/Info.plist"
for needle in \
  "dev.ownterm.OwnTerm" \
  "<string>ownterm</string>" \
  "<string>OwnTerm</string>" \
  "<string>0.5.1</string>" \
  "APPL"; do
  if ! grep -F -q -- "$needle" "$plist"; then
    fail "Info.plist is missing $needle"
  fi
done
if command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
  if [[ ! -f "$app/Contents/Resources/AppIcon.icns" ]]; then
    fail "icon tools were present but AppIcon.icns was not written"
  fi
  if ! grep -F -q "CFBundleIconFile" "$plist"; then
    fail "Info.plist does not name the icon"
  fi
else
  if grep -F -q "CFBundleIconFile" "$plist"; then
    fail "Info.plist names an icon that was not written"
  fi
  if [[ -e "$app/Contents/Resources/AppIcon.icns" ]]; then
    fail "an icns was written without icon tools"
  fi
fi

tools="$tmp/fake-tools"
mkdir -p "$tools"
cat >"$tools/sips" <<'EOF'
#!/usr/bin/env bash
prev=""
for arg in "$@"; do
  if [[ "$prev" == "--out" ]]; then
    mkdir -p "$(dirname "$arg")"
    printf 'png' >"$arg"
  fi
  prev="$arg"
done
EOF
cat >"$tools/iconutil" <<'EOF'
#!/usr/bin/env bash
while [[ $# -gt 0 ]]; do
  if [[ "$1" == "-o" ]]; then
    mkdir -p "$(dirname "$2")"
    printf 'icns' >"$2"
    exit 0
  fi
  shift
done
exit 1
EOF
chmod +x "$tools/sips" "$tools/iconutil"
PATH="$tools:$PATH" bash "$root/script/package-macos-dmg.sh" \
  --layout-only \
  --arm64 "$binary" \
  --version 0.5.1 \
  --icon "$root/app/channels/oss/icon/no-padding/512x512.png" \
  --out-dir "$tmp/icon"
icon_app="$tmp/icon/OwnTerm.app"
if [[ ! -f "$icon_app/Contents/Resources/AppIcon.icns" ]]; then
  fail "fake icon tools did not produce AppIcon.icns"
fi
if ! grep -F -q "<string>AppIcon</string>" "$icon_app/Contents/Info.plist"; then
  fail "Info.plist does not reference AppIcon"
fi

sample="$tmp/sample.bin"
printf 'checksum-me' >"$sample"
write_sha256 "$sample" "$tmp/sample.sha256"
read -r sum name <"$tmp/sample.sha256"
if [[ ! "$sum" =~ ^[0-9a-f]{64}$ ]]; then
  fail "checksum is not 64 hex chars: $sum"
fi
assert_eq "$name" "sample.bin" "checksum filename"
if [[ "$(wc -l <"$tmp/sample.sha256" | tr -d ' ')" != 1 ]]; then
  fail "checksum file should be one line"
fi

echo "dmg package test: ok"
