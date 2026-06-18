# Viscos MSI Build Script (Faz 8.0 stub)
#
# Release engineering sırasında çalıştırılacak. Faz 8.0'da sadece syntax / lint
# amacıyla tutulur; gerçek build Faz 8.x release zamanında.
#
# Gereksinimler (release engineering zamanı kurulacak):
#   - cargo build --release  (Cargo.toml'unda LTO + strip ile 15-25 MB binary)
#   - WiX 3 toolkit          (https://wixtoolset.org/releases/)
#   - signtool.exe           (Windows SDK ile gelir)
#
# Build adımları (release engineering checklist):
#   1. cargo build --release --bin viscos
#   2. signtool sign /fd sha256 /tr http://timestamp.digicert.com target/release/viscos.exe
#   3. candle.exe viscos.wxs
#   4. light.exe viscos.wixobj -out viscos-0.1.0-x86_64.msi
#
# Kullanım:
#   pwsh -File installer/build-installer.ps1
#
# Bu script Faz 8.0 stub'ı olarak davranır: gerçek build çağrıları yorum
# satırı olarak tutulur. Release engineering öncesi TODO'lar uncomment edilir.

[CmdletBinding()]
param(
    [switch]$SkipBuild = $false,
    [switch]$SkipSign = $false,
    [string]$Configuration = 'release'
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$CargoTargetDir = Join-Path $RepoRoot 'target' $Configuration
$OutputMsi = Join-Path $CargoTargetDir 'viscos-0.1.0-x86_64.msi'
$WxsFile = Join-Path $PSScriptRoot 'viscos.wxs'

Write-Host '== Viscos MSI build stub =='
Write-Host "Repo root:       $RepoRoot"
Write-Host "Cargo target:    $CargoTargetDir"
Write-Host "WiX source:      $WxsFile"
Write-Host "Output MSI:      $OutputMsi"
Write-Host ''

# Step 1 — cargo build
if (-not $SkipBuild) {
    Write-Host '[1/4] cargo build --release (stubbed — uncomment for release engineering)'
    # cargo build --release --bin viscos --manifest-path (Join-Path $RepoRoot 'Cargo.toml')
} else {
    Write-Host '[1/4] Skipping cargo build (--SkipBuild)'
}

# Step 2 — code sign (release engineering)
if (-not $SkipSign) {
    Write-Host '[2/4] signtool sign (stubbed — uncomment for release engineering)'
    # $certPath = $env:VISCOS_CERT_PATH
    # $certPassword = $env:VISCOS_CERT_PASSWORD
    # if (-not $certPath -or -not $certPassword) {
    #     throw 'VISCOS_CERT_PATH / VISCOS_CERT_PASSWORD environment variables must be set.'
    # }
    # $exe = Join-Path $CargoTargetDir 'viscos.exe'
    # & 'C:\Program Files (x86)\Windows Kits\10\bin\x64\signtool.exe' sign `
    #     /tr http://timestamp.digicert.com `
    #     /td sha256 `
    #     /fd sha256 `
    #     /f $certPath `
    #     /p $certPassword `
    #     $exe
} else {
    Write-Host '[2/4] Skipping code sign (--SkipSign)'
}

# Step 3 — candle (WiX compile)
Write-Host '[3/4] candle viscos.wxs (stubbed — uncomment for release engineering)'
# & 'C:\Program Files (x86)\WiX Toolset v3.14\bin\candle.exe' `
#     -dCargoTargetDir="$CargoTargetDir" `
#     -o (Join-Path $PSScriptRoot 'viscos.wixobj') `
#     $WxsFile

# Step 4 — light (WiX link)
Write-Host '[4/4] light viscos.wixobj (stubbed — uncomment for release engineering)'
# & 'C:\Program Files (x86)\WiX Toolset v3.14\bin\light.exe' `
#     -out $OutputMsi `
#     (Join-Path $PSScriptRoot 'viscos.wixobj')

Write-Host ''
Write-Host '== Stub complete =='
Write-Host 'Bu script release engineering sırasında yorum satırları kaldırılarak çalıştırılacak.'
Write-Host 'Faz 8.0 sürümünde sadece lint + sanity check amaçlıdır.'