/// Runtime-hosted camera gateway bridge.
///
/// This is the in-process runtime implementation of the `engine.camera` boundary
/// until camera providers are moved behind `camera.api` service plugins. Render
/// code talks to this bridge for a resolved view frame; it does not own camera
/// navigation state and does not import camera runtime crates directly.
pub struct CameraGatewayBridge {
    state: Arc<Mutex<CameraGatewayState>>,
}

impl CameraGatewayBridge {
    #[inline]
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(CameraGatewayState::default()));
        let bridge = Self { state };
        let _ = bridge.publish_gateway_best_effort();
        bridge
    }

    /// Publishes this bridge into the gateway registry of the currently active HostContext.
    ///
    /// This is intentionally repeatable because runtime launchers create instance-owned
    /// HostContexts after profile construction. Re-publication is a no-op when the
    /// authoritative camera route is already present in the active context.
    pub fn publish_gateway_best_effort(&self) -> bool {
        register_camera_gateway_service_best_effort(Arc::clone(&self.state))
    }

    /// Samples direct-player input before the fixed simulation schedule.
    ///
    /// Camera/view requests are consumed here exactly once. The render-phase camera
    /// resolution then observes the post-simulation player pose without adding an
    /// extra frame of movement/look latency.
    pub fn prepare_world_input(
        &self,
        world: &mut World,
        input: CameraGatewayInput,
        effective_play_mode: GameRunMode,
        frame_index: u64,
    ) {
        let active_view = {
            let mut state = self.state.lock();
            state.apply_authored_initial_view(world);
            state.apply_input_view_request(input.camera_view)
        };
        // World-runtime presentation (including the first-person weapon) runs before
        // `tick_world_frame`. Publish the active view here as well so the viewmodel never consumes
        // a stale third-person/first-person state for the current render frame.
        sync_player_view_listeners(world, matches!(active_view, CameraViewMode::FirstPerson));
        let service_config = camera_runtime_service_config(world, active_view);
        apply_runtime_input(
            world,
            input,
            effective_play_mode,
            service_config,
            frame_index,
        );
    }

    pub fn tick_world_frame(
        &self,
        world: &mut World,
        viewport: &ViewportBridge,
        input: CameraGatewayInput,
        play_mode: GameRunMode,
        effective_play_mode: GameRunMode,
        world_playable: bool,
        frame_index: u64,
        dt: f32,
        vp_w: u32,
        vp_h: u32,
        bounds: EngineBoundsSnap,
        selection_bounds: Option<EngineBoundsSnap>,
    ) -> CameraGatewayFrame {
        let camera_dt = sanitize_camera_dt(dt);
        let mut state = self.state.lock();
        state.apply_authored_initial_view(world);
        let mut nav_input = camera_nav_input(input.clone(), play_mode);
        let active_view = state.apply_input_view_request(input.camera_view);
        sync_player_view_listeners(world, matches!(active_view, CameraViewMode::FirstPerson));
        let cam_id = world
            .resource::<newengine_scene::SceneState>()
            .and_then(|s| s.active_camera.or(s.root))
            .unwrap_or_default();

        CameraRuntimeService::ensure_manager_resource(world);

        let existing_nav_mode = world
            .get::<RuntimeNavController>(cam_id)
            .map(|ctrl| ctrl.mode)
            .unwrap_or(RuntimeNavMode::Orbit);
        let player = first_player(world);
        let camera_profile = player
            .and_then(|player| {
                world.get::<newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile>(player)
            })
            .copied()
            .map(newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile::sanitized);
        let gate_blocked = play_mode.is_runtime() && !world_playable;
        let mut controller_z_phases = [f32::NAN; 5];
        controller_z_phases[0] = follow_controller_offset_z(world, cam_id);

        let suppress_game_nav = {
            let manager = world
                .resource_mut::<CameraManagerResource>()
                .expect("camera manager resource inserted");
            if let Some(profile) = camera_profile {
                manager.settings.gameplay.blend_in_sec = profile.gameplay_blend_in_seconds;
                manager.settings.gameplay.blend_out_sec = profile.gameplay_blend_out_seconds;
                manager.settings.gameplay.lock_input = profile.gameplay_blend_lock_input;
            }
            manager.advance(camera_dt);
            manager.sync_world_state(CameraRuntimeWorldState {
                game_nav_mode: existing_nav_mode,
                runtime_requested: play_mode.is_runtime(),
                public_runtime_active: effective_play_mode.is_runtime(),
                wants_direct_player_control: effective_play_mode.wants_direct_player_control(),
                gate_blocked,
                player,
                view_mode: active_view,
            });
            !manager.wants_navigation_input()
        };

        state.sync_play_mode_transition(world, cam_id, effective_play_mode);
        let service_config = camera_runtime_service_config(world, active_view);
        CameraRuntimeService::apply_pending_director_requests(world, cam_id, service_config);
        controller_z_phases[1] = follow_controller_offset_z(world, cam_id);
        let gameplay_capture =
            newengine_gameplay_world_runtime::gameplay::gameplay_input_capture(world);
        let mut routed_camera_input_for_trace = None;
        if effective_play_mode.wants_direct_player_control() {
            if let Some(player) = player {
                let routed_camera_input = route_player_input_channels(&input, gameplay_capture);
                routed_camera_input_for_trace = Some(routed_camera_input);
                refresh_camera_spring_arm_collision_world(world, player);
                let orbit_dolly_drag = matches!(
                    service_config.runner,
                    newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonOrbit
                ) && input.pan_drag;
                if !orbit_dolly_drag {
                    let _ = CameraRuntimeService::apply_gameplay_camera_orbit_look(
                        world,
                        cam_id,
                        player,
                        service_config,
                        routed_camera_input.look_delta,
                        routed_camera_input.look_active,
                    );
                }

                // Wheel/MMB dolly are gameplay-camera channels. Runtime navigation is deliberately
                // gated while the player owns the camera, so consume the wheel here before the
                // generic nav path zeros it. UI/script camera capture still blocks the zoom.
                if !input.camera_navigation_gated && !gameplay_capture.block_camera_navigation {
                    let _ = CameraRuntimeService::apply_gameplay_camera_zoom(
                        world,
                        cam_id,
                        service_config,
                        input.wheel_y,
                    );
                    if orbit_dolly_drag {
                        let _ = CameraRuntimeService::apply_gameplay_camera_drag_zoom(
                            world,
                            cam_id,
                            service_config,
                            input.dy_px,
                        );
                    }
                }

                // Gameplay view rotation is sampled at render cadence. Third-person cameras
                // smooth only their player anchor; angular orbit remains render-cadence direct.
                controller_z_phases[2] = follow_controller_offset_z(world, cam_id);
                let _ = CameraRuntimeService::sync_gameplay_camera_now(
                    world,
                    cam_id,
                    player,
                    service_config,
                    camera_dt,
                );
                controller_z_phases[3] = follow_controller_offset_z(world, cam_id);
            }
        }

        let params = CameraNavParams {
            dt: camera_dt,
            viewport: CameraViewport::from_size(vp_w, vp_h),
            channel: CameraChannelState::dominant(if effective_play_mode.is_runtime() {
                CameraChannel::Gameplay
            } else {
                CameraChannel::Runtime
            }),
            bounds: CamBoundsSphere {
                center: bounds.center,
                radius: bounds.radius,
            },
            selection_bounds: selection_bounds.map(|b| CamBoundsSphere {
                center: b.center,
                radius: b.radius,
            }),
        };

        let frame_req = CameraNavFrameRequest {
            seq: viewport.read_frame_request(),
            all: viewport.read_frame_all(),
        };

        if suppress_game_nav
            || effective_play_mode.wants_direct_player_control()
            || nav_input.navigation_gated
            || gameplay_capture.block_camera_navigation
        {
            nav_input.gate_navigation();
        }

        let out = step_camera_nav(
            &mut state.nav,
            world,
            cam_id,
            &mut nav_input,
            params,
            frame_req,
        );
        controller_z_phases[4] = follow_controller_offset_z(world, cam_id);

        let first_person_aiming =
            player.is_some_and(|player| active_weapon_aim_intent(world, player));
        let (base_resolved_frame, report, effects, resolved_view) =
            if let Some(manager) = world.resource_mut::<CameraManagerResource>() {
                manager.sync_runtime_nav_mode_from_controller(out.controller.mode);
                manager.set_last_cursor(out.cursor);
                let frame = manager.resolve_camera_frame(out.frame, dt);
                let view = manager.active_view_mode();
                (
                    frame,
                    Some(camera_report_snapshot(manager.report())),
                    manager.last_post_effects().unwrap_or_default(),
                    view,
                )
            } else {
                (out.frame, None, Default::default(), active_view)
            };

        // NearClipScanner runs after director/frame resolution because it protects the projection
        // of the actual rendered camera pose. It reuses the same camera collision world that was
        // refreshed before gameplay camera synchronization; there is no second query authority.
        let target_fov_y =
            gameplay_target_fov_y(resolved_view, first_person_aiming, service_config);
        let fallback_near = match base_resolved_frame.projection {
            Projection::Perspective(_perspective)
                if matches!(resolved_view, CameraViewMode::FirstPerson) =>
            {
                service_config.first_person_near
            }
            Projection::Perspective(perspective) => perspective.near,
            _ => service_config.first_person_near,
        };
        let resolved_near = if effective_play_mode.wants_direct_player_control() {
            player
                .map(|player| {
                    CameraRuntimeService::resolve_gameplay_near_clip(
                        world,
                        cam_id,
                        player,
                        base_resolved_frame.rig.position,
                        base_resolved_frame.rig.rotation,
                        target_fov_y,
                        base_resolved_frame.viewport.aspect(),
                        service_config,
                        camera_dt,
                    )
                })
                .unwrap_or(fallback_near)
        } else {
            fallback_near
        };
        let resolved_frame = apply_gameplay_view_lens(
            base_resolved_frame,
            resolved_view,
            first_person_aiming,
            service_config,
            resolved_near,
        );
        let snapshot = camera_frame_snapshot_for_view(resolved_frame, effects, resolved_view);

        trace_gameplay_camera_frame(
            frame_index,
            dt,
            &input,
            routed_camera_input_for_trace,
            active_view,
            world,
            player,
            cam_id,
            out.frame,
            resolved_frame,
            report.as_ref(),
            controller_z_phases,
        );

        if let Some(listener) =
            newengine_audio_client::audio_listener_from_camera_snapshot(&snapshot)
        {
            world.insert_resource(newengine_audio_world_api::AudioListenerRuntimeState {
                listener,
                listener_entity: player.map(EntityId::stable_u64),
                frame_index,
            });
        }
        newengine_audio_client::sync_audio_listener_from_camera_snapshot(&snapshot);
        debug_assert!(
            snapshot.finite,
            "authoritative camera snapshot must be finite"
        );
        debug_assert!(
            snapshot
                .view_cols
                .iter()
                .flatten()
                .all(|value| value.is_finite()),
            "authoritative camera view matrix must be finite"
        );
        debug_assert!(
            snapshot
                .projection_cols
                .iter()
                .flatten()
                .all(|value| value.is_finite()),
            "authoritative camera projection matrix must be finite"
        );
        debug_assert!(
            snapshot.position_ws.iter().all(|value| value.is_finite()),
            "authoritative camera position must be finite"
        );
        state.last_snapshot = Some(snapshot);
        let view = EngineViewFrame::from_camera_snapshot(snapshot);
        let cursor = if effective_play_mode.wants_direct_player_control()
            && input.active
            && !input.camera_navigation_gated
            && !gameplay_capture.release_cursor
        {
            CursorState::captured_locked()
        } else {
            cursor_state_for_nav(&nav_input)
        };

        CameraGatewayFrame {
            frame_index,
            camera_snapshot: snapshot,
            view,
            postfx: view_postfx_from_camera_snapshot(snapshot),
            report,
            cursor,
            effective_play_mode,
            world_playable,
        }
    }
}

#[inline]
fn active_weapon_aim_intent(world: &World, player: EntityId) -> bool {
    // Input actions are intent only. ADS/FOV may exist only when the authoritative active weapon
    // explicitly exposes the Aim capability. This keeps raw mouse input from becoming a camera
    // zoom for Unarmed, melee, an empty/incomplete equipment state, or a stale weapon command.
    active_equipped_weapon_can_aim(world, player)
        && (world
            .get::<PlayerCommandFrame>(player)
            .is_some_and(|commands| commands.actions.is_held("player.aim"))
            || active_equipped_weapon_aiming(world, player))
}

impl Default for CameraGatewayBridge {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
