# Viscos MSI Build Script (Faz 1.6 + Faz 8.0)
#
# Release engineering sırasında çalıştırılacak. Faz 1.6 Dalga 1b ile BACKEND
# param eklendi: WebView2 (Win10 default, ~25 MB MSI) veya Cef (Win11 default,
# ~250 MB MSI) için ayrı MSI üretir. Faz 8.0 sonrası signtool entegrasyonu.
#
# Gereksinimler (release engineering zamanı kurulacak):
#   - cargo build --release  (Cargo.toml'unda LTO + strip ile 15-25 MB binary)
#   - WiX 3 toolkit          (https://wixtoolset.org/releases/)
#   - signtool.exe           (Windows SDK ile gelir)
#
# Build adımları (release engineering checklist):
#   1. cargo build --release [--features viscos-webview/cef-backend]
#   2. signtool sign /fd sha256 /tr http://timestamp.digicert.com target/release/viscos.exe
#   3. candle.exe viscos.wxs -dBACKEND=WebView2|Cef
#   4. light.exe viscos.wixobj -out viscos-<backend>-0.2.0-x86_64.msi
#
# Kullanım:
#   pwsh -File installer/build-installer.ps1 -Backend WebView2
#   pwsh -File installer/build-installer.ps1 -Backend Cef
#   pwsh -File installer/build-installer.ps1 -Backend WebView2 -SkipBuild -Sign -CertPath .\viscos.pfx
#
# Bu script Faz 8.0 release engineering skeleton'ı olarak davranır; BACKEND
# ayrımı PR-6 ile geldi. Signtool entegrasyonu skeleton (env var-gated).

[CmdletBinding()]
param(
    [ValidateSet('WebView2', 'Cef')]
    [string]$Backend = 'WebView2',

    [switch]$SkipBuild = $false,
    [switch]$SkipSign = $false,

    # Signtool parameters (skeleton — real signing requires .pfx).
    [switch]$Sign = $false,
    [string]$CertPath = '',
    [string]$CertPassword = '',

    [string]$Configuration = 'release',
    [string]$ProductVersion = '0.2.0'
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$CargoTargetDir = Join-Path $RepoRoot 'target' $Configuration
$WxsFile = Join-Path $PSScriptRoot 'viscos.wxs'
$BackendLower = $Backend.ToLowerInvariant()
$OutputMsi = Join-Path $CargoTargetDir "viscos-$BackendLower-$ProductVersion-x86_64.msi"

Write-Host '== Viscos MSI build =='
Write-Host "Backend:          $Backend"
Write-Host "Product version:  $ProductVersion"
Write-Host "Repo root:        $RepoRoot"
Write-Host "Cargo target:     $CargoTargetDir"
Write-Host "WiX source:       $WxsFile"
Write-Host "Output MSI:       $OutputMsi"
Write-Host "Sign mode:        $(if ($Sign) { 'enabled' } else { 'disabled (skeleton)' })"
Write-Host ''

# Step 1 — cargo build (feature-gated by BACKEND)
if (-not $SkipBuild) {
    Write-Host "[1/5] cargo build --release (BACKEND=$Backend)"
    $featureFlag = if ($Backend -eq 'Cef') { '--features viscos-webview/cef-backend' } else { '' }
    $manifestPath = Join-Path $RepoRoot 'Cargo.toml'
    & cargo build --release --bin viscos --manifest-path $manifestPath @featureFlag
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build başarısız (exit code: $LASTEXITCODE)."
    }
} else {
    Write-Host "[1/5] Skipping cargo build (--SkipBuild)"
}

# Step 2 — code sign (release engineering; skeleton when -Sign + CertPath yok)
$exe = Join-Path $CargoTargetDir 'viscos.exe'
if ($Sign -and (Test-Path $CertPath)) {
    Write-Host "[2/5] signtool sign (env-gated, real signing)"
    $signtool = 'C:\Program Files (x86)\Windows Kits\10\bin\x64\signtool.exe'
    if (-not (Test-Path $signtool)) {
        throw "signtool.exe bulunamadı: $signtool. Windows SDK kurulu olmalı."
    }
    if (-not (Test-Path $exe)) {
        throw "Binary bulunamadı: $exe. -SkipBuild kullanmadan veya build başarısız."
    }
    & $signtool sign `
        /tr http://timestamp.digicert.com `
        /td sha256 `
        /fd sha256 `
        /f $CertPath `
        /p $CertPassword `
        $exe
    if ($LASTEXITCODE -ne 0) {
        throw "signtool sign başarısız (exit code: $LASTEXITCODE)."
    }
    Write-Host "  signed: $exe"
} else {
    Write-Host "[2/5] Skipping signtool (skeleton mode -- pass -Sign -CertPath path -CertPassword pw for real signing)"
}

# Step 3 — candle (WiX compile)
Write-Host "[3/5] candle viscos.wxs -dBACKEND=$Backend"
$candle = 'C:\Program Files (x86)\WiX Toolset v3.14\bin\candle.exe'
if (-not (Test-Path $candle)) {
    Write-Host "  candle.exe bulunamadı ($candle). WiX 3.14 kurulu olmalı."
    Write-Host "  Stub modunda candle çalıştırılmadı. Manual build için WiX kurun."
} else {
    & $candle `
        -dCargoTargetDir="$CargoTargetDir" `
        -dBackend="$Backend" `
        -dProductVersion="$ProductVersion" `
        -o (Join-Path $PSScriptRoot 'viscos.wixobj') `
        $WxsFile
    if ($LASTEXITCODE -ne 0) {
        throw "candle.exe başarısız (exit code: $LASTEXITCODE)."
    }
}

# Step 4 — light (WiX link)
Write-Host "[4/5] light viscos.wixobj -> $OutputMsi"
$light = 'C:\Program Files (x86)\WiX Toolset v3.14\bin\light.exe'
if (-not (Test-Path $light)) {
    Write-Host "  light.exe bulunamadı ($light). WiX 3.14 kurulu olmalı."
    Write-Host "  Stub modunda light çalıştırılmadı."
} else {
    & $light `
        -out $OutputMsi `
        (Join-Path $PSScriptRoot 'viscos.wixobj')
    if ($LASTEXITCODE -ne 0) {
        throw "light.exe başarısız (exit code: $LASTEXITCODE)."
    }
    Write-Host "  MSI oluşturuldu: $OutputMsi"
}

# Step 5 — post-build summary
Write-Host ''
Write-Host '[5/5] Build summary'
if (Test-Path $OutputMsi) {
    $size = (Get-Item $OutputMsi).Length
    $sizeMb = [math]::Round($size / 1MB, 2)
    Write-Host "  MSI: $OutputMsi ($sizeMb MB)"
} else {
    Write-Host "  MSI üretilmedi (WiX yüklü değil veya build atlandı)."
}
if (Test-Path $exe) {
    $exeSize = (Get-Item $exe).Length
    $exeSizeMb = [math]::Round($exeSize / 1MB, 2)
    Write-Host "  Binary: $exe ($exeSizeMb MB)"
}
Write-Host ''
Write-Host '== Build complete =='
Write-Host "Backend: $Backend | Output: $OutputMsi"
Write-Host 'WinGet manifest güncellemek için: installer/winget/manifests/.../<version>/Winterus20.Viscos.installer.yaml'
