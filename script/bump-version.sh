#!/usr/bin/env bash
#
# script/bump-version.sh — Bump OwnTerm version across project files and optionally tag
#
# Usage:
#   ./script/bump-version.sh <version> [--tag] [--push]
#
# Example:
#   ./script/bump-version.sh 0.1.1 --tag
#

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

if [ "$#" -lt 1 ]; then
  echo "Usage: $0 <new-version> [--tag] [--push]"
  echo "Example: $0 0.1.1 --tag"
  exit 1
fi

RAW_VERSION="$1"
# Strip leading 'v' if provided
VERSION="${RAW_VERSION#v}"
TAG_FLAG=false
PUSH_FLAG=false

for arg in "$@"; do
  if [ "$arg" == "--tag" ]; then
    TAG_FLAG=true
  elif [ "$arg" == "--push" ]; then
    PUSH_FLAG=true
    TAG_FLAG=true
  fi
done

# Validate semantic version pattern: X.Y.Z
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "Error: Version '$VERSION' is not a valid semantic version (e.g. 0.1.1 or 0.2.0-rc1)"
  exit 1
fi

echo "==> Bumping OwnTerm version to: ${VERSION} (tag: v${VERSION})"

# 1. Update app/Cargo.toml
if [ -f "app/Cargo.toml" ]; then
  echo "Updating app/Cargo.toml..."
  sed -i '' -E "s/^version = \"[0-9]+\.[0-9]+\.[0-9]+[^\"]*\"/version = \"${VERSION}\"/" app/Cargo.toml
fi

# 2. Update app/src/bin/oss.rs (CFBundleShortVersionString)
if [ -f "app/src/bin/oss.rs" ]; then
  echo "Updating app/src/bin/oss.rs..."
  python3 -c "
import re
with open('app/src/bin/oss.rs', 'r') as f:
    content = f.read()
updated = re.sub(
    r'(<key>CFBundleShortVersionString<\/key>\s*<string>)[^<]+(<\/string>)',
    rf'\g<1>${VERSION}\g<2>',
    content
)
with open('app/src/bin/oss.rs', 'w') as f:
    f.write(updated)
"
fi

# 3. Update script/windows/windows-installer.iss
if [ -f "script/windows/windows-installer.iss" ]; then
  echo "Updating script/windows/windows-installer.iss..."
  sed -i '' -E "s/#define MyAppVersion \"[^\"]+\"/#define MyAppVersion \"${VERSION}\"/" script/windows/windows-installer.iss
fi

# 4. Update script/windows/tui-installer.iss
if [ -f "script/windows/tui-installer.iss" ]; then
  echo "Updating script/windows/tui-installer.iss..."
  sed -i '' -E "s/#define MyAppVersion \"[^\"]+\"/#define MyAppVersion \"${VERSION}\"/" script/windows/tui-installer.iss
fi

# 5. Update README.md download links and version labels
if [ -f "README.md" ]; then
  echo "Updating README.md download links..."
  python3 -c "
import re
with open('README.md', 'r') as f:
    content = f.read()

# Update release version download paths: /vX.Y.Z/
content = re.sub(r'/download/v[0-9]+\.[0-9]+\.[0-9]+[^/]*/', f'/download/v${VERSION}/', content)
# Update release tag link: /releases/tag/vX.Y.Z
content = re.sub(r'/releases/tag/v[0-9]+\.[0-9]+\.[0-9]+[^\s\)\"]*', f'/releases/tag/v${VERSION}', content)
# Update bold version mention
content = re.sub(r'release bundles for \*\*v[0-9]+\.[0-9]+\.[0-9]+[^\*]*\*\*', f'release bundles for **v${VERSION}**', content)

with open('README.md', 'w') as f:
    f.write(content)
"
fi

# 6. Update docs/index.html download links, badges, and tag links
if [ -f "docs/index.html" ]; then
  echo "Updating docs/index.html..."
  python3 -c "
import re
with open('docs/index.html', 'r') as f:
    content = f.read()

content = re.sub(r'/download/v[0-9]+\.[0-9]+\.[0-9]+[^/]*/', f'/download/v${VERSION}/', content)
content = re.sub(r'/releases/tag/v[0-9]+\.[0-9]+\.[0-9]+[^\s\)\"]*', f'/releases/tag/v${VERSION}', content)
content = re.sub(r'OwnTerm v[0-9]+\.[0-9]+\.[0-9]+[^\s<]* Released', f'OwnTerm v${VERSION} Released', content)
content = re.sub(r'Download v[0-9]+\.[0-9]+\.[0-9]+[^\s<]*', f'Download v${VERSION}', content)

with open('docs/index.html', 'w') as f:
    f.write(content)
"
fi

echo "==> Version bump complete!"
git status --short app/Cargo.toml app/src/bin/oss.rs script/windows/ README.md docs/index.html

if [ "$TAG_FLAG" = true ]; then
  echo "==> Creating commit and git tag v${VERSION}..."
  git add app/Cargo.toml app/src/bin/oss.rs script/windows/windows-installer.iss script/windows/tui-installer.iss README.md docs/index.html
  git commit -m "chore: bump version to v${VERSION}"
  git tag "v${VERSION}"
  echo "==> Created git tag: v${VERSION}"
fi

if [ "$PUSH_FLAG" = true ]; then
  CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
  echo "==> Pushing ${CURRENT_BRANCH} and tag v${VERSION} to origin..."
  git push origin "${CURRENT_BRANCH}" "v${VERSION}"
  echo "==> Pushed! GitHub Actions workflow 'Release OwnTerm' will now build and publish release v${VERSION}."
else
  echo ""
  echo "Next steps to publish release:"
  echo "  1. Review changes: git diff"
  if [ "$TAG_FLAG" = false ]; then
    echo "  2. Commit changes: git commit -am \"chore: bump version to v${VERSION}\""
    echo "  3. Create tag:     git tag \"v${VERSION}\""
  fi
  echo "  4. Push to GitHub: git push origin master --tags"
  echo "     (This triggers .github/workflows/release-ownterm.yml to compile for macOS, Linux, and Windows)"
fi
