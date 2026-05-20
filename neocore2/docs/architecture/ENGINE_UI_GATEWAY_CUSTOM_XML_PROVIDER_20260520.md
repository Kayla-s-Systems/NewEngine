# Engine UI Gateway and Custom NewEngine UI Provider

## Verdict

UI must be a first-class engine gateway, not a set of ad-hoc overlays owned by
random subsystems.

```text
consumer / runtime system
  -> engine.ui
  -> ActiveGatewayRegistry
  -> selected ui.backend provider
  -> UiDrawList / UiSurface manifests / XML documents
  -> renderer UI composite pass
```

The new first-party direction is `Custom NewEngine UI Provider`, not Scaleform.
Scaleform remains useful as study material only: movie/view lifecycle,
resource-loader separation, font/glyph cache, input event funnel, and clear
resource ownership are worth borrowing. The Flash/SWF runtime and ActionScript
execution model are not part of NewEngine.

## Provider shape

```text
plugin id: engine.ui.custom_newengine
service:   newengine.ui.custom.api
capability: ui.backend
engine gateway: engine.ui
backend: custom_newengine_ui
priority: 250
```

The provider exposes XML-owned layout documents and the same binary draw-frame
hot path used by the renderer:

```text
surface_manifest_v1
surface_catalog_v1
layout_manifest_v1
action_manifest_v1
loading_shell_v1
document_xml_v1
debug_overlay_telemetry_v1
pause_menu_state_v1
draw_frame_v1
draw_frame_bin_v1
shutdown_v1
```

## XML direction

UI documents use a NewEngine-owned XML vocabulary rather than SWF/Flash:

```xml
<ui schema="newengine.ui.xml.v1" theme="newengine.dark.gold-magenta">
  <surface id="engine.loading" z="900" state="ScreenOverlayStatus">
    <panel id="boot.card" anchor="center" w="62%" h="46%" chrome="ksystems">
      <label id="boot.title" text="$loading.title" role="title" />
      <label id="boot.status" text="$loading.status" role="status" />
      <label id="boot.detail" text="$loading.detail" role="muted" />
      <progress id="boot.progress" value="$loading.progress_01" />
      <subsystem-list id="boot.subsystems" items="$loading.subsystems" />
    </panel>
  </surface>
</ui>
```

Initial standard controls:

```text
label
button
textbox / input
checkbox
select
image
progress
panel
surface
subsystem-list
```

Widgets emit declarative action ids. They do not execute gameplay, renderer,
platform or process side effects directly.

## Loading screen rule

`engine.loading` is no longer a special visual UI implementation. It is a
state/snapshot producer and fallback diagnostic gateway. Visual presentation is
an ordinary `engine.ui` surface named `engine.loading`.

```text
engine.loading snapshot / ScreenOverlayStatus
  -> engine.ui telemetry projection
  -> Custom NewEngine UI XML surface
  -> UiDrawList
  -> renderer UI composite
```

The platform-native compositor remains a pre-render fallback only. Before the
renderer can composite provider draw lists, it may present bootstrap pixels so
startup is never black. Once normal engine frames are ticking, loading and scene
launch overlays are provider-owned `engine.ui` surfaces.

## Hard invariants

- No Scaleform runtime dependency.
- No ad-hoc subsystem-specific UI drawing paths.
- No loading-screen-only visual renderer.
- Consumers call `engine.ui`, not provider service ids.
- UI textures must resolve through NewEngine runtime texture references such as
  `.neytd@selector` once texture-backed widgets are enabled.
- Input routing must flow through `engine.input` / `engine.input.bindings` and
  produce semantic UI actions, not raw key checks inside widgets.
