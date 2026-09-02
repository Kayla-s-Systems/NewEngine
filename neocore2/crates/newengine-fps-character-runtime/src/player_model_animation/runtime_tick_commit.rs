/// Phase 4: publish the evaluated frame back to ECS/resources only after all mutable binding work
/// has completed. This keeps side-effect ordering explicit and prevents nested world borrows.
fn commit_player_animation_frame(
    world: &mut newengine_ecs::World,
    player: newengine_ecs::EntityId,
    dt: f32,
    output: PlayerAnimationFrameOutput,
) {
    let PlayerAnimationFrameOutput {
        palette,
        clip_ref,
        active_state,
        foot_pose,
        turn_step_request,
        model_to_world,
        mut timeline_events,
        presentation_core_ms: _,
        finalize_ms: _,
        finalize_timing: _,
    } = output;
    if let Some(foot_pose) = foot_pose {
        let _ = world.insert(player, foot_pose);
    }

    let recycled_palette = if let Some(pose) =
        world.get_mut::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
    {
        let recycled = std::mem::replace(&mut pose.palette, palette);
        pose.revision = pose.revision.saturating_add(1).max(1);
        Some(recycled)
    } else {
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerSkinPose {
                palette,
                revision: 1,
            },
        );
        None
    };
    if let Some(recycled_palette) = recycled_palette {
        if let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) {
            binding.palette_scratch = recycled_palette;
        }
    }
    if let Some(yaw_delta) = turn_step_request {
        let _ = world.insert(
            player,
            newengine_sim::CharacterFacingTurnStepRequest { yaw_delta },
        );
    }
    crate::player_hair::publish_player_hair_pose(world, player, model_to_world);
    crate::animation_events::publish_timeline_events(world, &mut timeline_events);

    if dt > 0.0
        && world
            .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
            .is_some_and(|pose| pose.revision == 2)
    {
        newengine_ulog_api::ulog::info!(
            "fps-character: first animated player palette committed player={} state='{}' clip='{}'",
            player.stable_u64(),
            active_state.clip_hint(),
            clip_ref
        );
    }
}
