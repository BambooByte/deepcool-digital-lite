@echo off
setlocal

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\Build-Release.ps1"
set "build_exit_code=%errorlevel%"

echo.
if "%build_exit_code%"=="0" (
    echo Release build completed.
) else (
    echo Release build failed with exit code %build_exit_code%.
)
pause
exit /b %build_exit_code%
