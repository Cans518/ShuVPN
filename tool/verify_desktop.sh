#!/usr/bin/env bash
set -euo pipefail

run_check() {
  local name="$1"
  shift
  local log="$log_directory/$name.log"
  printf '[%s] %s\n' "$name" "$log"
  if "$@" >"$log" 2>&1; then
    return
  else
    local status=$?
    tail -n 35 "$log"
    printf '%s failed with exit code %s; see %s\n' "$name" "$status" "$log" >&2
    exit "$status"
  fi
}

main() {
  local repo_root
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  cd "$repo_root"
  local run_directory="$repo_root/build/desktop/$(date +%Y%m%d-%H%M%S)-linux"
  local log_directory="$run_directory/logs"
  mkdir -p "$log_directory"
  run_check dart-boundary-tests python3 -m unittest discover -s test/tool -p test_dart_boundary.py
  run_check dart-boundary python3 tool/verify_dart_boundary.py
  run_check packaging-tests python3 -m unittest discover -s test/tool -p 'test_package*.py'
  run_check gui-guard-tests python3 -m unittest discover -s test/tool -p 'test_*desktop*acceptance.py'
  run_check full-probe-tool-tests python3 -m unittest discover -s test/tool -p 'test_*full*.py'
  run_check installation-tests python3 -m unittest discover -s test/tool -p test_linux_install.py
  run_check rust-format cargo fmt --manifest-path rust/Cargo.toml --check
  run_check rust-clippy cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets --all-features -- -D warnings
  if [[ "${SHUVPN_RUST_TEST_NAMESPACE:-0}" == 1 ]]; then
    run_check rust-tests sudo bash tool/rust_linux_namespace_tests.sh \
      "$(id -un)" "$(command -v cargo)" --locked --all-features
  else
    run_check rust-tests cargo test --manifest-path rust/Cargo.toml --locked --all-features
  fi
  run_check full-probe-contract-tests cargo test --manifest-path rust/Cargo.toml --locked --all-features --example desktop_windows_full_smoke
  run_check rust-release cargo build --manifest-path rust/Cargo.toml --locked --release --lib --bin shuvpn_tun_helper --examples
  run_check flutter-dependencies flutter pub get --enforce-lockfile
  run_check flutter-analyze flutter analyze
  run_check flutter-release flutter build linux --release
  run_check native-tray-build cmake --build build/linux/x64/release --target shuvpn_tray_menu_test
  run_check native-tray-tests xvfb-run -a build/linux/x64/release/shuvpn_tray_menu_test
  run_check native-accessibility-build cmake --build build/linux/x64/release --target shuvpn_view_accessibility_test
  run_check native-accessibility-tests xvfb-run -a dbus-run-session build/linux/x64/release/shuvpn_view_accessibility_test
  if [[ "${SHUVPN_DESKTOP_GUI_ACCEPTANCE:-0}" == 1 ]]; then
    run_check desktop-gui python3 tool/desktop_rule_acceptance.py \
      --bundle "$repo_root/build/linux/x64/release/bundle" \
      --report "$run_directory/gui-acceptance" --dart "$(command -v dart)"
    run_check desktop-rule-management python3 tool/desktop_rule_management_acceptance.py \
      --bundle "$repo_root/build/linux/x64/release/bundle" \
      --report "$run_directory/rule-management-acceptance" --dart "$(command -v dart)"
    run_check desktop-rule-drag python3 tool/desktop_rule_drag_acceptance.py \
      --bundle "$repo_root/build/linux/x64/release/bundle" \
      --report "$run_directory/rule-drag-acceptance" --dart "$(command -v dart)"
    run_check desktop-tray python3 tool/desktop_tray_acceptance.py \
      --bundle "$repo_root/build/linux/x64/release/bundle" \
      --report "$run_directory/tray-acceptance" --dart "$(command -v dart)"
  fi
  run_check package python3 tool/package_desktop.py build --platform linux --bundle build/linux/x64/release/bundle --output "$run_directory"
  local archives=("$run_directory"/*.tar.gz)
  if [[ ${#archives[@]} -ne 1 || ! -f "${archives[0]}" ]]; then
    printf 'Expected one desktop archive in %s\n' "$run_directory" >&2
    exit 1
  fi
  local archive="${archives[0]}"
  run_check package-integrity python3 tool/package_desktop.py verify "$archive"
  local extracted_directory="$run_directory/extracted"
  mkdir -p "$extracted_directory"
  tar -xzf "$archive" -C "$extracted_directory"
  local bundle="$extracted_directory/$(basename "$archive" .tar.gz)"
  local library="$bundle/lib/libshuvpn_core.so"
  if [[ "${SHUVPN_LINUX_INSTALL_NAMESPACE:-0}" == 1 ]]; then
    run_check installation-probe dart compile exe tool/linux_install_smoke.dart \
      -o "$run_directory/linux-install-ffi"
    run_check installed-tun-probe dart compile exe tool/tun_dns_bridge_smoke.dart \
      -o "$run_directory/linux-installed-tun-ffi"
    run_check isolated-installation sudo bash tool/linux_install_smoke.sh \
      "$bundle" "$run_directory/linux-install-ffi" "$(id -un)" \
      "$run_directory/linux-installed-tun-ffi" "$repo_root/rust/target/release/examples/proxy_fixture"
  fi
  export FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR="$bundle/lib"
  local binary
  for binary in "$bundle/shuvpn" "$bundle/shuvpn_tun_helper" "$bundle"/lib/*.so; do
    local dependency_name="dependencies-$(basename "$binary")"
    run_check "$dependency_name" ldd "$binary"
    if grep -q 'not found' "$log_directory/$dependency_name.log"; then
      cat "$log_directory/$dependency_name.log" >&2
      exit 1
    fi
  done
  run_check bridge dart run tool/rust_bridge_smoke.dart "$library"
  run_check sso-storage dart run tool/newsso_storage_smoke.dart "$library"
  run_check account-storage dart run tool/account_storage_smoke.dart "$library"
  run_check settings-storage dart run tool/settings_storage_smoke.dart "$library"
  run_check flutter-tests flutter test
  printf 'DESKTOP_LINUX_PACKAGE_AND_FFI_PASSED %s\n' "$archive"
}

main "$@"
