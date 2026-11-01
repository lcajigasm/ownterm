#!/usr/bin/env bash
# Capture ownterm-tui from a real process under Xvfb. Does not draw a mock UI.
set -euo pipefail

bin=""
out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin)
      bin="$2"
      shift 2
      ;;
    --out-dir)
      out="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$bin" || -z "$out" ]]; then
  echo "usage: capture-ownterm.sh --bin PATH --out-dir DIR" >&2
  exit 1
fi
if [[ ! -x "$bin" ]]; then
  echo "binary is not executable: $bin" >&2
  exit 1
fi
for tool in Xvfb xterm xdotool import; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing $tool" >&2
    exit 1
  fi
done

mkdir -p "$out"
work="$(mktemp -d)"
home="$work/home"
mkdir -p "$home"
display_num=95
while [[ -e "/tmp/.X${display_num}-lock" ]]; do
  display_num=$((display_num + 1))
done
display=":$display_num"

xvfb_pid=""
cleanup() {
  if [[ -n "$xvfb_pid" ]]; then
    kill "$xvfb_pid" >/dev/null 2>&1 || true
    wait "$xvfb_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

Xvfb "$display" -screen 0 1280x800x24 >/dev/null 2>&1 &
xvfb_pid=$!
export DISPLAY="$display"
sleep 0.4

window_id() {
  local title="$1"
  local wid=""
  local attempt
  for attempt in 1 2 3 4 5 6 7 8 9 10; do
    wid="$(xdotool search --name "$title" 2>/dev/null | head -n 1 || true)"
    if [[ -n "$wid" ]]; then
      printf '%s\n' "$wid"
      return 0
    fi
    sleep 0.3
  done
  echo "no window titled $title" >&2
  return 1
}

shot_window() {
  local wid="$1"
  local dest="$2"
  import -window "$wid" "$dest"
  local bytes
  bytes="$(wc -c <"$dest" | tr -d ' ')"
  if [[ "$bytes" -lt 1500 ]]; then
    echo "capture $dest is too small ($bytes bytes)" >&2
    return 1
  fi
}

run_held() {
  local title="$1"
  local dest="$2"
  local geometry="$3"
  shift 3
  xterm -T "$title" -fa "DejaVu Sans Mono" -fs 13 -bg "#101418" -fg "#e6edf3" \
    -geometry "${geometry}+8+8" -hold -e "$@" >/dev/null 2>&1 &
  local pid=$!
  local wid
  wid="$(window_id "$title")"
  sleep 0.8
  shot_window "$wid" "$dest"
  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" 2>/dev/null || true
}

export HOME="$home"
export TERM=xterm-256color

# Command output only. The interactive zero state still says "Log in with Warp",
# and this Xvfb has no window manager, so keystrokes never reach that screen.
run_held "ownterm-models" "$out/ownterm-tui-models.png" "64x8" \
  "$bin" models --provider ollama
run_held "ownterm-help" "$out/ownterm-tui-help.png" "112x32" \
  "$bin" --help

echo "captured ownterm-tui into $out"
