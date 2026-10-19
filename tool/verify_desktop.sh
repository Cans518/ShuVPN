#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
python3 tool/release.py check
npm run build --prefix desktop
cargo fmt --manifest-path rust/Cargo.toml --check
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --target-dir rust/target --locked --all-targets -- -D warnings
sudo bash tool/rust_linux_namespace_tests.sh "$(id -un)" "$(command -v cargo)" --locked --all-features --all-targets
cargo test --manifest-path desktop/src-tauri/Cargo.toml --target-dir rust/target --locked --all-targets
cargo test --manifest-path third_party/sangfor/Cargo.toml --target-dir rust/target --workspace --locked
python3 -m unittest discover -s test/tool -p 'test_*.py'
cargo build --manifest-path desktop/src-tauri/Cargo.toml --target-dir rust/target --locked --features tauri/custom-protocol --bin shuvpn
if [[ "${SHUVPN_GUI_SMOKE:-0}" == 1 ]]; then
  mkdir -p build/desktop/tauri
  xvfb-run -a -s "-screen 0 1024x900x24" dbus-run-session -- /usr/bin/python3 tool/tauri_linux_smoke.py \
    --executable "$PWD/rust/target/debug/shuvpn" \
    --report "$PWD/build/desktop/tauri/linux-$(date +%Y%m%d-%H%M%S)"
fi
