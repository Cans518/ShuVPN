param([string]$Destination)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$buildRoot = [IO.Path]::GetFullPath((Join-Path $repository 'build')) + [IO.Path]::DirectorySeparatorChar
if (-not $Destination) { $Destination = Join-Path $buildRoot 'tools/inno-7.1.0' }
$Destination = [IO.Path]::GetFullPath($Destination)
if (-not $Destination.StartsWith($buildRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The portable compiler must be installed inside this repository build directory'
}
$compiler = Join-Path $Destination 'ISCC.exe'
$compilerHash = 'd06ebd38f38e3cee60a3c50cc45bd449d77e0bc6a5cabc607ea9886808e4de1a'
if (-not (Test-Path -LiteralPath $compiler)) {
    $parent = Split-Path -Parent $Destination
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    $download = Join-Path $parent 'innosetup-7.1.0-x64.exe'
    Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_0/innosetup-7.1.0-x64.exe' -OutFile $download
    $digest = (Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($digest -ne '0362a383ed217d4c4239b5933866dd96d3eb2102737da92f80f6057a4b40df2f') {
        throw 'Unexpected Inno Setup installer digest'
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $download
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike '*Pyrsys B.V.*') {
        throw 'Unexpected Inno Setup installer signature'
    }
    $process = Start-Process -FilePath $download -ArgumentList @(
        '/PORTABLE=1', '/CURRENTUSER', '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART',
        ('/DIR="{0}"' -f $Destination), ('/LOG="{0}"' -f (Join-Path $parent 'inno-extraction.log'))
    ) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Portable compiler extraction failed: $($process.ExitCode)" }
}
if ((Get-FileHash -LiteralPath $compiler -Algorithm SHA256).Hash.ToLowerInvariant() -ne $compilerHash) {
    throw 'Unexpected compiler digest'
}
Write-Output $compiler
