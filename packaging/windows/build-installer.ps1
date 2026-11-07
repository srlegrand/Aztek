# Builds the Windows installer from build\dist (run build.ps1 first): .\packaging\windows\build-installer.ps1 [-OutputDir dist]
# Needs Inno Setup 6: "choco install innosetup" or https://jrsoftware.org/isinfo.php
param([string]$OutputDir = "dist")
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."

$line = Select-String -Path "$root\Cargo.toml" -Pattern '^version *= *"(.*)"' | Select-Object -First 1
$version = $line.Matches[0].Groups[1].Value

$iscc = (Get-Command iscc -ErrorAction SilentlyContinue).Source
if (-not $iscc) { $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" }
if (-not (Test-Path $iscc)) {
    choco install innosetup -y --no-progress
    if ($LASTEXITCODE) { throw "could not install Inno Setup" }
}
if (-not (Test-Path "$root\build\dist\aztek.exe")) { throw "build\dist\aztek.exe is missing: run build.ps1" }

New-Item -ItemType Directory -Force "$root\$OutputDir" | Out-Null
$out = Resolve-Path "$root\$OutputDir"
& $iscc "/DAppVersion=$version" "/DOutputDir=$out" "$PSScriptRoot\aztek.iss"
if ($LASTEXITCODE) { throw "iscc failed" }
Write-Host "Built $out\aztek-$version-setup-x64.exe"
