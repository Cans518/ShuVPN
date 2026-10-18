#!/usr/bin/env bash
set -euo pipefail

overlay_directory() {
  local target="$1" label="$2" staging="$3"
  mkdir -p "$staging/$label-lower" "$staging/$label-upper" "$staging/$label-work"
  chmod 755 "$staging/$label-upper"
  mount --bind "$target" "$staging/$label-lower"
  mount -t overlay overlay \
    -o "lowerdir=$staging/$label-lower,upperdir=$staging/$label-upper,workdir=$staging/$label-work" \
    "$target"
}

inside() {
  local bundle="$1" probe="$2" account="$3" staging="$4"
  local tun_probe="${5:-}" fixture="${6:-}"
  [[ "$(readlink /proc/self/ns/mnt)" != "$SHUVPN_INSTALL_PARENT_MNT" ]]
  [[ "$(readlink /proc/self/ns/net)" != "$SHUVPN_INSTALL_PARENT_NET" ]]
  mount --make-rprivate /
  overlay_directory /etc etc "$staging"
  overlay_directory /usr/lib lib "$staging"
  mount -t tmpfs -o mode=755 tmpfs /opt
  mount -t tmpfs -o mode=755 tmpfs /usr/share/applications
  ip link set lo up
  install -m 755 "$probe" "$staging/ffi-probe"
  local library="$bundle/lib/libshuvpn_core.so"
  runuser -u "$account" -- "$staging/ffi-probe" "$library" unavailable
  python3 "$bundle/install.py" --user "$account"
  [[ "$(stat -c '%U:%G:%a' /usr/lib/shuvpn/shuvpn_tun_helper)" == root:shuvpn:750 ]]
  [[ -z "$(getcap /opt/shuvpn/shuvpn)" ]]
  [[ -z "$(getcap /opt/shuvpn/shuvpn_tun_helper)" ]]
  cmp "$bundle/shuvpn_tun_helper" /usr/lib/shuvpn/shuvpn_tun_helper
  runuser -u "$account" -- "$staging/ffi-probe" /opt/shuvpn/lib/libshuvpn_core.so ready
  if [[ -n "$tun_probe" ]]; then
    install -m 755 "$tun_probe" "$staging/tun-probe"
    install -m 755 "$fixture" "$staging/fixture"
    ip link add shuegress type dummy
    ip link set shuegress up
    ip address add 192.0.2.1/24 dev shuegress
    ip route add default via 192.0.2.254 dev shuegress
    runuser -u "$account" -- "$staging/tun-probe" --installed-helper \
      /opt/shuvpn/lib/libshuvpn_core.so "$staging/fixture"
  fi
  runuser -u nobody -- "$staging/ffi-probe" /opt/shuvpn/lib/libshuvpn_core.so unavailable
  chmod g+w /usr/lib/shuvpn/shuvpn_tun_helper
  runuser -u "$account" -- "$staging/ffi-probe" /opt/shuvpn/lib/libshuvpn_core.so unavailable
  chmod 750 /usr/lib/shuvpn/shuvpn_tun_helper
  setpriv --no-new-privs runuser -u "$account" -- \
    "$staging/ffi-probe" /opt/shuvpn/lib/libshuvpn_core.so unavailable
  # The execute bit alone must not make a helper without capabilities ready.
  setcap -r /usr/lib/shuvpn/shuvpn_tun_helper
  runuser -u "$account" -- "$staging/ffi-probe" /opt/shuvpn/lib/libshuvpn_core.so unavailable
  # Upgrade uses the same transaction and restores the file capabilities.
  python3 "$bundle/install.py" --user "$account"
  runuser -u "$account" -- "$staging/ffi-probe" /opt/shuvpn/lib/libshuvpn_core.so ready
  if [[ "${SHUVPN_INSTALL_GUI:-0}" == 1 ]]; then
    local report="${SHUVPN_INSTALL_GUI_REPORT:?Provide an absolute GUI report directory}"
    [[ "$report" == /* ]]
    install -d -m 700 -o "$(id -u "$account")" -g "$(id -g "$account")" "$report"
    install -d -m 700 -o "$(id -u "$account")" -g "$(id -g "$account")" "$report/session-runtime"
    runuser -u "$account" -- env XDG_RUNTIME_DIR="$report/session-runtime" \
      XDG_CONFIG_HOME="$report/session-config" XDG_DATA_HOME="$report/session-data" \
      XDG_CACHE_HOME="$report/session-cache" xvfb-run -a dbus-run-session \
      bash "$(dirname "$(realpath "$0")")/desktop_linux_smoke.sh" --check /opt/shuvpn "$report"
  fi
  python3 /opt/shuvpn/install.py --uninstall
  [[ ! -e /opt/shuvpn && ! -e /usr/lib/shuvpn ]]
  [[ ! -e /usr/share/applications/work.shuvpn.app.desktop ]]
  runuser -u "$account" -- "$staging/ffi-probe" "$library" unavailable
  # Every readiness call confirms the actual helper exit before returning.
  python3 - <<'PY'
from pathlib import Path
for process in Path("/proc").glob("[0-9]*"):
    try:
        executable = (process / "exe").readlink()
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        continue
    if str(executable).startswith("/usr/lib/shuvpn/shuvpn_tun_helper"):
        raise SystemExit(f"Installed helper still alive: {process.name}")
print("LINUX_ISOLATED_INSTALL_UPGRADE_UNINSTALL_AND_DEFAULT_FFI_PASSED")
PY
}

main() {
  if [[ "${1:-}" == --inside ]]; then
    shift
    inside "$@"
    return
  fi
  if [[ "$EUID" != 0 || ( "$#" != 3 && "$#" != 5 ) ]]; then
    printf 'Usage: sudo bash %s <extracted-package> <compiled-ffi-probe> <ordinary-account> [compiled-tun-probe gateway-fixture]\n' "$0" >&2
    exit 2
  fi
  local bundle probe account staging
  local tun_probe="" fixture=""
  local accounts_before
  bundle="$(realpath "$1")"
  probe="$(realpath "$2")"
  account="$3"
  if [[ "$#" == 5 ]]; then
    tun_probe="$(realpath "$4")"
    fixture="$(realpath "$5")"
  fi
  [[ "$(id -u "$account")" != 0 ]]
  export SHUVPN_INSTALL_PARENT_MNT SHUVPN_INSTALL_PARENT_NET
  SHUVPN_INSTALL_PARENT_MNT="$(readlink /proc/self/ns/mnt)"
  SHUVPN_INSTALL_PARENT_NET="$(readlink /proc/self/ns/net)"
  staging="$(mktemp -d /tmp/shuvpn-install-smoke.XXXXXX)"
  chmod 755 "$staging"
  trap 'rm -rf -- "$staging"' EXIT
  accounts_before="$(sha256sum /etc/passwd /etc/shadow /etc/group /etc/gshadow)"
  unshare --mount --net --fork bash "$(realpath "$0")" --inside \
    "$bundle" "$probe" "$account" "$staging" "$tun_probe" "$fixture"
  [[ "$(sha256sum /etc/passwd /etc/shadow /etc/group /etc/gshadow)" == "$accounts_before" ]]
  rm -rf -- "$staging"
  trap - EXIT
}

main "$@"
