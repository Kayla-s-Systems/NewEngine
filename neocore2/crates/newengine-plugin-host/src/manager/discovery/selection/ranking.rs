fn is_better_plugin_candidate(
    candidate: &super::graph::ScannedDynlib,
    current: &super::graph::ScannedDynlib,
) -> bool {
    let candidate_rank = plugin_candidate_rank(candidate);
    let current_rank = plugin_candidate_rank(current);
    candidate_rank
        .cmp(&current_rank)
        .then_with(|| {
            candidate
                .path
                .to_string_lossy()
                .cmp(&current.path.to_string_lossy())
        })
        .is_gt()
}

fn plugin_candidate_rank(item: &super::graph::ScannedDynlib) -> ((u64, u64, u64, u64), usize) {
    let (version, declared_capabilities) = match &item.kind {
        ScannedDynlibKind::Plugin {
            version,
            declared_capabilities,
            ..
        } => (semver_rank(version), declared_capabilities.unwrap_or(0)),
        _ => ((0, 0, 0, 0), 0),
    };

    // The final path string is only a deterministic tie-breaker for two artifacts
    // with equal id/version/capability metadata. It is not provider selection.
    (version, declared_capabilities)
}

fn semver_rank(version: &str) -> (u64, u64, u64, u64) {
    let core = version
        .split_once('+')
        .map(|(l, _)| l)
        .unwrap_or(version)
        .split_once('-')
        .map(|(l, _)| l)
        .unwrap_or(version);

    let mut parts = core.split('.').map(|part| part.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}
