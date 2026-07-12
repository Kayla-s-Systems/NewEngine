pub fn is_source_asset_ref(asset_ref: &str) -> bool {
    let path = asset_ref
        .split('@')
        .next()
        .unwrap_or(asset_ref)
        .replace('\\', "/")
        .to_ascii_lowercase();
    path.contains("/source/") || path.starts_with("source/") || path.starts_with("ui/src/")
}

pub fn source_runtime_counterpart(asset_ref: &str) -> Option<String> {
    let path = asset_ref
        .split('@')
        .next()
        .unwrap_or(asset_ref)
        .trim()
        .replace('\\', "/");
    if path.is_empty() {
        return None;
    }
    let lower = path.to_ascii_lowercase();

    if let Some(rest) = strip_prefix_case(&path, "maps/source/") {
        return rest
            .strip_suffix(".ymap.xml")
            .map(|stem| format!("maps/{stem}.ymap"));
    }
    if let Some(rest) = strip_prefix_case(&path, "materials/source/") {
        return rest
            .strip_suffix(".nemat.xml")
            .map(|stem| format!("materials/{stem}.nemat"));
    }
    if let Some(rest) = strip_prefix_case(&path, "definitions/source/") {
        return rest
            .strip_suffix(".ytyp.xml")
            .map(|stem| format!("definitions/{stem}.ytyp"));
    }
    if let Some(rest) = strip_prefix_case(&path, "ui/src/") {
        return rest
            .strip_suffix(".neui.xml")
            .map(|stem| format!("ui/{stem}.neui"));
    }
    if let Some(rest) = strip_prefix_case(&path, "items/source/") {
        return rest
            .strip_suffix(".json")
            .map(|stem| format!("items/{stem}.neitems"));
    }

    if lower.ends_with(".ymap") {
        return path
            .strip_prefix("maps/")
            .map(|rest| format!("maps/source/{rest}.xml"));
    }
    if lower.ends_with(".nemat") {
        return path
            .strip_prefix("materials/")
            .map(|rest| format!("materials/source/{rest}.xml"));
    }
    if lower.ends_with(".ytyp") {
        return path
            .strip_prefix("definitions/")
            .map(|rest| format!("definitions/source/{rest}.xml"));
    }
    if lower.ends_with(".neui") {
        return path
            .strip_prefix("ui/")
            .map(|rest| format!("ui/src/{rest}.xml"));
    }
    if lower.ends_with(".neitems") {
        return path
            .strip_prefix("items/")
            .map(|rest| format!("items/source/{}.json", rest.trim_end_matches(".neitems")));
    }

    // Model and texture runtime dictionaries can aggregate several source files.
    // Return the owning source directory rather than inventing a unique file.
    if lower.ends_with(".ydd") {
        let rest = path.strip_prefix("models/")?.trim_end_matches(".ydd");
        return Some(format!("models/source/{rest}"));
    }
    if lower.ends_with(".ytd") {
        let rest = path.strip_prefix("textures/")?.trim_end_matches(".ytd");
        return Some(format!("textures/source/{rest}"));
    }
    None
}

fn strip_prefix_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))?;
    value.get(prefix.len()..)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_bidirectionally_between_runtime_and_authored_xml() {
        assert_eq!(
            source_runtime_counterpart("maps/forest.ymap").as_deref(),
            Some("maps/source/forest.ymap.xml")
        );
        assert_eq!(
            source_runtime_counterpart("maps/source/forest.ymap.xml").as_deref(),
            Some("maps/forest.ymap")
        );
    }

    #[test]
    fn dictionary_counterpart_can_be_a_source_directory() {
        assert_eq!(
            source_runtime_counterpart("textures/world/terrain.ytd").as_deref(),
            Some("textures/source/world/terrain")
        );
    }
}
