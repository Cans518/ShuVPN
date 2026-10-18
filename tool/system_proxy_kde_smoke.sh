#!/usr/bin/env bash
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"
probe="$repo/rust/target/release/examples/system_proxy_smoke"
test -x "$probe"
if [[ "${SHUVPN_KDE_ISOLATED:-}" != "1" ]]; then
  sandbox="$(mktemp -d)"
  trap 'rm -rf -- "$sandbox"' EXIT
  mkdir -p "$sandbox/config" "$sandbox/system" "$sandbox/runtime" "$sandbox/recovery"
  chmod 700 "$sandbox/runtime"
  export XDG_CONFIG_HOME="$sandbox/config"
  export XDG_CONFIG_DIRS="$sandbox/system"
  export XDG_RUNTIME_DIR="$sandbox/runtime"
  export XDG_CURRENT_DESKTOP=KDE
  export SHUVPN_KDE_ISOLATED=1
  export SHUVPN_KDE_RECOVERY="$sandbox/recovery"
  dbus-run-session -- bash "$0" "$@"
  exit
fi
if command -v kwriteconfig6 >/dev/null; then
  writer=kwriteconfig6
  reader=kreadconfig6
else
  writer=kwriteconfig5
  reader=kreadconfig5
fi
config="$XDG_CONFIG_HOME/kioslaverc"
write() { "$writer" --file "$config" --group 'Proxy Settings' --key "$1" -- "$2"; }
read() { "$reader" --file "$config" --group 'Proxy Settings' --key "$1" --default '__ABSENT__'; }
snapshot() {
  for key in ProxyType httpProxy httpsProxy NoProxyFor ReversedException 'Proxy Config Script'; do
    printf '%s=' "$key"
    read "$key"
  done
  "$reader" --file "$config" --group OtherApp --key Preserve --default '__ABSENT__'
}
write ProxyType 2
write httpProxy 'http://original.invalid 8123'
"$writer" --file "$config" --group 'Proxy Settings' --key httpsProxy --delete
write NoProxyFor ''
write ReversedException true
write 'Proxy Config Script' 'https://original.invalid/proxy.pac'
"$writer" --file "$config" --group OtherApp --key Preserve -- '原设置=keep'
original="$(snapshot)"

dbus-monitor --session "type='signal',interface='org.kde.KIO.Scheduler',member='reparseSlaveConfiguration'" >"$SHUVPN_KDE_RECOVERY/signals.log" 2>&1 &
monitor_pid=$!
trap 'kill "$monitor_pid" 2>/dev/null || true' EXIT
for mode in stop http-stop cancel; do
  "$probe" "$mode" "$SHUVPN_KDE_RECOVERY"
  test "$(snapshot)" = "$original"
  test ! -f "$SHUVPN_KDE_RECOVERY/system-proxy.json"
done
grep -q 'member=reparseSlaveConfiguration' "$SHUVPN_KDE_RECOVERY/signals.log"
grep -q 'string ""' "$SHUVPN_KDE_RECOVERY/signals.log"

system_config="$XDG_CONFIG_DIRS/kioslaverc"
"$writer" --file "$system_config" --group 'Proxy Settings' --key httpsProxy -- 'http://inherited.invalid 9333'
inherited="$("$reader" --file kioslaverc --group 'Proxy Settings' --key httpsProxy)"
test "$inherited" = 'http://inherited.invalid 9333'
"$probe" stop "$SHUVPN_KDE_RECOVERY"
test "$(snapshot)" = "$original"
test "$("$reader" --file kioslaverc --group 'Proxy Settings' --key httpsProxy)" = "$inherited"

printf '%s\n' 'ProxyType[$i]=2' >>"$system_config"
if "$probe" stop "$SHUVPN_KDE_RECOVERY" >"$SHUVPN_KDE_RECOVERY/immutable.log" 2>&1; then
  echo "KDE policy lock was incorrectly ignored" >&2
  exit 1
fi
grep -q 'KDE 配置工具返回失败' "$SHUVPN_KDE_RECOVERY/immutable.log"
test "$(snapshot)" = "$original"
test ! -f "$SHUVPN_KDE_RECOVERY/system-proxy.json"
rm -- "$system_config"

cp "$config" "$SHUVPN_KDE_RECOVERY/original-config"
printf '%s\n' '[Proxy Settings]' 'httpProxy[$e]=http://expanded.invalid 8123' >>"$config"
expanded="$(cat "$config")"
if "$probe" stop "$SHUVPN_KDE_RECOVERY" >"$SHUVPN_KDE_RECOVERY/expanded.log" 2>&1; then
  echo "Unsupported expansion metadata was silently accepted" >&2
  exit 1
fi
grep -q '含展开或删除标记' "$SHUVPN_KDE_RECOVERY/expanded.log"
test "$(cat "$config")" = "$expanded"
cp "$SHUVPN_KDE_RECOVERY/original-config" "$config"

write NoProxyFor external.local
external="$(snapshot)"
write NoProxyFor ''
"$probe" external-bypass "$SHUVPN_KDE_RECOVERY"
test "$(snapshot)" = "$external"
write NoProxyFor ''

"$probe" crash "$SHUVPN_KDE_RECOVERY" >"$SHUVPN_KDE_RECOVERY/crash.log" 2>&1 &
probe_pid=$!
ready=0
for ((attempt=0; attempt<100; attempt++)); do
  if grep -q SYSTEM_PROXY_CRASH_READY "$SHUVPN_KDE_RECOVERY/crash.log"; then
    ready=1
    break
  fi
  kill -0 "$probe_pid"
  sleep 0.05
done
test "$ready" = 1
kill -KILL "$probe_pid"
wait "$probe_pid" 2>/dev/null || true
test "$(read ProxyType)" = 1
"$probe" recover "$SHUVPN_KDE_RECOVERY"
test "$(snapshot)" = "$original"

for mode in external-original external-original-crash; do
  if [[ "$mode" = external-original ]]; then
    "$probe" "$mode" "$SHUVPN_KDE_RECOVERY" 'http://original.invalid 8123'
  else
    "$probe" "$mode" "$SHUVPN_KDE_RECOVERY" 'http://original.invalid 8123' >"$SHUVPN_KDE_RECOVERY/original-crash.log" 2>&1 &
    probe_pid=$!
    ready=0
    for ((attempt=0; attempt<100; attempt++)); do
      if grep -q SYSTEM_PROXY_CRASH_READY "$SHUVPN_KDE_RECOVERY/original-crash.log"; then
        ready=1
        break
      fi
      kill -0 "$probe_pid"
      sleep 0.05
    done
    test "$ready" = 1
    kill -KILL "$probe_pid"
    wait "$probe_pid" 2>/dev/null || true
    "$probe" recover "$SHUVPN_KDE_RECOVERY"
  fi
  test "$(read ProxyType)" = 1
  test "$(read httpProxy)" = 'http://original.invalid 8123'
  write ProxyType 2
  test "$(snapshot)" = "$original"
done

if [[ $# -gt 0 ]]; then
  "$probe" consumer "$SHUVPN_KDE_RECOVERY" "$1"
  test "$(snapshot)" = "$original"
fi
if [[ $# -gt 1 ]]; then
  "$2" "$repo/build/linux/x64/release/bundle/lib/libshuvpn_core.so"
  test "$(snapshot)" = "$original"
fi
echo KDE_KCONFIG_LIFECYCLE_SIGNAL_EMPTY_ABSENT_EXTERNAL_AND_CRASH_RECOVERY_PASSED
