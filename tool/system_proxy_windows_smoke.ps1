param([string]$LibraryPath)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$probe = Join-Path $repoRoot 'rust/target/release/examples/system_proxy_smoke.exe'
if (-not (Test-Path -LiteralPath $probe -PathType Leaf)) { throw 'Build the native examples first.' }
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$directory = Join-Path $tempRoot ('shuvpn-proxy-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $directory | Out-Null
$directory = (Resolve-Path -LiteralPath $directory).Path
$registry = 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Internet Settings'
function Read-ProxyValues {
    $values = Get-ItemProperty -LiteralPath $registry
    return (@([int]$values.ProxyEnable, [string]$values.ProxyServer, [string]$values.ProxyOverride, [string]$values.AutoConfigURL) | ConvertTo-Json -Compress)
}
$original = Read-ProxyValues
$process = $null
$restored = $false
try {
    foreach ($mode in @('stop', 'http-stop', 'cancel')) {
        & $probe $mode $directory
        if ($LASTEXITCODE -ne 0) { throw "System proxy probe failed: $mode" }
        if ((Read-ProxyValues) -ne $original) { throw 'Original Windows proxy values differ.' }
    }
    $output = Join-Path $directory 'crash.log'
    $errors = Join-Path $directory 'crash.err'
    $process = Start-Process -FilePath $probe -ArgumentList @('crash', ('"' + $directory + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput $output -RedirectStandardError $errors
    $ready = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        if ($process.HasExited) { throw 'Crash probe exited before readiness.' }
        if ((Test-Path -LiteralPath $output) -and (Select-String -LiteralPath $output -Pattern 'SYSTEM_PROXY_CRASH_READY' -Quiet)) {
            $ready = $true
            break
        }
        Start-Sleep -Milliseconds 50
    }
    if (-not $ready) { throw 'Crash probe did not become ready.' }
    Stop-Process -Id $process.Id -Force
    $process.WaitForExit()
    $process = $null
    if (-not (Test-Path -LiteralPath (Join-Path $directory 'system-proxy.json'))) {
        throw 'Crash recovery record was lost.'
    }
    & $probe recover $directory
    if ($LASTEXITCODE -ne 0) { throw 'Crash recovery failed.' }
    if ($LibraryPath) {
        Push-Location -LiteralPath $repoRoot
        try {
            & dart run tool/system_proxy_bridge_smoke.dart $LibraryPath
            if ($LASTEXITCODE -ne 0) { throw 'Packaged-library FFI proxy probe failed.' }
        } finally { Pop-Location }
    }
} finally {
    if ($null -ne $process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        $null = $process.WaitForExit(5000)
    }
    & $probe recover $directory
    $restored = $LASTEXITCODE -eq 0 -and (Read-ProxyValues) -eq $original
    if ($restored) {
        $absolute = (Resolve-Path -LiteralPath $directory).Path
        if ([IO.Path]::GetDirectoryName($absolute).TrimEnd('\') -ne $tempRoot.TrimEnd('\') -or
            [IO.Path]::GetFileName($absolute) -notmatch '^shuvpn-proxy-[a-f0-9]{32}$') {
            throw 'Refusing to delete an unexpected smoke-test directory.'
        }
        Remove-Item -LiteralPath $absolute -Recurse -Force
    }
}
if (-not $restored) { throw "Proxy restoration did not finish. Recovery record retained at $directory" }
Write-Output 'WINDOWS_SYSTEM_PROXY_LIFECYCLE_TERMINATION_RECOVERY_AND_ORIGINAL_VALUES_PASSED'
