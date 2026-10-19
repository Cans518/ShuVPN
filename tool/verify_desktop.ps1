param([switch]$GuiSmoke)
$ErrorActionPreference = 'Stop'
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
$previousDriver = $env:SHUVPN_WINTUN_DLL
try {
    $env:SHUVPN_WINTUN_DLL = (Resolve-Path -LiteralPath desktop/resources/wintun/amd64/wintun.dll).Path
    $checks = @(
        @('python', 'tool/release.py', 'check'),
        @('npm.cmd', 'run', 'build', '--prefix', 'desktop'),
        @('npm.cmd', 'test', '--prefix', 'desktop'),
        @('python', '-m', 'unittest', 'discover', '-s', 'tests/tool', '-p', 'test_*.py'),
        @('cargo', 'fmt', '--check'),
        @('cargo', 'clippy', '--workspace', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings'),
        @('cargo', 'test', '--workspace', '--locked', '--all-features', '--all-targets'),
        @('cargo', 'test', '--manifest-path', 'third_party/sangfor/Cargo.toml', '--target-dir', 'target', '--workspace', '--locked'),
        @('cargo', 'build', '--locked', '-p', 'shuvpn_desktop', '--features', 'tauri/custom-protocol', '--bin', 'shuvpn'),
        @('cargo', 'build', '--locked', '-p', 'shuvpn_core', '--bin', 'shuvpn_tun_helper')
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
        & (Join-Path $PSScriptRoot 'windows_gui_user.ps1') -Application target/debug/shuvpn.exe -Report $report
    }
} finally {
    $env:SHUVPN_WINTUN_DLL = $previousDriver
    Pop-Location
}
