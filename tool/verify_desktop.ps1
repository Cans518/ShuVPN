param([switch]$GuiSmoke)
$ErrorActionPreference = 'Stop'
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$previousDriver = $env:SHUVPN_WINTUN_DLL
try {
    $env:SHUVPN_WINTUN_DLL = (Resolve-Path -LiteralPath desktop/resources/wintun/amd64/wintun.dll).Path
    $checks = @(
        @('python', 'tool/release.py', 'check'),
        @('npm.cmd', 'run', 'build', '--prefix', 'desktop'),
        @('python', '-m', 'unittest', 'discover', '-s', 'test/tool', '-p', 'test_*.py'),
        @('cargo', 'fmt', '--manifest-path', 'rust/Cargo.toml', '--check'),
        @('cargo', 'fmt', '--manifest-path', 'desktop/src-tauri/Cargo.toml', '--check'),
        @('cargo', 'clippy', '--manifest-path', 'rust/Cargo.toml', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings'),
        @('cargo', 'clippy', '--manifest-path', 'desktop/src-tauri/Cargo.toml', '--target-dir', 'rust/target', '--locked', '--all-targets', '--', '-D', 'warnings'),
        @('cargo', 'test', '--manifest-path', 'rust/Cargo.toml', '--locked', '--all-features', '--all-targets'),
        @('cargo', 'test', '--manifest-path', 'desktop/src-tauri/Cargo.toml', '--target-dir', 'rust/target', '--locked', '--all-targets'),
        @('cargo', 'test', '--manifest-path', 'third_party/sangfor/Cargo.toml', '--target-dir', 'rust/target', '--workspace', '--locked'),
        @('cargo', 'build', '--manifest-path', 'desktop/src-tauri/Cargo.toml', '--target-dir', 'rust/target', '--locked', '--features', 'tauri/custom-protocol', '--bin', 'shuvpn')
    )
    foreach ($check in $checks) {
        $executable = $check[0]
        $arguments = $check[1..($check.Count - 1)]
        & $executable @arguments
        if ($LASTEXITCODE) { throw "$executable failed: $LASTEXITCODE" }
    }
    if ($GuiSmoke) {
        $report = 'build/desktop/tauri/windows-' + (Get-Date -Format yyyyMMdd-HHmmss)
        New-Item -ItemType Directory -Path (Split-Path -Parent $report) -Force | Out-Null
        node tool/tauri_windows_smoke.mjs rust/target/debug/shuvpn.exe $report
        if ($LASTEXITCODE) { throw 'Windows WebView smoke failed' }
    }
} finally {
    $env:SHUVPN_WINTUN_DLL = $previousDriver
    Pop-Location
}
