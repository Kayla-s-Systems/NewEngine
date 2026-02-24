#![forbid(unsafe_op_in_unsafe_fn)]

mod camera;
mod icons;
mod panels;
mod util;

use newengine_platform_winit::egui;
use newengine_ui::markup::UiMarkupDoc;
use newengine_ui::{UiBuildFn, UiHub};

use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_ecs::EntityId;
use newengine_gizmo::egui::EguiGizmo;
use newengine_materials::MaterialId;
use newengine_materials::MaterialRef;
use newengine_primitives::Primitive;
use newengine_transform::Transform;
use newengine_viewport::Viewport;

use newengine_editor_core::{EditorCommand, EditorState, TransformSnapshot};

use crate::plugin_manager::PluginManagerUi;
use crate::scene_bridge::SceneBridge;
use crate::ui_contrib::plugin_manager::PluginManagerContributor;
use crate::viewport_bridge::ViewportBridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LightSpawnKind {
    Directional,
    Point,
}

impl Default for LightSpawnKind {
    #[inline]
    fn default() -> Self {
        Self::Point
    }
}

/// Minimal editor UI: foundation-first.
///
/// Responsibilities:
/// - Own UI state (selection cache, panel toggles, gizmo state).
/// - Bridge input/intent to the renderer via `ViewportBridge`.
/// - Render editor panels.
///
/// Non-goals (for now): undo/redo, docking, command bus. Those will be layered on top.
pub struct EditorUiBuild {
    pub(crate) shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,

    pub(crate) viewport: Viewport,

    pub(crate) viewport_bridge: Arc<ViewportBridge>,
    pub(crate) scene_bridge: Arc<SceneBridge>,
    pub(crate) previews: Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
    pub(crate) plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
    pub(crate) plugin_manager: Arc<Mutex<PluginManagerUi>>,

    pub(crate) ui_hub: UiHub,

    pub(crate) icons: icons::EditorIconLoader,

    // Viewport navigation interaction (UI-driven, not via global input plugin).
    //
    // Track MMB orbit/pan and RMB free-fly separately.
    // Mixing them causes a first-frame delta spike when capture toggles.
    pub(crate) last_nav_drag_pos: Option<egui::Pos2>,
    pub(crate) last_fly_drag_pos: Option<egui::Pos2>,

    /// Latched RMB free-fly capture state.
    pub(crate) fly_latch: newengine_viewport::nav::FlyRmbLatch,

    pub(crate) console_open: bool,
    pub(crate) console_input: String,

    // Lights UI.
    pub(crate) selected_light_kind: LightSpawnKind,

    // Primitives UI.
    pub(crate) selected_primitive: Option<newengine_primitives::PrimitiveId>,

    // Selection + inspector cache.
    pub(crate) selected_entity_cached: Option<EntityId>,
    pub(crate) insp_pos: [f32; 3],
    pub(crate) insp_rot_deg: [f32; 3],
    pub(crate) insp_scale: [f32; 3],
    pub(crate) insp_color: [f32; 4],
    pub(crate) insp_material: MaterialId,

    pub(crate) gizmo: EguiGizmo,

    pub(crate) editor: EditorState,
    pub(crate) gizmo_was_dragging: bool,
    pub(crate) gizmo_drag_begin: Option<(EntityId, TransformSnapshot)>,

    // Viewport picking is processed on render thread, but selection semantics (replace/add/toggle)
    // are decided by UI at click time.
    pub(crate) pending_pick: Option<PendingPick>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingPick {
    pub(crate) additive: bool,
    pub(crate) toggle: bool,
}

impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
        scene_bridge: Arc<SceneBridge>,
        previews: Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
    ) -> Self {
        let cam = scene_bridge
            .scene()
            .read()
            .active_camera()
            .expect("scene has no active camera");
        let viewport = Viewport::new(Some(cam));

        let plugin_manager = Arc::new(Mutex::new(PluginManagerUi::new(Arc::clone(
            &plugins_bridge,
        ))));

        let mut me = Self {
            shared_doc,
            viewport,
            viewport_bridge,
            scene_bridge,
            previews,
            plugins_bridge: Arc::clone(&plugins_bridge),
            plugin_manager: Arc::clone(&plugin_manager),
            ui_hub: UiHub::new(),

            icons: icons::EditorIconLoader::new(),

            last_nav_drag_pos: None,
            last_fly_drag_pos: None,
            fly_latch: newengine_viewport::nav::FlyRmbLatch::default(),

            console_open: false,
            console_input: String::new(),

            selected_light_kind: LightSpawnKind::default(),

            selected_primitive: None,

            selected_entity_cached: None,
            insp_pos: [0.0; 3],
            insp_rot_deg: [0.0; 3],
            insp_scale: [1.0, 1.0, 1.0],
            insp_color: [0.85, 0.85, 0.9, 1.0],
            insp_material: MaterialId::invalid(),

            gizmo: EguiGizmo::new(),

            editor: EditorState::new(),
            gizmo_was_dragging: false,
            gizmo_drag_begin: None,

            pending_pick: None,
        };

        me.ui_hub
            .register(Box::new(PluginManagerContributor::new(plugin_manager)));

        me
    }

    pub(crate) fn read_selected_pose(
        &self,
        e: EntityId,
    ) -> Option<(
        newengine_math::Vec3,
        newengine_math::Quat,
        newengine_math::Vec3,
        Option<[f32; 4]>,
    )> {
        let scene = self.scene_bridge.scene();
        let s = scene.read();
        let w = s.world();
        let t = w.get::<Transform>(e)?;
        let color = w.get::<Primitive>(e).map(|p| p.color);
        Some((t.position, t.rotation, t.scale, color))
    }

    pub(crate) fn refresh_inspector_cache(&mut self, entity: EntityId) {
        let scene = self.scene_bridge.scene();
        let s = scene.read();
        let w = s.world();

        if let Some(t) = w.get::<Transform>(entity) {
            self.insp_pos = [t.position.x, t.position.y, t.position.z];
            let (y, p, r) = t.yaw_pitch_roll();
            self.insp_rot_deg = [y.to_degrees(), p.to_degrees(), r.to_degrees()];
            self.insp_scale = [t.scale.x, t.scale.y, t.scale.z];
        }

        if let Some(p) = w.get::<Primitive>(entity) {
            self.insp_color = p.color;
        }

        if let Some(mr) = w.get::<MaterialRef>(entity) {
            self.insp_material = mr.id;
        } else {
            self.insp_material = MaterialId::invalid();
        }

        self.selected_entity_cached = Some(entity);
    }

    #[inline]
    fn build_ui(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

        // Keep editor icon textures hot and fully driven by AssetManager.
        self.icons.pump(ctx);

        let _maybe_doc = {
            self.shared_doc
                .lock()
                .ok()
                .and_then(|g| g.as_ref().cloned())
        };

        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.console_open = !self.console_open;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
            if let Ok(mut pm) = self.plugin_manager.lock() {
                pm.toggle();
            }
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::N)) {
            self.scene_bridge.cmd_new_scene();
            self.editor.commands.clear();
            self.editor.selection.clear();
        }

        // Undo/Redo (industry standard).
        let wants_kb = ctx.wants_keyboard_input();
        if !wants_kb {
            let undo = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z));
            let redo = ctx.input(|i| {
                (i.modifiers.command && i.key_pressed(egui::Key::Y))
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
            });

            if undo {
                if let Some(cmd) = self.editor.commands.pop_undo() {
                    self.apply_editor_command_undo(cmd);
                }
            } else if redo {
                if let Some(cmd) = self.editor.commands.pop_redo() {
                    self.apply_editor_command_redo(cmd);
                }
            }
        }

        panels::topbar::draw(self, ctx);
        panels::toolbar::draw(self, ctx);
        panels::hierarchy::draw(self, ctx);
        panels::inspector::draw(self, ctx);
        panels::viewport::draw(self, ctx);
        panels::console::draw(self, ctx);

        let mut user_data = ();
        self.ui_hub.run(ctx_any, &mut user_data);
    }

    #[inline]
    fn apply_editor_command_undo(&self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::SetTransform { entity, before, .. } => {
                self.scene_bridge.cmd_set_transform(
                    entity,
                    before.position,
                    before.rotation_ypr,
                    before.scale,
                );
            }
        }
    }

    #[inline]
    fn apply_editor_command_redo(&self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::SetTransform { entity, after, .. } => {
                self.scene_bridge.cmd_set_transform(
                    entity,
                    after.position,
                    after.rotation_ypr,
                    after.scale,
                );
            }
        }
    }
}

impl UiBuildFn for EditorUiBuild {
    #[inline]
    fn build(&mut self, ctx_any: &mut dyn Any) {
        self.build_ui(ctx_any);
    }
}
