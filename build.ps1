# Builds Aztek on Windows: .\build.ps1 [-Debug] [-Run] [-Qt C:\Qt\6.8.0\msvc2022_64]
# Needs Visual Studio 2022 (or Build Tools) with the C++ workload, Rust (rustup), CMake, Qt 6.4+ for MSVC, Python 3
# and the Vulkan runtime (comes with the GPU driver). Ninja ships with Visual Studio's CMake tools.
param(
    [switch]$Debug,
    [switch]$Run,
    [string]$Qt = ""
)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$type = if ($Debug) { "Debug" } else { "Release" }
$dir = if ($Debug) { "build-debug" } else { "build" }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Rust is missing: install it from https://rustup.rs"
}

# The MSVC environment, from Visual Studio's own locator.
if (-not $env:VCINSTALLDIR) {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { throw "Visual Studio 2022 or its Build Tools with the C++ workload are missing" }
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw "Visual Studio has no C++ tools: add the 'Desktop development with C++' workload" }
    $vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"
    cmd /c "`"$vcvars`" >nul && set" | ForEach-Object {
        if ($_ -match "^(.*?)=(.*)$") { Set-Item -Path "env:$($matches[1])" -Value $matches[2] }
    }
}

# Qt: -Qt, then QTDIR, then the newest MSVC kit under C:\Qt.
if (-not $Qt) { $Qt = $env:QTDIR }
if (-not $Qt) {
    $kit = Get-ChildItem "C:\Qt\6.*\msvc*_64" -Directory -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
    if ($kit) { $Qt = $kit.FullName }
}
if (-not $Qt -or -not (Test-Path "$Qt\lib\cmake\Qt6")) {
    throw "Qt 6 for MSVC not found: install it with the Qt online installer, or pass -Qt C:\Qt\<version>\msvc2022_64"
}
Write-Host "Qt: $Qt"

# The Python on PATH: what the exe links must be the one copied below, not another CMake finds in the registry.
$py = (Get-Command python -ErrorAction SilentlyContinue).Source
$pyArg = if ($py) { "-DPython3_EXECUTABLE=$py" } else { "-DAZTEK_PYTHON=OFF" }
cmake -B $dir -G Ninja "-DCMAKE_BUILD_TYPE=$type" "-DCMAKE_PREFIX_PATH=$Qt" $pyArg
if ($LASTEXITCODE) { throw "configure failed" }
cmake --build $dir
if ($LASTEXITCODE) { throw "build failed" }

# A runnable folder that needs nothing else installed: the exe, Qt's DLLs and plugins, the VC runtime, Python's DLL and
# standard library (Python finds Lib\os.py beside the exe), and the example scenes.
$dist = "$dir\dist"
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "$dir\aztek.exe" $dist -Force
& "$Qt\bin\windeployqt.exe" --no-translations --no-compiler-runtime "$dist\aztek.exe" | Out-Null
if ($LASTEXITCODE) { throw "windeployqt failed" }
if ($env:VCToolsRedistDir) {
    Get-ChildItem "$env:VCToolsRedistDir\x64\Microsoft.VC*.CRT\*.dll" | Copy-Item -Destination $dist -Force
}
if ($py) {
    $home_ = Split-Path $py
    Get-ChildItem $home_ -Filter "python3*.dll" | Copy-Item -Destination $dist -Force
    robocopy "$home_\Lib" "$dist\Lib" /E /NFL /NDL /NJH /NJS /NP /XD __pycache__ site-packages test tests idlelib tkinter turtledemo ensurepip | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "copying Python's Lib failed" }
    robocopy "$home_\DLLs" "$dist\DLLs" /E /NFL /NDL /NJH /NJS /NP /XD __pycache__ /XF tcl*.dll tk*.dll _tkinter.pyd | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "copying Python's DLLs failed" }
    $global:LASTEXITCODE = 0
}
if (Test-Path examples) { Copy-Item examples "$dist\examples" -Recurse -Force }
Write-Host "Built $dist\aztek.exe"

if ($Run) { & "$dist\aztek.exe" }
