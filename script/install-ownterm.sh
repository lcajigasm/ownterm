#!/usr/bin/env bash
# Install or replace OwnTerm from a GitHub Release after checking SHA256SUMS.
# Downloads stay on GitHub hosts. The app's own updater stays empty, so this
# script is the update path and it never contacts Warp.
set -euo pipefail

OWNTERM_RELEASE_REPO="${OWNTERM_RELEASE_REPO:-lcajigasm/ownterm}"

ownterm_asset_name() {
  local os="$1"
  local arch="$2"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64) printf '%s\n' ownterm-macos-arm64.tar.gz ;;
        x86_64) printf '%s\n' ownterm-macos-x86_64.tar.gz ;;
        *)
          echo "unsupported architecture: $arch" >&2
          return 1
          ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64 | amd64) printf '%s\n' ownterm-linux-x86_64.tar.gz ;;
        *)
          echo "unsupported architecture: $arch" >&2
          return 1
          ;;
      esac
      ;;
    MINGW* | MSYS* | CYGWIN*)
      case "$arch" in
        x86_64 | amd64) printf '%s\n' ownterm-windows-x86_64.tar.gz ;;
        *)
          echo "unsupported architecture: $arch" >&2
          return 1
          ;;
      esac
      ;;
    *)
      echo "unsupported OS: $os" >&2
      return 1
      ;;
  esac
}

ownterm_release_url() {
  local repo="$1"
  local tag="$2"
  local asset="$3"
  if [[ ! "$repo" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
    echo "refusing repository name: $repo" >&2
    return 1
  fi
  case "$asset" in
    SHA256SUMS | ownterm-macos-arm64.tar.gz | ownterm-macos-x86_64.tar.gz | ownterm-linux-x86_64.tar.gz | ownterm-windows-x86_64.tar.gz) ;;
    *)
      echo "refusing release asset: $asset" >&2
      return 1
      ;;
  esac
  if [[ -z "$tag" || "$tag" == "latest" ]]; then
    printf 'https://github.com/%s/releases/latest/download/%s\n' "$repo" "$asset"
    return 0
  fi
  if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "refusing tag: $tag" >&2
    return 1
  fi
  printf 'https://github.com/%s/releases/download/%s/%s\n' "$repo" "$tag" "$asset"
}

ownterm_assert_download_host() {
  local url="$1"
  local host
  host="$(printf '%s' "$url" | sed -E 's#^[a-zA-Z][a-zA-Z0-9+.-]*://([^/@]+@)?([^/:]+).*#\2#')"
  case "$host" in
    github.com | release-assets.githubusercontent.com | objects.githubusercontent.com | github-releases.githubusercontent.com)
      return 0
      ;;
    *)
      echo "refusing download host: $host" >&2
      return 1
      ;;
  esac
}

ownterm_fetch() {
  local url="$1"
  local dest="$2"
  local effective
  ownterm_assert_download_host "$url"
  effective="$(
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
      --output "$dest" --write-out '%{url_effective}' "$url"
  )"
  if ! ownterm_assert_download_host "$effective"; then
    rm -f "$dest"
    return 1
  fi
}

ownterm_file_sha256() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print tolower($1)}'
  else
    shasum -a 256 "$file" | awk '{print tolower($1)}'
  fi
}

ownterm_expected_sha256() {
  local sums="$1"
  local asset="$2"
  awk -v asset="$asset" '
    {
      hash = tolower($1)
      name = $2
      sub(/^\*/, "", name)
      if (name == asset) {
        print hash
        exit
      }
    }
  ' "$sums"
}

ownterm_verify_checksum() {
  local sums="$1"
  local asset="$2"
  local file="$3"
  local expected actual
  expected="$(ownterm_expected_sha256 "$sums" "$asset")"
  if [[ -z "$expected" ]]; then
    echo "SHA256SUMS has no entry for $asset" >&2
    return 1
  fi
  actual="$(ownterm_file_sha256 "$file")"
  if [[ "$expected" != "$actual" ]]; then
    echo "checksum mismatch for $asset" >&2
    return 1
  fi
}

ownterm_install_archive() {
  local archive="$1"
  local bindir="$2"
  local tmp entry name found=0
  tmp="$(mktemp -d)"
  while IFS= read -r entry; do
    case "$entry" in
      "" | ./) continue ;;
      /* | *../* | ../* | */.. | ..)
        echo "refusing archive path: $entry" >&2
        rm -rf "$tmp"
        return 1
        ;;
    esac
  done < <(tar -tzf "$archive")
  tar -xzf "$archive" -C "$tmp"
  mkdir -p "$bindir"
  for name in ownterm ownterm-tui ownterm.exe ownterm-tui.exe; do
    if [[ -f "$tmp/$name" ]]; then
      install -m 755 "$tmp/$name" "$bindir/$name"
      found=1
    fi
  done
  rm -rf "$tmp"
  if [[ "$found" -ne 1 ]]; then
    echo "archive did not contain ownterm binaries" >&2
    return 1
  fi
}

ownterm_reject_provider_value() {
  local key="$1"
  local value="$2"
  local lower
  # `[[ == ]]` globs do not treat an embedded newline as a match.
  if [[ "$value" == *"="* ]] || printf '%s' "$value" | grep -q '[[:cntrl:]]'; then
    echo "$key must be a single token" >&2
    return 1
  fi
  lower="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')"
  case "$lower" in
    *api_key* | *apikey* | *token* | *secret*)
      echo "$key must not carry a secret; set OWNTERM_API_KEY in the environment" >&2
      return 1
      ;;
  esac
}

ownterm_write_provider() {
  local dest="$1"
  local provider="$2"
  local model="$3"
  local base_url="${4:-}"
  case "$provider" in
    ollama | lmstudio | vllm | localai | openai | xai | openrouter | groq | compatible) ;;
    *)
      echo "unknown provider: $provider" >&2
      return 1
      ;;
  esac
  ownterm_reject_provider_value model "$model" || return 1
  if [[ -n "$base_url" ]]; then
    ownterm_reject_provider_value base_url "$base_url" || return 1
    case "$base_url" in
      https://* | http://localhost* | http://127.0.0.1*) ;;
      *)
        echo "base_url must use HTTPS, or HTTP on localhost" >&2
        return 1
        ;;
    esac
  fi
  mkdir -p "$(dirname "$dest")"
  (
    umask 077
    {
      printf 'provider=%s\n' "$provider"
      printf 'model=%s\n' "$model"
      if [[ -n "$base_url" ]]; then
        printf 'base_url=%s\n' "$base_url"
      fi
    } >"$dest"
  )
}

ownterm_install_main() {
  local tag="" prefix="${OWNTERM_INSTALL_PREFIX:-$HOME/.local}" from_dir="" provider="" model="" base_url=""
  local provider_file="${HOME}/.ownterm/provider.env" repo="$OWNTERM_RELEASE_REPO"
  local asset work sums_url asset_url
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --tag)
        tag="$2"
        shift 2
        ;;
      --prefix)
        prefix="$2"
        shift 2
        ;;
      --from-dir)
        from_dir="$2"
        shift 2
        ;;
      --provider)
        provider="$2"
        shift 2
        ;;
      --model)
        model="$2"
        shift 2
        ;;
      --base-url)
        base_url="$2"
        shift 2
        ;;
      --provider-file)
        provider_file="$2"
        shift 2
        ;;
      --repo)
        repo="$2"
        shift 2
        ;;
      *)
        echo "unknown argument: $1" >&2
        return 1
        ;;
    esac
  done
  if [[ -n "$provider" && -z "$model" ]]; then
    echo "--provider requires --model" >&2
    return 1
  fi
  asset="$(ownterm_asset_name "$(uname -s)" "$(uname -m)")"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' RETURN
  if [[ -n "$from_dir" ]]; then
    cp "$from_dir/SHA256SUMS" "$work/SHA256SUMS"
    cp "$from_dir/$asset" "$work/$asset"
  else
    sums_url="$(ownterm_release_url "$repo" "$tag" SHA256SUMS)"
    asset_url="$(ownterm_release_url "$repo" "$tag" "$asset")"
    ownterm_fetch "$sums_url" "$work/SHA256SUMS"
    ownterm_fetch "$asset_url" "$work/$asset"
  fi
  ownterm_verify_checksum "$work/SHA256SUMS" "$asset" "$work/$asset"
  ownterm_install_archive "$work/$asset" "$prefix/bin"
  if [[ -n "$provider" ]]; then
    ownterm_write_provider "$provider_file" "$provider" "$model" "$base_url"
  fi
  printf 'Installed %s into %s\n' "$asset" "$prefix/bin"
  case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *)
      printf 'Add %s to PATH to run ownterm and ownterm-tui\n' "$prefix/bin"
      ;;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  ownterm_install_main "$@"
fi
