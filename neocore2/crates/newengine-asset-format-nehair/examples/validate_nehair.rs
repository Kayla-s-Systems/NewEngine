use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        env::args()
            .nth(1)
            .ok_or("usage: validate_nehair <asset.nehair>")?,
    );
    if env::args().nth(2).is_some() {
        return Err("too many arguments".into());
    }
    let bytes = fs::read(&path)?;
    let groom = newengine_asset_format_nehair::decode_nehair(&bytes)?;
    println!(
        "valid NEHAIR: path={} groom={} points={} strands={} segments={} capsules={} followers={} bytes={}",
        path.display(),
        groom.groom.as_str(),
        groom.guide_points.len(),
        groom.guide_strands.len(),
        groom.guide_segment_count(),
        groom.collision_capsules.len(),
        groom.follow_strands_per_guide,
        bytes.len(),
    );
    Ok(())
}
