#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::{Arc, Mutex};

use newengine_assets::AssetServiceClient;
use newengine_editor_core::EditorState;
use newengine_gizmo::egui::EguiGizmo;
use newengine_materials::MaterialId;
use newengine_plugin_host::{default_host_api, has_service};
use newengine_scene_io::SceneIoClient;
use newengine_ui::input::keys as ui_keys;
use newengine_ui::markup::UiMarkupDoc;
use newengine_ui::{UiHub, UiInputFrame};
use newengine_viewport::Viewport;

use crate::material_pipeline::MaterialPipeline;
use crate::plugin_manager::PluginManagerUi;
use crate::scene_bridge::SceneBridge;
use crate::ui_contrib::plugin_manager::PluginManagerContributor;
use crate::viewport_bridge::ViewportBridge;

use super::commands::EditorCommandBus;
use super::extension_abi;
use super::schema::{self, EditorSchemaRegistry};
use super::state::*;
use super::{dock, icons};

impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<Arc<UiMarkupDoc>>>>,
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
        scene_bridge: Arc<SceneBridge>,
        _previews: Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
        schema_registry: Arc<parking_lot::RwLock<EditorSchemaRegistry>>,
        extension_registry: Arc<parking_lot::RwLock<extension_abi::EditorExtensionAbiRegistry>>,
    ) -> Self {
        let cam = scene_bridge.scene().read().active_camera();
        let viewport = Viewport::new(cam);

        let plugin_manager = Arc::new(Mutex::new(PluginManagerUi::new(Arc::clone(
            &plugins_bridge,
        ))));

        let mut me = Self {
            schema_registry,
            extension_registry,
            shared_doc,
            viewport,
            last_viewport_extent: None,
            viewport_bridge,
            scene_bridge,
            plugins_bridge: Arc::clone(&plugins_bridge),
            plugin_manager: Arc::clone(&plugin_manager),
            ui_hub: UiHub::new(),
            markup_state: newengine_ui::markup::UiState::default(),
            icons: icons::EditorIconLoader::new(),
            material_pipeline: MaterialPipeline::new(),
            dock_state: dock::dock_state_for_preset(WorkspacePreset::Editing),
            saved_dock_layout: None,
            workspace_preset: WorkspacePreset::Editing,
            viewport_mode: ViewportMode::Lit,
            transform_snap: TransformSnapSettings::default(),
            camera_speed: CameraSpeedSettings::default(),
            command_palette: CommandPaletteState::default(),
            assets: if has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
                Some(AssetServiceClient::new(default_host_api()))
            } else {
                None
            },
            asset_ui: AssetManagerUiState::default(),
            asset_spawn_request: None,
            scene_io: if has_service(newengine_scene_io::SCENE_IO_SERVICE_ID) {
                Some(SceneIoClient::new(default_host_api()))
            } else {
                None
            },
            scene_io_ui: SceneIoUiState::default(),
            outliner_filter: String::new(),
            details_filter: String::new(),
            asset_browser_filter: String::new(),
            console_filter: String::new(),
            scene_layers: SceneLayerVisibility::default(),
            frame_input: UiInputFrame::default(),
            last_nav_drag_pos: None,
            last_fly_drag_pos: None,
            fly_latch: newengine_viewport::nav::FlyRmbLatch::default(),
            console_open: false,
            console_input: String::new(),
            hierarchy_drag_source: None,
            selected_entity_cached: None,
            insp_pos: [0.0; 3],
            insp_rot_deg: [0.0; 3],
            insp_scale: [1.0, 1.0, 1.0],
            insp_color: [0.85, 0.85, 0.9, 1.0],
            insp_material: MaterialId::invalid(),
            gizmo: EguiGizmo::new(),
            command_bus: EditorCommandBus::default(),
            editor: EditorState::new(),
            gizmo_was_dragging: false,
            gizmo_drag_begin: None,
            pending_pick: None,
        };

        me.ui_hub
            .register(Box::new(PluginManagerContributor::new(plugin_manager)));

        me
    }

    #[inline]
    #[allow(dead_code)]
    fn update_markup_vars(&mut self) {
        let entities = self.scene_bridge.scene().read().world().entity_count();
        self.markup_state
            .set_var("stats.entities", entities.to_string());

        let mut primitives = self.scene_bridge.primitives_snapshot();
        primitives.sort_by(|a, b| a.0.cmp(&b.0));

        let mut json = String::from("[");
        for (index, (name, id)) in primitives.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str("{\"value\":\"");
            json.push_str(&id.0.to_string());
            json.push_str("\",\"label\":\"");
            push_json_escaped(&mut json, name);
            json.push_str("\"}");
        }
        json.push(']');
        self.markup_state.set_var("editor.prims_options", json);

        if !self.markup_state.vars.contains_key("editor.primitive_sel") {
            if let Some((_, id)) = primitives.first() {
                self.markup_state
                    .set_var("editor.primitive_sel", id.0.to_string());
            }
        }

        if !self.markup_state.vars.contains_key("editor.light_sel") {
            self.markup_state.set_var("editor.light_sel", "point");
        }
    }

    #[inline]
    #[allow(dead_code)]
    fn dispatch_markup_actions(&mut self) {
        for event in self.markup_state.drain_events() {
            for action in event.actions.iter() {
                self.exec_markup_action(action);
            }
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn key_down(&self, key: u32) -> bool {
        self.frame_input.is_key_down(key)
    }

    #[inline]
    pub(crate) fn key_pressed(&self, key: u32) -> bool {
        self.frame_input.is_key_pressed(key)
    }

    #[inline]
    pub(crate) fn shift_down(&self) -> bool {
        self.key_down(ui_keys::SHIFT_LEFT) || self.key_down(ui_keys::SHIFT_RIGHT)
    }

    #[inline]
    pub(crate) fn command_down(&self) -> bool {
        self.key_down(ui_keys::CONTROL_LEFT) || self.key_down(ui_keys::CONTROL_RIGHT)
    }

    #[inline]
    pub(crate) fn command_pressed(&self, key: u32) -> bool {
        self.command_down() && self.key_pressed(key)
    }

    #[inline]
    pub(crate) fn play_mode_label(&self) -> &'static str {
        match self.scene_bridge.play_mode() {
            crate::gameplay::EditorPlayMode::Edit => "Edit",
            crate::gameplay::EditorPlayMode::Simulate => "Simulate",
            crate::gameplay::EditorPlayMode::Play => "Play",
        }
    }

    #[inline]
    pub(crate) fn active_tool_label(&self) -> &'static str {
        match self.editor.active_tool {
            newengine_editor_core::ToolId::Select => "Select",
            newengine_editor_core::ToolId::Translate => "Move",
            newengine_editor_core::ToolId::Rotate => "Rotate",
            newengine_editor_core::ToolId::Scale => "Scale",
        }
    }

    #[inline]
    pub(crate) fn surface_context(&self) -> schema::EditorSurfaceContext {
        schema::build_surface_context(self)
    }

    #[inline]
    pub(crate) fn snapped_position(&self, value: newengine_math::Vec3) -> newengine_math::Vec3 {
        if !self.transform_snap.translate_enabled || self.transform_snap.translate_step <= 0.0 {
            return value;
        }
        let step = self.transform_snap.translate_step;
        newengine_math::Vec3::new(
            snap_scalar(value.x, step),
            snap_scalar(value.y, step),
            snap_scalar(value.z, step),
        )
    }

    #[inline]
    pub(crate) fn snapped_rotation_ypr(
        &self,
        yaw: f32,
        pitch: f32,
        roll: f32,
    ) -> (f32, f32, f32) {
        if !self.transform_snap.rotate_enabled || self.transform_snap.rotate_step_deg <= 0.0 {
            return (yaw, pitch, roll);
        }
        let step = self.transform_snap.rotate_step_deg;
        (
            snap_scalar(yaw.to_degrees(), step).to_radians(),
            snap_scalar(pitch.to_degrees(), step).to_radians(),
            snap_scalar(roll.to_degrees(), step).to_radians(),
        )
    }

    #[inline]
    pub(crate) fn snapped_scale(&self, value: newengine_math::Vec3) -> newengine_math::Vec3 {
        if !self.transform_snap.scale_enabled || self.transform_snap.scale_step <= 0.0 {
            return value;
        }
        let step = self.transform_snap.scale_step;
        newengine_math::Vec3::new(
            snap_scalar(value.x, step).max(step),
            snap_scalar(value.y, step).max(step),
            snap_scalar(value.z, step).max(step),
        )
    }

    pub(crate) fn apply_transform_snap_to_inspector(&mut self) {
        let position = self.snapped_position(newengine_math::Vec3::new(
            self.insp_pos[0],
            self.insp_pos[1],
            self.insp_pos[2],
        ));
        self.insp_pos = [position.x, position.y, position.z];

        let (yaw, pitch, roll) = self.snapped_rotation_ypr(
            self.insp_rot_deg[0].to_radians(),
            self.insp_rot_deg[1].to_radians(),
            self.insp_rot_deg[2].to_radians(),
        );
        self.insp_rot_deg = [yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees()];

        let scale = self.snapped_scale(newengine_math::Vec3::new(
            self.insp_scale[0],
            self.insp_scale[1],
            self.insp_scale[2],
        ));
        self.insp_scale = [scale.x, scale.y, scale.z];
    }
}
