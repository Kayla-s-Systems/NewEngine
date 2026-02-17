#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;
use newengine_ui::markup::{UiMarkupDoc, UiState};
use newengine_ui::UiBuildFn;

use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_ecs::EntityId;
use newengine_gizmo::{GizmoAxis, GizmoMode};
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialId, MaterialRef};
use newengine_primitives::Primitive;
use newengine_scene::components::Name;
use newengine_transform::Transform;
use newengine_viewport::Viewport;

use crate::plugin_manager::PluginManagerUi;
use crate::scene_bridge::{GridSettings, SceneBridge};
use crate::viewport_bridge::ViewportBridge;

#[derive(Clone, Copy, Debug, PartialEq)]
struct GizmoDrag {
    mode: GizmoMode,
    axis: GizmoAxis,
    start_mouse: egui::Pos2,
    start_pos: newengine_math::Vec3,
    start_rot: newengine_math::Quat,
    start_scale: newengine_math::Vec3,
    ndc_z: f32,
}

/// Minimal editor UI: foundation-first.
///
/// - Viewport is the only primary panel.
/// - Console is hidden by default.
pub struct EditorUiBuild {
    shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
    state: UiState,

    viewport: Viewport,

    viewport_bridge: Arc<ViewportBridge>,
    scene_bridge: Arc<SceneBridge>,
    plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
    plugin_manager: PluginManagerUi,

    // Orbit interaction (UI-driven, not via global input plugin).
    last_drag_pos: Option<egui::Pos2>,

    console_open: bool,
    console_input: String,

    // Primitives UI.
    selected_primitive: Option<newengine_primitives::PrimitiveId>,

    // Selection + inspector cache.
    selected_entity_cached: Option<EntityId>,
    insp_pos: [f32; 3],
    insp_rot_deg: [f32; 3],
    insp_scale: [f32; 3],
    insp_color: [f32; 4],
    insp_material: MaterialId,

    gizmo_mode: GizmoMode,
    gizmo_drag: Option<GizmoDrag>,
}


fn infer_model_exts(snap: &newengine_core::plugins::PluginsSnapshot) -> Vec<String> {
    use std::collections::BTreeSet;

    // Best-effort extraction based on declared plugin capabilities.
    // Importer plugins are expected to expose capability ids that contain format tokens.
    let mut out: BTreeSet<String> = BTreeSet::new();
    let tokens: [(&str, &[&str]); 8] = [
        ("obj", &["obj"]),
        ("gltf", &["gltf"]),
        ("glb", &["glb"]),
        ("fbx", &["fbx"]),
        ("dae", &["dae", "collada"]),
        ("stl", &["stl"]),
        ("ply", &["ply"]),
        ("blend", &["blend"]),
    ];

    for p in &snap.plugins {
        for c in &p.capabilities {
            let id = c.id.to_ascii_lowercase();
            for (ext, keys) in tokens {
                if keys.iter().any(|k| id.contains(k)) {
                    out.insert(format!(".{ext}"));
                }
            }
        }
    }

    out.into_iter().collect()
}

impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
        scene_bridge: Arc<SceneBridge>,
    ) -> Self {
        let cam = scene_bridge
            .scene()
            .read()
            .active_camera()
            .expect("scene has no active camera");
        let viewport = Viewport::new(Some(cam));

        Self {
            shared_doc,
            state: UiState::default(),
            viewport,
            viewport_bridge,
            scene_bridge,
            plugins_bridge: Arc::clone(&plugins_bridge),
            plugin_manager: PluginManagerUi::new(plugins_bridge),
            last_drag_pos: None,
            console_open: false,
            console_input: String::new(),

            selected_primitive: None,

            selected_entity_cached: None,
            insp_pos: [0.0; 3],
            insp_rot_deg: [0.0; 3],
            insp_scale: [1.0, 1.0, 1.0],
            insp_color: [0.85, 0.85, 0.9, 1.0],
            insp_material: MaterialId::invalid(),

            gizmo_mode: GizmoMode::Translate,
            gizmo_drag: None,
        }
    }

    #[inline]
    fn axis_vec(axis: GizmoAxis) -> newengine_math::Vec3 {
        axis.vec3()
    }

    #[inline]
    fn axis_color(axis: GizmoAxis) -> egui::Color32 {
        match axis {
            GizmoAxis::X => egui::Color32::from_rgb(220, 70, 70),
            GizmoAxis::Y => egui::Color32::from_rgb(80, 210, 110),
            GizmoAxis::Z => egui::Color32::from_rgb(80, 140, 255),
        }
    }

    #[inline]
    fn world_to_screen(
        frame: &crate::viewport_bridge::ViewportCameraFrame,
        rect: egui::Rect,
        world: newengine_math::Vec3,
    ) -> Option<(egui::Pos2, f32)> {
        let v = frame.viewproj * newengine_math::Vec4::new(world.x, world.y, world.z, 1.0);
        if !v.w.is_finite() || v.w.abs() < 1e-6 {
            return None;
        }
        let ndc = v / v.w;
        if !ndc.x.is_finite() || !ndc.y.is_finite() || !ndc.z.is_finite() {
            return None;
        }
        // Map NDC [-1..1] to viewport pixels, then to egui points.
        let sx_px = (ndc.x * 0.5 + 0.5) * frame.vp_w as f32;
        let sy_px = (ndc.y * 0.5 + 0.5) * frame.vp_h as f32;

        let ppp = (rect.width() / frame.vp_w as f32).max(1e-6);
        let x_pt = rect.min.x + sx_px * ppp;
        let y_pt = rect.min.y + sy_px * ppp;
        Some((egui::pos2(x_pt, y_pt), ndc.z))
    }

    #[inline]
    fn screen_to_world_at_ndc_z(
        frame: &crate::viewport_bridge::ViewportCameraFrame,
        rect: egui::Rect,
        screen: egui::Pos2,
        ndc_z: f32,
    ) -> newengine_math::Vec3 {
        let ppp = (rect.width() / frame.vp_w as f32).max(1e-6);
        let px = ((screen.x - rect.min.x) / ppp).clamp(0.0, frame.vp_w as f32);
        let py = ((screen.y - rect.min.y) / ppp).clamp(0.0, frame.vp_h as f32);

        let x = (px / frame.vp_w as f32) * 2.0 - 1.0;
        let y = (py / frame.vp_h as f32) * 2.0 - 1.0;

        let h = frame.inv_viewproj * newengine_math::Vec4::new(x, y, ndc_z, 1.0);
        if h.w.abs() < 1e-6 {
            return newengine_math::Vec3::ZERO;
        }
        (h / h.w).truncate()
    }

    fn read_selected_pose(&self, e: EntityId) -> Option<(newengine_math::Vec3, newengine_math::Quat, newengine_math::Vec3, Option<[f32; 4]>)> {
        let scene = self.scene_bridge.scene();
        let s = scene.read();
        let w = s.world();
        let t = w.get::<Transform>(e)?;
        let color = w.get::<Primitive>(e).map(|p| p.color);
        Some((t.position, t.rotation, t.scale, color))
    }

    fn draw_selection_outline(
        &self,
        painter: &egui::Painter,
        frame: &crate::viewport_bridge::ViewportCameraFrame,
        rect: egui::Rect,
        pos: newengine_math::Vec3,
        rot: newengine_math::Quat,
        scale: newengine_math::Vec3,
    ) {
        // Approximate a box in local space and project its edges.
        let hx = 0.5 * scale.x.abs().max(0.001);
        let hy = 0.5 * scale.y.abs().max(0.001);
        let hz = 0.5 * scale.z.abs().max(0.001);
        let corners_local = [
            newengine_math::Vec3::new(-hx, -hy, -hz),
            newengine_math::Vec3::new(hx, -hy, -hz),
            newengine_math::Vec3::new(hx, -hy, hz),
            newengine_math::Vec3::new(-hx, -hy, hz),
            newengine_math::Vec3::new(-hx, hy, -hz),
            newengine_math::Vec3::new(hx, hy, -hz),
            newengine_math::Vec3::new(hx, hy, hz),
            newengine_math::Vec3::new(-hx, hy, hz),
        ];

        let mut pts: [Option<egui::Pos2>; 8] = [None; 8];
        for (i, c) in corners_local.iter().enumerate() {
            let wpos = pos + (rot * *c);
            pts[i] = Self::world_to_screen(frame, rect, wpos).map(|x| x.0);
        }

        let edges: &[(usize, usize)] = &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];

        let stroke_outer = egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120));
        let stroke_inner = egui::Stroke::new(1.5, egui::Color32::from_rgb(235, 210, 90));

        for (a, b) in edges {
            let (Some(pa), Some(pb)) = (pts[*a], pts[*b]) else { continue; };
            painter.line_segment([pa, pb], stroke_outer);
            painter.line_segment([pa, pb], stroke_inner);
        }
    }

    fn gizmo_pick_axis(
        &self,
        center: egui::Pos2,
        x_end: egui::Pos2,
        y_end: egui::Pos2,
        z_end: egui::Pos2,
        mouse: egui::Pos2,
    ) -> Option<GizmoAxis> {
        fn dist_to_seg(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
            let ab = b - a;
            let ap = p - a;
            let ab2 = ab.x * ab.x + ab.y * ab.y;
            if ab2 <= 1e-6 {
                return ap.length();
            }
            let t = ((ap.x * ab.x + ap.y * ab.y) / ab2).clamp(0.0, 1.0);
            let q = a + ab * t;
            (p - q).length()
        }

        let dx = dist_to_seg(mouse, center, x_end);
        let dy = dist_to_seg(mouse, center, y_end);
        let dz = dist_to_seg(mouse, center, z_end);

        let (axis, d) = if dx <= dy && dx <= dz {
            (GizmoAxis::X, dx)
        } else if dy <= dz {
            (GizmoAxis::Y, dy)
        } else {
            (GizmoAxis::Z, dz)
        };

        if d <= 10.0 {
            Some(axis)
        } else {
            None
        }
    }


    fn ui_toolbar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("toolbar")
            .resizable(false)
            .exact_width(56.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);

                ui.vertical_centered(|ui| {
                    ui.label("Tools");
                });
                ui.separator();
                ui.add_space(4.0);

                let button = |ui: &mut egui::Ui, label: &str, active: bool| -> egui::Response {
                    let mut b = egui::Button::new(label).min_size(egui::vec2(44.0, 36.0));
                    if active {
                        b = b.fill(ui.visuals().selection.bg_fill);
                    }
                    ui.add(b)
                };

                // (Select mode will be added when we have object picking under mouse)
                ui.vertical(|ui| {
                    if button(ui, "W", self.gizmo_mode == GizmoMode::Translate)
                        .on_hover_text("Move (W)")
                        .clicked()
                    {
                        self.gizmo_mode = GizmoMode::Translate;
                        self.gizmo_drag = None;
                    }

                    if button(ui, "E", self.gizmo_mode == GizmoMode::Rotate)
                        .on_hover_text("Rotate (E)")
                        .clicked()
                    {
                        self.gizmo_mode = GizmoMode::Rotate;
                        self.gizmo_drag = None;
                    }

                    if button(ui, "R", self.gizmo_mode == GizmoMode::Scale)
                        .on_hover_text("Scale (R)")
                        .clicked()
                    {
                        self.gizmo_mode = GizmoMode::Scale;
                        self.gizmo_drag = None;
                    }
                });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("NewEngine");
                });
            });
    }

    fn ui_hierarchy(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("hierarchy")
            .resizable(true)
            .default_width(240.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Hierarchy");
                ui.add_space(6.0);

                let selected = self.scene_bridge.selection();

                let scene = self.scene_bridge.scene();
                let world = scene.read();
                let w = world.world();

                // Deterministic list: sort by name then by id.
                let mut items: Vec<(String, EntityId, bool)> = Vec::new();
                for (e, name) in w.query::<Name>() {
                    let has_prim = w.get::<Primitive>(e).is_some();
                    items.push((name.as_str().to_string(), e, has_prim));
                }
                items.sort_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then_with(|| a.1.stable_u64().cmp(&b.1.stable_u64()))
                });

                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    for (name, e, has_prim) in items {
                        let mut label = name;
                        if has_prim {
                            label.push_str("  [Prim]");
                        }

                        let is_sel = selected == Some(e);
                        if ui.selectable_label(is_sel, label).clicked() {
                            self.scene_bridge.set_selection(Some(e));
                        }
                    }
                });

                ui.separator();
                if ui.button("Deselect").clicked() {
                    self.scene_bridge.set_selection(None);
                }
            });
    }

    fn refresh_inspector_cache(&mut self, entity: EntityId) {
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

    fn ui_inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .resizable(true)
            .default_width(300.0)
            .min_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                ui.add_space(6.0);

                // Viewport / Grid (editor-only).
                {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Viewport").strong());
                        ui.add_space(4.0);

                        let mut gs: GridSettings = self.scene_bridge.grid_settings();
                        let mut changed = false;

                        changed |= ui.checkbox(&mut gs.auto_spacing, "Auto grid spacing").changed();

                        ui.add_enabled_ui(!gs.auto_spacing, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Spacing");
                                changed |= ui.add(egui::DragValue::new(&mut gs.spacing).speed(0.05).clamp_range(0.001..=10_000.0)).changed();
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label("Extent");
                            changed |= ui.add(egui::DragValue::new(&mut gs.half_lines).speed(1).clamp_range(8..=4096)).changed();
                            ui.label("half-lines");
                        });

                        ui.horizontal(|ui| {
                            ui.label("Major every");
                            changed |= ui.add(egui::DragValue::new(&mut gs.major_every).speed(1).clamp_range(1..=256)).changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Minor");
                            changed |= ui.color_edit_button_rgba_unmultiplied(&mut gs.minor_color).changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Major");
                            changed |= ui.color_edit_button_rgba_unmultiplied(&mut gs.major_color).changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Background");
                            changed |= ui.color_edit_button_rgba_unmultiplied(&mut gs.background_color).changed();
                        });

                        if changed {
                            self.scene_bridge.set_grid_settings(gs);
                        }
                    });
                    ui.add_space(8.0);
                }



                let selected = self.scene_bridge.selection();
                let Some(e) = selected else {
                    ui.label("No selection.");
                    return;
                };

                if self.selected_entity_cached != Some(e) {
                    self.refresh_inspector_cache(e);
                }

                // Name
                {
                    let scene = self.scene_bridge.scene();
                    let s = scene.read();
                    let w = s.world();
                    let name = w.get::<Name>(e).map(|n| n.as_str()).unwrap_or("<unnamed>");
                    ui.label(format!("Entity: {}", name));
                }

                ui.separator();

                // Transform
                ui.collapsing("Transform", |ui| {
                    let mut changed = false;

                    ui.horizontal(|ui| {
                        ui.label("Position");
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_pos[0]).speed(0.05)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_pos[1]).speed(0.05)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_pos[2]).speed(0.05)).changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rotation (deg)");
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_rot_deg[0]).speed(0.25)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_rot_deg[1]).speed(0.25)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_rot_deg[2]).speed(0.25)).changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label("Scale");
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_scale[0]).speed(0.05)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_scale[1]).speed(0.05)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut self.insp_scale[2]).speed(0.05)).changed();
                    });

                    if changed {
                        let pos = newengine_math::Vec3::new(self.insp_pos[0], self.insp_pos[1], self.insp_pos[2]);
                        let ypr = (
                            self.insp_rot_deg[0].to_radians(),
                            self.insp_rot_deg[1].to_radians(),
                            self.insp_rot_deg[2].to_radians(),
                        );
                        let scale = newengine_math::Vec3::new(self.insp_scale[0], self.insp_scale[1], self.insp_scale[2]);
                        self.scene_bridge.cmd_set_transform(e, pos, ypr, scale);
                    }
                });

                // Primitive
                ui.collapsing("Primitive", |ui| {
                    let scene = self.scene_bridge.scene();
                    let s = scene.read();
                    let w = s.world();
                    let prim = w.get::<Primitive>(e);

                    if let Some(p) = prim {
                        let reg = self.scene_bridge.primitives();
                        let reg = reg.read();
                        let prim_name = reg.name(p.id).unwrap_or("<unknown>");
                        ui.label(format!("Kind: {}", prim_name));

                        let mut rgba = self.insp_color;
                        let changed = ui
                            .color_edit_button_rgba_unmultiplied(&mut rgba)
                            .changed();
                        if changed {
                            self.insp_color = rgba;
                            self.scene_bridge.cmd_set_primitive_color(e, rgba);
                        }
                    } else {
                        ui.label("(no Primitive component)");
                    }
                });

                // Materials (foundation step).
                ui.collapsing("Material", |ui| {
                    let mats = self.scene_bridge.materials_snapshot();
                    let current_label = mats
                        .iter()
                        .find(|x| x.1 == self.insp_material)
                        .map(|x| x.0.as_str())
                        .unwrap_or("<none>");

                    egui::ComboBox::from_id_source("material_combo")
                        .width(180.0)
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for (name, id) in &mats {
                                ui.selectable_value(&mut self.insp_material, *id, name);
                            }
                        });

                    if self.insp_material != MaterialId::invalid() {
                        // Assign to entity on change.
                        // (We keep it immediate; later this becomes a transaction/undo entry.)
                        let scene = self.scene_bridge.scene();
                        let s = scene.read();
                        let w = s.world();
                        let current = w.get::<MaterialRef>(e).map(|mr| mr.id).unwrap_or(MaterialId::invalid());
                        if current != self.insp_material {
                            self.scene_bridge.cmd_set_material(e, self.insp_material);
                        }

                        // Edit material parameters (shared in registry).
                        let reg = self.scene_bridge.materials();
                        let reg = reg.read();
                        if let Some(mut desc) = reg.get(self.insp_material) {
                            let mut changed = false;

                            ui.horizontal(|ui| {
                                ui.label("Base color");
                                changed |= ui
                                    .color_edit_button_rgba_unmultiplied(&mut desc.base_color)
                                    .changed();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Metallic");
                                changed |= ui
                                    .add(egui::Slider::new(&mut desc.metallic, 0.0..=1.0))
                                    .changed();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Roughness");
                                changed |= ui
                                    .add(egui::Slider::new(&mut desc.roughness, 0.02..=1.0))
                                    .changed();
                            });

                            if changed {
                                self.scene_bridge.cmd_update_material(self.insp_material, desc);
                            }
                        } else {
                            ui.label("(material not found in registry)");
                        }
                    } else {
                        ui.label("(no Material assigned)");
                    }
                });
            });
    }

    fn ui_topbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("NewEngine Editor (Foundation)");
                ui.separator();

                let entities = self.scene_bridge.scene().read().world().entity_count();
                ui.label(format!("entities: {entities}"));

                ui.separator();

                if ui.button("New Scene").clicked() {
                    self.scene_bridge.cmd_new_scene();
                }

                // Dynamic primitives dropdown (registry-driven).
                {
                    let prims = self.scene_bridge.primitives_snapshot();

                    if self.selected_primitive.is_none() {
                        self.selected_primitive = prims.first().map(|p| p.1);
                    }

                    // If selected primitive was removed/unregistered, fall back to first.
                    if let Some(sel) = self.selected_primitive {
                        if !prims.iter().any(|p| p.1 == sel) {
                            self.selected_primitive = prims.first().map(|p| p.1);
                        }
                    }

                    let current_label = self
                        .selected_primitive
                        .and_then(|id| prims.iter().find(|x| x.1 == id).map(|x| x.0.as_str()))
                        .unwrap_or("<none>");

                    egui::ComboBox::from_id_source("add_primitive_combo")
                        .width(160.0)
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for (name, id) in &prims {
                                ui.selectable_value(&mut self.selected_primitive, Some(*id), name);
                            }
                        });

                    if ui.button("Add").clicked() {
                        if let Some(id) = self.selected_primitive {
                            let name = prims
                                .iter()
                                .find(|x| x.1 == id)
                                .map(|x| x.0.clone())
                                .unwrap_or_else(|| "Primitive".to_string());

                            // Spawn slightly above floor to avoid z-fighting and keep it visible.
                            self.scene_bridge
                                .cmd_spawn_primitive(id, name, newengine_math::Vec3::new(0.0, 0.5, 0.0));
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.plugin_manager.topbar_button(ui);
                    if ui.button("Console").clicked() {
                        self.console_open = !self.console_open;
                    }
                });
            });
        });
    }

    fn ui_viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());

            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_rgb(12, 12, 14));

            ui.painter().rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
                egui::StrokeKind::Inside,
            );

            let ppp = ctx.pixels_per_point().max(0.0001);
            let px_w = (rect.width() * ppp).round().max(1.0) as u32;
            let px_h = (rect.height() * ppp).round().max(1.0) as u32;

            self.viewport.set_extent(px_w, px_h);
            self.viewport_bridge.publish_extent(px_w, px_h);

            let shift = ctx.input(|i| i.modifiers.shift);
            let nav_rotate = resp.dragged_by(egui::PointerButton::Middle) && !shift;
            let nav_pan = resp.dragged_by(egui::PointerButton::Middle) && shift;
            let nav_drag = nav_rotate || nav_pan;

            let active = resp.hovered() || nav_drag;

            // Gizmo hotkeys:
            // - W/E/R (industry standard) when RMB is NOT held (RMB is reserved for camera navigation)
            // - 1/2/3 as an always-available fallback.
            if active && !ctx.wants_keyboard_input() {
                let rmb = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
                if !rmb {
                    ctx.input(|i| {
                        if i.key_pressed(egui::Key::W) || i.key_pressed(egui::Key::Num1) {
                            self.gizmo_mode = GizmoMode::Translate;
                            self.gizmo_drag = None;
                        }
                        if i.key_pressed(egui::Key::E) || i.key_pressed(egui::Key::Num2) {
                            self.gizmo_mode = GizmoMode::Rotate;
                            self.gizmo_drag = None;
                        }
                        if i.key_pressed(egui::Key::R) || i.key_pressed(egui::Key::Num3) {
                            self.gizmo_mode = GizmoMode::Scale;
                            self.gizmo_drag = None;
                        }
                    });
                } else {
                    // Still allow 1/2/3 while navigating.
                    ctx.input(|i| {
                        if i.key_pressed(egui::Key::Num1) {
                            self.gizmo_mode = GizmoMode::Translate;
                            self.gizmo_drag = None;
                        }
                        if i.key_pressed(egui::Key::Num2) {
                            self.gizmo_mode = GizmoMode::Rotate;
                            self.gizmo_drag = None;
                        }
                        if i.key_pressed(egui::Key::Num3) {
                            self.gizmo_mode = GizmoMode::Scale;
                            self.gizmo_drag = None;
                        }
                    });
                }
            }

            // Determine whether gizmo wants to capture input this frame (prevents orbit/selection conflicts).
            let mut gizmo_capture_now = self.gizmo_drag.is_some();
            if !gizmo_capture_now {
                if let (Some(frame), Some(e)) = (self.viewport_bridge.read_camera_frame(), self.scene_bridge.selection()) {
                    if let Some((pos, rot, _scale, _)) = self.read_selected_pose(e) {
                        if let Some((center, _ndc_z)) = Self::world_to_screen(&frame, rect, pos) {
                            let desired_len = 72.0;
                            let axis_end = |axis: GizmoAxis| -> egui::Pos2 {
                                let dir_world = (rot * Self::axis_vec(axis)).normalize_or_zero();
                                let unit = pos + dir_world;
                                let Some((unit_s, _)) = Self::world_to_screen(&frame, rect, unit) else {
                                    return center;
                                };
                                let d = (unit_s - center).length().max(1.0);
                                let len_world = desired_len / d;
                                let end_world = pos + dir_world * len_world;
                                Self::world_to_screen(&frame, rect, end_world)
                                    .map(|x| x.0)
                                    .unwrap_or(center)
                            };
                            let x_end = axis_end(GizmoAxis::X);
                            let y_end = axis_end(GizmoAxis::Y);
                            let z_end = axis_end(GizmoAxis::Z);
                            if let Some(m) = ctx.input(|i| i.pointer.interact_pos()) {
                                if rect.contains(m) {
                                    let hovered_axis = self.gizmo_pick_axis(center, x_end, y_end, z_end, m);
                                    if hovered_axis.is_some() && ctx.input(|i| i.pointer.primary_down()) {
                                        gizmo_capture_now = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Click-to-select (picking handled on render thread).
            // Suppress selection when the gizmo captures the interaction.
            if resp.clicked_by(egui::PointerButton::Primary) && !nav_drag && !gizmo_capture_now {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let local = pos - rect.min;
                    let ppp = ctx.pixels_per_point().max(0.0001);
                    let x_px = (local.x * ppp).clamp(0.0, rect.width() * ppp);
                    let y_px = (local.y * ppp).clamp(0.0, rect.height() * ppp);
                    self.viewport_bridge.publish_pick_request(x_px, y_px);
                }
            }

            let mut dx_px = 0.0f32;
            let mut dy_px = 0.0f32;
            if nav_drag {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if let Some(prev) = self.last_drag_pos {
                        let d = pos - prev;
                        dx_px = d.x * ppp;
                        dy_px = d.y * ppp;
                    }
                    self.last_drag_pos = Some(pos);
                }
            } else {
                self.last_drag_pos = None;
            }

            let wheel_y_points = if active {
                ctx.input(|i| i.raw_scroll_delta.y)
            } else {
                0.0
            };

            let wheel_y = (wheel_y_points / 240.0).clamp(-2.0, 2.0);
            // Drag & drop models onto the viewport.
            // The engine-side asset pipeline decides which formats are supported; we display a best-effort
            // list from registered plugin capabilities.
            let snap = self.plugins_bridge.read();
            let exts = infer_model_exts(&snap);

            if active {
                let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
                for f in dropped {
                    if let Some(path) = f.path {
                        let p = path.display().to_string();
                        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                        let dot_ext = if ext.is_empty() { String::new() } else { format!(".{ext}") };

                        if !dot_ext.is_empty() && (exts.is_empty() || exts.iter().any(|e| e == &dot_ext)) {
                            log::warn!("model drop is currently disabled (no asset->scene contract yet): '{}'", p);
                        } else {
                            log::warn!("dropped file has unsupported extension: '{}'", p);
                        }
                    }
                }
            }


            let look_drag = nav_rotate && !gizmo_capture_now;
            let pan_drag = nav_pan && !gizmo_capture_now;
            // UI busy flag is critical for renderer-side camera framing logic.
            // When the user manipulates an object with the gizmo, we must treat the camera
            // as "user busy" to prevent auto-framing from moving the orbit pivot, which
            // makes the world grid appear to move while transforming.
            let ui_busy = self.gizmo_drag.is_some();
            self.viewport_bridge
                .publish_orbit_input(dx_px, dy_px, wheel_y, active, look_drag, pan_drag, ui_busy);

            let wants_kb = ctx.wants_keyboard_input();
            let mut move_mask: u64 = 0;

            // Explicit framing (Blender-like): press F while the viewport is active.
            // This is intentionally explicit to keep the world reference stable while editing.
            if active && !wants_kb && ctx.input(|i| i.key_pressed(egui::Key::F)) {
                self.viewport_bridge.publish_frame_request();
            }

            let rmb = active && ctx.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
            if rmb && !wants_kb {
                ctx.input(|i| {
                    if i.key_down(egui::Key::W) {
                        move_mask |= 1 << 0;
                    }
                    if i.key_down(egui::Key::A) {
                        move_mask |= 1 << 1;
                    }
                    if i.key_down(egui::Key::S) {
                        move_mask |= 1 << 2;
                    }
                    if i.key_down(egui::Key::D) {
                        move_mask |= 1 << 3;
                    }
                    if i.key_down(egui::Key::Q) {
                        move_mask |= 1 << 4;
                    }
                    if i.key_down(egui::Key::E) {
                        move_mask |= 1 << 5;
                    }
                    if i.modifiers.shift {
                        move_mask |= 1 << 6;
                    }
                });
            }

            self.viewport_bridge.publish_move_keys(move_mask);

            let tex_user = self.viewport_bridge.read_tex_user();

            ui.allocate_ui_at_rect(rect, |ui| {
                if tex_user != 0 {
                    let tid = egui::TextureId::User(tex_user);
                    let st = egui::load::SizedTexture::new(tid, rect.size());
                    ui.add(egui::Image::new(st).fit_to_exact_size(rect.size()));
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Viewport: waiting for render target...");
                    });
                }

                // Viewport overlay: supported model extensions.
                if active {
                    let snap = self.plugins_bridge.read();
                    let exts = infer_model_exts(&snap);
                    if !exts.is_empty() {
                        let msg = format!("Drop model: {}", exts.join(", "));
                        let pos = rect.left_bottom() + egui::vec2(8.0, -8.0);
                        ui.painter().text(
                            pos,
                            egui::Align2::LEFT_BOTTOM,
                            msg,
                            egui::FontId::monospace(12.0),
                            egui::Color32::from_gray(140),
                        );
                    }
                }

                // Viewport overlay: selection highlight + gizmo (renderer publishes camera matrices).
                let frame = self.viewport_bridge.read_camera_frame();
                let selected = self.scene_bridge.selection();
                if let (Some(frame), Some(e)) = (frame, selected) {
                    if let Some((pos, rot, scale, _color)) = self.read_selected_pose(e) {
                        // Outline.
                        self.draw_selection_outline(ui.painter(), &frame, rect, pos, rot, scale);

                        // Gizmo.
                        if let Some((center, ndc_z)) = Self::world_to_screen(&frame, rect, pos) {
                            let desired_len = 72.0;

                            let axis_end = |axis: GizmoAxis| -> egui::Pos2 {
                                let dir_world = (rot * Self::axis_vec(axis)).normalize_or_zero();
                                let unit = pos + dir_world;
                                let Some((unit_s, _)) = Self::world_to_screen(&frame, rect, unit) else {
                                    return center;
                                };
                                let d = (unit_s - center).length().max(1.0);
                                let len_world = desired_len / d;
                                let end_world = pos + dir_world * len_world;
                                Self::world_to_screen(&frame, rect, end_world)
                                    .map(|x| x.0)
                                    .unwrap_or(center)
                            };

                            let x_end = axis_end(GizmoAxis::X);
                            let y_end = axis_end(GizmoAxis::Y);
                            let z_end = axis_end(GizmoAxis::Z);

                            let mouse = ctx.input(|i| i.pointer.hover_pos());

                            // Determine hovered axis.
                            let mut hovered_axis: Option<GizmoAxis> = None;
                            if let Some(m) = mouse {
                                if rect.contains(m) {
                                    hovered_axis = self.gizmo_pick_axis(center, x_end, y_end, z_end, m);
                                }
                            }

                            // Drag start.
                            let just_pressed = resp.drag_started_by(egui::PointerButton::Primary);
                            if self.gizmo_drag.is_none() && just_pressed {
                                if let Some(axis) = hovered_axis {
                                    if let Some(m) = resp.interact_pointer_pos() {
                                        self.gizmo_drag = Some(GizmoDrag {
                                            mode: self.gizmo_mode,
                                            axis,
                                            start_mouse: m,
                                            start_pos: pos,
                                            start_rot: rot,
                                            start_scale: scale,
                                            ndc_z,
                                        });
                                    }
                                }
                            }

                            // Drag update.
                            if let Some(drag) = self.gizmo_drag {
                                let lmb_down = ctx.input(|i| i.pointer.primary_down());
                                if !lmb_down {
                                    self.gizmo_drag = None;
                                } else if let Some(m) = ctx.input(|i| i.pointer.interact_pos()) {
                                    // Axis vectors in world space.
                                    let axis_world = match drag.axis {
                                        GizmoAxis::X => drag.start_rot * newengine_math::Vec3::X,
                                        GizmoAxis::Y => drag.start_rot * newengine_math::Vec3::Y,
                                        GizmoAxis::Z => drag.start_rot * newengine_math::Vec3::Z,
                                    };
                                    let axis_world = axis_world.normalize_or_zero();

                                    match drag.mode {
                                        GizmoMode::Translate => {
                                            let ws0 = Self::screen_to_world_at_ndc_z(&frame, rect, drag.start_mouse, drag.ndc_z);
                                            let ws1 = Self::screen_to_world_at_ndc_z(&frame, rect, m, drag.ndc_z);
                                            let delta = (ws1 - ws0).dot(axis_world);
                                            let new_pos = drag.start_pos + axis_world * delta;

                                            self.insp_pos = [new_pos.x, new_pos.y, new_pos.z];
                                            let (y, p, r) = (drag.start_rot).to_euler(newengine_math::EulerRot::YXZ);
                                            self.insp_rot_deg = [y.to_degrees(), p.to_degrees(), r.to_degrees()];
                                            self.insp_scale = [drag.start_scale.x, drag.start_scale.y, drag.start_scale.z];

                                            self.scene_bridge.cmd_set_transform(
                                                e,
                                                new_pos,
                                                (y, p, r),
                                                drag.start_scale,
                                            );
                                        }
                                        GizmoMode::Scale => {
                                            let ws0 = Self::screen_to_world_at_ndc_z(&frame, rect, drag.start_mouse, drag.ndc_z);
                                            let ws1 = Self::screen_to_world_at_ndc_z(&frame, rect, m, drag.ndc_z);
                                            let delta = (ws1 - ws0).dot(axis_world);

                                            let mut new_scale = drag.start_scale;
                                            match drag.axis {
                                                GizmoAxis::X => new_scale.x = (new_scale.x + delta).max(0.001),
                                                GizmoAxis::Y => new_scale.y = (new_scale.y + delta).max(0.001),
                                                GizmoAxis::Z => new_scale.z = (new_scale.z + delta).max(0.001),
                                            }

                                            self.insp_pos = [drag.start_pos.x, drag.start_pos.y, drag.start_pos.z];
                                            let (y, p, r) = (drag.start_rot).to_euler(newengine_math::EulerRot::YXZ);
                                            self.insp_rot_deg = [y.to_degrees(), p.to_degrees(), r.to_degrees()];
                                            self.insp_scale = [new_scale.x, new_scale.y, new_scale.z];

                                            self.scene_bridge.cmd_set_transform(
                                                e,
                                                drag.start_pos,
                                                (y, p, r),
                                                new_scale,
                                            );
                                        }
                                        GizmoMode::Rotate => {
                                            // Screen-space rotation around the gizmo center.
                                            let v0 = drag.start_mouse - center;
                                            let v1 = m - center;
                                            let a0 = v0.y.atan2(v0.x);
                                            let a1 = v1.y.atan2(v1.x);
                                            let mut da = a1 - a0;
                                            // Wrap to [-pi..pi].
                                            while da > core::f32::consts::PI {
                                                da -= 2.0 * core::f32::consts::PI;
                                            }
                                            while da < -core::f32::consts::PI {
                                                da += 2.0 * core::f32::consts::PI;
                                            }

                                            let q = newengine_math::Quat::from_axis_angle(axis_world, da);
                                            let new_rot = q * drag.start_rot;
                                            let (y, p, r) = new_rot.to_euler(newengine_math::EulerRot::YXZ);

                                            self.insp_pos = [drag.start_pos.x, drag.start_pos.y, drag.start_pos.z];
                                            self.insp_rot_deg = [y.to_degrees(), p.to_degrees(), r.to_degrees()];
                                            self.insp_scale = [drag.start_scale.x, drag.start_scale.y, drag.start_scale.z];

                                            self.scene_bridge.cmd_set_transform(
                                                e,
                                                drag.start_pos,
                                                (y, p, r),
                                                drag.start_scale,
                                            );
                                        }
                                    }
                                }
                            }

                            // Draw gizmo with mode-specific handles.
                            let active_axis = self.gizmo_drag.map(|d| d.axis);
                            let painter = ui.painter();

                            // Helpers (2D overlay; true AAA 3D gizmo comes via overlay pipeline).
                            let draw_arrow = |p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: egui::Stroke| {
                                p.line_segment([a, b], stroke);

                                let dir = (b - a);
                                let len = dir.length().max(1.0);
                                let n = dir / len;
                                let perp = egui::vec2(-n.y, n.x);

                                let tip = b;
                                let back = b - n * 10.0;
                                let l = back + perp * 5.0;
                                let r = back - perp * 5.0;

                                p.add(egui::Shape::convex_polygon(vec![tip, l, r], stroke.color, egui::Stroke::NONE));
                            };

                            let draw_cube = |p: &egui::Painter, b: egui::Pos2, col: egui::Color32, w: f32| {
                                let s = 8.0 + (w - 2.0).clamp(0.0, 2.0);
                                let rect = egui::Rect::from_center_size(b, egui::vec2(s, s));
                                p.rect_filled(rect, 1.0, col);
                            };

                            let draw_ring = |p: &egui::Painter, c: egui::Pos2, radius: f32, stroke: egui::Stroke| {
                                p.circle_stroke(c, radius, stroke);
                            };

                            match self.gizmo_mode {
                                GizmoMode::Rotate => {
                                    let r0 = desired_len * 0.78;
                                    let r1 = desired_len * 0.70;
                                    let r2 = desired_len * 0.62;
                                    for (axis, radius) in [
                                        (GizmoAxis::X, r0),
                                        (GizmoAxis::Y, r1),
                                        (GizmoAxis::Z, r2),
                                    ] {
                                        let mut stroke = egui::Stroke::new(2.0, Self::axis_color(axis));
                                        if Some(axis) == hovered_axis {
                                            stroke.width = 3.0;
                                        }
                                        if Some(axis) == active_axis {
                                            stroke.width = 4.0;
                                        }
                                        draw_ring(painter, center, radius, stroke);
                                    }
                                }
                                GizmoMode::Translate | GizmoMode::Scale => {
                                    for (axis, end) in [
                                        (GizmoAxis::X, x_end),
                                        (GizmoAxis::Y, y_end),
                                        (GizmoAxis::Z, z_end),
                                    ] {
                                        let mut stroke = egui::Stroke::new(2.0, Self::axis_color(axis));
                                        if Some(axis) == hovered_axis {
                                            stroke.width = 3.0;
                                        }
                                        if Some(axis) == active_axis {
                                            stroke.width = 4.0;
                                        }

                                        match self.gizmo_mode {
                                            GizmoMode::Translate => draw_arrow(painter, center, end, stroke),
                                            GizmoMode::Scale => {
                                                painter.line_segment([center, end], stroke);
                                                draw_cube(painter, end, stroke.color, stroke.width);
                                            }
                                            GizmoMode::Rotate => {}
                                        }
                                    }
                                }
                            }

                            // Mode hint.
                            let mode_txt = match self.gizmo_mode {
                                GizmoMode::Translate => "Gizmo: Translate (1)",
                                GizmoMode::Rotate => "Gizmo: Rotate (2)",
                                GizmoMode::Scale => "Gizmo: Scale (3)",
                            };
                            let pos = rect.right_top() + egui::vec2(-8.0, 8.0);
                            ui.painter().text(
                                pos,
                                egui::Align2::RIGHT_TOP,
                                mode_txt,
                                egui::FontId::monospace(12.0),
                                egui::Color32::from_gray(160),
                            );
                        }
                    }
                }
            });
        });
    }


    fn ui_console(&mut self, ctx: &egui::Context) {
        if !self.console_open {
            return;
        }

        egui::Window::new("Console")
            .open(&mut self.console_open)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label("Foundation mode: console is intentionally minimal for now.");
                ui.add_space(6.0);

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.console_input)
                        .hint_text("type a command (no-op)")
                        .desired_width(f32::INFINITY),
                );

                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.console_input.clear();
                }
            });
    }

    #[inline]
    fn build_ui(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

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
            self.plugin_manager.toggle();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::N)) {
            self.scene_bridge.cmd_new_scene();
        }

        self.ui_topbar(ctx);
        self.ui_toolbar(ctx);
        self.ui_hierarchy(ctx);
        self.ui_inspector(ctx);
        self.ui_viewport(ctx);
        self.ui_console(ctx);
        self.plugin_manager.show(ctx);
    }
}

impl UiBuildFn for EditorUiBuild {
    #[inline]
    fn build(&mut self, ctx_any: &mut dyn Any) {
        self.build_ui(ctx_any);
    }
}