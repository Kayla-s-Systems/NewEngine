#[inline]
fn vec3_array_distance(value: [f32; 3], point: Vec3) -> f32 {
    Vec3::new(value[0], value[1], value[2]).distance(point)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PortalRouteMetric {
    Direct,
    Indirect,
}

fn portal_edge_gain(portal: &AudioPortal, metric: PortalRouteMetric) -> f32 {
    match metric {
        PortalRouteMetric::Direct => portal.direct_route_gain(),
        PortalRouteMetric::Indirect => portal.route_gain(),
    }
}

/// Max-product routing with deterministic path reconstruction. Because every portal edge is in
/// `[0,1]`, selecting the currently strongest unvisited route is the multiplicative equivalent of
/// Dijkstra and cannot be improved later by a cycle.
fn strongest_portal_route_map(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
    metric: PortalRouteMetric,
) -> BTreeMap<String, PortalRoute> {
    let known_zone_ids = zones
        .iter()
        .map(|zone| zone.zone.zone_id.clone())
        .collect::<BTreeSet<_>>();
    let listener_id = zones
        .get(listener_zone_index)
        .map(|zone| zone.zone.zone_id.clone())
        .unwrap_or_default();
    if listener_id.is_empty() {
        return BTreeMap::new();
    }

    let mut routes = BTreeMap::<String, PortalRoute>::new();
    routes.insert(
        listener_id,
        PortalRoute {
            gain: 1.0,
            portal_ids: Vec::new(),
        },
    );
    let mut visited = BTreeSet::<String>::new();

    loop {
        let current_zone = routes
            .iter()
            .filter(|(zone_id, route)| !visited.contains(*zone_id) && route.gain > 0.0)
            .max_by(|(zone_a, route_a), (zone_b, route_b)| {
                route_a
                    .gain
                    .total_cmp(&route_b.gain)
                    // For equal gain, lexically smaller zone id wins deterministically.
                    .then_with(|| zone_b.cmp(zone_a))
            })
            .map(|(zone_id, _)| zone_id.clone());
        let Some(current_zone) = current_zone else {
            break;
        };
        let current = routes
            .get(&current_zone)
            .cloned()
            .expect("selected route exists");
        visited.insert(current_zone.clone());

        for portal in portals {
            let next_zone = if portal.zone_a == current_zone {
                &portal.zone_b
            } else if portal.zone_b == current_zone {
                &portal.zone_a
            } else {
                continue;
            };
            if !known_zone_ids.contains(next_zone) || visited.contains(next_zone) {
                continue;
            }
            let edge = portal_edge_gain(portal, metric);
            if edge <= 0.0 {
                continue;
            }
            let mut candidate = current.clone();
            candidate.gain = (candidate.gain * edge).clamp(0.0, 1.0);
            candidate.portal_ids.push(portal.portal_id.clone());
            let replace = routes.get(next_zone).is_none_or(|existing| {
                candidate.gain > existing.gain + 1.0e-7
                    || ((candidate.gain - existing.gain).abs() <= 1.0e-7
                        && candidate.portal_ids < existing.portal_ids)
            });
            if replace {
                routes.insert(next_zone.clone(), candidate);
            }
        }
    }
    routes
}

fn strongest_direct_portal_routes(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
) -> BTreeMap<String, PortalRoute> {
    strongest_portal_route_map(
        zones,
        portals,
        listener_zone_index,
        PortalRouteMetric::Direct,
    )
}

#[cfg(test)]
fn strongest_portal_routes(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
) -> BTreeMap<String, f32> {
    let routes = strongest_portal_route_map(
        zones,
        portals,
        listener_zone_index,
        PortalRouteMetric::Indirect,
    );
    zones
        .iter()
        .map(|zone| {
            let gain = routes
                .get(&zone.zone.zone_id)
                .map(|route| route.gain)
                .unwrap_or(0.0);
            (zone.zone.zone_id.clone(), gain)
        })
        .collect()
}

#[cfg(test)]
fn direct_portal_response(
    portal: &AudioPortal,
    center: Vec3,
    emitter_position: Vec3,
    listener_position: Vec3,
) -> AudioDirectPathResponse {
    let route = PortalRoute {
        gain: portal.direct_route_gain(),
        portal_ids: vec![portal.portal_id.clone()],
    };
    let mut portals = vec![portal.clone()];
    portals.sort_by(|a, b| a.portal_id.cmp(&b.portal_id));
    let centers = BTreeMap::from([(portal.portal_id.clone(), center)]);
    direct_portal_route_response(
        &route,
        &portals,
        &centers,
        emitter_position,
        listener_position,
    )
    .map(|(response, _, _)| response)
    .unwrap_or(AudioDirectPathResponse {
        gain: route.gain,
        ..AudioDirectPathResponse::clear()
    })
}

fn portal_route_boundary_centers(
    route: &PortalRoute,
    portal_centers: &BTreeMap<String, Vec3>,
) -> Option<(Option<Vec3>, Option<Vec3>)> {
    if route.portal_ids.is_empty() {
        return None;
    }
    // Stored listener->destination. Emitter-side boundary is therefore the final id; the
    // listener-side boundary is the first id.
    let source = portal_centers.get(route.portal_ids.last()?).copied()?;
    let listener = portal_centers.get(route.portal_ids.first()?).copied()?;
    Some((Some(source), Some(listener)))
}

/// Resolves the strongest topological direct route into actual portal waypoints. Route portal ids
/// are stored listener->destination, so they are reversed here to walk emitter->listener.
/// Every aperture contributes its own diffraction loss while extra delay is derived once from the
/// complete polyline length.
fn direct_portal_route_response(
    route: &PortalRoute,
    portals: &[AudioPortal],
    portal_centers: &BTreeMap<String, Vec3>,
    emitter_position: Vec3,
    listener_position: Vec3,
) -> Option<(AudioDirectPathResponse, Option<Vec3>, Option<Vec3>)> {
    if route.portal_ids.is_empty() || route.gain <= 0.0 {
        return None;
    }
    let mut route_portals = Vec::<AudioPortal>::with_capacity(route.portal_ids.len());
    let mut centers = Vec::<Vec3>::with_capacity(route.portal_ids.len());
    for portal_id in route.portal_ids.iter().rev() {
        let portal = portals
            .iter()
            .find(|portal| portal.portal_id == *portal_id)?;
        let center = portal_centers.get(portal_id).copied()?;
        route_portals.push(portal.clone().sanitized());
        centers.push(center);
    }

    let direct_length = emitter_position.distance(listener_position).max(1.0e-4);
    let mut routed_length = 0.0_f32;
    let mut previous = emitter_position;
    for center in &centers {
        routed_length += previous.distance(*center);
        previous = *center;
    }
    routed_length += previous.distance(listener_position);
    let total_excess = (routed_length - direct_length).max(0.0);

    let mut gain = 1.0_f32;
    let mut high_frequency_gain = 1.0_f32;
    for index in 0..route_portals.len() {
        let portal = &route_portals[index];
        let center = centers[index];
        let previous = if index == 0 {
            emitter_position
        } else {
            centers[index - 1]
        };
        let next = if index + 1 == centers.len() {
            listener_position
        } else {
            centers[index + 1]
        };
        let local_direct = previous.distance(next).max(1.0e-4);
        let local_routed = previous.distance(center) + center.distance(next);
        let local_excess = (local_routed - local_direct).max(0.0);
        let aperture_radius =
            (2.0 * portal.half_extents[0].min(portal.half_extents[1])) * portal.openness.sqrt();
        let aperture_factor = (aperture_radius / (aperture_radius + 0.20)).clamp(0.0, 1.0);
        let bend = local_excess / (aperture_radius + 0.10);
        let geometric_gain = (0.75 + 0.25 * aperture_factor) / (1.0 + 0.55 * bend);
        let edge_hf = (aperture_factor / (1.0 + 0.90 * bend)).clamp(0.02, 1.0);
        gain *= portal.direct_route_gain() * geometric_gain;
        high_frequency_gain *= edge_hf;
    }

    let source_boundary = centers.first().copied();
    let listener_boundary = centers.last().copied();
    let response = AudioDirectPathResponse {
        gain: gain.clamp(0.0, 1.0),
        high_frequency_gain: high_frequency_gain.clamp(0.001, 1.0),
        low_pass_hz: (900.0 + 19_100.0 * high_frequency_gain.sqrt()).clamp(900.0, 20_000.0),
        extra_delay_ms: (total_excess / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 500.0),
    }
    .sanitized();
    Some((response, source_boundary, listener_boundary))
}
