@echo off
setlocal EnableExtensions
if not defined ROOT-DIR (
  for /f "usebackq delims=" %%I in (`git -C "%~dp0" rev-parse --show-toplevel 2^>nul`) do set "ROOT-DIR=%%I"
)
if not defined ROOT-DIR (
  echo import_blender_map: ROOT-DIR is required and repository root auto-detection failed. 1>&2
  exit /b 2
)
where py >nul 2>nul
if %ERRORLEVEL% EQU 0 (
  py -3 "%ROOT-DIR%\NewEngine\neocore2\scripts\import_blender_map.py" --root "%ROOT-DIR%" %*
) else (
  python "%ROOT-DIR%\NewEngine\neocore2\scripts\import_blender_map.py" --root "%ROOT-DIR%" %*
)
endlocal
