#!/usr/bin/env bash
# For trusted locally built ShuVPN packages; this is not a malware sandbox.
set -euo pipefail
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH

overlay_directory() {
  local target="$1" label="$2" staging="$3"
  mkdir -p "$staging/$label-lower" "$staging/$label-upper" "$staging/$label-work"
  chmod 755 "$staging/$label-upper"
  mount --bind "$target" "$staging/$label-lower"
  mount -t overlay overlay -o "lowerdir=$staging/$label-lower,upperdir=$staging/$label-upper,workdir=$staging/$label-work" "$target"
}

inside() {
  local deb="$1" probe="$2" account="$3" staging="$4"
  local tun_probe="${5:-}" fixture="${6:-}"
  local tun_rounds="${SHUVPN_TUN_SMOKE_ROUNDS:-3}"
  [[ "$tun_rounds" =~ ^([1-9]|[1-9][0-9]|100)$ ]] || {
    echo 'SHUVPN_TUN_SMOKE_ROUNDS must be an integer from 1 to 100' >&2
    return 2
  }
  [[ "$(readlink /proc/self/ns/mnt)" != "$SHUVPN_INSTALL_PARENT_MNT" ]]
  [[ "$(readlink /proc/self/ns/net)" != "$SHUVPN_INSTALL_PARENT_NET" ]]
  mount --make-rprivate /
  local preserved_sdk=""
  if [[ -n "${SHUVPN_DEB_FLUTTER_ROOT:-}" ]]; then
    SHUVPN_DEB_FLUTTER_ROOT="$(realpath "$SHUVPN_DEB_FLUTTER_ROOT")"
    [[ -x "$SHUVPN_DEB_FLUTTER_ROOT/bin/cache/dart-sdk/bin/dart" ]]
    [[ -f "$SHUVPN_DEB_FLUTTER_ROOT/bin/cache/flutter_tools.snapshot" ]]
    if [[ "$SHUVPN_DEB_FLUTTER_ROOT" == /opt/* ]]; then
      [[ "$SHUVPN_DEB_FLUTTER_ROOT" != /opt/shuvpn* ]]
      preserved_sdk="$staging/flutter-sdk"
      mkdir "$preserved_sdk"
      mount --bind "$SHUVPN_DEB_FLUTTER_ROOT" "$preserved_sdk"
    fi
  fi
  overlay_directory /etc etc "$staging"
  overlay_directory /usr/lib lib "$staging"
  overlay_directory /usr/sbin sbin "$staging"
  overlay_directory /var/lib/dpkg dpkg "$staging"
  overlay_directory /usr/share share "$staging"
  mount -t tmpfs -o mode=755 tmpfs /opt
  if [[ -n "$preserved_sdk" ]]; then
    mkdir -p "$SHUVPN_DEB_FLUTTER_ROOT"
    mkdir "$staging/flutter-upper" "$staging/flutter-work"
    chown --reference="$preserved_sdk" "$staging/flutter-upper"
    chmod --reference="$preserved_sdk" "$staging/flutter-upper"
    mount -t overlay overlay \
      -o "lowerdir=$preserved_sdk,upperdir=$staging/flutter-upper,workdir=$staging/flutter-work" \
      "$SHUVPN_DEB_FLUTTER_ROOT"
    [[ "$(stat -f -c %T "$SHUVPN_DEB_FLUTTER_ROOT")" == overlayfs ]]
    local sdk_marker=".shuvpn-$(basename "$staging")"
    runuser -u "$account" -- python3 -c \
      'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text("private cache")' \
      "$SHUVPN_DEB_FLUTTER_ROOT/bin/cache/$sdk_marker"
    [[ ! -e "$preserved_sdk/bin/cache/$sdk_marker" ]]
    rm "$SHUVPN_DEB_FLUTTER_ROOT/bin/cache/$sdk_marker"
    echo 'LINUX_DEB_FLUTTER_SDK_PRESERVED_UNDER_OPT_WITH_PRIVATE_WRITES'
  fi
  [[ "$(findmnt -n -o FSTYPE /usr/share)" == overlay ]]
  # This removal is confined to the verified private overlay.
  rm -rf -- /usr/share/applications /usr/share/doc/shuvpn
  ip link set lo up
  local dpkg_command=(dpkg "--log=$staging/dpkg.log")
  install -m 755 "$probe" "$staging/ffi-probe"
  # Refuse foreign installations and symlinks without changing their contents.
  mkdir /opt/shuvpn
  printf 'foreign\n' > /opt/shuvpn/foreign
  if "${dpkg_command[@]}" --install "$deb"; then
    echo 'DEB overwrote an unmanaged installation' >&2; exit 1
  fi
  [[ "$(cat /opt/shuvpn/foreign)" == foreign ]]
  rm /opt/shuvpn/foreign
  rmdir /opt/shuvpn
  mkdir "$staging/victim"
  printf 'victim\n' > "$staging/victim/sentinel"
  ln -s "$staging/victim" /opt/shuvpn
  if "${dpkg_command[@]}" --install "$deb"; then
    echo 'DEB followed an installation symlink' >&2; exit 1
  fi
  [[ "$(cat "$staging/victim/sentinel")" == victim ]]
  [[ -L /opt/shuvpn ]]
  rm /opt/shuvpn

  # A synthetic older control version exercises dpkg's upgrade path with the
  # same verified runtime payload, not a claim about an unavailable old release.
  dpkg-deb --raw-extract "$deb" "$staging/previous"
  python3 - "$staging/previous/DEBIAN/control" <<'PY'
import re, sys
from pathlib import Path
p = Path(sys.argv[1])
p.write_text(re.sub(r"^Version:.*$", "Version: 0~installer-smoke", p.read_text(), flags=re.M))
PY
  dpkg-deb --root-owner-group --build "$staging/previous" "$staging/previous.deb"
  cp /usr/sbin/setcap "$staging/setcap-original"
  printf '#!/bin/sh\nexit 1\n' > /usr/sbin/setcap
  chmod 755 /usr/sbin/setcap
  if "${dpkg_command[@]}" --install "$staging/previous.deb"; then
    echo 'DEB hid a capability configuration failure' >&2; exit 1
  fi
  [[ -z "$(getcap /usr/lib/shuvpn/shuvpn_tun_helper)" ]]
  cp "$staging/setcap-original" /usr/sbin/setcap
  chmod 755 /usr/sbin/setcap
  "${dpkg_command[@]}" --configure shuvpn
  [[ "$(stat -c '%U:%G:%a' /usr/lib/shuvpn/shuvpn_tun_helper)" == root:shuvpn:750 ]]
  [[ -z "$(getcap /opt/shuvpn/shuvpn)" && -z "$(getcap /opt/shuvpn/shuvpn_tun_helper)" ]]
  local library=/opt/shuvpn/lib/libshuvpn_core.so
  if id -nG "$account" | tr ' ' '\n' | grep -qx shuvpn; then
    gpasswd -d "$account" shuvpn
  fi
  runuser -u "$account" -- "$staging/ffi-probe" "$library" unavailable
  usermod --append --groups shuvpn -- "$account"
  runuser -u "$account" -- "$staging/ffi-probe" "$library" ready
  runuser -u nobody -- "$staging/ffi-probe" "$library" unavailable
  printf 'retained user data\n' > "$staging/user-data"
  local user_data_before
  user_data_before="$(sha256sum "$staging/user-data")"
  printf 'local note\n' > /opt/shuvpn/local-note.txt
  local account_hash=""
  if [[ -n "${SHUVPN_DART_RUNTIME:-}" ]]; then
    install -d -m 700 -o "$(id -u "$account")" -g "$(id -g "$account")" "$staging/account-data"
    runuser -u "$account" -- "$SHUVPN_DART_RUNTIME" run "$(dirname "$(realpath "$0")")/account_storage_smoke.dart" "$library" "$staging/account-data" import
    account_hash="$(find "$staging/account-data" -type f -exec sha256sum {} \; | sort)"
  fi
  if python3 /opt/shuvpn/install.py --uninstall; then
    echo 'Portable installer removed dpkg-managed files' >&2; exit 1
  fi
  cp /bin/sleep /opt/shuvpn/installer-running-fixture
  /opt/shuvpn/installer-running-fixture 60 &
  local busy_pid=$!
  if "${dpkg_command[@]}" --remove shuvpn; then
    kill "$busy_pid"; wait "$busy_pid" || true
    echo 'DEB removed a running application' >&2; exit 1
  fi
  kill "$busy_pid"
  wait "$busy_pid" || true
  rm /opt/shuvpn/installer-running-fixture
  setcap -r /usr/lib/shuvpn/shuvpn_tun_helper
  runuser -u "$account" -- "$staging/ffi-probe" "$library" unavailable
  "${dpkg_command[@]}" --install "$deb"
  [[ "$(dpkg-query -W -f='${Version}' shuvpn)" == "$(dpkg-deb -f "$deb" Version)" ]]
  runuser -u "$account" -- "$staging/ffi-probe" "$library" ready
  if [[ -n "$tun_probe" ]]; then
    install -m 755 "$tun_probe" "$staging/tun-probe"
    install -m 755 "$fixture" "$staging/fixture"
    ip link add shuegress type dummy
    ip link set shuegress up
    ip address add 192.0.2.1/24 dev shuegress
    ip route add default via 192.0.2.254 dev shuegress
    for ((round = 1; round <= tun_rounds; round++)); do
      echo "DEB_TUN_ROUND_$round"
      timeout --kill-after=5s 45s runuser -u "$account" -- \
        "$staging/tun-probe" --installed-helper "$library" "$staging/fixture"
    done
  fi
  if [[ -n "${SHUVPN_DEB_FLUTTER_ROOT:-}" ]]; then
    [[ -n "$fixture" && -n "${SHUVPN_DEB_FULL_REPORT:-}" ]]
    install -m 755 "$fixture" "$staging/full-fixture"
    unshare --net python3 "$(dirname "$(realpath "$0")")/installed_full_capture_smoke.py" \
      --account "$account" --flutter "$SHUVPN_DEB_FLUTTER_ROOT" \
      --payload "$staging/previous" --fixture "$staging/full-fixture" \
      --deb "$deb" --report "$SHUVPN_DEB_FULL_REPORT"
  fi
  if [[ -n "${SHUVPN_DEB_GUI_REPORT:-}" ]]; then
    report="$SHUVPN_DEB_GUI_REPORT"
    [[ "$report" == /* && ! -e "$report" ]]
    install -d -m 700 -o "$(id -u "$account")" -g "$(id -g "$account")" "$report"
    if [[ -n "${SHUVPN_GUI_GDB:-}" ]]; then
      (
        declare -A observed_gui=()
        for ((attempt = 0; attempt < 1600; attempt++)); do
          for process in /proc/[0-9]*; do
            if [[ "$(readlink "$process/exe" 2>/dev/null || true)" == /opt/shuvpn/shuvpn ]]; then
              [[ -z "${observed_gui[${process##*/}]:-}" ]] || continue
              observed_gui[${process##*/}]=1
              LD_LIBRARY_PATH="${SHUVPN_GUI_GDB_LIB:-}" "$SHUVPN_GUI_GDB" -nx -batch \
                -ex 'set pagination off' -ex continue -ex 'thread apply all bt' -ex continue \
                -p "${process##*/}" > "$report/gdb-${process##*/}.log" 2>&1 || true
              if [[ "${#observed_gui[@]}" == 3 ]]; then exit; fi
            fi
          done
          sleep 0.05
        done
        echo 'GUI debugger did not find the owned installed process' > "$report/gdb-backtrace.log"
      ) &
    fi
    if [[ "${SHUVPN_DEB_GUI_ACCEPTANCE:-0}" == 1 ]]; then
      [[ -x "${SHUVPN_DART_RUNTIME:-}" ]]
      runuser -u "$account" -- python3 "$(dirname "$(realpath "$0")")/desktop_rule_acceptance.py" \
        --bundle /opt/shuvpn --report "$report/session" --dart "$SHUVPN_DART_RUNTIME"
      runuser -u "$account" -- python3 "$(dirname "$(realpath "$0")")/desktop_rule_management_acceptance.py" \
        --bundle /opt/shuvpn --report "$report/rule-management" --dart "$SHUVPN_DART_RUNTIME"
      runuser -u "$account" -- python3 "$(dirname "$(realpath "$0")")/desktop_rule_drag_acceptance.py" \
        --bundle /opt/shuvpn --report "$report/rule-drag" --dart "$SHUVPN_DART_RUNTIME"
    else
      runuser -u "$account" -- xvfb-run -a dbus-run-session \
        bash "$(dirname "$(realpath "$0")")/desktop_linux_smoke.sh" --check /opt/shuvpn "$report"
    fi
  fi
  if [[ -n "${SHUVPN_DEB_TRAY_REPORT:-}" ]]; then
    report="$SHUVPN_DEB_TRAY_REPORT"
    [[ "$report" == /* && ! -e "$report" ]]
    [[ -x "${SHUVPN_DART_RUNTIME:-}" ]]
    install -d -m 700 -o "$(id -u "$account")" -g "$(id -g "$account")" "$report"
    runuser -u "$account" -- python3 "$(dirname "$(realpath "$0")")/desktop_tray_acceptance.py" \
      --bundle /opt/shuvpn --report "$report/session" --dart "$SHUVPN_DART_RUNTIME"
  fi
  # Preserve a library outside the install tree to check readiness after removal.
  cp "$library" "$staging/libshuvpn_core.so"
  chmod 755 "$staging/libshuvpn_core.so"
  "${dpkg_command[@]}" --remove shuvpn
  "${dpkg_command[@]}" --purge shuvpn
  [[ "$(cat /opt/shuvpn/local-note.txt)" == "local note" ]]
  [[ "$(cat /opt/shuvpn/.package-manager)" == dpkg ]]
  "${dpkg_command[@]}" --install "$deb"
  runuser -u "$account" -- "$staging/ffi-probe" "$library" ready
  rm /opt/shuvpn/local-note.txt
  "${dpkg_command[@]}" --remove shuvpn
  "${dpkg_command[@]}" --purge shuvpn
  [[ ! -e /opt/shuvpn && ! -e /usr/lib/shuvpn ]]
  [[ ! -e /usr/share/applications/work.shuvpn.app.desktop ]]
  [[ "$(sha256sum "$staging/user-data")" == "$user_data_before" ]]
  id -nG "$account" | tr ' ' '\n' | grep -qx shuvpn
  runuser -u "$account" -- "$staging/ffi-probe" "$staging/libshuvpn_core.so" unavailable
  if [[ -n "${SHUVPN_DART_RUNTIME:-}" ]]; then
    [[ "$(find "$staging/account-data" -type f -exec sha256sum {} \; | sort)" == "$account_hash" ]]
    runuser -u "$account" -- "$SHUVPN_DART_RUNTIME" run "$(dirname "$(realpath "$0")")/account_storage_smoke.dart" "$staging/libshuvpn_core.so" "$staging/account-data" restore
    echo 'LINUX_DEB_NATIVE_ACCOUNT_PRESERVED_AFTER_UPGRADE_AND_PURGE'
  fi
  echo 'LINUX_DEB_INSTALL_UPGRADE_REMOVE_PURGE_AND_AUTHORIZATION_PASSED'
}

if [[ "${1:-}" == --inside ]]; then
  shift
  inside "$@"
else
  [[ "$EUID" == 0 && ( "$#" == 3 || "$#" == 5 ) ]] || {
    echo "Usage: sudo bash $0 <deb> <compiled-readiness-probe> <account> [tun-probe fixture]" >&2
    exit 2
  }
  deb="$(realpath "$1")"
  [[ "$(dpkg-deb -f "$deb" Package)" == shuvpn ]]
  [[ "$(dpkg-deb -f "$deb" Architecture)" == amd64 ]]
  probe="$(realpath "$2")"
  account="$3"
  [[ -n "$account" && "$account" != -* ]]
  account_uid="$(id -u -- "$account")"
  [[ "$account_uid" != 0 ]]
  staging="$(mktemp -d /tmp/shuvpn-deb-smoke.XXXXXX)"
  chmod 755 "$staging"
  trap 'rm -rf -- "$staging"' EXIT
  export SHUVPN_INSTALL_PARENT_MNT="$(readlink /proc/self/ns/mnt)"
  export SHUVPN_INSTALL_PARENT_NET="$(readlink /proc/self/ns/net)"
  before="$(sha256sum /etc/passwd /etc/shadow /etc/group /etc/gshadow /var/lib/dpkg/status)"
  extra=()
  if [[ "$#" == 5 ]]; then extra=("$(realpath "$4")" "$(realpath "$5")"); fi
  result=0
  unshare --mount --net --pid --mount-proc --fork bash "$(realpath "$0")" --inside "$deb" "$probe" "$account" "$staging" "${extra[@]}" || result=$?
  [[ "$(sha256sum /etc/passwd /etc/shadow /etc/group /etc/gshadow /var/lib/dpkg/status)" == "$before" ]]
  echo 'LINUX_DEB_HOST_ACCOUNTS_AND_PACKAGE_DATABASE_UNCHANGED'
  exit "$result"
fi
