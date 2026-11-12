#!/usr/bin/env bash
# Offline checks for the GitHub-release installer. No network.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=script/install-ownterm.sh
source "$root/script/install-ownterm.sh"

fail() {
  echo "install test: $*" >&2
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

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

assert_eq \
  "$(ownterm_asset_name Darwin arm64)" \
  "ownterm-macos-arm64.tar.gz" \
  "macos arm64 asset"
assert_eq \
  "$(ownterm_asset_name Darwin x86_64)" \
  "ownterm-macos-x86_64.tar.gz" \
  "macos x86_64 asset"
if ownterm_asset_name Linux x86_64 >/dev/null 2>&1; then
  fail "a Linux asset was offered"
fi
if ownterm_release_url lcajigasm/ownterm latest ownterm-linux-x86_64.tar.gz >/dev/null 2>&1; then
  fail "a Linux asset URL was accepted"
fi
assert_eq \
  "$(ownterm_release_url lcajigasm/ownterm v0.2.0 ownterm-macos-x86_64.tar.gz)" \
  "https://github.com/lcajigasm/ownterm/releases/download/v0.2.0/ownterm-macos-x86_64.tar.gz" \
  "tagged release url"
assert_eq \
  "$(ownterm_release_url lcajigasm/ownterm latest SHA256SUMS)" \
  "https://github.com/lcajigasm/ownterm/releases/latest/download/SHA256SUMS" \
  "latest sums url"

if ownterm_release_url lcajigasm/ownterm v0.2 ownterm-macos-arm64.tar.gz >/dev/null 2>&1; then
  fail "a non-semver tag was accepted"
fi
if ownterm_release_url 'https://releases.example/ownterm' latest SHA256SUMS >/dev/null 2>&1; then
  fail "a URL was accepted as a repository name"
fi
if ownterm_assert_download_host "https://releases.warp.dev/stable/ownterm.tar.gz" >/dev/null 2>&1; then
  fail "a Warp download host was accepted"
fi
ownterm_assert_download_host "https://github.com/lcajigasm/ownterm/releases/latest/download/SHA256SUMS"
ownterm_assert_download_host "https://release-assets.githubusercontent.com/example"

asset="ownterm-macos-arm64.tar.gz"
stage="$tmp/stage"
mkdir -p "$stage"
printf '#!/bin/sh\necho ownterm\n' >"$stage/ownterm"
printf '#!/bin/sh\necho ownterm-tui\n' >"$stage/ownterm-tui"
chmod 755 "$stage/ownterm" "$stage/ownterm-tui"
release="$tmp/release"
mkdir -p "$release"
tar -C "$stage" -czf "$release/$asset" ownterm ownterm-tui
hash="$(ownterm_file_sha256 "$release/$asset")"
printf '%s  %s\n' "$hash" "$asset" >"$release/SHA256SUMS"

prefix="$tmp/prefix"
provider_file="$tmp/provider.env"
# The test runs on Linux in CI. The installer only serves macOS, so it is told it is on one.
fake_bin="$tmp/fake-bin"
mkdir -p "$fake_bin"
cat >"$fake_bin/uname" <<'SH'
#!/bin/sh
case "$1" in
  -s) echo Darwin ;;
  -m) echo arm64 ;;
esac
SH
chmod 755 "$fake_bin/uname"

install_from_dir() {
  PATH="$fake_bin:$PATH" bash "$root/script/install-ownterm.sh" "$@"
}

install_from_dir \
  --from-dir "$release" \
  --prefix "$prefix" \
  --provider ollama \
  --model llama3.3 \
  --provider-file "$provider_file"
[[ -x "$prefix/bin/ownterm" && -x "$prefix/bin/ownterm-tui" ]] || fail "binaries were not installed"
grep -F -q 'provider=ollama' "$provider_file" || fail "provider was not written"
grep -F -q 'model=llama3.3' "$provider_file" || fail "model was not written"
if grep -E -q 'api_key|token|secret' "$provider_file"; then
  fail "provider file contains a secret field"
fi

bad="$tmp/bad-release"
mkdir -p "$bad"
cp "$release/$asset" "$bad/$asset"
printf '0000000000000000000000000000000000000000000000000000000000000000  %s\n' "$asset" >"$bad/SHA256SUMS"
if install_from_dir --from-dir "$bad" --prefix "$tmp/not-installed"; then
  fail "a bad checksum was installed"
fi
[[ ! -e "$tmp/not-installed/bin/ownterm" ]] || fail "a bad checksum left a binary behind"

if ownterm_write_provider "$tmp/secret.env" ollama $'llama\napi_key=sk' "" >/dev/null 2>&1; then
  fail "a multiline model was accepted"
fi
if ownterm_write_provider "$tmp/http.env" ollama llama3.3 "http://example.com/v1" >/dev/null 2>&1; then
  fail "a public HTTP endpoint was accepted"
fi

python3 - "$tmp/slip.tar.gz" <<'PY'
import io
import sys
import tarfile

path, = sys.argv[1:]
payload = b"x\n"
with tarfile.open(path, "w:gz") as archive:
    info = tarfile.TarInfo("nested/../../ownterm")
    info.size = len(payload)
    archive.addfile(info, io.BytesIO(payload))
PY
if ownterm_install_archive "$tmp/slip.tar.gz" "$tmp/slip-bin"; then
  fail "an archive path with .. was extracted"
fi
[[ ! -e "$tmp/slip-bin/ownterm" && ! -e "$tmp/ownterm" ]] || fail "a slipped path was written"

echo "install test: ok"
