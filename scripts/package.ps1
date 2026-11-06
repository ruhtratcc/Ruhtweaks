param(
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]*$')][string]$OutputName = 'Ruhtweaks-0.3.3-windows-x64',
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]*$')][string]$SourceOutputName = 'Ruhtweaks-0.3.3-source'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    & "$PSScriptRoot/dev.ps1" build --release --locked
    $destination = Join-Path (Join-Path $repo 'outputs') $OutputName
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath target/release/ruhtweaks.exe -Destination $destination
    Copy-Item -LiteralPath README.md,LICENSE -Destination $destination
    Copy-Item -LiteralPath docs -Destination $destination -Recurse -Force
    Get-ChildItem -LiteralPath $destination -File -Recurse | Where-Object { $_.Name -ne 'SHA256SUMS.txt' } | ForEach-Object {
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $($_.FullName.Substring($destination.Length + 1).Replace('\','/'))"
    } | Set-Content -Encoding utf8 (Join-Path $destination 'SHA256SUMS.txt')
    Compress-Archive -Path "$destination/*" -DestinationPath (Join-Path 'outputs' "$OutputName.zip") -Force
    Compress-Archive -Path Cargo.toml,Cargo.lock,rust-toolchain.toml,build.rs,README.md,LICENSE,.gitignore,src,ui,tests,examples,docs,scripts,.github -DestinationPath (Join-Path 'outputs' "$SourceOutputName.zip") -Force
    Write-Output $destination
} finally { Pop-Location }

