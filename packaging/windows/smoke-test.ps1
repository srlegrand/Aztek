# Runs the installer silently into a temporary folder, starts the installed exe with a Python script, then uninstalls.
# Python and Qt must come from the install, so PATH is cut down to Windows' own folders first.
param([string]$Setup = "")
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."
if (-not $Setup) { $Setup = (Get-ChildItem "$root\dist\aztek-*-setup-x64.exe" | Select-Object -First 1).FullName }

$dir = Join-Path $env:RUNNER_TEMP "aztek-install"
$log = Join-Path $env:RUNNER_TEMP "aztek-install.log"
$p = Start-Process $Setup -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/DIR=`"$dir`"", "/LOG=`"$log`"" -Wait -PassThru
if ($p.ExitCode) { Get-Content $log | Select-Object -Last 30; throw "installer exit code $($p.ExitCode)" }

$exe = Join-Path $dir "aztek.exe"
if (-not (Test-Path $exe)) { throw "aztek.exe was not installed" }
if (-not (Test-Path (Join-Path $dir "unins000.exe"))) { throw "no uninstaller" }

$out = Join-Path $env:RUNNER_TEMP "aztek-smoke"
New-Item -ItemType Directory -Force $out | Out-Null
$txt = (Join-Path $out "python.txt").Replace("\", "/")
$png = Join-Path $out "window.png"
$script = Join-Path $out "check.py"
@"
import sys, json, ctypes, ssl, sqlite3, zlib
import aztek.cmds as cmds
cube = cmds.polyCube()
with open("$txt", "w") as f:
    f.write(sys.version + "\n" + sys.prefix + "\n" + json.dumps(cube) + "\n")
"@ | Set-Content -Encoding utf8 $script

$env:PATH = "$env:SystemRoot\system32;$env:SystemRoot"
Remove-Item Env:pythonLocation, Env:PYTHONHOME, Env:PYTHONPATH, Env:QT_ROOT_DIR, Env:QT_PLUGIN_PATH -ErrorAction SilentlyContinue
$p = Start-Process $exe -ArgumentList "--script", "`"$script`"", "--screenshot", "`"$png`"" -PassThru
$null = $p.Handle # Without the handle held, ExitCode reads empty once the process has gone.
if (-not $p.WaitForExit(180000)) { $p.Kill(); throw "aztek.exe did not finish in three minutes" }
Write-Host "aztek.exe exit code $($p.ExitCode)"
if (-not (Test-Path $txt)) { throw "the Python script did not run" }
Get-Content $txt
if ($p.ExitCode) { throw "aztek.exe exit code $($p.ExitCode)" }
if (-not (Test-Path $png)) { throw "no screenshot" }

$p = Start-Process (Join-Path $dir "unins000.exe") -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait -PassThru
if ($p.ExitCode) { throw "uninstaller exit code $($p.ExitCode)" }
Write-Host "passed"
