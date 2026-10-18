#!/usr/bin/env bash
set -euo pipefail

main() {
  [[ "$(id -u)" == 0 ]] || { echo "Run the Rust namespace harness as root" >&2; exit 1; }
  [[ $# -ge 2 ]] || { echo "Usage: $0 <ordinary-user> <cargo-path> [cargo-test-options...]" >&2; exit 1; }
  local account="$1" cargo_program="$2" repo_root baseline
  [[ "$(id -u -- "$account")" != 0 ]] || { echo "Cargo tests must use an ordinary UID" >&2; exit 1; }
  [[ "$cargo_program" == /* && -x "$cargo_program" ]] ||
    { echo "Pass the absolute path to the user's cargo executable" >&2; exit 1; }
  shift 2
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  baseline="$(readlink /proc/self/ns/net)"
  unshare --net bash -c '
    set -euo pipefail
    account="$1"; cargo_program="$2"; repo_root="$3"; baseline="$4"
    shift 4
    [[ "$(readlink /proc/self/ns/net)" != "$baseline" ]] ||
      { echo "Refusing tests in the parent network namespace" >&2; exit 1; }
    ip link set lo up
    cd "$repo_root"
    exec runuser -u "$account" -- /usr/bin/env \
      "PATH=$(dirname "$cargo_program"):$PATH" \
      "$cargo_program" test --manifest-path rust/Cargo.toml "$@"
  ' rust-tests "$account" "$cargo_program" "$repo_root" "$baseline" "$@"
}

main "$@"
