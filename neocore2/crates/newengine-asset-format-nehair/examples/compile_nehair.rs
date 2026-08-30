use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let source = PathBuf::from(args.next().ok_or(
        "usage: compile_nehair <groom.json> <skeleton.json> <output.nehair> [logical_groom_ref]",
    )?);
    let skeleton = PathBuf::from(args.next().ok_or("missing skeleton.json")?);
    let output = PathBuf::from(args.next().ok_or("missing output.nehair")?);
    let logical_groom_ref = args.next().unwrap_or_else(|| {
        output
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let source_bytes = fs::read(&source)?;
    let skeleton_bytes = fs::read(&skeleton)?;
    let skeleton: newengine_model_skeleton_api::ModelSkeletonMetadata =
        serde_json::from_slice(&skeleton_bytes)?;
    let groom = newengine_asset_format_nehair::compile_authored_groom_json(
        &source_bytes,
        &logical_groom_ref,
        &skeleton,
    )?;
    let encoded = newengine_asset_format_nehair::encode_nehair_v1(&groom)?;
    let decoded = newengine_asset_format_nehair::decode_nehair(&encoded)?;
    if decoded != groom {
        return Err("NEHAIR compiler self-verification mismatch".into());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &encoded)?;
    println!(
        "compiled NEHAIR: source={} output={} groom={} points={} strands={} capsules={} bytes={}",
        source.display(),
        output.display(),
        groom.groom.as_str(),
        groom.guide_points.len(),
        groom.guide_strands.len(),
        groom.collision_capsules.len(),
        encoded.len(),
    );
    Ok(())
}
