#[inline]
fn edge_direct_path_distance_sq(
    edge: CachedWorldEdge,
    source: [f32; 3],
    receiver: [f32; 3],
) -> f32 {
    let midpoint = [
        (edge.endpoints[0][0] + edge.endpoints[1][0]) * 0.5,
        (edge.endpoints[0][1] + edge.endpoints[1][1]) * 0.5,
        (edge.endpoints[0][2] + edge.endpoints[1][2]) * 0.5,
    ];
    let direct = [
        receiver[0] - source[0],
        receiver[1] - source[1],
        receiver[2] - source[2],
    ];
    let direct_len_sq = direct[0] * direct[0] + direct[1] * direct[1] + direct[2] * direct[2];
    if direct_len_sq <= 1.0e-8 {
        let dx = midpoint[0] - source[0];
        let dy = midpoint[1] - source[1];
        let dz = midpoint[2] - source[2];
        return dx * dx + dy * dy + dz * dz;
    }
    let from_source = [
        midpoint[0] - source[0],
        midpoint[1] - source[1],
        midpoint[2] - source[2],
    ];
    let t =
        ((from_source[0] * direct[0] + from_source[1] * direct[1] + from_source[2] * direct[2])
            / direct_len_sq)
            .clamp(0.0, 1.0);
    let closest = [
        source[0] + direct[0] * t,
        source[1] + direct[1] * t,
        source[2] + direct[2] * t,
    ];
    let dx = midpoint[0] - closest[0];
    let dy = midpoint[1] - closest[1];
    let dz = midpoint[2] - closest[2];
    dx * dx + dy * dy + dz * dz
}
