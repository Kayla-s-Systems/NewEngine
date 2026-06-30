# Vulkan UI Renderer Contract





This document defines the target architecture for North Star editor UI. Vulkan is the GPU backend. It does not own product UI, editor tools, domain panels, input policy, or layout semantics.





## Ownership boundaries





```text


NEUI


  owns: declarative UI structure, theme tokens, spacing, typography, component templates,


        resource references such as .ytd@entryName and .neftd@entryName





Aurelia UI


  owns: retained UI tree, layout, focus, capture, hit-test, interaction state,


        event routing, state patches, paint command generation





Vulkan UI Renderer


  owns: GPU composition, batching, clipping, text atlases, vector primitives,


        render passes, transitions, effects, texture/material binding





Editor / Tools / Plugins


  own: product data, tool state, commands, capability-specific panels


  consume: ready UI capabilities and publish DTO / UI tree / state patch data


```





Renderer code must never know about domain tools such as Logger, Profiler, Asset Browser, Material Browser, Memory Visualizer, Particle Editor, Symbol Tool, or Engine Registry.





Correct flow:





```text


Tool / Plugin


  -> emits DTO / UiNodeRequest / UiSurfaceNode / UiStatePatch


  -> Aurelia layout + interaction


  -> UiPaintCommand list


  -> Vulkan UI renderer


```





Incorrect flow:





```text


VulkanRenderer::draw_logger_window()


VulkanRenderer::draw_profiler()


VulkanRenderer::draw_asset_browser()


VulkanRenderer::draw_engine_registry()


```





## Visual target





The realistic target is a native game-engine editor / professional tooling UI, not an HTML panel over a window.





Target quality:





```text


Unity / Unreal style editor tooling density


+ Figma-like crisp layout


+ JetBrains-like professional dense panels


+ VS Code-like tool productivity


+ game-engine native overlays


+ GPU-driven transitions and effects


```





North Star visual direction:





```text


white theme:


  deep navy text


  blue accent


  faint compass watermark


  soft cards


  precision lines





dark theme:


  graphite panels


  cyan accent


  low glow


  strong contrast


```





Effects must be restrained. The goal is engineering premium, not visual noise.





## Component target set





### Basic components





```text


Button


IconButton


SplitButton


ToggleButton


Checkbox


RadioButton


Switch


Slider


RangeSlider


ProgressBar


CircularProgress


TextInput


PasswordInput


TextArea


SearchInput


Dropdown


ComboBox


SegmentedControl


TagInput


ColorPicker


DateInput


NumberStepper


Tooltip


Popover


ContextMenu


Toast


Modal


```





### Layout components





```text


Panel


Card


Row


Column


Grid


Stack


ScrollView


Tabs


Accordion


Splitter


DockPanel


ViewportPanel


InspectorPanel


Toolbar


StatusBar


Sidebar


Breadcrumb


```





### Editor-level components





```text


TreeView


AssetBrowser


PropertyGrid


DataTable


VirtualizedList


LogConsole


ProfilerTimeline


NodeGraph


MaterialGraph


CurveEditor


Timeline


ViewportOverlay


GizmoOverlay


CommandPalette


Docking Workspace


```





Editor-level components are composed from NEUI/Aurelia primitives and product DTOs. They are not hardcoded in Vulkan renderer.





## North Star Editor Shell target





```text


North Star Editor


  ├── Top menu / command palette


  ├── Toolbar


  ├── Docking workspace


  ├── Viewport


  ├── Scene hierarchy


  ├── Inspector


  ├── Asset browser


  ├── Console / log viewer


  ├── Profiler panels


  ├── Diagnostics


  ├── Theme editor


  └── Plugin / tool panels


```





The shell is assembled from declarative `.neui` and runtime state patches. Tools provide data and commands. Aurelia owns behavior. Vulkan paints the resolved command stream.





## Vulkan renderer responsibilities





Vulkan may implement low-level rendering systems only:





```text


renderer/


  backend/


    vulkan_device.rs


    swapchain.rs


    command_pool.rs


    frame_context.rs


    sync.rs





  resources/


    buffers.rs


    images.rs


    samplers.rs


    descriptor_sets.rs


    pipeline_cache.rs





  ui/


    paint_command.rs


    primitive.rs


    batcher.rs


    clip_stack.rs


    scissor.rs


    transform.rs


    layer.rs





  text/


    font_manager.rs


    glyph_cache.rs


    glyph_atlas.rs


    text_run.rs


    text_batch.rs


    caret.rs


    selection.rs





  shapes/


    rect.rs


    rounded_rect.rs


    border.rs


    line.rs


    circle.rs


    path.rs





  effects/


    shadow.rs


    blur.rs


    gradient.rs


    glow.rs





  composition/


    render_pass.rs


    ui_pass.rs


    overlay_pass.rs


    debug_pass.rs





  diagnostics/


    gpu_timing.rs


    batch_stats.rs


    overdraw.rs


    atlas_view.rs


```





These modules are renderer infrastructure, not product UI modules.





## Paint command model





Aurelia must not call Vulkan directly. It emits renderer-neutral commands:





```rust


pub enum UiPaintCommand {


    Rect(RectCommand),


    RoundedRect(RoundedRectCommand),


    Border(BorderCommand),


    Text(TextCommand),


    Image(ImageCommand),


    Icon(IconCommand),


    ClipBegin(ClipCommand),


    ClipEnd,


    LayerBegin(LayerCommand),


    LayerEnd,


}


```





Renderer pipeline:





```text


paint commands


  -> normalize


  -> resolve theme tokens


  -> resolve fonts / icons / images


  -> apply clipping


  -> batch


  -> upload instance buffers


  -> record command buffer


  -> submit


```





This keeps each layer independently testable:





```text


Aurelia can be tested without Vulkan


Vulkan renderer can be benchmarked without full UI


NEUI can compile declarative layout without knowing GPU details


Tools can publish data without knowing renderer internals


```





## Text quality requirements





Text is a first-class renderer system. A professional editor UI requires:





```text


font loading


NEFTD font refs through .neftd@entryName


text shaping


font fallback


emoji / symbol fallback


glyph atlas


subpixel positioning


baseline correctness


selection highlight


caret rendering


multiline layout


line wrapping


text clipping


scrolling text areas


log viewer rendering


monospace alignment


IME path readiness


```





The renderer must not hardcode font files. Fonts are referenced through NEUI / UI DTO data and resolved through asset services.





## Texture and material requirements





Textures are referenced through NEUI/ListFile selectors:





```text


assets/ui/skin.ytd@button_primary


assets/ui/skin.ytd@panel_background


assets/textures/ui/icons/builtin_icons.ytd@search


```





Rules:





```text


.ytd@entryName is required for texture refs


.neftd@entryName is required for font refs


renderer receives resolved GPU-ready texture/material handles


asset container parsing remains asset-side


```





## Acceptance invariants





1. No `draw_<product>_window` functions in Vulkan renderer.


2. Renderer modules use generic primitives, batches, passes, resources, and diagnostics only.


3. `.neui` owns authored UI structure and references resources by selector.


4. Aurelia owns layout, hit-test, input, focus, capture, state, and event routing.


5. Vulkan owns GPU paint execution only.


6. Button visual bounds and hit bounds must match.


7. Text must use the configured font stack / `.neftd@entryName` references, with fallback only when asset resolution fails.


8. Editor tools emit DTOs/state; they do not reach into renderer internals.
