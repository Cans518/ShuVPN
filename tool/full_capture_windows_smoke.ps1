param(
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$InterfaceAlias,
    [Parameter(Mandatory)][string]$PeerConfig,
    [Parameter(Mandatory)][string]$ReportDirectory,
    [ValidateSet('Plan', 'Helper', 'Elevated')][string]$Mode = 'Plan',
    [ValidatePattern('^[a-f0-9]{32}$')][string]$ObservationToken,
    [string]$ObserverLog
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$allowed = [IO.Path]::GetFullPath((Join-Path $repository 'build/desktop')) + [IO.Path]::DirectorySeparatorChar
$report = [IO.Path]::GetFullPath($ReportDirectory)
if (-not $report.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Full capture evidence must stay inside build/desktop'
}
if (Test-Path -LiteralPath $report) { throw 'Use a new report directory' }
if ($Mode -eq 'Helper' -and -not $ObservationToken) {
    throw 'Helper mode requires a separate privileged observer and ObservationToken'
}
if ($ObservationToken -and -not $ObserverLog) { throw 'ObservationToken requires ObserverLog' }
$probe = (Resolve-Path -LiteralPath $ProbePath).Path
$configuration = (Resolve-Path -LiteralPath $PeerConfig).Path
New-Item -ItemType Directory -Path $report | Out-Null

function Get-FullNetworkState {
    $scope = if ($Mode -eq 'Plan') { 'Current' } else { 'All' }
    & (Join-Path $PSScriptRoot 'full_capture_windows_snapshot.ps1') -Scope $scope
}

function Get-ObserverCompletion {
    $prefix = 'WINDOWS_FULL_OBSERVER_COMPLETE '
    $deadline = [DateTime]::UtcNow.AddSeconds(35)
    do {
        if (Test-Path -LiteralPath $ObserverLog) {
            $records = @(Get-Content -LiteralPath $ObserverLog | Where-Object { $_.StartsWith($prefix) })
            if ($records.Count -gt 1) { throw 'Observer log contains multiple completions' }
            if ($records.Count -eq 1) {
                try { $completion = $records[0].Substring($prefix.Length) | ConvertFrom-Json }
                catch { $completion = $null } # A currently flushing final line may be incomplete.
                if ($completion) { return $completion }
            }
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Independent observer did not complete; observation remains unproven'
}

function Assert-ObserverCompletion($Completion) {
    if ($Completion.token -cne $ObservationToken -or $Completion.mode -cne $execution -or
        $Completion.passed -ne $true -or $Completion.network_restored -ne $true -or
        $Completion.probe_exit_code -ne 0) { throw 'Independent observer did not prove successful completion' }
    $prefix = 'WINDOWS_FULL_EXTERNAL_EVENT '
    $events = @(Get-Content -LiteralPath (Join-Path $report 'probe.log') |
        Where-Object { $_.StartsWith($prefix) } |
        ForEach-Object { $_.Substring($prefix.Length) | ConvertFrom-Json })
    $expectedEvents = if ($Mode -eq 'Plan') { 3 } else { 6 }
    if ($events.Count -ne $expectedEvents) { throw 'Incomplete native observation transcript' }
    for ($index = 0; $index -lt $events.Count; $index++) {
        $event = $events[$index]
        if ($event.request.sequence -ne $index -or $event.reply.sequence -ne $index -or
            $event.request.token -cne $ObservationToken -or $event.reply.token -cne $ObservationToken -or
            $event.request.pid -ne $Completion.probe_identity.pid -or $event.reply.ok -ne $true) {
            throw 'Native observation transcript identity/order mismatch'
        }
    }
    if ($events[0].request.stage -cne 'begin' -or $events[-1].request.stage -cne 'finish' -or
        ($Completion.observer_identity | ConvertTo-Json -Depth 5 -Compress) -cne
        ($events[0].reply.observer_identity | ConvertTo-Json -Depth 5 -Compress)) {
        throw 'Final observer identity differs from the kernel-bound native pipe peer'
    }
    $scope = if ($Mode -eq 'Plan') { 'Current' } else { 'All' }
    if ($Completion.scope -cne $scope) { throw 'Independent observer snapshot scope mismatch' }
    foreach ($key in @('routes','addresses','interfaces','adapters','dns')) {
        if ($null -eq $Completion.before.$key -or $null -eq $Completion.after.$key -or
            @($Completion.before.$key).Count -eq 0 -or @($Completion.after.$key).Count -eq 0) {
            throw 'Independent observer returned an empty network table'
        }
    }
    if ($Mode -eq 'Helper') {
        if ($Completion.observer_identity.elevated -ne $true -or $Completion.probe_identity.elevated -ne $false -or
            @($Completion.helpers).Count -ne 2) { throw 'Helper observation token roles/lifetimes incomplete' }
        for ($round = 0; $round -lt 2; $round++) {
            $witness = $Completion.helpers[$round]
            if ($witness.round -ne $round -or $witness.successful_exit -ne $true -or
                $witness.identity.elevated -ne $true -or -not $witness.identity.created_ticks) {
                throw 'Exact helper process exit evidence incomplete'
            }
        }
    }
}

$before = $null
if (-not $ObservationToken) {
    $before = Get-FullNetworkState
    $before | Set-Content -LiteralPath (Join-Path $report 'network-before.json') -Encoding utf8
}
$execution = switch ($Mode) {
    'Plan' { '--plan' }
    'Helper' { '--run-privileged' }
    'Elevated' { '--run-local-elevated' }
}
$probeExit = -1
$restored = $false
$observerPassed = -not [bool]$ObservationToken
$observerCompletion = $null
$observerError = $null
$previousObservationToken = $env:SHUVPN_WINDOWS_FULL_OBSERVATION_TOKEN
$expected = if ($Mode -eq 'Plan') {
    'WINDOWS_FULL_FORMAL_PREVIEW_AND_NO_TUN_PASSED'
} else {
    'WINDOWS_FULL_TWO_NORMAL_ROUNDS_DATA_AND_SESSION_CLEANUP_PASSED'
}
try {
    if ($ObservationToken) { $env:SHUVPN_WINDOWS_FULL_OBSERVATION_TOKEN = $ObservationToken }
    else { Remove-Item Env:SHUVPN_WINDOWS_FULL_OBSERVATION_TOKEN -ErrorAction SilentlyContinue }
    & $probe $execution $InterfaceAlias $configuration *> (Join-Path $report 'probe.log')
    $probeExit = $LASTEXITCODE
} finally {
    $env:SHUVPN_WINDOWS_FULL_OBSERVATION_TOKEN = $previousObservationToken
    if ($ObservationToken) {
        try {
            $observerCompletion = Get-ObserverCompletion
            Assert-ObserverCompletion $observerCompletion
            $before = $observerCompletion.before | ConvertTo-Json -Depth 12
            $after = $observerCompletion.after | ConvertTo-Json -Depth 12
            $observerPassed = $true
            $before | Set-Content -LiteralPath (Join-Path $report 'network-before.json') -Encoding utf8
        } catch {
            $observerError = $_.Exception.Message
            $after = $null
        }
    } else { $after = Get-FullNetworkState }
    $after | Set-Content -LiteralPath (Join-Path $report 'network-after.json') -Encoding utf8
    $restored = $null -ne $before -and $null -ne $after -and $before -ceq $after
    $completed = (Test-Path -LiteralPath (Join-Path $report 'probe.log')) -and
        (Select-String -LiteralPath (Join-Path $report 'probe.log') -SimpleMatch $expected -Quiet)
    $summary = [ordered]@{
        mode = $Mode
        probe = $probe
        probe_sha256 = (Get-FileHash -LiteralPath $probe -Algorithm SHA256).Hash.ToLowerInvariant()
        peer_config_sha256 = (Get-FileHash -LiteralPath $configuration -Algorithm SHA256).Hash.ToLowerInvariant()
        exit_code = $probeExit
        network_configuration_restored = $restored
        expected_stage_completed = [bool]$completed
        passed = $probeExit -eq 0 -and $restored -and $completed -and $observerPassed
        observer = $observerCompletion
        observer_error = $observerError
        network_scope = if ($Mode -eq 'Plan') { 'current-compartment-preview-only' } else { 'all-compartments' }
        scope = 'formal preview or two normal Full rounds; helper crash and coverage-change matrix are separate'
    }
    $summary | ConvertTo-Json -Depth 16 |
        Set-Content -LiteralPath (Join-Path $report 'verification.json') -Encoding utf8
}
if (-not $observerPassed) { throw "Independent observation failed: $observerError; see $report" }
if (-not $restored) { throw "Network configuration differs; see $report. No blanket restoration was attempted." }
if ($probeExit -ne 0) { throw "Full probe failed with exit $probeExit; see $report" }
if (-not (Select-String -LiteralPath (Join-Path $report 'probe.log') -SimpleMatch $expected -Quiet)) {
    throw 'Full probe did not complete the expected scope'
}
Write-Output "WINDOWS_FULL_${Mode}_AND_NETWORK_RESTORATION_PASSED $report"
