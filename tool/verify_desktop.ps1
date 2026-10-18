param(
    [string]$FlutterPath = 'flutter',
    [string]$PythonPath = 'python'
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$runDirectory = Join-Path $repoRoot ('build/desktop/' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-windows')
$logDirectory = Join-Path $runDirectory 'logs'
New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null
$flutterCommand = (Get-Command $FlutterPath -ErrorAction Stop).Source
$dartCommand = Join-Path (Split-Path -Parent $flutterCommand) 'dart.bat'
$previousLibrary = $env:FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR
$previousWintun = $env:SHUVPN_WINTUN_DLL
$previousHelper = $env:SHUVPN_TEST_TUN_HELPER

function Invoke-ShuDesktopCheck {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Executable,
        [string[]]$Arguments = @()
    )
    $log = Join-Path $logDirectory ($Name + '.log')
    Write-Host "[$Name] $log"
    & $Executable @Arguments *> $log
    if ($LASTEXITCODE -ne 0) {
        Get-Content -LiteralPath $log -Tail 35
        throw "$Name failed with exit code $LASTEXITCODE; see $log"
    }
}

Push-Location -LiteralPath $repoRoot
try {
    $env:SHUVPN_WINTUN_DLL = Join-Path $repoRoot 'windows/third_party/wintun/amd64/wintun.dll'
    Invoke-ShuDesktopCheck 'dart-boundary-tests' $PythonPath @('-m', 'unittest', 'discover', '-s', 'test/tool', '-p', 'test_dart_boundary.py')
    Invoke-ShuDesktopCheck 'dart-boundary' $PythonPath @('tool/verify_dart_boundary.py')
    Invoke-ShuDesktopCheck 'packaging-tests' $PythonPath @('-m', 'unittest', 'discover', '-s', 'test/tool', '-p', 'test_package*.py')
    Invoke-ShuDesktopCheck 'gui-guard-tests' $PythonPath @('-m', 'unittest', 'discover', '-s', 'test/tool', '-p', 'test_*desktop*acceptance.py')
    Invoke-ShuDesktopCheck 'full-probe-tool-tests' $PythonPath @('-m', 'unittest', 'discover', '-s', 'test/tool', '-p', 'test_*full*.py')
    Invoke-ShuDesktopCheck 'rust-format' 'cargo' @('fmt', '--manifest-path', 'rust/Cargo.toml', '--check')
    Invoke-ShuDesktopCheck 'rust-clippy' 'cargo' @('clippy', '--manifest-path', 'rust/Cargo.toml', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings')
    Invoke-ShuDesktopCheck 'rust-tests' 'cargo' @('test', '--manifest-path', 'rust/Cargo.toml', '--locked', '--all-features')
    Invoke-ShuDesktopCheck 'full-probe-contract-tests' 'cargo' @('test', '--manifest-path', 'rust/Cargo.toml', '--locked', '--all-features', '--example', 'desktop_windows_full_smoke')
    Invoke-ShuDesktopCheck 'rust-release' 'cargo' @('build', '--manifest-path', 'rust/Cargo.toml', '--locked', '--release', '--lib', '--examples')
    Invoke-ShuDesktopCheck 'flutter-dependencies' $flutterCommand @('pub', 'get', '--enforce-lockfile')
    Invoke-ShuDesktopCheck 'flutter-analyze' $flutterCommand @('analyze')
    Invoke-ShuDesktopCheck 'flutter-release' $flutterCommand @('build', 'windows', '--release')
    Invoke-ShuDesktopCheck 'native-tray-build' 'cmake' @('--build', 'build/windows/x64', '--config', 'Release', '--target', 'shuvpn_tray_menu_test')
    Invoke-ShuDesktopCheck 'native-tray' (Join-Path $repoRoot 'build/windows/x64/Release/shuvpn_tray_menu_test.exe')
    Invoke-ShuDesktopCheck 'package' $PythonPath @('tool/package_desktop.py', 'build', '--platform', 'windows', '--bundle', 'build/windows/x64/runner/Release', '--output', $runDirectory)
    $archives = @(Get-ChildItem -LiteralPath $runDirectory -Filter '*.zip')
    if ($archives.Count -ne 1) { throw "Expected one desktop archive in $runDirectory" }
    $archive = $archives[0]
    Invoke-ShuDesktopCheck 'package-integrity' $PythonPath @('tool/package_desktop.py', 'verify', $archive.FullName)
    $extractedDirectory = Join-Path $runDirectory 'extracted'
    Expand-Archive -LiteralPath $archive.FullName -DestinationPath $extractedDirectory
    $bundle = Join-Path $extractedDirectory $archive.BaseName
    $library = Join-Path $bundle 'shuvpn_core.dll'
    $env:FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR = $bundle
    Remove-Item Env:SHUVPN_WINTUN_DLL -ErrorAction SilentlyContinue
    Invoke-ShuDesktopCheck 'bridge' $dartCommand @('run', 'tool/rust_bridge_smoke.dart', $library)
    Invoke-ShuDesktopCheck 'sso-storage' $dartCommand @('run', 'tool/newsso_storage_smoke.dart', $library)
    Invoke-ShuDesktopCheck 'account-storage' $dartCommand @('run', 'tool/account_storage_smoke.dart', $library)
    Invoke-ShuDesktopCheck 'settings-storage' $dartCommand @('run', 'tool/settings_storage_smoke.dart', $library)
    $env:SHUVPN_TEST_TUN_HELPER = Join-Path $bundle 'shuvpn_tun_helper.exe'
    Invoke-ShuDesktopCheck 'packaged-helper' 'cargo' @('test', '--manifest-path', 'rust/Cargo.toml', '--locked', '--all-features', '--test', 'windows_tun_helper')
    Invoke-ShuDesktopCheck 'flutter-tests' $flutterCommand @('test')
    Write-Host "DESKTOP_WINDOWS_PACKAGE_AND_FFI_PASSED $($archive.FullName)"
} finally {
    $env:FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR = $previousLibrary
    $env:SHUVPN_WINTUN_DLL = $previousWintun
    $env:SHUVPN_TEST_TUN_HELPER = $previousHelper
    Pop-Location
}
