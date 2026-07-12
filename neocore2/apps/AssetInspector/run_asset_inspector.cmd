@echo off
setlocal
cd /d "%~dp0\..\.."
set "NEWENGINE_ASSET_INSPECTOR_ASSETS_DIR=%~dp0\..\..\..\..\gameAssets"
if exist "target\release\asset-inspector.exe" (
  "target\release\asset-inspector.exe"
) else (
  cargo run -p asset-inspector
)
endlocal
