#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_dir="$(cd -- "$script_dir/.." && pwd)"
checkout_dir="$project_dir/upstream"

if [[ -e "$checkout_dir" ]]; then
  echo "Refusing to overwrite existing path: $checkout_dir" >&2
  exit 1
fi

git clone --origin origin https://github.com/lcajigasm/ownterm.git "$checkout_dir"
git -C "$checkout_dir" remote add upstream https://github.com/warpdotdev/warp.git

echo "Created checkout at $checkout_dir"
echo "Before editing, read: $checkout_dir/AGENTS.md"
