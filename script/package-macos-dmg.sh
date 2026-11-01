#!/usr/bin/env bash
# Build OwnTerm.app and a macOS disk image from the release binaries.
# A universal image is produced when lipo can combine arm64 and x86_64.
# Otherwise each architecture gets its own image. Nothing here is signed.
set -euo pipefail

app_bundle_name() {
  printf '%s\n' "OwnTerm.app"
}

dmg_filename() {
  case "$1" in
    universal) printf '%s\n' ownterm-macos-universal.dmg ;;
    arm64) printf '%s\n' ownterm-macos-arm64.dmg ;;
    x86_64) printf '%s\n' ownterm-macos-x86_64.dmg ;;
    *) return 1 ;;
  esac
}

# universal when both Mach-O inputs combine; split when they do not.
package_mode() {
  local have_arm="$1"
  local have_x86="$2"
  local lipo_ok="$3"
  if [[ "$have_arm" == 1 && "$have_x86" == 1 ]]; then
    if [[ "$lipo_ok" == 1 ]]; then
      printf '%s\n' universal
    else
      printf '%s\n' split
    fi
  elif [[ "$have_arm" == 1 ]]; then
    printf '%s\n' arm64
  elif [[ "$have_x86" == 1 ]]; then
    printf '%s\n' x86_64
  else
    return 1
  fi
}

validate_version() {
  local version="$1"
  if [[ ! "$version" =~ ^[0-9]+(\.[0-9]+){1,3}$ ]]; then
    echo "version must look like 0.5.1, got: $version" >&2
    return 1
  fi
}

write_info_plist() {
  local path="$1"
  local version="$2"
  local has_icon="$3"
  local icon_keys=""
  if [[ "$has_icon" == 1 ]]; then
    icon_keys=$'    <key>CFBundleIconFile</key>\n    <string>AppIcon</string>\n'
  fi
  cat >"$path" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleDisplayName</key>
    <string>OwnTerm</string>
    <key>CFBundleExecutable</key>
    <string>ownterm</string>
    <key>CFBundleIdentifier</key>
    <string>dev.ownterm.OwnTerm</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>OwnTerm</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
${icon_keys}    <key>CFBundleShortVersionString</key>
    <string>${version}</string>
    <key>CFBundleVersion</key>
    <string>${version}</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>UIDesignRequiresCompatibility</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>OwnTerm</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>ownterm</string>
            </array>
        </dict>
    </array>
    <key>NSHumanReadableCopyright</key>
    <string>© 2026, Denver Technologies, Inc</string>
</dict>
</plist>
EOF
}

make_icns() {
  local png="$1"
  local dest="$2"
  if [[ ! -f "$png" ]]; then
    return 1
  fi
  if ! command -v sips >/dev/null 2>&1 || ! command -v iconutil >/dev/null 2>&1; then
    return 1
  fi
  local tmp iconset
  tmp="$(mktemp -d)"
  iconset="$tmp/AppIcon.iconset"
  mkdir -p "$iconset"
  if ! (
    sips -z 16 16 "$png" --out "$iconset/icon_16x16.png" >/dev/null
    sips -z 32 32 "$png" --out "$iconset/icon_16x16@2x.png" >/dev/null
    sips -z 32 32 "$png" --out "$iconset/icon_32x32.png" >/dev/null
    sips -z 64 64 "$png" --out "$iconset/icon_32x32@2x.png" >/dev/null
    sips -z 128 128 "$png" --out "$iconset/icon_128x128.png" >/dev/null
    sips -z 256 256 "$png" --out "$iconset/icon_128x128@2x.png" >/dev/null
    sips -z 256 256 "$png" --out "$iconset/icon_256x256.png" >/dev/null
    sips -z 512 512 "$png" --out "$iconset/icon_256x256@2x.png" >/dev/null
    sips -z 512 512 "$png" --out "$iconset/icon_512x512.png" >/dev/null
    cp "$png" "$iconset/icon_512x512@2x.png"
    iconutil -c icns "$iconset" -o "$dest"
  ); then
    rm -rf "$tmp"
    return 1
  fi
  rm -rf "$tmp"
}

assemble_app() {
  local dest_app="$1"
  local binary="$2"
  local version="$3"
  local icon_png="${4:-}"
  if [[ ! -f "$binary" ]]; then
    echo "missing binary: $binary" >&2
    return 1
  fi
  if [[ ! -s "$binary" ]]; then
    echo "empty binary: $binary" >&2
    return 1
  fi
  rm -rf "$dest_app"
  mkdir -p "$dest_app/Contents/MacOS" "$dest_app/Contents/Resources"
  cp "$binary" "$dest_app/Contents/MacOS/ownterm"
  chmod +x "$dest_app/Contents/MacOS/ownterm"
  local has_icon=0
  if [[ -n "$icon_png" && -f "$icon_png" ]]; then
    if make_icns "$icon_png" "$dest_app/Contents/Resources/AppIcon.icns"; then
      has_icon=1
    fi
  fi
  write_info_plist "$dest_app/Contents/Info.plist" "$version" "$has_icon"
}

try_universal_binary() {
  local arm="$1"
  local x86="$2"
  local out="$3"
  if ! command -v lipo >/dev/null 2>&1; then
    echo "lipo is not available; packaging each architecture separately" >&2
    return 1
  fi
  if ! lipo -create -output "$out" "$arm" "$x86"; then
    rm -f "$out"
    echo "lipo failed; packaging each architecture separately" >&2
    return 1
  fi
  local info
  info="$(lipo -info "$out")"
  echo "$info"
  if [[ "$info" != *arm64* || "$info" != *x86_64* ]]; then
    rm -f "$out"
    echo "lipo output is not a universal arm64+x86_64 binary" >&2
    return 1
  fi
  chmod +x "$out"
}

write_sha256() {
  local file="$1"
  local checksum_path="$2"
  local name
  name="$(basename "$file")"
  local line
  if command -v sha256sum >/dev/null 2>&1; then
    line="$(sha256sum "$file" | awk '{print $1}')"
  else
    line="$(shasum -a 256 "$file" | awk '{print $1}')"
  fi
  printf '%s  %s\n' "$line" "$name" >"$checksum_path"
}

create_dmg() {
  local app="$1"
  local dmg="$2"
  if ! command -v hdiutil >/dev/null 2>&1; then
    echo "hdiutil is required to create a disk image" >&2
    return 1
  fi
  local stage mount
  stage="$(mktemp -d)"
  cp -R "$app" "$stage/$(app_bundle_name)"
  rm -f "$dmg"
  hdiutil create -volname "OwnTerm" -srcfolder "$stage" -ov -format UDZO "$dmg"
  rm -rf "$stage"
  mount="$(mktemp -d)"
  hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$dmg"
  if [[ ! -x "$mount/$(app_bundle_name)/Contents/MacOS/ownterm" ]]; then
    hdiutil detach "$mount" >/dev/null || true
    echo "disk image is missing OwnTerm.app" >&2
    return 1
  fi
  hdiutil detach "$mount" >/dev/null
  rmdir "$mount"
}

emit_dmg() {
  local app="$1"
  local mode="$2"
  local out_dir="$3"
  local name
  name="$(dmg_filename "$mode")"
  local dmg="$out_dir/$name"
  create_dmg "$app" "$dmg"
  write_sha256 "$dmg" "$dmg.sha256"
  echo "wrote $dmg"
}

main() {
  local arm64="" x86_64="" icon="" version="0.1.0" out_dir="" layout_only=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --arm64)
        arm64="$2"
        shift 2
        ;;
      --x86_64)
        x86_64="$2"
        shift 2
        ;;
      --icon)
        icon="$2"
        shift 2
        ;;
      --version)
        version="$2"
        shift 2
        ;;
      --out-dir)
        out_dir="$2"
        shift 2
        ;;
      --layout-only)
        layout_only=1
        shift
        ;;
      *)
        echo "unknown argument: $1" >&2
        return 1
        ;;
    esac
  done

  validate_version "$version"
  if [[ -z "$out_dir" ]]; then
    echo "--out-dir is required" >&2
    return 1
  fi
  mkdir -p "$out_dir"

  local have_arm=0 have_x86=0
  if [[ -n "$arm64" && -f "$arm64" ]]; then
    have_arm=1
  fi
  if [[ -n "$x86_64" && -f "$x86_64" ]]; then
    have_x86=1
  fi
  if [[ "$have_arm" == 0 && "$have_x86" == 0 ]]; then
    echo "pass --arm64 and/or --x86_64 pointing at an ownterm binary" >&2
    return 1
  fi

  if [[ "$layout_only" == 1 ]]; then
    local source="$arm64"
    if [[ "$have_arm" == 0 ]]; then
      source="$x86_64"
    fi
    assemble_app "$out_dir/$(app_bundle_name)" "$source" "$version" "$icon"
    return 0
  fi

  local work
  work="$(mktemp -d)"
  local lipo_ok=0
  if [[ "$have_arm" == 1 && "$have_x86" == 1 ]]; then
    if try_universal_binary "$arm64" "$x86_64" "$work/ownterm"; then
      lipo_ok=1
    fi
  fi
  local mode
  mode="$(package_mode "$have_arm" "$have_x86" "$lipo_ok")"

  if [[ "$mode" == universal ]]; then
    assemble_app "$work/$(app_bundle_name)" "$work/ownterm" "$version" "$icon"
    emit_dmg "$work/$(app_bundle_name)" universal "$out_dir"
  elif [[ "$mode" == split ]]; then
    assemble_app "$work/arm/$(app_bundle_name)" "$arm64" "$version" "$icon"
    assemble_app "$work/x86/$(app_bundle_name)" "$x86_64" "$version" "$icon"
    emit_dmg "$work/arm/$(app_bundle_name)" arm64 "$out_dir"
    emit_dmg "$work/x86/$(app_bundle_name)" x86_64 "$out_dir"
  elif [[ "$mode" == arm64 ]]; then
    assemble_app "$work/$(app_bundle_name)" "$arm64" "$version" "$icon"
    emit_dmg "$work/$(app_bundle_name)" arm64 "$out_dir"
  else
    assemble_app "$work/$(app_bundle_name)" "$x86_64" "$version" "$icon"
    emit_dmg "$work/$(app_bundle_name)" x86_64 "$out_dir"
  fi
  rm -rf "$work"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
