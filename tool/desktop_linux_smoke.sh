#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export GDK_BACKEND=x11
check=false
bundle="$PWD/build/linux/x64/release/bundle"
report=""
if [[ "${1:-}" == --check ]]; then
  [[ "$#" == 3 ]] || { echo "Usage: $0 --check <bundle> <new-report-directory>" >&2; exit 2; }
  check=true
  bundle="$(realpath "$2")"
  report="$(realpath -m "$3")"
  mkdir -p "$report"
  probe_config="$report/profile"
  mkdir "$probe_config"
else
  [[ "$#" == 0 || ( "$#" == 1 && "${1:-}" == --panel ) ]] ||
    { echo "Usage: $0 [--panel] or --check <bundle> <new-report-directory>" >&2; exit 2; }
  probe_config="$(mktemp -d /tmp/shuvpn-desktop-smoke.XXXXXX)"
fi
export XDG_CONFIG_HOME="$probe_config"
export XDG_DATA_HOME="$probe_config/data"
export XDG_CACHE_HOME="$probe_config/cache"
export XDG_RUNTIME_DIR="$probe_config/runtime"
mkdir -m 700 "$XDG_RUNTIME_DIR"
mkdir -p "$probe_config/xfce4/xfconf/xfce-perchannel-xml"
openbox --sm-disable >"$probe_config/openbox.log" 2>&1 &
wm_pid=$!
panel_pid=""
app_pid=""
cleanup() {
  if [[ -n "$app_pid" ]]; then kill "$app_pid" 2>/dev/null || true; fi
  if [[ -n "$panel_pid" ]]; then kill "$panel_pid" 2>/dev/null || true; fi
  kill "$wm_pid" 2>/dev/null || true
}
trap cleanup EXIT
for _ in {1..50}; do
  if [[ "$(xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null)" =~ 0x[1-9a-f] ]]; then break; fi
  sleep 0.1
done
if [[ "${1:-}" == "--panel" ]]; then
  cp /etc/xdg/xfce4/panel/default.xml "$probe_config/xfce4/xfconf/xfce-perchannel-xml/xfce4-panel.xml"
  xfce4-panel --disable-wm-check >"$probe_config/panel.log" 2>&1 &
  panel_pid=$!
  tray_ready=false
  for _ in {1..50}; do
    tray_reply="$(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.kde.StatusNotifierWatcher)"
    if [[ "$tray_reply" == *"(true,"* ]]; then
      tray_ready=true
      break
    fi
    sleep 0.1
  done
  if [[ "$tray_ready" != true ]]; then cat "$probe_config/panel.log"; exit 1; fi
fi
if [[ "$check" == false ]]; then
  "$bundle/shuvpn"
  exit
fi
"$bundle/shuvpn" >"$report/application.log" 2>&1 &
app_pid=$!
window=""
for _ in {1..100}; do
  kill -0 "$app_pid" 2>/dev/null || { cat "$report/application.log"; exit 1; }
  window="$(xdotool search --onlyvisible --pid "$app_pid" 2>/dev/null | head -n 1 || true)"
  if [[ -n "$window" ]]; then break; fi
  sleep 0.1
done
[[ -n "$window" ]] || { echo "Installed application did not show a window" >&2; exit 1; }
xdotool getwindowgeometry --shell "$window" >"$report/window.txt"
for _ in {1..100}; do
  if grep -F " $bundle/lib/libshuvpn_core.so" "/proc/$app_pid/maps" >"$report/native-library.txt"; then break; fi
  kill -0 "$app_pid" 2>/dev/null || { cat "$report/application.log"; exit 1; }
  sleep 0.1
done
grep -F "libshuvpn_core.so" "/proc/$app_pid/maps" >"$report/native-library-all.txt" || true
[[ -s "$report/native-library.txt" ]] ||
  { echo "Application did not load its bundled Rust core" >&2; exit 1; }
sleep 1
python3 - "$report/screenshot.png" <<'PY'
import sys, gi
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk
Gdk.init([])
root = Gdk.get_default_root_window()
image = Gdk.pixbuf_get_from_window(root, 0, 0, root.get_width(), root.get_height())
if image is None:
    raise RuntimeError("Could not capture the private X11 display")
image.savev(sys.argv[1], "png", [], [])
PY
xdotool windowactivate --sync "$window"
xdotool key --clearmodifiers alt+F4
for _ in {1..100}; do
  if ! kill -0 "$app_pid" 2>/dev/null; then break; fi
  sleep 0.1
done
if kill -0 "$app_pid" 2>/dev/null; then
  echo "Application did not exit after closing its window without a tray" >&2
  exit 1
fi
wait "$app_pid"
app_pid=""
printf 'LINUX_INSTALLED_GUI_WINDOW_AND_NO_TRAY_EXIT_PASSED %s\n' "$report"
