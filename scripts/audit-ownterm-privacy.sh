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
require app/src/bin/oss.rs "WarpServerConfig::disabled()"
require crates/warp_tui/src/bin/oss.rs "telemetry_config: None"
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

if grep -R -n --include='*.rs' -F "AIzaSyBdy3O3S9hrdayLJxJ7mriBR4qgUaUygAs" .; then
  fail "the Warp Firebase client key is embedded in Rust source"
fi

if grep -n "git push origin" .github/workflows/ci.yml | grep -q -- "--force"; then
  fail "CI must not force-push tags"
fi

echo "privacy audit: ok"
