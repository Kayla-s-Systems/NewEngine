@echo off
setlocal EnableExtensions EnableDelayedExpansion
chcp 65001 >nul

rem Resolve repository root once, then use only relative paths.
set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%.") do set "NEWENGINE_REPO_ROOT=%%~fI"
pushd "%NEWENGINE_REPO_ROOT%" >nul || (echo [ERROR] Failed to enter repository root: %NEWENGINE_REPO_ROOT% & call :pause_on_error 1 & exit /b 1)

call :cecho "[INFO] Cleaning Rust target directories under %NEWENGINE_REPO_ROOT%"
set "CLEAN_ERRORS=0"

call :remove_target "NewEngine\neocore2\target"
for /D %%D in ("Plugins\*") do call :remove_target "%%~D\target"
for /D %%D in ("Importers\*") do call :remove_target "%%~D\target"

if not "%CLEAN_ERRORS%"=="0" (
  popd >nul
  call :pause_on_error %CLEAN_ERRORS%
  exit /b %CLEAN_ERRORS%
)

call :cecho "[OK] Target cleanup completed"
popd >nul
exit /b 0

:remove_target
set "TARGET_DIR=%~1"
if not exist "%TARGET_DIR%" (
  call :cecho "[SKIP] %TARGET_DIR%"
  exit /b 0
)
call :cecho "[CLEAN] deleting %TARGET_DIR%"
rd /S /Q "%TARGET_DIR%" >nul 2>nul
if errorlevel 1 (
  call :cecho "[ERROR] failed to delete %TARGET_DIR%"
  set /A CLEAN_ERRORS+=1
)
exit /b 0

:pause_on_error
set "ERR=%~1"
if "%ERR%"=="0" exit /b 0
if defined NEWENGINE_NO_PAUSE exit /b 0
if defined CI exit /b 0
echo.
call :cecho "[ERROR] cleanTargets failed with exit code %ERR%."
call :cecho "[ERROR] Console is kept open for diagnostics. Press any key to close..."
pause >nul
exit /b 0

:cecho
set "NE_MSG=%~1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "chcp 65001 > $null; try { . 'Plugins\console_highlight.ps1'; Write-NewEngineHighlightedConsoleLine $env:NE_MSG } catch { Write-Host $env:NE_MSG }"
exit /b 0
