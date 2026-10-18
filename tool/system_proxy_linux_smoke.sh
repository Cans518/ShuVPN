#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
probe="$repo/rust/target/release/examples/system_proxy_smoke"
test -x "$probe"
if [[ "${SHUVPN_PROXY_ISOLATED:-}" != "1" ]]; then
  sandbox="$(mktemp -d)"
  trap 'rm -rf -- "$sandbox"' EXIT
  mkdir -p "$sandbox/config"
  export XDG_CONFIG_HOME="$sandbox/config"
  export XDG_CURRENT_DESKTOP=GNOME
  export GSETTINGS_BACKEND=dconf
  export SHUVPN_PROXY_ISOLATED=1
  export SHUVPN_PROXY_RECOVERY="$sandbox/recovery"
  exec_status=0
  dbus-run-session -- bash "$0" "$@" || exec_status=$?
  exit "$exec_status"
fi

# This private D-Bus starts its own dconf service with the temporary config home.
test -n "${SHUVPN_PROXY_RECOVERY:-}"
mkdir -p "$SHUVPN_PROXY_RECOVERY"
gsettings set org.gnome.system.proxy mode auto
gsettings set org.gnome.system.proxy autoconfig-url 'https://original.invalid/proxy.pac'
gsettings set org.gnome.system.proxy ignore-hosts "[]"
gsettings set org.gnome.system.proxy.http host original.invalid
gsettings set org.gnome.system.proxy.http port 8123
gsettings set org.gnome.system.proxy.http use-authentication true
gsettings reset org.gnome.system.proxy.https host
gsettings reset org.gnome.system.proxy.https port
original="$(gsettings list-recursively org.gnome.system.proxy)"
if GSETTINGS_BACKEND=memory "$probe" stop "$SHUVPN_PROXY_RECOVERY" >"$SHUVPN_PROXY_RECOVERY/unsupported.log" 2>&1; then
  echo "A process-private settings backend was incorrectly accepted" >&2
  exit 1
fi
grep -q '进程私有设置后端' "$SHUVPN_PROXY_RECOVERY/unsupported.log"
test ! -f "$SHUVPN_PROXY_RECOVERY/system-proxy.json"
test "$(gsettings list-recursively org.gnome.system.proxy)" = "$original"
for mode in stop http-stop cancel; do
  "$probe" "$mode" "$SHUVPN_PROXY_RECOVERY"
  actual="$(gsettings list-recursively org.gnome.system.proxy)"
  test "$actual" = "$original"
  test ! -f "$SHUVPN_PROXY_RECOVERY/system-proxy.json"
done

gsettings set org.gnome.system.proxy ignore-hosts "['original.local']"
original="$(gsettings list-recursively org.gnome.system.proxy)"
gsettings set org.gnome.system.proxy ignore-hosts "['external.local']"
external="$(gsettings list-recursively org.gnome.system.proxy)"
gsettings set org.gnome.system.proxy ignore-hosts "['original.local']"
"$probe" external-bypass "$SHUVPN_PROXY_RECOVERY"
test "$(gsettings list-recursively org.gnome.system.proxy)" = "$external"
gsettings set org.gnome.system.proxy ignore-hosts "['original.local']"
"$probe" crash "$SHUVPN_PROXY_RECOVERY" >"$SHUVPN_PROXY_RECOVERY/crash.log" 2>&1 &
probe_pid=$!
ready=0
for ((attempt=0; attempt<100; attempt++)); do
  if grep -q SYSTEM_PROXY_CRASH_READY "$SHUVPN_PROXY_RECOVERY/crash.log"; then
    ready=1
    break
  fi
  kill -0 "$probe_pid"
  sleep 0.05
done
test "$ready" = 1
kill -KILL "$probe_pid"
wait "$probe_pid" 2>/dev/null || true
test -f "$SHUVPN_PROXY_RECOVERY/system-proxy.json"
test "$(gsettings get org.gnome.system.proxy mode)" = "'manual'"
"$probe" recover "$SHUVPN_PROXY_RECOVERY"
test "$(gsettings list-recursively org.gnome.system.proxy)" = "$original"
test ! -f "$SHUVPN_PROXY_RECOVERY/system-proxy.json"
if [[ $# -gt 0 ]]; then
  dart run "$repo/tool/system_proxy_bridge_smoke.dart" "$1"
  test "$(gsettings list-recursively org.gnome.system.proxy)" = "$original"
fi
echo GNOME_SYSTEM_PROXY_DCONF_LIFECYCLE_AND_SIGKILL_RECOVERY_PASSED
