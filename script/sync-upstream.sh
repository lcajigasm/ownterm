#!/usr/bin/env bash
# Merge warpdotdev/warp into the current branch of the checkout this is run from.
#
# Paths listed in .github/upstream-sync-keep-ours stay exactly as OwnTerm has them, and a file
# OwnTerm deleted stays deleted when upstream edits it. Everything else is an ordinary merge.
#
# Exit status: 0 when the merge is committed or there is nothing new, 3 when conflicts are left
# in the working tree for a person to resolve (then `git commit`, or `git merge --abort`).
#
# Usage: bash script/sync-upstream.sh [upstream-ref]
set -euo pipefail

upstream_url="${OWNTERM_UPSTREAM_URL:-https://github.com/warpdotdev/warp.git}"
upstream_ref="${1:-master}"

cd "$(git rev-parse --show-toplevel)"
keep_ours_file=".github/upstream-sync-keep-ours"

if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "sync-upstream: commit or stash your changes first" >&2
  exit 1
fi

git fetch --no-tags "$upstream_url" "$upstream_ref"
theirs="$(git rev-parse FETCH_HEAD)"
if git merge-base --is-ancestor "$theirs" HEAD; then
  echo "sync-upstream: already contains upstream ${theirs:0:12}"
  exit 0
fi

echo "sync-upstream: merging $(git rev-list --count "HEAD..$theirs") upstream commits up to ${theirs:0:12}"
git merge --no-commit --no-ff "$theirs" >/dev/null 2>&1 || true
if ! git rev-parse -q --verify MERGE_HEAD >/dev/null; then
  echo "sync-upstream: git could not start the merge" >&2
  exit 1
fi

if [[ -f "$keep_ours_file" ]]; then
  while IFS= read -r path || [[ -n "$path" ]]; do
    path="${path%%#*}"
    path="${path%"${path##*[![:space:]]}"}"
    [[ -z "$path" ]] && continue
    # Upstream may add files under a kept path, so the index is checked as well as HEAD.
    if [[ -n "$(git ls-files -- "$path" | head -n 1)" ]] || git cat-file -e "HEAD:${path%/}" 2>/dev/null; then
      git restore --source=HEAD --staged --worktree -- "$path"
    fi
  done <"$keep_ours_file"
fi

# "DU" is a file OwnTerm deleted and upstream changed.
while IFS= read -r -d '' entry; do
  if [[ "${entry:0:2}" == "DU" ]]; then
    git rm -q -- "${entry:3}"
  fi
done < <(git status --porcelain -z)

conflicts="$(git diff --name-only --diff-filter=U)"
if [[ -n "$conflicts" ]]; then
  echo "sync-upstream: these files need a manual merge:" >&2
  printf '%s\n' "$conflicts" | sed 's/^/  /' >&2
  exit 3
fi

git commit -q --no-edit -m "Merge warpdotdev/warp@${theirs:0:12} into OwnTerm."
echo "sync-upstream: merged upstream ${theirs:0:12}"
