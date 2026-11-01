#!/usr/bin/env bash
# Fails when OwnTerm regains a network path for Warp telemetry, crash upload,
# or a mandatory login flag on a shipped channel binary.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

fail() {
  echo "privacy audit: $*" >&2
  exit 1
}

require() {
  local path="$1"
  local needle="$2"
  if ! grep -F -q -- "$needle" "$path"; then
    fail "$path is missing '$needle'"
  fi
}

require app/src/bin/oss.rs "telemetry_config: None"
require app/src/bin/oss.rs "crash_reporting_config: None"
require app/src/bin/oss.rs "autoupdate_config: None"
require app/src/bin/oss.rs "WarpServerConfig::disabled()"
require crates/warp_tui/src/bin/oss.rs "telemetry_config: None"
require crates/warp_tui/src/bin/oss.rs "autoupdate_config: None"
require crates/warp_tui/src/bin/oss.rs "WarpServerConfig::disabled()"

for bin in stable dev local preview; do
  require "app/src/bin/${bin}.rs" "without_warp_services()"
  require "crates/warp_tui/src/bin/${bin}.rs" "without_warp_services()"
done

if grep -R -n --include='*.rs' -F "FeatureFlag::ForceLogin" app/src/bin crates/warp_tui/src/bin; then
  fail "channel binaries must not force a Warp login"
fi

require app/src/server/telemetry/mod.rs "may_transmit_telemetry()"
require app/src/server/telemetry/collector.rs "may_transmit_telemetry()"
require app/src/crash_reporting/mod.rs "is_crash_reporting_available()"
require app/src/settings/ai.rs "Channel::Oss"
require crates/warp_core/src/channel/state.rs "fn may_transmit_telemetry()"
require crates/warp_core/src/telemetry.rs "may_transmit_telemetry()"
require crates/warpui_core/src/telemetry/mod.rs "fn analytics_event_construction_enabled()"
require crates/warpui_core/src/telemetry/mod.rs "AtomicBool::new(false)"
require crates/warpui_core/src/telemetry/mod.rs "if !analytics_event_construction_enabled()"
require app/src/server/telemetry/macros.rs "may_transmit_telemetry()"
require app/src/lib.rs "may_transmit_telemetry()"
require app/src/auth/auth_manager.rs "may_transmit_telemetry()"
require app/src/autoupdate/mod.rs "may_transmit_telemetry()"
require app/src/terminal/view.rs "may_transmit_telemetry()"

python3 - <<'PY'
from pathlib import Path
import sys

def fail(message):
    print(f"privacy audit: {message}", file=sys.stderr)
    sys.exit(1)

for path in (
    "crates/warp_core/src/telemetry.rs",
    "app/src/server/telemetry/macros.rs",
):
    text = Path(path).read_text()
    guard = text.find("may_transmit_telemetry()")
    bind = text.find("let event = $event")
    if guard < 0 or bind < 0 or bind < guard:
        fail(f"{path} constructs a telemetry event before the transmit guard")

hits = []
root = Path(".")
for path in root.rglob("*.rs"):
    if any(part in {".git", "target"} for part in path.parts):
        continue
    text = path.read_text(errors="ignore")
    if "set_analytics_event_construction_enabled_for_test" not in text:
        continue
    allowed = {
        Path("crates/warpui_core/src/telemetry/mod.rs"),
        Path("app/src/server/telemetry/mod_tests.rs"),
    }
    if path not in allowed:
        hits.append(str(path))
if hits:
    fail("analytics event construction is enabled outside the persist test: " + ", ".join(hits))
PY

if grep -R -n --include='*.rs' -F "AIzaSyBdy3O3S9hrdayLJxJ7mriBR4qgUaUygAs" .; then
  fail "the Warp Firebase client key is embedded in Rust source"
fi

if grep -n "git push origin" .github/workflows/ci.yml | grep -q -- "--force"; then
  fail "CI must not force-push tags"
fi

require script/install-ownterm.sh "SHA256SUMS"
require script/install-ownterm.sh "ownterm_assert_download_host"
require docs/usage.md "provider.env"
require docs/usage.md "SHA256SUMS"
require docs/usage.md "~/.ownterm/workflows"
require docs/usage.md "configure-provider"
require docs/usage.md "api_key_env"
require crates/ownterm_provider/src/form.rs "fn save_provider_form"
require app/src/settings_view/ownterm_provider_form.rs "OwnTermProviderFormView"
require app/src/settings_view/warp_agent_page.rs "OwnTermProviderFormWidget"
require crates/ownterm_provider/src/lib.rs "fn write_provider_file"
require crates/ownterm_provider/src/lib.rs "fn parse_model_ids"
require crates/ownterm_provider/src/lib.rs "fn suggested_models"
require crates/warp_tui/src/session.rs "ConfigureProvider"
require crates/warp_tui/src/session.rs "ListModels"
require app/src/settings_view/mod.rs "fn local_settings_nav"
require .github/workflows/ci.yml "cargo check -p warp --bin ownterm"
require .github/workflows/ci.yml "cargo check -p warp_tui --bin ownterm-tui"
require .github/workflows/ci.yml "bash script/test_install_ownterm.sh"
require .github/workflows/ci.yml "bash script/test_package_macos_dmg.sh"
require .github/workflows/release.yml "ownterm-macos-universal.dmg"
require .github/workflows/release.yml "script/package-macos-dmg.sh"
require .github/workflows/release.yml "ownterm-macos-arm64.tar.gz"
require docs/usage.md "ownterm-macos-universal.dmg"
require docs/usage.md "OwnTerm.app"
require script/package-macos-dmg.sh "OwnTerm.app"

if grep -n -E 'releases\.warp\.dev|rudderstack|sentry\.io|segment\.(com|io)|warp\.dev' \
  script/install-ownterm.sh docs/usage.md; then
  fail "the installer or the usage guide names a Warp or telemetry host"
fi

bash script/test_install_ownterm.sh

echo "privacy audit: ok"
