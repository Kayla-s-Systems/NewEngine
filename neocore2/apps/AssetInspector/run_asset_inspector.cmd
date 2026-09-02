@echo off
setlocal EnableExtensions
if not defined ROOT-DIR (
  for /f "usebackq delims=" %%I in (`git -C "%~dp0" rev-parse --show-toplevel 2^>nul`) do set "ROOT-DIR=%%I"
)
if not defined ROOT-DIR (
  echo AssetInspector: ROOT-DIR is required and repository root auto-detection failed. 1>&2
  exit /b 2
)
cd /d "%ROOT-DIR%\NewEngine\neocore2"
set "NEWENGINE_ASSET_INSPECTOR_ASSETS_DIR=%ROOT-DIR%\gameAssets"
if exist "target\release\asset-inspector.exe" (
  "target\release\asset-inspector.exe"
) else (
  cargo run -p asset-inspector
)
endlocal
