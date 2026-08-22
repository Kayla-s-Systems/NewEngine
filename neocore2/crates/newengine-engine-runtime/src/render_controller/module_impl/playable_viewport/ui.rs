use super::*;

#[inline]
fn viewport_texture_color() -> u32 {
    255 | (255 << 8) | (255 << 16) | (255 << 24)
}

pub(super) fn prepend_viewport_slot_quad(
    ui: &mut UiDrawList,
    slot: &UiViewportSlot,
    texture_id: u32,
) {
    if texture_id == 0 || slot.w_px <= 1.0 || slot.h_px <= 1.0 {
        return;
    }
    let old_vertices = core::mem::take(&mut ui.mesh.vertices);
    let old_indices = core::mem::take(&mut ui.mesh.indices);
    let old_cmds: Vec<UiDrawCmd> = ui.mesh.cmds.drain(..).collect();

    let x = slot.x_px.round();
    let y = slot.y_px.round();
    let w = slot.w_px.round().max(1.0);
    let h = slot.h_px.round().max(1.0);
    let color = viewport_texture_color();
    ui.mesh.vertices.extend_from_slice(&[
        UiVertex {
            pos: [x, y],
            uv: [0.0, 0.0],
            color,
        },
        UiVertex {
            pos: [x + w, y],
            uv: [1.0, 0.0],
            color,
        },
        UiVertex {
            pos: [x + w, y + h],
            uv: [1.0, 1.0],
            color,
        },
        UiVertex {
            pos: [x, y + h],
            uv: [0.0, 1.0],
            color,
        },
    ]);
    ui.mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    ui.mesh.cmds.push(UiDrawCmd {
        texture: UiTexId(texture_id),
        clip_rect: UiRect {
            min_x: x,
            min_y: y,
            max_x: x + w,
            max_y: y + h,
        },
        index_range: 0..6,
    });

    let vertex_offset = 4u32;
    let index_offset = 6u32;
    ui.mesh.vertices.extend(old_vertices);
    ui.mesh
        .indices
        .extend(old_indices.into_iter().map(|idx| idx + vertex_offset));
    for mut cmd in old_cmds {
        cmd.index_range =
            (cmd.index_range.start + index_offset)..(cmd.index_range.end + index_offset);
        ui.mesh.cmds.push(cmd);
    }
}

impl RuntimeRenderController {
    pub(super) fn refresh_modal_ui_draw_list<E: Send + 'static>(
        &self,
        ctx: &ModuleCtx<'_, E>,
        ui_layers: &mut UiLayerDrawPacketSet,
        primary_domain: UiLayerDomain,
        primary_state: &UiSurfaceNode,
        primary_was_open: bool,
        external_capture: &UiInputCaptureState,
        scope: RenderFrameScope,
    ) -> EngineResult<Option<UiInputCaptureState>> {
        if primary_state.visible || primary_was_open {
            // Publish both visible and hidden states. engine.ui owns retained node
            // lifecycle; if runtime does not send the hidden node on close, the
            // provider can legally keep the previous retained menu on screen.
            ui_gateway::publish_surface_node(primary_state);
        }

        let external_refresh =
            external_capture.draw_refresh_requested || external_capture.requests_capture();

        if !primary_state.visible && !primary_was_open && !external_refresh {
            return Ok(None);
        }

        let needs_clear_packet = (!primary_state.visible && primary_was_open)
            || (external_capture.draw_refresh_requested && !external_capture.requests_capture());

        let mut provider_capture = None;
        match ui_gateway::request_frame_output(
            self.frame.frame_index,
            scope.dt,
            [scope.w, scope.h],
            1.0,
        ) {
            Ok(Some(output)) => {
                provider_capture = Some(output.input_capture);
                set_primary_domain_draw_list(
                    ctx,
                    ui_layers,
                    primary_domain,
                    self.frame.frame_index,
                    output.draw_list,
                );
            }
            Ok(None) => {
                if needs_clear_packet {
                    set_primary_domain_draw_list(
                        ctx,
                        ui_layers,
                        primary_domain,
                        self.frame.frame_index,
                        clear_ui_draw_list([scope.w, scope.h]),
                    );
                }
            }
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "modal ui: same-frame draw-list refresh failed: {e}"
                );
                if needs_clear_packet {
                    set_primary_domain_draw_list(
                        ctx,
                        ui_layers,
                        primary_domain,
                        self.frame.frame_index,
                        clear_ui_draw_list([scope.w, scope.h]),
                    );
                }
            }
        }

        Ok(provider_capture)
    }
}

fn set_primary_domain_draw_list<E: Send + 'static>(
    ctx: &ModuleCtx<'_, E>,
    ui_layers: &mut UiLayerDrawPacketSet,
    primary_domain: UiLayerDomain,
    frame_index: u64,
    draw_list: UiDrawList,
) {
    if let Some(existing) = ui_layers.draw_list_mut(primary_domain) {
        *existing = draw_list;
        return;
    }

    let packet = ctx
        .resources()
        .get::<newengine_ui_api::UiLayerCompositionPlan>()
        .filter(|plan| plan.domain == primary_domain)
        .map(|plan| plan.draw_packet(draw_list.clone()))
        .unwrap_or_else(|| {
            newengine_ui_api::UiLayerDrawPacket::new(primary_domain, frame_index, draw_list)
        });
    ui_layers.push(packet);
}

pub(super) fn merge_ui_input_capture(
    mut out: UiInputCaptureState,
    incoming: UiInputCaptureState,
) -> UiInputCaptureState {
    out.sampling_alive = true;
    out.camera_navigation_gated |= incoming.camera_navigation_gated;
    out.gameplay_movement_gated |= incoming.gameplay_movement_gated;
    out.modal |= incoming.modal;
    out.draw_refresh_requested |= incoming.draw_refresh_requested;
    for surface in incoming.surfaces {
        if !out.surfaces.iter().any(|it| it == &surface) {
            out.surfaces.push(surface);
        }
    }
    for contributor in incoming.contributors {
        if !out.contributors.iter().any(|it| it == &contributor) {
            out.contributors.push(contributor);
        }
    }
    let incoming_reason = incoming.reason.trim();
    if !incoming_reason.is_empty() && incoming_reason != "none" {
        if out.reason.trim().is_empty() || out.reason == "none" {
            out.reason = incoming.reason;
        } else if out.reason != incoming.reason {
            out.reason = format!("{} + {}", out.reason, incoming.reason);
        }
    }
    out
}

pub(super) fn clear_ui_draw_list(surface_size_px: [u32; 2]) -> UiDrawList {
    let mut draw_list = UiDrawList::new();
    draw_list.screen_size_px = surface_size_px;
    draw_list.pixels_per_point = 1.0;
    draw_list
}
