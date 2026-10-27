#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="${1:-$(cd -- "$script_dir/.." && pwd)}"

if [[ -z "$repo_dir" || ! -d "$repo_dir" ]]; then
  echo "Usage: $0 /path/to/warp-checkout" >&2
  exit 2
fi

if ! command -v rg >/dev/null 2>&1; then
  echo "This audit requires ripgrep (rg)." >&2
  exit 2
fi

pattern='warp\.dev|app\.warp|graphql|telemetry|analytics|sentry|billing|credit|entitlement|subscription|workspace|team|drive|cloud.?object|oauth|auth|oz'

echo "Potential business/service coupling in: $repo_dir"
echo "Review every hit; matching text alone is not proof that code should be removed."
rg --hidden --glob '!.git/**' --glob '!target/**' --glob '!Cargo.lock' \
  --line-number --ignore-case "$pattern" "$repo_dir"
