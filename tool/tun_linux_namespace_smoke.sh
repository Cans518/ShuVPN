#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
probe_exe="$(pwd)/target/release/examples/tun_namespace_smoke"
[[ -x "$probe_exe" ]] || { echo "Build tun_namespace_smoke first"; exit 1; }
[[ "$(id -u)" == 0 ]] || { echo "Run this isolated probe as root"; exit 1; }
probe_dir="$(mktemp -d /tmp/shuvpn-tun-probe.XXXXXXXX)"
cp "$probe_exe" "$probe_dir/tun_probe"
chmod 755 "$probe_dir" "$probe_dir/tun_probe"
probe_exe="$probe_dir/tun_probe"

unshare -n bash -c '
  set -e
  ip link set lo up
  ip link add baseline type dummy
  ip link set baseline up
  ip address add 192.0.2.1/24 dev baseline
  ip route add default via 192.0.2.254 dev baseline
  "$1"
' probe "$probe_exe"

unshare -n bash -c '
  set -e
  ip link add shuvpntest type dummy
  ip link set shuvpntest up
  "$1" collision
' probe "$probe_exe"

unshare -n bash -c '
  set -e
  ip link add baseline type dummy
  ip link set baseline up
  ip address add 192.0.2.1/24 dev baseline
  ip route add 10.21.0.0/16 dev baseline
  "$1" conflict
  if ip link show shuvpntest >/dev/null 2>&1; then exit 1; fi
' probe "$probe_exe"

unshare -n bash -c '
  set -e
  ip link set lo up
  setpriv --reuid=65534 --regid=65534 --clear-groups --bounding-set=-all "$1" denied
' probe "$probe_exe"

unshare -n bash -c '
  set -e
  sysctl -q -w net.ipv6.conf.default.disable_ipv6=1
  "$1" partial
  if ip link show shuvpntest >/dev/null 2>&1; then exit 1; fi
' probe "$probe_exe"

unshare -n python3 - "$probe_exe" <<'PY'
import select, subprocess, sys
child = subprocess.Popen([sys.argv[1], "hold"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    if not select.select([child.stdout], [], [], 5)[0]:
        raise RuntimeError("TUN crash probe startup timed out")
    ready = child.stdout.readline().strip()
    if ready != "TUN_HOLD_READY":
        raise RuntimeError("TUN crash probe did not become ready: " + child.stderr.read())
    child.kill()
    child.wait(timeout=5)
    result = subprocess.run(["/usr/sbin/ip", "link", "show", "dev", "shuvpntest"], capture_output=True)
    if result.returncode == 0:
        raise RuntimeError("TUN interface survived SIGKILL")
    print("TUN_CRASH_CLEANUP_PASSED")
finally:
    if child.poll() is None:
        child.kill()
        child.wait(timeout=5)
PY
