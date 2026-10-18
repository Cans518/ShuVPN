param(
    [Parameter(Mandatory)][string]$InstallerPath,
    [Parameter(Mandatory)][string]$BundlePath,
    [Parameter(Mandatory)][string]$IsccPath,
    [Parameter(Mandatory)][string]$ReportDirectory,
    [string]$DartPath = 'dart'
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$allowed = [IO.Path]::GetFullPath((Join-Path $repository 'build/desktop')) + [IO.Path]::DirectorySeparatorChar
$report = [IO.Path]::GetFullPath($ReportDirectory)
if (-not $report.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Installer smoke reports and installation must stay inside build/desktop'
}
function Assert-NoReparseAncestors([string]$Path) {
    while ($Path) {
        if (Test-Path -LiteralPath $Path) {
            if ((Get-Item -LiteralPath $Path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Refusing an installation test reparse point: $Path"
            }
        }
        $Path = Split-Path -Parent $Path
    }
}
Assert-NoReparseAncestors $report
if (Test-Path -LiteralPath $report) { throw 'Use a new smoke report directory' }
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$bundle = (Resolve-Path -LiteralPath $BundlePath).Path
$compiler = (Resolve-Path -LiteralPath $IsccPath).Path
$registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\work.shuvpn.app_is1'
$registration32 = 'HKCU:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\work.shuvpn.app_is1'
if ((Test-Path $registration) -or (Test-Path $registration32)) {
    throw 'An existing ShuVPN installation must not be replaced by this smoke test'
}
foreach ($name in @('ShuVPN.Desktop.Running','Global\ShuVPN.Desktop.Running')) {
    try {
        $existing = [Threading.Mutex]::OpenExisting($name)
        $existing.Dispose()
        throw 'Close ShuVPN before the installer smoke test'
    } catch [Threading.WaitHandleCannotBeOpenedException] {}
}
New-Item -ItemType Directory -Path $report | Out-Null
$target = Join-Path $report 'installed'
$manifest = Get-Content -LiteralPath (Join-Path $bundle 'package.json') -Raw | ConvertFrom-Json
$sentinel = Join-Path $target 'user-notes/sentinel.txt'

function Assert-TestPaths {
    if (-not $target.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Installer test path left the intended build directory'
    }
    Assert-NoReparseAncestors $target
}

function Run-Setup([string]$Executable, [string]$Log, [switch]$Uninstall) {
    Assert-TestPaths
    foreach ($key in @($registration, $registration32)) {
        if (Test-Path $key) {
            $location = [IO.Path]::GetFullPath((Get-ItemProperty $key).InstallLocation).TrimEnd('\')
            if ($location -ne $target.TrimEnd('\')) { throw 'Another installation owns the application registration' }
        }
    }
    $arguments = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/AUTOMATION=1',('/LOG="{0}"' -f $Log))
    if (-not $Uninstall) { $arguments += @('/NOICONS',('/DIR="{0}"' -f $target)) }
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    return $process.ExitCode
}

function Verify-Payload {
    foreach ($entry in $manifest.files.PSObject.Properties) {
        $path = [IO.Path]::GetFullPath((Join-Path $target $entry.Name))
        if (-not $path.StartsWith($target + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Manifest path escaped the installation directory'
        }
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.Value.sha256) {
            throw "Installed file mismatch: $($entry.Name)"
        }
    }
    if ((Get-Content -LiteralPath (Join-Path $target '.package-manager') -Raw).Trim() -ne 'inno') {
        throw 'Missing installed distribution marker'
    }
}

$marker = $null
$smokeLock = $null
$testInstallationMarker = $null
$ownSmoke = $false
try {
    $created = $false
    $smokeLock = [Threading.Mutex]::new($false, 'Global\ShuVPN.Desktop.InstallerSmoke', [ref]$created)
    if (-not $created) { throw 'Another installer smoke test is running' }
    $ownSmoke = $true
    if ((Test-Path $registration) -or (Test-Path $registration32)) { throw 'Application registration changed before smoke ownership was acquired' }
    $testInstallationMarker = [Threading.Mutex]::new($false, 'Global\ShuVPN.Desktop.Installing')
    $marker = [Threading.Mutex]::new($false, 'ShuVPN.Desktop.Running')
    if ((Run-Setup $installer (Join-Path $report 'running-install.log')) -eq 0) {
        throw 'Installer ignored the running application marker'
    }
    if (Test-Path -LiteralPath (Join-Path $target 'shuvpn.exe')) {
        throw 'Rejected installation wrote application files'
    }
    $marker.Dispose()
    $marker = $null
    $previous = Join-Path $report 'previous'
    New-Item -ItemType Directory -Path $previous | Out-Null
    & $compiler --quiet-progress "--define=BundleDir=$bundle" '--define=AppVersion=0.0.0+0' "--define=OutputDir=$previous" (Join-Path $repository 'windows/installer/shuvpn.iss') > (Join-Path $report 'previous-build.log') 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Failed to build the synthetic prior installer' }
    $previousInstaller = Join-Path $previous 'ShuVPN-0.0.0+0-windows-x64-setup.exe'
    if ((Run-Setup $previousInstaller (Join-Path $report 'initial-install.log')) -ne 0) { throw 'Initial installation failed' }
    Verify-Payload
    New-Item -ItemType Directory -Path (Split-Path -Parent $sentinel) | Out-Null
    Set-Content -LiteralPath $sentinel -Value 'retained user notes'
    $sentinelHash = (Get-FileHash -LiteralPath $sentinel).Hash
    $accountData = Join-Path $report 'account-data'
    & $DartPath run (Join-Path $repository 'tool/account_storage_smoke.dart') (Join-Path $target 'shuvpn_core.dll') $accountData import > (Join-Path $report 'account-before-upgrade.log') 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Failed to create the native account preservation fixture' }
    $accountHashes = @(Get-ChildItem -LiteralPath $accountData -File -Force -Recurse | Sort-Object FullName | Get-FileHash | Select-Object -ExpandProperty Hash) -join ','
    if ((Run-Setup $installer (Join-Path $report 'upgrade.log')) -ne 0) { throw 'Upgrade failed' }
    Verify-Payload
    if ((Get-ItemProperty $registration).DisplayVersion -ne $manifest.version) { throw 'Upgrade version not registered' }
    if ((Get-FileHash -LiteralPath $sentinel).Hash -ne $sentinelHash) { throw 'Upgrade changed unowned files' }

    # Exercise the actual runner's pre-Flutter guard, without opening account data.
    $marker = [Threading.Mutex]::new($false, 'ShuVPN.Desktop.Installing')
    $application = Start-Process -FilePath (Join-Path $target 'shuvpn.exe') -WorkingDirectory $target -WindowStyle Hidden -PassThru
    if (-not $application.WaitForExit(10000)) {
        Stop-Process -Id $application.Id -Force
        throw 'Runner did not honor the installation marker; guard verification failed'
    }
    if ($application.ExitCode -ne 1618) { throw "Runner did not report installation-busy status: $($application.ExitCode)" }
    $marker.Dispose()
    $marker = $null
    foreach ($probe in @('rust_bridge','newsso_storage','account_storage','settings_storage')) {
        & $DartPath run (Join-Path $repository "tool/${probe}_smoke.dart") (Join-Path $target 'shuvpn_core.dll') > (Join-Path $report "$probe.log") 2>&1
        if ($LASTEXITCODE -ne 0) { throw "Installed payload probe failed: $probe" }
    }
    $uninstaller = Join-Path $target 'unins000.exe'
    $marker = [Threading.Mutex]::new($false, 'Global\ShuVPN.Desktop.Transaction')
    if ((Run-Setup $uninstaller (Join-Path $report 'concurrent-uninstall.log') -Uninstall) -eq 0) { throw 'Uninstaller ignored another installation transaction' }
    Verify-Payload
    $marker.Dispose()
    $marker = $null
    $marker = [Threading.Mutex]::new($false, 'ShuVPN.Desktop.Running')
    if ((Run-Setup $uninstaller (Join-Path $report 'running-uninstall.log') -Uninstall) -eq 0) { throw 'Uninstaller ignored the running marker' }
    Verify-Payload
    $marker.Dispose()
    $marker = $null
    if ((Run-Setup $uninstaller (Join-Path $report 'uninstall.log') -Uninstall) -ne 0) { throw 'Uninstall failed' }
    if ((Test-Path -LiteralPath (Join-Path $target 'shuvpn.exe')) -or (Test-Path $registration)) { throw 'Uninstall left owned installation state' }
    if ((Get-FileHash -LiteralPath $sentinel).Hash -ne $sentinelHash) { throw 'Uninstall removed unowned files' }
    $after = @(Get-ChildItem -LiteralPath $accountData -File -Force -Recurse | Sort-Object FullName | Get-FileHash | Select-Object -ExpandProperty Hash) -join ','
    if ($after -ne $accountHashes) { throw 'Upgrade or uninstall changed native account data' }
    & $DartPath run (Join-Path $repository 'tool/account_storage_smoke.dart') (Join-Path $bundle 'shuvpn_core.dll') $accountData restore > (Join-Path $report 'account-after-uninstall.log') 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Native account data could not be restored after uninstall' }
    Write-Output 'WINDOWS_INSTALLER_NATIVE_ACCOUNT_PRESERVATION_PASSED'
    Write-Output 'WINDOWS_INSTALL_UPGRADE_UNINSTALL_GUARDS_AND_DATA_PRESERVATION_PASSED'
} finally {
    if ($null -ne $marker) { $marker.Dispose() }
    try {
        $uninstaller = Join-Path $target 'unins000.exe'
        if ($ownSmoke -and (Test-Path $registration) -and (Test-Path -LiteralPath $uninstaller)) {
            $location = [IO.Path]::GetFullPath((Get-ItemProperty $registration).InstallLocation).TrimEnd('\')
            if ($location -eq $target.TrimEnd('\')) {
                $status = Run-Setup $uninstaller (Join-Path $report 'cleanup-uninstall.log') -Uninstall
                if ($status -ne 0) { Write-Warning "Smoke installation cleanup failed: $status; evidence retained at $report" }
            } else {
                Write-Warning 'Installation registration changed; cleanup was not attempted'
            }
        }
    } finally {
        if ($null -ne $testInstallationMarker) { $testInstallationMarker.Dispose() }
        if ($null -ne $smokeLock) { $smokeLock.Dispose() }
    }
}
