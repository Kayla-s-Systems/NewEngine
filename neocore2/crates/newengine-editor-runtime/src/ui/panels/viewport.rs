#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_gizmo::egui::GizmoTransform;
use newengine_gizmo::GizmoMode;
use newengine_ui::input::keys as ui_keys;

use super::super::camera::FrameCamera;
use super::super::util;
use super::super::{providers, EditorUiBuild};

#[inline]
fn normalize_wheel_delta(raw_points: f32) -> f32 {
    if !raw_points.is_finite() {
        return 0.0;
    }

    let units = raw_points / 240.0;
    let compressed = units / (1.0 + units.abs());
    compressed.clamp(-1.0, 1.0)
}


#[inline]
fn read_fps_demo_state(me: &EditorUiBuild) -> Option<crate::gameplay::FpsDemoState> {
    let scene = me.scene_bridge.scene();
    let scene = scene.read();
    scene.world().resource::<crate::gameplay::FpsDemoState>().cloned()
}

fn draw_fps_demo_hud(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    overlay_frame: &egui::Frame,
    state: Option<crate::gameplay::FpsDemoState>,
) {
    let hud_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(10.0, 10.0),
        egui::vec2(430.0_f32.min(rect.width() - 20.0).max(260.0), 92.0),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(hud_rect), |ui| {
        overlay_frame.clone().show(ui, |ui| {
            ui.set_width(ui.available_width());

            match state.as_ref() {
                Some(state) => {
                    ui.label(egui::RichText::new(state.title.as_str()).strong().size(15.0));
                    ui.label(egui::RichText::new(state.progress_label()).monospace().size(13.0));
                    ui.label(state.objective.as_str());
                    ui.label(egui::RichText::new(state.status.as_str()).monospace().size(12.0));
                }
                None => {
                    ui.label(egui::RichText::new("KΛYLΛ FPS").strong().size(15.0));
                    ui.label("Game state is not initialized yet.");
                }
            }
        });
    });

    if let Some(state) = state.as_ref() {
        if state.completed || state.failed {
            let text = if state.completed { "EXTRACTION COMPLETE" } else { "MISSION FAILED" };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(32.0),
                egui::Color32::WHITE,
            );
        }
    }
}

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        draw_content(me, ctx, ui);
    });
}

pub(crate) fn draw_content(me: &mut EditorUiBuild, ctx: &egui::Context, ui: &mut egui::Ui) {
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

    if me.last_viewport_extent != Some((px_w, px_h)) {
        me.viewport.set_extent(px_w, px_h);
        me.viewport_bridge.publish_extent(px_w, px_h);
        me.last_viewport_extent = Some((px_w, px_h));
    }

        let (rmb_pressed, rmb_released, dropped_files) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.button_released(egui::PointerButton::Secondary),
                i.raw.dropped_files.clone(),
            )
        });
        let shift = me.shift_down();
        let raw_scroll_y = me.frame_input.mouse_wheel.1;
        let esc_pressed = me.key_pressed(ui_keys::ESCAPE);
    let play_mode = me.scene_bridge.play_mode();
    let play_active = play_mode.wants_direct_player_control();

        let nav_rotate = resp.dragged_by(egui::PointerButton::Middle) && !shift;
        let nav_pan = resp.dragged_by(egui::PointerButton::Middle) && shift;
        let nav_drag = nav_rotate || nav_pan;

        let hovered = resp.hovered() || nav_drag;

        // Gizmo hotkeys:
        // - Q/W/E/R when RMB free-fly is NOT active
        // - 1/2/3 always available
    if hovered && !ctx.wants_keyboard_input() && !play_mode.is_runtime() {
            let allow_qwer = !(me.fly_latch.is_captured() || rmb_pressed);

            let pressed_q = allow_qwer && me.key_pressed(ui_keys::KEY_Q);
            let pressed_w = (allow_qwer && me.key_pressed(ui_keys::KEY_W)) || me.key_pressed(ui_keys::DIGIT1);
            let pressed_e = (allow_qwer && me.key_pressed(ui_keys::KEY_E)) || me.key_pressed(ui_keys::DIGIT2);
            let pressed_r = (allow_qwer && me.key_pressed(ui_keys::KEY_R)) || me.key_pressed(ui_keys::DIGIT3);

            if pressed_q {
                me.editor.active_tool = newengine_editor_core::ToolId::Select;
            }
            if pressed_w {
                me.gizmo.set_mode(GizmoMode::Translate);
                me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Translate;
                me.editor.active_tool = newengine_editor_core::ToolId::Translate;
            }
            if pressed_e {
                me.gizmo.set_mode(GizmoMode::Rotate);
                me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Rotate;
                me.editor.active_tool = newengine_editor_core::ToolId::Rotate;
            }
            if pressed_r {
                me.gizmo.set_mode(GizmoMode::Scale);
                me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Scale;
                me.editor.active_tool = newengine_editor_core::ToolId::Scale;
            }
        }

        // Sync selection from render-thread picking into editor state.
        let picked = me.scene_bridge.selection();
        if picked != me.editor.selection.primary() {
            if let Some(e) = picked {
                let pending = me.pending_pick.take();
                if let Some(p) = pending {
                    if p.toggle {
                        me.editor.selection.toggle(e);
                    } else if p.additive {
                        me.editor.selection.add(e);
                    } else {
                        me.editor.selection.set_single(Some(e));
                    }
                } else {
                    me.editor.selection.set_single(Some(e));
                }

                me.scene_bridge.set_selection(me.editor.selection.primary());

                if let Some(primary) = me.editor.selection.primary() {
                    me.refresh_inspector_cache(primary);
                }
            } else {
                me.editor.selection.clear();
            }
        }

        // Determine whether gizmo wants to capture input this frame.
        let mut gizmo_capture_now = false;
    let gizmo_enabled = !play_mode.is_runtime()
        && me.editor.active_tool != newengine_editor_core::ToolId::Select;
        if gizmo_enabled {
            if let (Some(frame), Some(e)) = (me.viewport_bridge.read_camera_frame(), me.editor.selection.primary()) {
                if let Some((pos, rot, scale, _)) = me.read_selected_pose(e) {
                    let cam = FrameCamera { frame: &frame };
                    gizmo_capture_now = me
                        .gizmo
                        .wants_capture_now(ctx, rect, &cam, GizmoTransform::new(pos, rot, scale));
                }
            }
        }

        // RMB free-fly capture is **latched**.
        //
        // AAA policy:
        // - toggle only on explicit press/release edges
        // - ignore transient `button_down` flaps caused by pointer-lock
        // - allow Esc to force-cancel capture
        if esc_pressed {
            me.fly_latch.cancel();
            if play_active {
                me.scene_bridge
                    .cmd_set_play_mode(crate::gameplay::EditorPlayMode::Edit);
            }
        }

    let (fly_rmb, fly_rmb_changed) = if play_active {
        me.fly_latch.cancel();
        (false, false)
    } else {
        me.fly_latch
            .update(rmb_pressed, rmb_released, hovered && !gizmo_capture_now)
    };

        if fly_rmb_changed {
            // Cursor lock/unlock can warp the pointer; drop any baseline to avoid a delta spike.
            me.last_fly_drag_pos = None;
            me.last_nav_drag_pos = None;
        }

        // While RMB capture is active we must treat the viewport as active even if the backend
        // temporarily reports pointer outside of the rect.
    let active = if play_active { true } else { hovered || fly_rmb };

        // Click-to-select (picking handled on render thread).
    if !play_mode.is_runtime()
        && resp.clicked_by(egui::PointerButton::Primary)
        && !nav_drag
        && !gizmo_capture_now
    {
            if let Some(pos) = resp.interact_pointer_pos() {
                let toggle = me.command_down();
                let additive = me.shift_down();
                me.pending_pick = Some(super::super::PendingPick { additive, toggle });

                let local = pos - rect.min;
                let x_px = (local.x * ppp).clamp(0.0, rect.width() * ppp);
                let y_px = (local.y * ppp).clamp(0.0, rect.height() * ppp);
                me.viewport_bridge.publish_pick_request(x_px, y_px);
            }
        }

        let wants_kb = ctx.wants_keyboard_input();

        // NOTE: avoid UI-level logging for camera capture/navigation.
        // Navigation diagnostics are logged by the runtime camera system.

        // Middle-drag uses explicit drag tracking (absolute positions).
        // RMB free-fly uses relative motion (`pointer.delta()`), robust to cursor warp.
        let (mut dx_px, mut dy_px) = (0.0f32, 0.0f32);
    if nav_drag && !play_active {
            // Prevent stale baseline if the user switches from RMB free-fly to MMB nav.
            me.last_fly_drag_pos = None;
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some(prev) = me.last_nav_drag_pos {
                    let d = pos - prev;
                    dx_px = d.x * ppp;
                    dy_px = d.y * ppp;
                }
                me.last_nav_drag_pos = Some(pos);
            }
        } else {
            me.last_nav_drag_pos = None;

        if play_active || fly_rmb {
                dx_px = me.frame_input.mouse_delta.0;
                dy_px = me.frame_input.mouse_delta.1;
            }
            me.last_fly_drag_pos = None;
        }

        // Defensive: cursor lock/unlock (platform-level) may warp the cursor, causing a huge one-frame delta.
        // Treat that as a baseline reset instead of a camera impulse.
        // We use a fixed cap (in px/frame) to remain stable across viewport sizes.
        let max_delta_px = 160.0;
        if dx_px.abs() > max_delta_px || dy_px.abs() > max_delta_px {
            dx_px = 0.0;
            dy_px = 0.0;
            me.last_fly_drag_pos = None;
            me.last_nav_drag_pos = None;
        }

        let wheel_y_points = if active { raw_scroll_y } else { 0.0 };
    let mut wheel_y = normalize_wheel_delta(wheel_y_points);

        // Suppress synthetic deltas around RMB capture transitions.
    if !play_active {
        me.fly_latch
            .suppress_motion_if_needed(&mut dx_px, &mut dy_px, &mut wheel_y);
    }

        // Drag & drop models onto the viewport.
        if active {
            let snap = me.plugins_bridge.read();
            let exts = util::infer_model_exts(&snap);

            for f in dropped_files {
                if let Some(path) = f.path {
                    let p = path.display().to_string();
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let dot_ext = if ext.is_empty() { String::new() } else { format!(".{ext}") };

                    if !dot_ext.is_empty() && (exts.is_empty() || exts.iter().any(|e| e == &dot_ext)) {
                        me.queue_asset_spawn_from_path(p.clone(), "viewport_drop");
                        me.spawn_pending_asset_near_camera();
                    } else {
                        log::warn!("dropped file has unsupported extension: '{}'", p);
                    }
                }
            }
        }

    let look_drag = if play_active {
        active
    } else {
        (nav_rotate || fly_rmb) && !gizmo_capture_now
    };
    let pan_drag = if play_active {
        false
    } else {
        nav_pan && !gizmo_capture_now
    };
    let ui_busy = if play_active {
        false
    } else {
        gizmo_capture_now || me.gizmo.is_dragging()
    };
        let mut move_mask: u64 = 0;

        // Explicit framing:
        // - F: frame selection
        // - Shift+F: frame entire scene
    if !play_mode.is_runtime() && active && !wants_kb {
            let frame_sel = me.key_pressed(ui_keys::KEY_F) && !me.shift_down();
            let frame_all = me.key_pressed(ui_keys::KEY_F) && me.shift_down();
            if frame_sel {
                me.viewport_bridge.publish_frame_request(false);
            } else if frame_all {
                me.viewport_bridge.publish_frame_request(true);
            }
        }

    if (fly_rmb || play_active) && (!wants_kb || play_active) {
            if me.key_down(ui_keys::KEY_W) {
                move_mask |= newengine_viewport::input::MOVE_W;
            }
            if me.key_down(ui_keys::KEY_A) {
                move_mask |= newengine_viewport::input::MOVE_A;
            }
            if me.key_down(ui_keys::KEY_S) {
                move_mask |= newengine_viewport::input::MOVE_S;
            }
            if me.key_down(ui_keys::KEY_D) {
                move_mask |= newengine_viewport::input::MOVE_D;
            }
            if me.key_down(ui_keys::KEY_Q) {
                move_mask |= newengine_viewport::input::MOVE_UP;
            }
            if me.key_down(ui_keys::KEY_E) {
                move_mask |= newengine_viewport::input::MOVE_DOWN;
            }
            if me.shift_down() {
                move_mask |= newengine_viewport::input::MOVE_SHIFT;
            }
        }
        me.viewport_bridge.publish_camera_input(
            dx_px,
            dy_px,
            wheel_y,
            active,
            look_drag,
            pan_drag,
            ui_busy,
            if play_active { false } else { fly_rmb },
            move_mask,
            me.camera_speed.scalar,
        );

        let tex_user = me.viewport_bridge.read_tex_user();

        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            if tex_user != 0 {
                let tid = egui::TextureId::User(tex_user);
                let st = egui::load::SizedTexture::new(tid, rect.size());
                ui.put(rect, egui::Image::new(st).fit_to_exact_size(rect.size()));
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Viewport: waiting for render target...");
                });
            }

            let supported_model_exts = if active {
                let snap = me.plugins_bridge.read();
                util::infer_model_exts(&snap)
            } else {
                Vec::new()
            };

            let overlay_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(18, 20, 24, 220))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(6));

            if play_mode.is_runtime() {
                draw_fps_demo_hud(ui, rect, &overlay_frame, read_fps_demo_state(me));
            } else {
                let header_rect = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(10.0, 10.0),
                    egui::vec2(620.0_f32.min(rect.width() - 20.0).max(260.0), 32.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(header_rect), |ui| {
                    overlay_frame.clone().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("Perspective").strong());
                            ui.separator();
                            for desc in providers::viewport_mode_actions(me) {
                                if ui
                                    .add_enabled(desc.enabled, egui::Button::selectable(desc.selected, desc.label.as_ref()))
                                    .clicked()
                                {
                                    me.execute_ui_action(&desc.action);
                                }
                            }
                            ui.separator();
                            ui.menu_button("Show", |ui| {
                                let mut collision_wire = me.scene_bridge.collision_wireframe_enabled();
                                if ui.checkbox(&mut collision_wire, "Collision").changed() {
                                    me.scene_bridge.cmd_set_collision_wireframe(collision_wire);
                                }
                            });
                            ui.menu_button("Snap", |ui| {
                                ui.checkbox(&mut me.transform_snap.translate_enabled, "Move");
                                ui.add(
                                    egui::DragValue::new(&mut me.transform_snap.translate_step)
                                        .speed(0.25)
                                        .range(0.1..=4096.0)
                                        .suffix(" uu"),
                                );
                                ui.separator();
                                ui.checkbox(&mut me.transform_snap.rotate_enabled, "Rotate");
                                ui.add(
                                    egui::DragValue::new(&mut me.transform_snap.rotate_step_deg)
                                        .speed(0.25)
                                        .range(1.0..=180.0)
                                        .suffix(" deg"),
                                );
                                ui.separator();
                                ui.checkbox(&mut me.transform_snap.scale_enabled, "Scale");
                                ui.add(
                                    egui::DragValue::new(&mut me.transform_snap.scale_step)
                                        .speed(0.01)
                                        .range(0.01..=10.0),
                                );
                            });
                            ui.menu_button("Cam", |ui| {
                                for choice in providers::camera_speed_choices(me) {
                                    if ui.add_enabled(choice.enabled, egui::Button::selectable(choice.selected, choice.label)).clicked() {
                                        me.execute_ui_action(&providers::UiAction::SetCameraSpeedPreset(choice.value));
                                        ui.close();
                                    }
                                }
                            });
                            if ui.button("Frame").clicked() {
                                me.execute_ui_action(&providers::UiAction::FrameSelection);
                            }
                        });
                    });
                });
            }

            if !play_mode.is_runtime() {
                resp.context_menu(|ui| {
                    let selection_ctx = me
                        .editor
                        .selection
                        .primary()
                        .map(|entity| super::super::schema::build_selection_context(me, entity));
                    for action in super::super::schema::selection_context_actions(me, selection_ctx.as_ref()) {
                        if ui
                            .add_enabled(action.enabled, egui::Button::selectable(action.selected, action.label))
                            .clicked()
                        {
                            me.execute_context_action(action.id);
                            ui.close();
                        }
                    }
                });
            }

            if !play_mode.is_runtime() && !supported_model_exts.is_empty() {
                let hint_rect = egui::Rect::from_min_size(
                    rect.left_bottom() + egui::vec2(10.0, -34.0),
                    egui::vec2((rect.width() - 20.0).max(140.0), 24.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(hint_rect), |ui| {
                    overlay_frame.show(ui, |ui| {
                        ui.label(format!("Drop model: {}", supported_model_exts.join(", ")));
                    });
                });
            }

            // Viewport overlay: selection highlight + gizmo.
            let frame = me.viewport_bridge.read_camera_frame();
            let selected = me.editor.selection.primary();
            if !play_mode.is_runtime() {
                if let (Some(frame), Some(e)) = (frame, selected) {
                    if let Some((pos, rot, scale, _color)) = me.read_selected_pose(e) {
                    util::draw_selection_outline(ui.painter(), &frame, rect, pos, rot, scale);

                    let mut gizmo_out = None;
                    if gizmo_enabled {
                        let cam = FrameCamera { frame: &frame };
                        let gizmo_in = GizmoTransform::new(pos, rot, scale);
                        gizmo_out = Some(me.gizmo.run(ui.painter(), ctx, rect, &cam, gizmo_in));
                    }

                    let is_dragging = gizmo_enabled && me.gizmo.is_dragging();

                    if is_dragging && !me.gizmo_was_dragging {
                        if let Some((p0, r0, s0, _)) = me.read_selected_pose(e) {
                            let (y0, p0e, r0e) = r0.to_euler(newengine_math::EulerRot::YXZ);
                            me.gizmo_drag_begin = Some((
                                e,
                                newengine_editor_core::TransformSnapshot::new(p0, (y0, p0e, r0e), s0),
                            ));
                        }
                    }

                    if let Some(t) = gizmo_out.and_then(|o| o.transform) {
                        let pos = me.snapped_position(t.pos);
                        let (y, p, r) = t.rot.to_euler(newengine_math::EulerRot::YXZ);
                        let (y, p, r) = me.snapped_rotation_ypr(y, p, r);
                        let scale = me.snapped_scale(t.scale);
                        me.insp_pos = [pos.x, pos.y, pos.z];
                        me.insp_rot_deg = [y.to_degrees(), p.to_degrees(), r.to_degrees()];
                        me.insp_scale = [scale.x, scale.y, scale.z];
                        me.scene_bridge.cmd_set_transform(e, pos, (y, p, r), scale);
                    }

                    if !is_dragging && me.gizmo_was_dragging {
                        if let Some((ent, before)) = me.gizmo_drag_begin.take() {
                            if let Some((p1, r1, s1, _)) = me.read_selected_pose(ent) {
                                let (y1, p1e, r1e) = r1.to_euler(newengine_math::EulerRot::YXZ);
                                let after =
                                    newengine_editor_core::TransformSnapshot::new(p1, (y1, p1e, r1e), s1);
                                if before != after {
                                    me.editor.commands.push(newengine_editor_core::EditorCommand::SetTransform {
                                        entity: ent,
                                        before,
                                        after,
                                    });
                                }
                            }
                        }
                    }

                    me.gizmo_was_dragging = is_dragging;

                    let mode_txt = if gizmo_enabled {
                        match me.gizmo.mode() {
                            GizmoMode::Translate => "Gizmo: Translate (W/1)",
                            GizmoMode::Rotate => "Gizmo: Rotate (E/2)",
                            GizmoMode::Scale => "Gizmo: Scale (R/3)",
                        }
                    } else {
                        "Tool: Select (Q)"
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
}
