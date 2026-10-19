#!/usr/bin/env bash
set -euo pipefail

main() {
  [[ "$(id -u)" == 0 ]] || { echo "Run the isolated helper probe as root" >&2; exit 1; }
  local repo_root profile probe_dir helper_source
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  profile="${1:-$repo_root/target/release}"
  helper_source="${2:-$profile/shuvpn_tun_helper}"
  [[ -x "$helper_source" && -x "$profile/examples/linux_tun_helper_smoke" ]] ||
    { echo "Build shuvpn_tun_helper and linux_tun_helper_smoke with tun-probe first" >&2; exit 1; }
  command -v setcap >/dev/null || { echo "Install libcap2-bin first" >&2; exit 1; }
  export SHUVPN_PROBE_HOST_NETNS
  SHUVPN_PROBE_HOST_NETNS="$(readlink /proc/self/ns/net)"
  probe_dir="$(mktemp -d /tmp/shuvpn-linux-helper.XXXXXXXX)"
  trap 'rm -rf -- "$probe_dir"' EXIT
  chown 0:65534 "$probe_dir"
  chmod 750 "$probe_dir"
  cp "$helper_source" "$probe_dir/helper"
  cp "$helper_source" "$probe_dir/helper-denied"
  cp "$profile/examples/linux_tun_helper_smoke" "$probe_dir/probe"
  chown 0:65534 "$probe_dir/helper" "$probe_dir/helper-denied" "$probe_dir/probe"
  chmod 750 "$probe_dir/helper" "$probe_dir/helper-denied" "$probe_dir/probe"
  setcap cap_net_admin,cap_net_raw+ep "$probe_dir/helper"

  local mode
  local modes=(normal generated denied collision partial abandoned drop helper-crash)
  # The injected transfer error is compiled out of the shipped helper. Every
  # normal operation and both process-crash cases also run on that exact binary.
  if [[ $# -lt 2 ]]; then modes+=(transfer-failure); fi
  for mode in "${modes[@]}"; do
    unshare --net bash -c '
      set -euo pipefail
      ip link set lo up
      helper="$1/helper"
      if [[ "$2" == denied ]]; then helper="$1/helper-denied"; fi
      if [[ "$2" == collision ]]; then ip link add shuvpnhlp type dummy; fi
      if [[ "$2" == partial ]]; then sysctl -q -w net.ipv6.conf.default.disable_ipv6=1; fi
      setpriv --reuid=65534 --regid=65534 --clear-groups --inh-caps=-all --ambient-caps=-all \
        "$1/probe" "$helper" "$2"
      if [[ "$2" == collision ]]; then
        ip -j link show shuvpnhlp | python3 -c \
          "import json,sys; assert json.load(sys.stdin)[0][\"link_type\"] == \"ether\""
      elif ip link show shuvpnhlp >/dev/null 2>&1; then
        echo "TUN leaked after $2" >&2; exit 1
      fi
    ' probe "$probe_dir" "$mode"
  done

  unshare --net python3 - "$probe_dir" <<'PY'
import os
import ctypes
import re
import select
import subprocess
import sys
import time

directory = sys.argv[1]
# Reap the deliberately orphaned helper ourselves, including on WSL systems
# whose PID 1 does not promptly reap a child terminated by PDEATHSIG.
if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
    raise OSError(ctypes.get_errno(), "cannot become probe child subreaper")
subprocess.run(["/usr/sbin/ip", "link", "set", "lo", "up"], check=True)
child = subprocess.Popen([
    "setpriv", "--reuid=65534", "--regid=65534", "--clear-groups",
    "--inh-caps=-all", "--ambient-caps=-all",
    directory + "/probe", directory + "/helper", "hold",
], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
try:
    deadline = time.monotonic() + 8
    output = b""
    helper_pid = None
    while time.monotonic() < deadline:
        if select.select([child.stdout], [], [], 0.1)[0]:
            chunk = os.read(child.stdout.fileno(), 4096)
            if not chunk:
                raise RuntimeError("helper hold failed: " + child.stderr.read().decode())
            output += chunk
            match = re.search(rb"LINUX_TUN_HELPER_HOLD_READY helper_pid=(\d+)", output)
            if match:
                helper_pid = int(match.group(1))
                break
    if helper_pid is None:
        raise RuntimeError("helper hold startup timeout")
    child.kill()
    child.wait(timeout=3)
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        try:
            os.waitpid(helper_pid, os.WNOHANG)
        except ChildProcessError:
            pass
        interface = subprocess.run(
            ["/usr/sbin/ip", "link", "show", "dev", "shuvpnhlp"], capture_output=True)
        if interface.returncode != 0 and not os.path.exists(f"/proc/{helper_pid}"):
            print("LINUX_TUN_HELPER_PARENT_CRASH_AND_PROCESS_CLEANUP_PASSED")
            break
        time.sleep(0.01)
    else:
        raise RuntimeError("TUN or helper process survived parent crash")
finally:
    if child.poll() is None:
        child.kill()
        child.wait(timeout=3)
PY
  rm -rf -- "$probe_dir"
  trap - EXIT
}

main "$@"
