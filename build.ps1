# Builds Aztek on Windows: .\build.ps1 [-Debug] [-Run] [-Qt C:\Qt\6.8.0\msvc2022_64]
# Needs Visual Studio 2022 (or Build Tools) with the C++ workload, Rust (rustup), CMake, Qt 6.4+ for MSVC and the Vulkan
# runtime (comes with the GPU driver). Ninja ships with Visual Studio's CMake tools. Python 3.12 is the only version
# Aztek embeds: an installed 3.12 is used (the py launcher's, else python on PATH), else one is fetched into the build
# folder. Either way it is copied beside the exe, so the installer needs no Python on the machine it installs on.
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

# Python 3.12: what the exe links must be the one copied below, not another CMake finds in the registry.
function Get-Python312 {
    if (Get-Command py -ErrorAction SilentlyContinue) {
        $p = & py -3.12 -c "import sys; print(sys.executable)" 2>$null
        if ($LASTEXITCODE -eq 0 -and $p) { return $p.Trim() }
    }
    $global:LASTEXITCODE = 0
    $onPath = (Get-Command python -ErrorAction SilentlyContinue).Source
    if ($onPath) {
        $v = & $onPath -c "import sys; print('%d.%d' % sys.version_info[:2])" 2>$null
        if ($v -and $v.Trim() -eq "3.12") { return $onPath }
    }
    $global:LASTEXITCODE = 0
    return $null
}
$py = Get-Python312
if (-not $py) {
    $bundle = & "$PSScriptRoot\tools\python\fetch-python.ps1" (Join-Path $PSScriptRoot "$dir\python")
    $py = Join-Path $bundle "python.exe"
}
Write-Host "Python: $py"
$env:PYO3_PYTHON = $py
cmake -B $dir -G Ninja "-DCMAKE_BUILD_TYPE=$type" "-DCMAKE_PREFIX_PATH=$Qt" "-DPython3_EXECUTABLE=$py"
if ($LASTEXITCODE) { throw "configure failed" }
cmake --build $dir
if ($LASTEXITCODE) { throw "build failed" }

# A runnable folder that needs nothing else installed: the exe, Qt's DLLs and plugins, the VC runtime, Python's DLL and
# standard library (Python finds Lib\os.py beside the exe), and the example scenes.
$dist = "$dir\dist"
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "$dir\aztek.exe" $dist -Force
# aztekpy, the reference's mayapy, beside it.
if (Test-Path "$dir\aztekpy.exe") { Copy-Item "$dir\aztekpy.exe" $dist -Force }
# Harold's command-line renderer, for .hss scene files.
Copy-Item "$dir\harold.exe" $dist -Force
& "$Qt\bin\windeployqt.exe" --no-translations --no-compiler-runtime "$dist\aztek.exe" | Out-Null
if ($LASTEXITCODE) { throw "windeployqt failed" }
if ($env:VCToolsRedistDir) {
    Get-ChildItem "$env:VCToolsRedistDir\x64\Microsoft.VC*.CRT\*.dll" | Copy-Item -Destination $dist -Force
}
if ($py) {
    $home_ = Split-Path $py
    Get-ChildItem $home_ -Filter "python3*.dll" | Copy-Item -Destination $dist -Force
    robocopy "$home_\Lib" "$dist\Lib" /E /NFL /NDL /NJH /NJS /NP /XD __pycache__ site-packages test tests idlelib tkinter turtledemo | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "copying Python's Lib failed" }
    robocopy "$home_\DLLs" "$dist\DLLs" /E /NFL /NDL /NJH /NJS /NP /XD __pycache__ /XF tcl*.dll tk*.dll _tkinter.pyd | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "copying Python's DLLs failed" }
    $global:LASTEXITCODE = 0
    # pip in Aztek's own Python, as the reference's mayapy has it: `aztekpy -m pip install ...`. Not fatal: the program
    # runs without it.
    if (Test-Path "$dist\aztekpy.exe") {
        # Windows PowerShell turns a native program's stderr into errors, which "Stop" would make fatal.
        $ErrorActionPreference = "Continue"
        & "$dist\aztekpy.exe" -m ensurepip --default-pip 2>&1 | Out-Host
        $ErrorActionPreference = "Stop"
        if ($LASTEXITCODE -ne 0) { Write-Warning "pip was not added to the bundled Python" }
        $global:LASTEXITCODE = 0
    }
}
if (Test-Path examples) { Copy-Item examples "$dist\examples" -Recurse -Force }
# The plug-ins that ship with Aztek (plugins\), where Aztek looks for them: plug-ins\ beside the exe.
if (Test-Path "$dir\plug-ins") { Copy-Item "$dir\plug-ins" "$dist\plug-ins" -Recurse -Force }
Write-Host "Built $dist\aztek.exe"

if ($Run) { & "$dist\aztek.exe" }
