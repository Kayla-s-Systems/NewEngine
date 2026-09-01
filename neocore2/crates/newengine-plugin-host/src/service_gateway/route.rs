#[inline]
pub(crate) fn provider_route_extends_gateway_parent(
    gateway_id: &str,
    provider_route_id: &str,
) -> bool {
    if provider_route_id == gateway_id {
        return false;
    }

    let gateway_parts = gateway_id.split('.').collect::<Vec<_>>();
    let provider_parts = provider_route_id.split('.').collect::<Vec<_>>();
    if gateway_parts.len() < 2 || provider_parts.len() <= gateway_parts.len() {
        return false;
    }
    if gateway_parts.first() != Some(&"engine")
        || provider_parts.first() != Some(&"engine")
        || gateway_parts.get(1) != provider_parts.get(1)
    {
        return false;
    }

    // A provider may extend the exact gateway directly, e.g.
    // engine.input.bindings -> engine.input.bindings.provider. This is the canonical
    // shape for mods that implement one child gateway without a provider namespace.
    if provider_parts.starts_with(&gateway_parts) {
        return true;
    }

    // Provider-namespaced child gateway implementation: engine.assets.uid ->
    // engine.assets.starvault.uid, engine.assets.textures ->
    // engine.assets.formats.textures, engine.ui.text -> engine.ui.aurelia.text.
    // Provider identity lives directly below the engine domain; the API tail stays intact.
    let child_tail = &gateway_parts[2..];
    let Some(provider_tail) = provider_parts.get(3..) else {
        return false;
    };
    provider_tail.starts_with(child_tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_direct_and_provider_namespaced_child_gateways() {
        assert!(provider_route_extends_gateway_parent(
            "engine.assets.maps",
            "engine.assets.maps.discrete"
        ));
        assert!(provider_route_extends_gateway_parent(
            "engine.assets.textures",
            "engine.assets.formats.textures"
        ));
        assert!(provider_route_extends_gateway_parent(
            "engine.game.module",
            "engine.game.module.fps"
        ));
        assert!(provider_route_extends_gateway_parent(
            "engine.ui.text",
            "engine.ui.aurelia.text"
        ));
        assert!(provider_route_extends_gateway_parent(
            "engine.assets.uid",
            "engine.assets.starvault.uid"
        ));
        assert!(provider_route_extends_gateway_parent(
            "engine.assets.import_queue",
            "engine.assets.starvault.import_queue"
        ));
        assert!(provider_route_extends_gateway_parent(
            "engine.input.bindings",
            "engine.input.bindings.provider"
        ));
    }

    #[test]
    fn accepts_root_provider_route_and_rejects_wrong_tail_or_domain() {
        assert!(provider_route_extends_gateway_parent(
            "engine.render",
            "engine.render.vulkan"
        ));
        assert!(!provider_route_extends_gateway_parent(
            "engine.ui.text",
            "engine.ui.aurelia.layout"
        ));
        assert!(!provider_route_extends_gateway_parent(
            "engine.ui.text",
            "engine.assets.starvault.text"
        ));
        assert!(!provider_route_extends_gateway_parent(
            "engine.ui.text",
            "engine.ui.text"
        ));
    }
}
