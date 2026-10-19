#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Every privileged operation runs through a harness that creates its own network
# namespace. Cargo and the desktop application continue to run as the caller.
cargo build --locked -p shuvpn_core --release --lib --examples
sudo bash tool/tun_linux_namespace_smoke.sh
sudo bash tool/tun_dns_linux_smoke.sh
sudo unshare --net python3 tool/tun_data_plane_linux_smoke.py
cargo build --locked -p shuvpn_core --release --features tun-probe,egress-probe --bin shuvpn_tun_helper --examples
sudo unshare --net python3 tool/tun_data_plane_linux_smoke.py --egress
sudo bash tool/tun_linux_helper_smoke.sh
sudo unshare --net python3 tool/tun_data_plane_linux_smoke.py --desktop-helper target/release target/release/shuvpn_tun_helper
sudo unshare --net python3 tool/tun_data_plane_linux_smoke.py --desktop-helper-crash target/release target/release/shuvpn_tun_helper
