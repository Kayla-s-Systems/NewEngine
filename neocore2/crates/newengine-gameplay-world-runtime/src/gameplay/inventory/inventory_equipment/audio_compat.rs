use super::*;

/// Legacy audio helpers are kept as a compatibility API for external clients. The engine
/// equipment/combat path no longer calls them directly; projects subscribe to semantic
/// gameplay events and choose their own cues/actions.
pub fn preload_weapon_audio_definition(audio: &WeaponAudioDefinition) {
    for action in [
        WeaponAudioAction::Fire,
        WeaponAudioAction::ReloadStart,
        WeaponAudioAction::ReloadComplete,
        WeaponAudioAction::Equip,
        WeaponAudioAction::Unequip,
        WeaponAudioAction::Empty,
        WeaponAudioAction::ShellEject,
    ] {
        let Some(reference) = audio.clip(action) else {
            continue;
        };
        let result = if is_ysncd_cue_reference(reference) {
            newengine_audio_client::preload_audio_cue(
                &newengine_audio_api::AudioCuePreloadRequest {
                    cue: newengine_audio_api::SoundCueRef::new(reference.to_owned()),
                },
            )
        } else {
            newengine_audio_client::preload_audio_clip(&newengine_audio_api::AudioPreloadRequest {
                clip: newengine_audio_api::AudioClipRef::new(reference.to_owned()),
            })
        };
        match result {
            Ok(Some(ack)) if ack.accepted => {
                newengine_ulog_api::ulog::info!(
                    "weapon audio preload: action={:?} ref='{}' kind='{}' provider='{}' bytes={} cached={} status='ready'",
                    action,
                    reference,
                    if is_ysncd_cue_reference(reference) { "ysncd-cue" } else { "clip" },
                    ack.provider,
                    ack.bytes,
                    ack.cached,
                );
                for diagnostic in &ack.diagnostics {
                    newengine_ulog_api::ulog::info!("{}", diagnostic);
                }
            }
            Ok(Some(ack)) => newengine_ulog_api::ulog::warn!(
                "weapon audio preload rejected: action={:?} ref='{}' provider='{}'",
                action,
                reference,
                ack.provider,
            ),
            Ok(None) => newengine_ulog_api::ulog::warn!(
                "weapon audio preload unavailable: action={:?} ref='{}' reason='engine.audio returned no provider response'",
                action,
                reference,
            ),
            Err(error) => newengine_ulog_api::ulog::warn!(
                "weapon audio preload failed: action={:?} ref='{}' err='{}'",
                action,
                reference,
                error,
            ),
        }
    }
}

#[inline]
fn is_ysncd_cue_reference(reference: &str) -> bool {
    newengine_assets_api::parse_asset_reference(reference)
        .map(|reference| {
            reference.has_extension("ysncd")
                && reference
                    .entry
                    .as_deref()
                    .is_some_and(|entry| !entry.trim().is_empty())
        })
        .unwrap_or(false)
}

pub fn play_weapon_item_audio(
    world: &World,
    owner: EntityId,
    item: ItemId,
    action: WeaponAudioAction,
) {
    let component_audio_override = world
        .get::<EquippedWeaponBinding>(owner)
        .copied()
        .filter(|binding| binding.item == item)
        .and_then(|_| active_equipped_weapon_component_overrides(world, owner).0);
    let Some(reference) = component_audio_override.or_else(|| {
        world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(item))
            .and_then(|definition| definition.weapon_audio.clip(action))
            .map(ToOwned::to_owned)
    }) else {
        return;
    };
    let component_gain = world
        .get::<EquippedWeaponBinding>(owner)
        .copied()
        .filter(|binding| binding.item == item)
        .map(|_| active_equipped_weapon_component_modifiers(world, owner).audio_gain_multiplier)
        .unwrap_or(1.0);
    let spatial_position = match action {
        WeaponAudioAction::Fire | WeaponAudioAction::ShellEject => world
            .get::<EquippedWeaponMuzzle>(owner)
            .map(|muzzle| muzzle.position)
            .or_else(|| {
                world
                    .get::<Transform>(owner)
                    .map(|transform| transform.position)
            }),
        _ => world
            .get::<Transform>(owner)
            .map(|transform| transform.position),
    };

    let is_cue = is_ysncd_cue_reference(&reference);
    let result = if is_cue {
        let mut request = newengine_audio_api::AudioCuePlayRequest::new(reference.clone());
        request.gain = component_gain;
        request.position = spatial_position.map(|position| [position.x, position.y, position.z]);
        request.scope_id = Some(owner.stable_u64());
        newengine_audio_client::play_audio_cue(&request)
    } else {
        let mut request = newengine_audio_api::AudioPlayRequest::new(reference.clone());
        request.gain = component_gain;
        request.spatial =
            spatial_position.map(|position| newengine_audio_api::AudioSpatialParams {
                position: [position.x, position.y, position.z],
            });
        newengine_audio_client::play_audio_clip(&request)
    };

    match result {
        Ok(Some(ack)) if ack.accepted => {
            if matches!(action, WeaponAudioAction::Fire | WeaponAudioAction::Empty) {
                newengine_ulog_api::ulog::info!(
                    "weapon audio play: action={:?} ref='{}' kind='{}' provider='{}' voice_id={:?} virtualized={} status='accepted'",
                    action,
                    reference,
                    if is_cue { "ysncd-cue" } else { "clip" },
                    ack.provider,
                    ack.voice_id,
                    ack.virtualized,
                );
            }
            for diagnostic in &ack.diagnostics {
                newengine_ulog_api::ulog::info!("{}", diagnostic);
            }
        }
        Ok(Some(ack)) => newengine_ulog_api::ulog::warn!(
            "weapon audio play rejected: action={:?} ref='{}' provider='{}' message='{}'",
            action,
            reference,
            ack.provider,
            ack.message,
        ),
        Ok(None) => newengine_ulog_api::ulog::warn!(
            "weapon audio play unavailable: action={:?} ref='{}' reason='engine.audio returned no provider response'",
            action,
            reference,
        ),
        Err(error) => newengine_ulog_api::ulog::warn!(
            "weapon audio play failed: action={:?} ref='{}' err='{}'",
            action,
            reference,
            error,
        ),
    }
}

pub fn play_equipped_weapon_audio(world: &World, owner: EntityId, action: WeaponAudioAction) {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner) else {
        return;
    };
    play_weapon_item_audio(world, owner, binding.item, action);
}
