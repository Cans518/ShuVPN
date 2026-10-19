#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
python3 tool/release.py check
npm run build --prefix desktop
npm test --prefix desktop
cargo fmt --check
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
sudo bash tool/rust_linux_namespace_tests.sh "$(id -un)" "$(command -v cargo)" --locked --all-features --all-targets
cargo test --locked -p shuvpn_desktop --all-targets
cargo test --manifest-path third_party/sangfor/Cargo.toml --target-dir target --workspace --locked
python3 -m unittest discover -s tests/tool -p 'test_*.py'
cargo build --locked -p shuvpn_desktop --features tauri/custom-protocol --bin shuvpn
cargo build --locked -p shuvpn_core --bin shuvpn_tun_helper
if [[ "${SHUVPN_NETWORK_SMOKE:-0}" == 1 ]]; then
  bash tool/verify_network_linux.sh
fi
if [[ "${SHUVPN_GUI_SMOKE:-0}" == 1 ]]; then
  mkdir -p build/desktop/tauri
  xvfb-run -a -s "-screen 0 1024x900x24" dbus-run-session -- /usr/bin/python3 tool/tauri_linux_smoke.py \
    --executable "$PWD/target/debug/shuvpn" \
    --report "$PWD/build/desktop/tauri/linux-$(date +%Y%m%d-%H%M%S)"
fi
