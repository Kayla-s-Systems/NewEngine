use super::*;

pub(super) fn entry_matches(raw: &RawDefinitionEntryV1, selector: &str) -> bool {
    if raw.name.eq_ignore_ascii_case(selector) {
        return true;
    }
    if let Some(rest) = selector.strip_prefix("hash:") {
        return rest
            .parse::<u64>()
            .map(|hash| hash == effective_hash(raw))
            .unwrap_or(false);
    }
    false
}

pub(super) fn effective_kind(raw: &RawDefinitionEntryV1) -> String {
    for candidate in [&raw.kind, &raw.entry_kind] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    "archetype_definition".to_owned()
}

pub(super) fn effective_hash(raw: &RawDefinitionEntryV1) -> u64 {
    if raw.stable_hash == 0 {
        stable_hash_from_text(&raw.name)
    } else {
        raw.stable_hash
    }
}

fn value_collect_tags(value: &serde_json::Value, key_hint: &str, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(text) => {
            if key_hint.contains("tag") || key_hint.contains("domain") {
                let t = text.trim();
                if !t.is_empty() {
                    out.insert(t.to_owned());
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                value_collect_tags(item, key_hint, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                value_collect_tags(v, k, out);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_tags(raw: &RawDefinitionEntryV1) -> (Vec<String>, Vec<String>) {
    let mut semantic = BTreeSet::new();
    let mut domain = BTreeSet::new();
    for tag in &raw.semantic_tags {
        let t = tag.trim();
        if !t.is_empty() {
            semantic.insert(t.to_owned());
        }
    }
    for tag in &raw.domain_tags {
        let t = tag.trim();
        if !t.is_empty() {
            domain.insert(t.to_owned());
        }
    }
    for (ns, value) in &raw.namespaces {
        if !ns.trim().is_empty() {
            domain.insert(ns.to_owned());
        }
        value_collect_tags(value, ns, &mut semantic);
    }
    for (ns, value) in &raw.metadata {
        if ns.contains("domain") || ns.starts_with("engine.") {
            domain.insert(ns.to_owned());
        }
        value_collect_tags(value, ns, &mut semantic);
    }
    (semantic.into_iter().collect(), domain.into_iter().collect())
}

fn classify_ref(reference: &str, role: &str, domain: &str, refs: &mut DefinitionRefsV1) {
    let reference = normalize_logical_ref(reference);
    if reference.is_empty() {
        return;
    }
    let lower = reference.to_ascii_lowercase();
    let hint = format!(
        "{} {}",
        role.to_ascii_lowercase(),
        domain.to_ascii_lowercase()
    );
    let bucket = if lower.contains(".ytyd@") || hint.contains("uv") || hint.contains("unwrap") {
        &mut refs.uv_layout_refs
    } else if lower.contains(".ydd@") || hint.contains("drawable") || hint.contains("model") {
        &mut refs.drawable_refs
    } else if lower.contains(".nemat@") || hint.contains("material") {
        &mut refs.material_refs
    } else if lower.contains(".ytd@") || hint.contains("texture") {
        &mut refs.texture_refs
    } else if lower.contains(".ybn@") || lower.contains(".ycol@") || hint.contains("collision") {
        &mut refs.collision_refs
    } else if hint.contains("physics") {
        &mut refs.physics_refs
    } else if lower.contains(".nebrain@")
        || lower.contains(".nepat@")
        || lower.contains(".nemem@")
        || hint.contains("ai")
    {
        &mut refs.ai_refs
    } else if hint.contains("stream") {
        &mut refs.streaming_refs
    } else if hint.contains("editor") {
        &mut refs.editor_refs
    } else {
        &mut refs.other_refs
    };
    if !bucket.iter().any(|it| it == &reference) {
        bucket.push(reference);
    }
}

fn collect_refs_from_value(value: &serde_json::Value, key_hint: &str, refs: &mut DefinitionRefsV1) {
    match value {
        serde_json::Value::String(text) => {
            let normalized = normalize_logical_ref(text);
            let lower = normalized.to_ascii_lowercase();
            if [
                ".ydd@",
                ".nemat@",
                ".ytd@",
                ".ytyd@",
                ".ybn@",
                ".ycol@",
                ".nebrain@",
                ".nepat@",
                ".nemem@",
                ".ytyp@",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
            {
                classify_ref(&normalized, key_hint, key_hint, refs);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_refs_from_value(item, key_hint, refs);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                collect_refs_from_value(v, k, refs);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_refs(raw: &RawDefinitionEntryV1) -> DefinitionRefsV1 {
    let mut refs = DefinitionRefsV1::default();
    for dep in &raw.dependencies {
        classify_ref(&dep.reference, &dep.role, &dep.domain, &mut refs);
    }
    for binding in &raw.material_bindings {
        classify_ref(
            &binding.material_ref,
            &format!("material_slot/{}", binding.slot),
            "engine.assets.materials",
            &mut refs,
        );
    }
    if let Some(target) = &raw.target {
        collect_refs_from_value(target, "target", &mut refs);
    }
    for (key, value) in &raw.namespaces {
        collect_refs_from_value(value, key, &mut refs);
    }
    for (key, value) in &raw.metadata {
        collect_refs_from_value(value, key, &mut refs);
    }
    refs
}

fn imperative_field_name(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "run_code"
            | "script"
            | "script_body"
            | "eval"
            | "function"
            | "call"
            | "callback"
            | "command"
            | "imperative"
            | "spawn_logic"
    )
}

fn side_effect_from_value(
    value: &serde_json::Value,
    out: &mut Vec<DefinitionSideEffectV1>,
    errors: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                side_effect_from_value(item, out, errors);
            }
        }
        serde_json::Value::Object(map) => {
            for key in map.keys() {
                if imperative_field_name(key) {
                    errors.push(format!("imperative side-effect field '{key}' is forbidden; use descriptive domain/effect/target metadata only"));
                }
            }
            let domain = map
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim();
            let effect = map
                .get("effect")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim();
            let target = map
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim();
            if !domain.is_empty() || !effect.is_empty() || !target.is_empty() {
                if domain.is_empty() || effect.is_empty() || target.is_empty() {
                    errors.push(
                        "side-effect declaration requires domain, effect and target".to_owned(),
                    );
                } else {
                    let metadata = map
                        .iter()
                        .filter(|(k, _)| *k != "domain" && *k != "effect" && *k != "target")
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    out.push(DefinitionSideEffectV1 {
                        domain: domain.to_owned(),
                        effect: effect.to_owned(),
                        target: target.to_owned(),
                        metadata,
                    });
                }
            }
            for (k, v) in map {
                if k == "domain" || k == "effect" || k == "target" {
                    continue;
                }
                side_effect_from_value(v, out, errors);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_side_effects(
    raw: &RawDefinitionEntryV1,
) -> Result<Vec<DefinitionSideEffectV1>, Vec<String>> {
    let mut side_effects = raw.side_effects.clone();
    let mut errors = Vec::new();
    for effect in &side_effects {
        if effect.domain.trim().is_empty()
            || effect.effect.trim().is_empty()
            || effect.target.trim().is_empty()
        {
            errors.push(
                "side-effect declaration requires non-empty domain, effect and target".to_owned(),
            );
        }
        for key in effect.metadata.keys() {
            if imperative_field_name(key) {
                errors.push(format!("imperative side-effect field '{key}' is forbidden"));
            }
        }
    }
    for key in [
        "side_effects",
        "sideEffects",
        "effects",
        "runtime_side_effects",
    ] {
        if let Some(value) = raw.metadata.get(key).or_else(|| raw.namespaces.get(key)) {
            side_effect_from_value(value, &mut side_effects, &mut errors);
        }
    }
    if let Some(target) = &raw.target {
        side_effect_from_value(target, &mut side_effects, &mut errors);
    }
    if errors.is_empty() {
        Ok(side_effects)
    } else {
        Err(errors)
    }
}

fn arbitrary_metadata(raw: &RawDefinitionEntryV1) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "namespaces".to_owned(),
        serde_json::to_value(&raw.namespaces).unwrap_or_default(),
    );
    metadata.insert(
        "metadata".to_owned(),
        serde_json::to_value(&raw.metadata).unwrap_or_default(),
    );
    metadata.insert(
        "target".to_owned(),
        raw.target.clone().unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "dependencies".to_owned(),
        serde_json::to_value(&raw.dependencies).unwrap_or_default(),
    );
    metadata.insert(
        "material_bindings".to_owned(),
        serde_json::to_value(&raw.material_bindings).unwrap_or_default(),
    );
    metadata.insert("flags".to_owned(), serde_json::json!(raw.flags));
    let mut unknown = BTreeSet::new();
    for key in raw.namespaces.keys().chain(raw.metadata.keys()) {
        if !key.starts_with("newengine.") && !key.starts_with("engine.") {
            unknown.insert(key.clone());
        }
    }
    metadata.insert(
        "unknown_metadata_namespaces".to_owned(),
        serde_json::json!(unknown.into_iter().collect::<Vec<_>>()),
    );
    metadata
}

fn raw_has_tag(raw: &RawDefinitionEntryV1, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    raw.semantic_tags
        .iter()
        .chain(raw.domain_tags.iter())
        .any(|tag| tag.to_ascii_lowercase() == needle)
}

fn value_string_for_key(value: &serde_json::Value, wanted_key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if map
                .get("key")
                .and_then(|v| v.as_str())
                .map(|key| key.eq_ignore_ascii_case(wanted_key))
                .unwrap_or(false)
            {
                if let Some(text) = map.get("value").and_then(|v| v.as_str()) {
                    return Some(text.to_owned());
                }
            }
            if let Some(text) = map.get(wanted_key).and_then(|v| v.as_str()) {
                return Some(text.to_owned());
            }
            for value in map.values() {
                if let Some(found) = value_string_for_key(value, wanted_key) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| value_string_for_key(item, wanted_key)),
        _ => None,
    }
}

fn raw_render_string(raw: &RawDefinitionEntryV1, key: &str) -> Option<String> {
    raw.metadata
        .get("render")
        .or_else(|| raw.metadata.get("newengine.render"))
        .or_else(|| raw.namespaces.get("render"))
        .or_else(|| raw.namespaces.get("newengine.render"))
        .and_then(|value| value_string_for_key(value, key))
}

fn render_options_from_role(role: &str) -> Option<MeshRenderOptions> {
    match role.trim().to_ascii_lowercase().as_str() {
        "world_opaque" | "opaque" => Some(MeshRenderOptions::world_opaque()),
        "terrain_patch" | "terrain" => Some(MeshRenderOptions::terrain_patch()),
        "foliage_instanced" | "foliage" | "tree" => Some(MeshRenderOptions::foliage_instanced()),
        "character_body" | "character" | "player" => Some(MeshRenderOptions::character_body()),
        "first_person_view_model" | "view_model" | "fps_view_model" => {
            Some(MeshRenderOptions::first_person_view_model())
        }
        "sky_background" | "sky" => Some(MeshRenderOptions::sky_background()),
        "celestial_billboard" => Some(MeshRenderOptions::celestial_billboard()),
        _ => None,
    }
}

fn shadow_policy_from_string(value: &str) -> Option<MeshShadowPolicy> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "none" | "off" | "disabled" => Some(MeshShadowPolicy::None),
        "cast" | "cast_only" | "caster" => Some(MeshShadowPolicy::CastOnly),
        "receive" | "receive_only" | "receiver" | "recive" | "recive_only" => {
            Some(MeshShadowPolicy::ReceiveOnly)
        }
        "cast_and_receive" | "cast_receive" | "cast_and_recive" | "cast_recive" => {
            Some(MeshShadowPolicy::CastAndReceive)
        }
        "profile" | "profile_controlled" | "profiled" => Some(MeshShadowPolicy::ProfileControlled),
        _ => None,
    }
}

fn apply_shadow_policy_from_metadata(
    raw: &RawDefinitionEntryV1,
    mut options: MeshRenderOptions,
) -> MeshRenderOptions {
    if let Some(policy) = raw_render_string(raw, "shadow_policy")
        .or_else(|| raw_render_string(raw, "shadow.policy"))
        .or_else(|| raw_render_string(raw, "render.shadow_policy"))
        .or_else(|| raw_render_string(raw, "render.shadow.policy"))
        .and_then(|value| shadow_policy_from_string(&value))
    {
        options.shadow_policy = policy;
    }
    options
}

fn infer_render_options(raw: &RawDefinitionEntryV1, _refs: &DefinitionRefsV1) -> MeshRenderOptions {
    let options = if let Some(options) = raw_render_string(raw, "mesh.role")
        .or_else(|| raw_render_string(raw, "role"))
        .and_then(|role| render_options_from_role(&role))
    {
        options
    } else if raw_has_tag(raw, "sky") {
        MeshRenderOptions::sky_background()
    } else if raw_has_tag(raw, "terrain") {
        MeshRenderOptions::terrain_patch()
    } else if raw_has_tag(raw, "foliage") || raw_has_tag(raw, "tree") {
        MeshRenderOptions::foliage_instanced()
    } else if raw_has_tag(raw, "player") || raw_has_tag(raw, "character") {
        MeshRenderOptions::character_body()
    } else {
        MeshRenderOptions::world_opaque()
    };
    apply_shadow_policy_from_metadata(raw, options)
}

fn build_model_explanation(
    source: &str,
    raw: &RawDefinitionEntryV1,
    refs: &DefinitionRefsV1,
) -> ModelExplanationV1 {
    let drawable_ref = refs.drawable_refs.first().cloned();
    ModelExplanationV1 {
        source: source.to_owned(),
        model_ref: drawable_ref.clone(),
        drawable_ref,
        material_bindings: raw.material_bindings.clone(),
        material_refs: refs.material_refs.clone(),
        texture_refs: refs.texture_refs.clone(),
        uv_layout_refs: refs.uv_layout_refs.clone(),
        physics_refs: refs.physics_refs.clone(),
        collision_refs: refs.collision_refs.clone(),
        render_options: infer_render_options(raw, refs),
        collision_policy: raw_render_string(raw, "collision.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        uv_policy: raw_render_string(raw, "uv.policy").unwrap_or_else(|| "authored".to_owned()),
        physics_policy: raw_render_string(raw, "physics.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        lod_policy: raw_render_string(raw, "lod.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        streaming_policy: raw_render_string(raw, "streaming.policy")
            .unwrap_or_else(|| "unspecified".to_owned()),
        explanation:
            "YTYP descriptor binds .ydd slots to materials and declares render/collision/LOD policy"
                .to_owned(),
        ..Default::default()
    }
}

pub(super) fn build_entry(
    source: &str,
    raw: RawDefinitionEntryV1,
    inherited_warnings: &[String],
) -> Result<DefinitionEntryV1, String> {
    let name = raw.name.trim().to_owned();
    if name.is_empty() {
        return Err(".ytyp Definition Entry has empty identity.name".to_owned());
    }
    let side_effects = collect_side_effects(&raw).map_err(|errors| errors.join("; "))?;
    let stable_hash = effective_hash(&raw);
    let refs = collect_refs(&raw);
    let model_explanation = build_model_explanation(source, &raw, &refs);
    let (semantic_tags, domain_tags) = collect_tags(&raw);
    Ok(DefinitionEntryV1 {
        identity: DefinitionIdentityV1 {
            name: name.clone(),
            source: source.to_owned(),
            definition_ref: format!("{source}@{name}"),
        },
        kind: effective_kind(&raw),
        stable_hash,
        semantic_tags,
        domain_tags,
        refs,
        model_explanation,
        side_effects,
        arbitrary_metadata: arbitrary_metadata(&raw),
        warnings: inherited_warnings.to_vec(),
        ..Default::default()
    })
}
