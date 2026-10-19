param([switch]$Refresh)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'WebView2 Runtime requires Windows' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
Write-Output ("WebView2 host: system={0}, interactive={1}, session={2}" -f
    $identity.IsSystem, [Environment]::UserInteractive, [Diagnostics.Process]::GetCurrentProcess().SessionId)
if ($identity.IsSystem) { throw 'WebView2 cannot run as the SYSTEM account' }

function Get-WebViewRuntimeVersion {
    $client = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $locations = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$client",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$client",
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$client"
    )
    foreach ($location in $locations) {
        $entry = Get-ItemProperty -LiteralPath $location -Name pv -ErrorAction SilentlyContinue
        $parsed = [version]'0.0.0.0'
        if ($entry -and [version]::TryParse([string]$entry.pv, [ref]$parsed) -and $parsed -gt [version]'0.0.0.0') {
            return $parsed.ToString()
        }
    }
}

$runtime = Get-WebViewRuntimeVersion
if ($runtime) { Write-Output "Existing WebView2 Runtime: $runtime" }
if (-not $runtime -or $Refresh) {
    $repository = Split-Path -Parent $PSScriptRoot
    $tools = Join-Path $repository 'build/tools'
    New-Item -ItemType Directory -Force -Path $tools | Out-Null
    $installer = Join-Path $tools 'MicrosoftEdgeWebview2Setup.exe'
    Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $installer
    $signature = Get-AuthenticodeSignature -LiteralPath $installer
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike '*O=Microsoft Corporation*') {
        throw 'Unexpected WebView2 bootstrapper signature'
    }
    $process = Start-Process -FilePath $installer -ArgumentList @('/silent', '/install') -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "WebView2 Runtime installation failed: $($process.ExitCode)" }
    $runtime = Get-WebViewRuntimeVersion
    if (-not $runtime) { throw 'WebView2 Runtime is still unavailable after installation' }
}
Write-Output "WebView2 Runtime: $runtime"
