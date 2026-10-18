$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repoRoot
try {
    $generatorVersion = & flutter_rust_bridge_codegen --version
    if ($LASTEXITCODE -ne 0 -or $generatorVersion -ne 'flutter_rust_bridge_codegen 2.11.1') {
        throw 'Install the pinned generator: cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked'
    }
    & flutter_rust_bridge_codegen generate --config-file flutter_rust_bridge.yaml
    if ($LASTEXITCODE -ne 0) {
        throw 'Rust bridge generation failed.'
    }
} finally {
    Pop-Location
}
