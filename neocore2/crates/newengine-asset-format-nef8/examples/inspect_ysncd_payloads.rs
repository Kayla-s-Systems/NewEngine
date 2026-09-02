use std::{collections::BTreeMap, env, fs, path::PathBuf};

use newengine_asset_format_nef8::{decode_ysncd_nef8, YSNCD_BINARY_SCHEMA_VERSION};
use newengine_assets_api::LIST_FILE_CONTENT_KIND_YSNCD;

fn main() {
    if let Err(error) = run() {
        eprintln!("inspect_ysncd_payloads failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: inspect_ysncd_payloads <file.ysncd>".to_owned())?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("read '{}' failed: {error}", path.display()))?;
    let dictionary = decode_ysncd_nef8(
        &bytes,
        &path.to_string_lossy(),
        LIST_FILE_CONTENT_KIND_YSNCD,
        YSNCD_BINARY_SCHEMA_VERSION,
    )?;
    let mut codecs = BTreeMap::<String, usize>::new();
    let mut magics = BTreeMap::<String, usize>::new();
    let mut clips = 0usize;
    for cue in &dictionary.cues {
        for clip in &cue.clips {
            clips += 1;
            *codecs.entry(clip.codec.clone()).or_default() += 1;
            let magic = clip
                .bytes
                .get(0..4)
                .map(|value| String::from_utf8_lossy(value).into_owned())
                .unwrap_or_else(|| "<short>".to_owned());
            *magics.entry(magic).or_default() += 1;
        }
    }
    println!(
        "YSNCD cues={} clips={} codecs={codecs:?} magics={magics:?}",
        dictionary.cues.len(),
        clips
    );
    Ok(())
}
