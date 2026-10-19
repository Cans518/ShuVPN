#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(id -u)" == 0 ]] || { echo "Root namespace privileges required"; exit 1; }
probe_exe="$(pwd)/target/release/examples/dns_capture_namespace_smoke"
[[ -x "$probe_exe" ]] || { echo "Build dns_capture_namespace_smoke first"; exit 1; }
unshare --net bash -c '
  set -e
  ip link set lo up
  "$1"
' probe "$probe_exe"
