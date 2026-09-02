use super::*;

pub(super) fn spawn_authored_audio_emitters(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    emitters: &[AuthoredFpsAudioEmitterSpec],
) {
    for spec in emitters {
        let entity = spawn_named(world, format!("Scene/Audio/{}", spec.id));
        let _ = set_parent(world, entity, Some(parent));
        let _ = world.insert(
            entity,
            Transform {
                position: spec.position,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = world.insert(entity, spec.emitter.clone());
        newengine_ulog_api::ulog::info!(
            "fps-authored audio emitter: entity={:?} id='{}' cue='{}' position={:?} spatial={} gain={:.3} autoplay={} occlusion={} source='YMAP profile.audio.emitters'",
            entity,
            spec.id,
            spec.emitter.cue,
            spec.position,
            spec.emitter.spatial,
            spec.emitter.gain,
            spec.emitter.autoplay,
            spec.emitter.occlusion.enabled,
        );
    }
}
