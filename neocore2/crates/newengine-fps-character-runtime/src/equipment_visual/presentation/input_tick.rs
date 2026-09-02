/// Samples RMB/aim once before animation so body IK and rendered rifle consume the exact same
/// presentation alpha in the same world-runtime frame.
pub(crate) fn tick_equipped_weapon_presentation_input(world: &mut newengine_ecs::World, dt: f32) {
    let roots = world
        .query::<EquippedWeaponVisualRoot>()
        .map(|(entity, visual)| (entity, *visual))
        .collect::<Vec<_>>();
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    for (root, visual) in roots {
        // RMB is a weapon state, not a first-person-only state. Third-person aim must drive the
        // same ReadyHold/ADS contract as full-body first person.
        let obstruction_alpha = world
            .get::<WeaponObstructionState>(visual.owner)
            .map(|state| state.alpha.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let active_binding =
            newengine_engine_runtime::gameplay::active_equipped_weapon_binding(world, visual.owner)
                .filter(|binding| binding.instance_id == visual.instance_id);
        let active_visual = active_binding.is_some();
        let presentation = world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(visual.item))
            .map(|definition| definition.weapon_presentation.clone().sanitized())
            .filter(|presentation| presentation.enabled);
        let aim_target =
            if active_visual && equipped_weapon_aim_held(world, visual.owner, visual.instance_id) {
                // Aim-blocked keeps the intention to aim, but physically relaxes the weapon out of
                // full ADS as the barrel approaches geometry. This mirrors the original add/sub layer.
                (1.0 - obstruction_alpha * 0.82).clamp(0.0, 1.0)
            } else {
                0.0
            };
        let aim_alpha = smooth_first_person_aim_alpha(
            visual.aim_alpha,
            aim_target,
            dt,
            presentation
                .as_ref()
                .map(|presentation| presentation.aim_response_hz)
                .unwrap_or_else(|| {
                    newengine_engine_runtime::gameplay::WeaponPresentationDefinition::default()
                        .aim_response_hz
                }),
        );
        let shot_sequence = if active_visual {
            world
                .get::<PlayerWeaponState>(visual.owner)
                .map(|state| state.shot_sequence)
                .unwrap_or(visual.last_shot_sequence)
        } else {
            visual.last_shot_sequence
        };
        let new_shot = shot_sequence != visual.last_shot_sequence;
        let tuning = world
            .get::<HitscanWeaponTuning>(visual.owner)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let recoil_recovery_hz = tuning.recoil_recovery_hz;
        // NorthStar-style recoil is layered: gameplay/camera recoil and weapon/arms presentation are
        // independent. The previous ratio `camera_kick / authored_visual_kick` collapsed the
        // authored weapon kick back to the tiny camera angle, making the rifle look almost static.
        let visual_recoil_recovery_hz = presentation
            .as_ref()
            .map(|presentation| 2.6 / presentation.fire_kick_duration_seconds.max(0.001))
            .unwrap_or(recoil_recovery_hz)
            .clamp(0.1, 120.0);
        let recoil_scale = 1.0 + (tuning.ads_recoil_multiplier - 1.0) * aim_alpha;
        let signed_noise = |salt: u64| {
            let bits =
                (newengine_math::avalanche_u64(shot_sequence ^ salt) >> 40) as u32 & 0x00ff_ffff;
            (bits as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
        };
        let recoil_alpha = if new_shot {
            presentation
                .as_ref()
                .filter(|presentation| presentation.fire_kick_pitch_radians.abs() > 1.0e-6)
                .map(|_| {
                    // Keep small deterministic shot-to-shot variation, but never derive the
                    // weapon-space amplitude from the camera-space kick. The authored visual kick
                    // owns the rifle/arms layer exactly as NorthStar's fire-start/fire-loop layers do.
                    let variation = 1.0
                        + signed_noise(0x243f_6a88_85a3_08d3)
                            * (tuning.recoil_pitch_random_radians
                                / tuning.recoil_pitch_radians.max(1.0e-4))
                            .clamp(0.0, 0.22);
                    (variation * recoil_scale).clamp(0.0, 2.0)
                })
                .unwrap_or(0.0)
        } else if dt > 0.0 {
            (visual.recoil_alpha * (-visual_recoil_recovery_hz * dt).exp()).clamp(0.0, 4.0)
        } else {
            visual.recoil_alpha
        };
        let recoil_yaw_radians = if new_shot {
            (tuning.recoil_yaw_bias_radians
                + signed_noise(0x1319_8a2e_0370_7344) * tuning.recoil_yaw_radians)
                * recoil_scale
        } else if dt > 0.0 {
            visual.recoil_yaw_radians * (-visual_recoil_recovery_hz * dt).exp()
        } else {
            visual.recoil_yaw_radians
        };
        if let Some(state) = world.get_mut::<EquippedWeaponVisualRoot>(root) {
            state.aim_alpha = aim_alpha;
            state.last_shot_sequence = shot_sequence;
            state.recoil_alpha = recoil_alpha;
            state.recoil_yaw_radians = recoil_yaw_radians;
        }
        if active_visual {
            let weapon_state = world
                .get::<PlayerWeaponState>(visual.owner)
                .copied()
                .unwrap_or_default();
            let reload_active = weapon_state.reload_remaining > 0.0;
            let reload_duration = world
                .get::<HitscanWeaponTuning>(visual.owner)
                .map(|tuning| tuning.sanitized().reload_duration)
                .filter(|duration| *duration > 1.0e-4)
                .unwrap_or(2.0);
            let reload_progress = if reload_active {
                (1.0 - weapon_state.reload_remaining / reload_duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let event = equipment_animation_event(
                active_binding
                    .expect("active visual must have an active binding")
                    .weapon
                    .weapon_type,
                reload_active,
                aim_alpha,
            );
            if let Err(error) = newengine_engine_runtime::gameplay::emit_animation_state(
                world,
                visual.owner,
                "character.equipment",
                event,
                serde_json::json!({
                    "aim_alpha": aim_alpha,
                    "reload_progress": reload_progress,
                    "shot_sequence": shot_sequence,
                    "recoil_alpha": recoil_alpha,
                    "recoil_yaw_radians": recoil_yaw_radians,
                    "obstruction_alpha": obstruction_alpha,
                }),
            ) {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: equipment animation semantic publish failed player={} err='{}'",
                    visual.owner.stable_u64(),
                    error
                );
            }
        }
    }
}
