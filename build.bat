@echo off
rem Builds Aztek on Windows; arguments pass to build.ps1 (-Debug, -Run, -Qt path).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
