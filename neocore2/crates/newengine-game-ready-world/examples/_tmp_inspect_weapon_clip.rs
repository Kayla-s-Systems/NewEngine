use std::{env, fs};
fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: _tmp_inspect_weapon_clip FILE.ycd")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YCD,
        &path,
    )?;
    let dictionary = newengine_animation_runtime::decode_ycd_dictionary(&decoded.body)?;
    for clip in dictionary.clips.iter() {
        println!(
            "name={} skeleton='{}' joints={} frames={} duration={}",
            clip.name,
            clip.skeleton_ref,
            clip.joint_count(),
            clip.frame_count(),
            clip.duration_seconds
        );
    }
    Ok(())
}
