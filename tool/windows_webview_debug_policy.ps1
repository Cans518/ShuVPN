param(
    [Parameter(Mandatory)][ValidateSet('Set', 'Restore')][string]$Action,
    [Parameter(Mandatory)][string]$StatePath,
    [ValidateRange(1, 65535)][int]$DebugPort = 1
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Registry debugging is limited to isolated GitHub Actions tests' }
$policyPath = 'HKCU:\SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments'
$application = 'shuvpn.exe'

function Restore-DebugPolicy {
    $state = [IO.File]::ReadAllText($StatePath, [Text.Encoding]::UTF8) | ConvertFrom-Json
    $key = Get-Item -LiteralPath $policyPath -ErrorAction SilentlyContinue
    $current = if ($key) { $key.GetValue($application, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } else { $null }
    if ($current -eq $state.TestValue) {
        if ($state.Existed) {
            New-ItemProperty -LiteralPath $policyPath -Name $application -PropertyType String -Value $state.OriginalValue -Force | Out-Null
        } else {
            Remove-ItemProperty -LiteralPath $policyPath -Name $application
        }
    } elseif ($current -ne $state.OriginalValue) {
        throw 'WebView2 test arguments changed concurrently; preserving the current value'
    }
    $key = Get-Item -LiteralPath $policyPath -ErrorAction SilentlyContinue
    if ($state.CreatedKey -and $key -and $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0) {
        Remove-Item -LiteralPath $policyPath
    }
    Remove-Item -LiteralPath $StatePath
}

if ($Action -eq 'Set') {
    if (Test-Path -LiteralPath $StatePath) { throw 'Existing WebView2 registry recovery state must not be overwritten' }
    $machine = Get-Item -LiteralPath 'HKLM:\SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments' -ErrorAction SilentlyContinue
    if ($machine -and (($machine.GetValueNames() -contains $application) -or ($machine.GetValueNames() -contains '*'))) {
        throw 'An existing machine browser-argument policy takes precedence over the test configuration'
    }
    $key = Get-Item -LiteralPath $policyPath -ErrorAction SilentlyContinue
    $existed = $key -and ($key.GetValueNames() -contains $application)
    if ($existed -and $key.GetValueKind($application) -ne [Microsoft.Win32.RegistryValueKind]::String) {
        throw 'The existing application browser-argument policy is not a string'
    }
    $original = if ($existed) { [string]$key.GetValue($application) } else { $null }
    $testValue = ("$original --remote-debugging-address=127.0.0.1 --remote-debugging-port=$DebugPort").Trim()
    $state = [PSCustomObject]@{Existed=[bool]$existed;OriginalValue=$original;TestValue=$testValue;CreatedKey=($null -eq $key)}
    [IO.File]::WriteAllText($StatePath, ($state | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    try {
        if (-not $key) { New-Item -Path $policyPath -Force | Out-Null }
        New-ItemProperty -LiteralPath $policyPath -Name $application -PropertyType String -Value $testValue -Force | Out-Null
    } catch {
        Restore-DebugPolicy
        throw
    }
} else {
    Restore-DebugPolicy
}
[PSCustomObject]@{Action=$Action;Application=$application;Scope='HKCU'} | ConvertTo-Json -Compress
