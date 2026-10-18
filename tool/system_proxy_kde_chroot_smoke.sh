#!/usr/bin/env bash
set -euo pipefail
test "$#" -ge 1
rootfs="$(realpath "$1")"
case "$(basename "$rootfs")" in
  kde6-rootfs|shuvpn-kde6-rootfs) ;;
  *) echo "Expected an explicitly named KDE6 test root" >&2; exit 1 ;;
esac
test "$rootfs" != /
test -x "$rootfs/usr/bin/kreadconfig6"
test -x "$rootfs/work/tool/system_proxy_bridge_smoke"
if [[ "${2:-}" != "private-mounts" ]]; then
  exec unshare --mount --propagation private bash "$0" "$rootfs" private-mounts
fi
test -d "$rootfs/proc"
test ! -L "$rootfs/proc"
# Dart AOT locates its snapshot and stack mappings through /proc/self.
# The mount disappears with this private mount namespace.
mount -t proc proc "$rootfs/proc"
chroot "$rootfs" /bin/bash /work/tool/system_proxy_kde_smoke.sh \
  /work/build/kde-proxy-consumer/kde_proxy_consumer \
  /work/tool/system_proxy_bridge_smoke
