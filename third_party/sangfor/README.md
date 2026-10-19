# Native aTrust protocol modules

This directory contains only the Rust `sangfor-core` and `sangfor-tls` crates
from upstream revision `91a7efb2533ae87708ce6c221b3dea4d7fdf1098`.
The source repository and original file hashes are recorded in `UPSTREAM.json`.
The original MIT license is preserved in `LICENSE` and distributed in the
application's third-party notices.

The upstream repository also contains Flutter applications and FFI/platform
hosts. Those components are not included or fetched by this application.
Cargo uses local paths to these two protocol crates.

Local changes are limited to workspace wiring, documentation, test names and
fixture paths. Protocol implementation code and reference bytes are preserved.
`sangfor-core/tests/fixtures/native_atrust.json` is a frozen upstream vector
file with SHA-256:

```text
29e8faf248d5e0289925cd6d6d17366b0136aacf5d302cbf972d258d94b71195
```

The tests consume this file directly. They do not invoke an external fixture
generator or require another language SDK:

```bash
cargo test --manifest-path third_party/sangfor/Cargo.toml --workspace --locked --target-dir rust/target
```

When updating the protocol modules, compare against a fixed upstream revision,
review the changes to the original-file hash inventory, preserve the license,
and run the native protocol and application integration suites.
