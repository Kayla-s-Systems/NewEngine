# NorthStar GUI Editor



`northstar-gui-editor` is the descriptor-driven host layer for the future Rust GUI editor.



It is intentionally not a hardcoded format combiner. The editor discovers tool, codec and editor providers, builds a capability registry and composes UI/preview/inspector surfaces from provider-declared capabilities.



## Core rule



```text

Asset path

  -> discovery

  -> format registry

  -> selected provider

  -> capabilities

  -> schema/preview/inspector composition

```



Forbidden model:



```text

if ext == ".ydd" { open_ydd_editor() }

if ext == ".ytd" { open_ytd_editor() }

```



Correct model:



```text

provider manifest

  -> format support

  -> capability ids

  -> editor schema

  -> preview surface

  -> read/write/validate/diff operations

```



## Discovery roots



Default discovery roots are relative to `EngineRepo/NewEngine`:



```text

tools/first_party/**/tool.json

../tools/toolsSrc/**/tool.json

pluginsRuntime/codecs/codec_manifest.json

pluginsRuntime/codecs/*.dll

```



The external `../tools/toolsSrc` scan is a bridge for the current repository layout where first-party native tools already exist before the final `tools/first_party` convention is normalized.



## Commands



From this directory:



```bat

cargo run -- doctor --root ..

cargo run -- doctor --with-tools --root ..

cargo run -- list --root ..

cargo run -- tools-list --root ..

cargo run -- tools-doctor --root ..

cargo run -- open --root .. --asset assets/textures/ui/icons/builtin_icons.ytd

```



Command roles:



```text

doctor

  Runs local editor-host discovery and capability checks.



doctor --with-tools

  Runs local editor-host discovery, then calls the external tool-plane diagnostics.



tools-list

  Calls the existing script-plane tool registry list command through ToolPlaneBridge.



tools-doctor

  Calls the existing script-plane tool registry doctor command through ToolPlaneBridge.



open

  Resolves an asset through the provider registry and prints composed shell/preview/inspector DTOs.

```



## Tool-plane integration



The editor host can call the existing Python script plane without making it a hard runtime dependency:



```text

NorthStar GUI Editor

  -> ToolPlaneBridge

  -> tools/scripts/takesome.py tools list

  -> tools/scripts/takesome.py tools doctor

  -> ToolPlaneResult diagnostics

```



If the script plane is missing, `doctor --with-tools` reports a warning and keeps local editor discovery usable. If the script plane is present and returns a non-zero exit code, `doctor --with-tools`, `tools-list` or `tools-doctor` fail visibly.



## Product-layer boundary



The editor lives under:



```text

EngineRepo/NewEngine/editor/northstar-gui-editor

```



It is intentionally outside:



```text

EngineRepo/NewEngine/neocore2/Cargo.toml

```



Runtime crates must not depend on editor GUI code. The editor may read runtime/plugin manifests and may host providers through ABI/CLI adapters.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/editor/northstar-gui-editor`

**Role:** Editor-facing application/tooling area.

**Local contents:** 2 direct subdirectories, 6 direct files.

**Direct file examples:** `Cargo.lock`, `Cargo.toml`, `editor_settings.json`, `runtime_tool_mounts.json`, `tool.json`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
