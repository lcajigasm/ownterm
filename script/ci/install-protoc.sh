#!/usr/bin/env bash
# Install a pinned protoc on a GitHub runner and put it on PATH for later steps.
# The prebuilt archive comes from the protobuf project's releases and is checked
# against the sums below, so no third-party action runs in the build.
set -euo pipefail

version="25.8"

case "$(uname -s)" in
  Darwin)
    asset="protoc-${version}-osx-universal_binary.zip"
    sha256="ca1a4ec682e3ee6d64c6d4b50e9dd24374f863cd4727fe0d36316b3f2580456c"
    ;;
  Linux)
    asset="protoc-${version}-linux-x86_64.zip"
    sha256="f7873a2b57811575c661faffa847e0e9b1bb06a1aea5021eccb05e8c04b260d5"
    ;;
  *)
    echo "unsupported runner: $(uname -s)" >&2
    exit 1
    ;;
esac

dest="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/protoc-${version}"
archive="${dest}.zip"
curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  --retry 3 --max-time 120 \
  --output "$archive" \
  "https://github.com/protocolbuffers/protobuf/releases/download/v${version}/${asset}"

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$archive" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
fi
if [[ "$actual" != "$sha256" ]]; then
  echo "checksum mismatch for $asset" >&2
  exit 1
fi

rm -rf "$dest"
mkdir -p "$dest"
unzip -q "$archive" -d "$dest"
rm -f "$archive"

if [[ -n "${GITHUB_PATH:-}" ]]; then
  printf '%s\n' "$dest/bin" >>"$GITHUB_PATH"
fi
"$dest/bin/protoc" --version
