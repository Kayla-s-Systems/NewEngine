use super::*;

pub(super) fn encode_skeleton_xml(skeleton: &DecodedSkeleton) -> Vec<u8> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Metadata>\n");
    out.push_str(&format!(
        "  <Skeleton source_format=\"northstar.northstar.pc.joint_hierarchy.v1\" name=\"{}\">\n",
        xml_escape(&skeleton.name)
    ));
    for joint in &skeleton.joints {
        let parent = joint
            .parent_index
            .map(|index| skeleton.joints[index as usize].name.as_str());
        out.push_str("    <Joint");
        out.push_str(&format!(
            " index=\"{}\" tag=\"{}\" name=\"{}\"",
            joint.index,
            joint.tag,
            xml_escape(&joint.name)
        ));
        if let Some(parent) = parent {
            out.push_str(&format!(
                " parent=\"{}\" parent_index=\"{}\"",
                xml_escape(parent),
                joint.parent_index.unwrap_or_default()
            ));
        } else {
            out.push_str(" parent_index=\"-1\"");
        }
        out.push_str(&format!(
            " tx=\"{:.9}\" ty=\"{:.9}\" tz=\"{:.9}\" qx=\"{:.9}\" qy=\"{:.9}\" qz=\"{:.9}\" qw=\"{:.9}\" sx=\"{:.9}\" sy=\"{:.9}\" sz=\"{:.9}\" />\n",
            joint.position_ls[0], joint.position_ls[1], joint.position_ls[2],
            joint.rotation_ls[0], joint.rotation_ls[1], joint.rotation_ls[2], joint.rotation_ls[3],
            joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2],
        ));
    }
    out.push_str(&format!(
        "    <Anchors root=\"{}\" hips=\"{}\" head=\"{}\" left_hand=\"{}\" right_hand=\"{}\" left_foot=\"{}\" right_foot=\"{}\" eye=\"{}\" eye_height=\"{:.6}\" />\n",
        xml_escape(&skeleton.root),
        xml_escape(&skeleton.hips),
        xml_escape(&skeleton.head),
        xml_escape(&skeleton.left_hand),
        xml_escape(&skeleton.right_hand),
        xml_escape(&skeleton.left_foot),
        xml_escape(&skeleton.right_foot),
        xml_escape(&skeleton.eye),
        skeleton.eye_height,
    ));
    out.push_str("  </Skeleton>\n</Metadata>\n");
    out.into_bytes()
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("failed to read '{}': {error}", path.display()))
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!(
        "{}.importing",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("asset")
    ));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("failed to write '{}': {error}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("failed to replace '{}': {error}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("failed to publish '{}': {error}", path.display()))
}
