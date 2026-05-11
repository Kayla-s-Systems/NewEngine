@echo off
setlocal EnableExtensions EnableDelayedExpansion
chcp 65001 >nul
set "CARGO_TERM_COLOR=never"
if not defined NEWENGINE_TERMINAL_TYPEWRITER set "NEWENGINE_TERMINAL_TYPEWRITER=1"
if not defined NEWENGINE_TERMINAL_TYPEWRITER_DELAY_MS set "NEWENGINE_TERMINAL_TYPEWRITER_DELAY_MS=1"

rem Root policy:
rem   Resolve repository root once, switch to it, then use relative paths.
set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%.") do set "NEWENGINE_REPO_ROOT=%%~fI"
pushd "%NEWENGINE_REPO_ROOT%" >nul || (echo [ERROR] Failed to enter repository root: %NEWENGINE_REPO_ROOT% & call :pause_on_error 1 & exit /b 1)

set "ENGINE_ROOT_REL=NewEngine\neocore2"
set "PLUGIN_OUT_REL=%ENGINE_ROOT_REL%\plugins"
set "RUN_LOG_DIR=%ENGINE_ROOT_REL%\logs\run"
set "BUILD_TYPE=release"

if /I "%~1"=="dev" set "BUILD_TYPE=dev"
if /I "%~1"=="debug" set "BUILD_TYPE=debug"
if /I "%~1"=="release" set "BUILD_TYPE=release"

if not exist "%ENGINE_ROOT_REL%\Cargo.toml" (
  echo [ERROR] NewEngine root not found: %ENGINE_ROOT_REL%
  popd >nul
  call :pause_on_error 1
  exit /b 1
)

if not exist "%RUN_LOG_DIR%" mkdir "%RUN_LOG_DIR%" >nul 2>nul
for /F %%I in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmmss"') do set "RUN_TS=%%I"
set "RUN_LOG_FILE=%RUN_LOG_DIR%\game-ready-fps-!RUN_TS!.log"
set "RUN_LOG_LATEST=%RUN_LOG_DIR%\game-ready-fps-latest.log"
for %%I in ("!RUN_LOG_FILE!") do set "RUN_LOG_FILE_ABS=%%~fI"
for %%I in ("!RUN_LOG_LATEST!") do set "RUN_LOG_LATEST_ABS=%%~fI"

set "NEWENGINE_REPO_ROOT=%NEWENGINE_REPO_ROOT%"
set "NEWENGINE_REQUIRE_RENDER_BACKEND=1"

call :cecho Blue "[runGame] syncing runtime plugins into %PLUGIN_OUT_REL% as *-%BUILD_TYPE%.dll"
set "NEWENGINE_PARENT_SCRIPT=runGame"
call "Plugins\build_all_plugins.cmd" %BUILD_TYPE%
set "SYNC_RESULT=%ERRORLEVEL%"
set "NEWENGINE_PARENT_SCRIPT="
if not "%SYNC_RESULT%"=="0" (
  popd >nul
  call :pause_on_error %SYNC_RESULT%
  exit /b %SYNC_RESULT%
)

call :invalidate_runtime_cargo_cache
if errorlevel 1 (
  set "CACHE_RESULT=%ERRORLEVEL%"
  popd >nul
  call :pause_on_error !CACHE_RESULT!
  exit /b !CACHE_RESULT!
)

call :cecho Blue "[runGame] running game-ready-fps"
call :cecho Blue "[runGame] run log: %RUN_LOG_FILE%"
powershell -NoProfile -ExecutionPolicy Bypass -File "Plugins\run_with_log.ps1" -LogFile "!RUN_LOG_FILE_ABS!" -LatestLog "!RUN_LOG_LATEST_ABS!" -Command "cargo" -ArgumentLine "run -p game-ready-fps" -WorkingDirectory "%ENGINE_ROOT_REL%"
set "RUN_RESULT=%ERRORLEVEL%"
call :cecho Blue "[runGame] latest run log: %RUN_LOG_LATEST%"
popd >nul
if not "%RUN_RESULT%"=="0" call :pause_on_error %RUN_RESULT%
exit /b %RUN_RESULT%


:invalidate_runtime_cargo_cache
set "RUNTIME_STAMP_FILE=Plugins\build-state\stamps\runtime\game-ready-fps.source.json"
powershell -NoProfile -ExecutionPolicy Bypass -File "Plugins\runtime_needs_rebuild.ps1" -WorkspaceDir "%ENGINE_ROOT_REL%" -StampFile "!RUNTIME_STAMP_FILE!"
set "RUNTIME_STALE=%ERRORLEVEL%"
if "%RUNTIME_STALE%"=="0" exit /b 0
if "%RUNTIME_STALE%"=="1" (
  call :cecho Yellow "[runGame] runtime sources changed; cleaning stale cargo artifacts for startup graph/runtime crates"
  pushd "%ENGINE_ROOT_REL%" >nul
  cargo clean -p newengine-core -p newengine-platform-api -p newengine-runtime-host -p newengine-game-runtime -p newengine-editor-runtime -p game-ready-fps
  set "CLEAN_RESULT=!ERRORLEVEL!"
  popd >nul
  if not "!CLEAN_RESULT!"=="0" exit /b !CLEAN_RESULT!
  exit /b 0
)
exit /b %RUNTIME_STALE%

:pause_on_error
set "ERR=%~1"
if "%ERR%"=="0" exit /b 0
if defined NEWENGINE_NO_PAUSE exit /b 0
if defined CI exit /b 0
echo.
call :cecho Red "[ERROR] runGame failed with exit code %ERR%."
call :cecho Red "[ERROR] Console is kept open for diagnostics. Press any key to close..."
pause >nul
exit /b 0

:cecho
set "NE_COLOR=%~1"
set "NE_MSG=%~2"
powershell -NoProfile -ExecutionPolicy Bypass -Command "chcp 65001 > $null; try { . 'Plugins\console_highlight.ps1'; Write-NewEngineHighlightedConsoleLine $env:NE_MSG } catch { Write-Host $env:NE_MSG }"
exit /b 0
