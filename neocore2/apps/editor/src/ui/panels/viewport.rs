#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_gizmo::egui::GizmoTransform;
use newengine_gizmo::GizmoMode;
use newengine_platform_winit::egui;

use super::super::camera::FrameCamera;
use super::super::util;
use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
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

        me.viewport.set_extent(px_w, px_h);
        me.viewport_bridge.publish_extent(px_w, px_h);

        let shift = ctx.input(|i| i.modifiers.shift);
        let nav_rotate = resp.dragged_by(egui::PointerButton::Middle) && !shift;
        let nav_pan = resp.dragged_by(egui::PointerButton::Middle) && shift;
        let nav_drag = nav_rotate || nav_pan;

        let active = resp.hovered() || nav_drag;

        // Gizmo hotkeys:
        // - W/E/R (industry standard) when RMB is NOT held
        // - 1/2/3 as an always-available fallback.
        if active && !ctx.wants_keyboard_input() {
            let rmb = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
            if !rmb {
                ctx.input(|i| {
                    if i.key_pressed(egui::Key::Q) {
                        me.editor.active_tool = newengine_editor_core::ToolId::Select;
                    }
                    if i.key_pressed(egui::Key::W) || i.key_pressed(egui::Key::Num1) {
                        me.gizmo.set_mode(GizmoMode::Translate);
                        me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Translate;
                        me.editor.active_tool = newengine_editor_core::ToolId::Translate;
                    }
                    if i.key_pressed(egui::Key::E) || i.key_pressed(egui::Key::Num2) {
                        me.gizmo.set_mode(GizmoMode::Rotate);
                        me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Rotate;
                        me.editor.active_tool = newengine_editor_core::ToolId::Rotate;
                    }
                    if i.key_pressed(egui::Key::R) || i.key_pressed(egui::Key::Num3) {
                        me.gizmo.set_mode(GizmoMode::Scale);
                        me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Scale;
                        me.editor.active_tool = newengine_editor_core::ToolId::Scale;
                    }
                });
            } else {
                ctx.input(|i| {
                    if i.key_pressed(egui::Key::Num1) {
                        me.gizmo.set_mode(GizmoMode::Translate);
                        me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Translate;
                        me.editor.active_tool = newengine_editor_core::ToolId::Translate;
                    }
                    if i.key_pressed(egui::Key::Num2) {
                        me.gizmo.set_mode(GizmoMode::Rotate);
                        me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Rotate;
                        me.editor.active_tool = newengine_editor_core::ToolId::Rotate;
                    }
                    if i.key_pressed(egui::Key::Num3) {
                        me.gizmo.set_mode(GizmoMode::Scale);
                        me.editor.gizmo_mode = newengine_editor_core::GizmoMode::Scale;
                        me.editor.active_tool = newengine_editor_core::ToolId::Scale;
                    }
                });
            }
        }

        // Sync selection from render-thread picking into editor state.
        // Render thread only reports the last picked entity; UI applies selection semantics
        // (replace/add/toggle) based on modifiers captured at click time.
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
                    // Fallback: replace selection.
                    me.editor.selection.set_single(Some(e));
                }

                // Keep scene_bridge primary selection in sync with editor primary.
                me.scene_bridge.set_selection(me.editor.selection.primary());

                if let Some(primary) = me.editor.selection.primary() {
                    me.refresh_inspector_cache(primary);
                }
            } else {
                me.editor.selection.clear();
            }
        }

        // Determine whether gizmo wants to capture input this frame (prevents orbit/selection conflicts).
        let mut gizmo_capture_now = false;
        let gizmo_enabled = me.editor.active_tool != newengine_editor_core::ToolId::Select;
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

        // Click-to-select (picking handled on render thread).
        // We capture modifiers here to apply deterministic multi-selection semantics later.
        if resp.clicked_by(egui::PointerButton::Primary) && !nav_drag && !gizmo_capture_now {
            if let Some(pos) = resp.interact_pointer_pos() {
                let mods = ctx.input(|i| i.modifiers);
                let toggle = mods.command; // Ctrl on Windows/Linux, Cmd on macOS.
                let additive = mods.shift;
                me.pending_pick = Some(super::super::PendingPick { additive, toggle });

                let local = pos - rect.min;
                let ppp = ctx.pixels_per_point().max(0.0001);
                let x_px = (local.x * ppp).clamp(0.0, rect.width() * ppp);
                let y_px = (local.y * ppp).clamp(0.0, rect.height() * ppp);
                me.viewport_bridge.publish_pick_request(x_px, y_px);
            }
        }

        let mut dx_px = 0.0f32;
        let mut dy_px = 0.0f32;
        if nav_drag {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some(prev) = me.last_drag_pos {
                    let d = pos - prev;
                    dx_px = d.x * ppp;
                    dy_px = d.y * ppp;
                }
                me.last_drag_pos = Some(pos);
            }
        } else {
            me.last_drag_pos = None;
        }

        let wheel_y_points = if active {
            ctx.input(|i| i.raw_scroll_delta.y)
        } else {
            0.0
        };
        let wheel_y = (wheel_y_points / 240.0).clamp(-2.0, 2.0);

        // Drag & drop models onto the viewport.
        let snap = me.plugins_bridge.read();
        let exts = util::infer_model_exts(&snap);

        if active {
            let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
            for f in dropped {
                if let Some(path) = f.path {
                    let p = path.display().to_string();
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let dot_ext = if ext.is_empty() { String::new() } else { format!(".{ext}") };

                    if !dot_ext.is_empty() && (exts.is_empty() || exts.iter().any(|e| e == &dot_ext)) {
                        log::warn!(
                            "model drop is currently disabled (no asset->scene contract yet): '{}'",
                            p
                        );
                    } else {
                        log::warn!("dropped file has unsupported extension: '{}'", p);
                    }
                }
            }
        }

        let look_drag = nav_rotate && !gizmo_capture_now;
        let pan_drag = nav_pan && !gizmo_capture_now;
        // UI busy flag is critical for renderer-side camera framing logic.
        let ui_busy = gizmo_capture_now || me.gizmo.is_dragging();
        me.viewport_bridge
            .publish_orbit_input(dx_px, dy_px, wheel_y, active, look_drag, pan_drag, ui_busy);

        let wants_kb = ctx.wants_keyboard_input();
        let mut move_mask: u64 = 0;

        // Explicit framing (Blender-like):
        // - F: frame selection (or scene if nothing selected)
        // - Shift+F: frame entire scene
        if active && !wants_kb {
            let frame_sel = ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.shift);
            let frame_all = ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.shift);
            if frame_sel {
                me.viewport_bridge.publish_frame_request(false);
            } else if frame_all {
                me.viewport_bridge.publish_frame_request(true);
            }
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
        me.viewport_bridge.publish_move_keys(move_mask);

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

            // Viewport overlay: supported model extensions.
            if active {
                let snap = me.plugins_bridge.read();
                let exts = util::infer_model_exts(&snap);
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

            // Viewport overlay: selection highlight + gizmo.
            let frame = me.viewport_bridge.read_camera_frame();
            let selected = me.editor.selection.primary();
            if let (Some(frame), Some(e)) = (frame, selected) {
                if let Some((pos, rot, scale, _color)) = me.read_selected_pose(e) {
                    util::draw_selection_outline(ui.painter(), &frame, rect, pos, rot, scale);

                    let mut gizmo_out = None;
                    if gizmo_enabled {
                        let cam = FrameCamera { frame: &frame };
                        let gizmo_in = GizmoTransform::new(pos, rot, scale);
                        gizmo_out = Some(me.gizmo.run(ui.painter(), ctx, rect, &cam, gizmo_in));
                    }

                    // Live transform update + command commit on drag end (undo/redo).
                    let is_dragging = gizmo_enabled && me.gizmo.is_dragging();

                    if is_dragging && !me.gizmo_was_dragging {
                        // Drag begins: capture "before" snapshot.
                        if let Some((p0, r0, s0, _)) = me.read_selected_pose(e) {
                            let (y0, p0e, r0e) = r0.to_euler(newengine_math::EulerRot::YXZ);
                            me.gizmo_drag_begin = Some((
                                e,
                                newengine_editor_core::TransformSnapshot::new(p0, (y0, p0e, r0e), s0),
                            ));
                        }
                    }

                    if let Some(t) = gizmo_out.and_then(|o| o.transform) {
                        me.insp_pos = [t.pos.x, t.pos.y, t.pos.z];
                        let (y, p, r) = t.rot.to_euler(newengine_math::EulerRot::YXZ);
                        me.insp_rot_deg = [y.to_degrees(), p.to_degrees(), r.to_degrees()];
                        me.insp_scale = [t.scale.x, t.scale.y, t.scale.z];
                        me.scene_bridge.cmd_set_transform(e, t.pos, (y, p, r), t.scale);
                    }

                    if !is_dragging && me.gizmo_was_dragging {
                        // Drag ended: commit a single command.
                        if let Some((ent, before)) = me.gizmo_drag_begin.take() {
                            if let Some((p1, r1, s1, _)) = me.read_selected_pose(ent) {
                                let (y1, p1e, r1e) = r1.to_euler(newengine_math::EulerRot::YXZ);
                                let after = newengine_editor_core::TransformSnapshot::new(p1, (y1, p1e, r1e), s1);
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
        });
    });
}
