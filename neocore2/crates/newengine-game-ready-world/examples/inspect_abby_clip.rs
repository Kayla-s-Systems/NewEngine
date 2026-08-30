use std::{env, fs};

fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: inspect_abby_clip FILE.ycd SELECTOR")?;
    let selector = env::args().nth(2).ok_or("missing selector")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YCD,
        &path,
    )?;
    let clip = newengine_animation_runtime::decode_ycd_body(&decoded.body, Some(&selector))?;
    println!(
        "name={} joints={} frames={} rate={} duration={}",
        clip.name,
        clip.joint_count(),
        clip.frame_count(),
        clip.sample_rate_hz,
        clip.duration_seconds
    );
    for (i, tag) in clip.joint_tags.iter().copied().enumerate() {
        let p = clip.poses[i];
        println!(
            "{} {} t={:?} q={:?} s={:?}",
            i, tag, p.translation, p.rotation, p.scale
        );
    }
    Ok(())
}
