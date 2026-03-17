# Render plugin unification fix

## Problem

Render backends exporting the legacy `newengine_render_backend_create_v1` symbol were classified as a special runtime
kind before normal plugin metadata was considered. That kept Vulkan on a hybrid path instead of the standard
`PluginModuleV3` discovery/load path.

## Fix

- plugin metadata now wins over the legacy render backend ABI marker
- legacy render-backend-only DLLs are reported explicitly as unsupported legacy units instead of being treated as
  runtime selections
- duplicated probe identity extraction was collapsed into one helper
- discovery diagnostics now distinguish:
    - platform runtime candidates
    - legacy render-backend-only dynlibs
    - normal bootstrap/engine plugin candidates
- host context comment updated to reflect that only platform runtime remains external

## Result

- Vulkan-like backends that export both plugin metadata and the old render symbol are loaded as normal runtime plugins
- the runtime bridge can keep consuming `render.api.v1` without a special discovery path
- hybrid branching in discovery/selection is reduced
- failure mode for stale legacy-only backends is deterministic and visible in logs
