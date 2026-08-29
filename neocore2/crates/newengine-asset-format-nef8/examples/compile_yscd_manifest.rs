use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::{
    encode_yscd_binary_body, yscd, YscdClip, YscdCue, YscdCueDescriptor, YscdDictionary,
};
use newengine_assets_api::{
    encode_list_file, stable_hash_from_text, AssetEntryManifest, ListFileEncodeRequest,
    ListFileHeaderMetadata,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct AuthoredManifest {
    schema: String,
    version: u32,
    cues: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct AuthoredClip {
    name: String,
    source: String,
    weight: f32,
    gain: f32,
    pitch: f32,
    codec: String,
}

impl Default for AuthoredClip {
    fn default() -> Self {
        Self {
            name: String::new(),
            source: String::new(),
            weight: 1.0,
            gain: 1.0,
            pitch: 1.0,
            codec: String::new(),
        }
    }
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err(
            "usage: compile_yscd_manifest <source.yscd.json> <output.yscd> <logical/path.yscd>"
                .to_owned(),
        );
    }
    let source_path = PathBuf::from(&args[0]);
    let output_path = PathBuf::from(&args[1]);
    let logical_path = args[2].replace('\\', "/");
    let source_bytes = fs::read(&source_path)
        .map_err(|error| format!("read '{}' failed: {error}", source_path.display()))?;
    let authored: AuthoredManifest = serde_json::from_slice(&source_bytes)
        .map_err(|error| format!("parse '{}' failed: {error}", source_path.display()))?;
    if authored.schema != "newengine.yscd.manifest.v1" || authored.version != 1 {
        return Err(format!(
            "unsupported YSCD authoring contract schema='{}' version={}",
            authored.schema, authored.version
        ));
    }
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    let mut cues = Vec::with_capacity(authored.cues.len());
    for cue_value in authored.cues {
        let mut object = cue_value
            .as_object()
            .cloned()
            .ok_or("YSCD cue must be an object")?;
        let name = object
            .remove("name")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or("YSCD cue requires string name")?;
        let clips_value = object
            .remove("clips")
            .ok_or_else(|| format!("YSCD cue '{name}' requires clips"))?;
        let authored_clips: Vec<AuthoredClip> = serde_json::from_value(clips_value)
            .map_err(|error| format!("YSCD cue '{name}' clips invalid: {error}"))?;
        let descriptor: YscdCueDescriptor = serde_json::from_value(Value::Object(object))
            .map_err(|error| format!("YSCD cue '{name}' descriptor invalid: {error}"))?;
        let mut clips = Vec::with_capacity(authored_clips.len());
        for clip in authored_clips {
            if clip.name.trim().is_empty() || clip.source.trim().is_empty() {
                return Err(format!("YSCD cue '{name}' has clip with empty name/source"));
            }
            let physical = source_dir.join(&clip.source);
            let bytes = fs::read(&physical).map_err(|error| {
                format!(
                    "YSCD clip '{}' read '{}' failed: {error}",
                    clip.name,
                    physical.display()
                )
            })?;
            let codec = if clip.codec.trim().is_empty() {
                physical
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("wav")
                    .to_ascii_lowercase()
            } else {
                clip.codec.trim().to_ascii_lowercase()
            };
            clips.push(YscdClip {
                name: clip.name,
                source: clip.source.replace('\\', "/"),
                codec,
                weight: clip.weight,
                gain: clip.gain,
                pitch: clip.pitch,
                payload_hash: *blake3::hash(&bytes).as_bytes(),
                bytes,
            });
        }
        for layer in &descriptor.layers {
            for clip_name in &layer.clip_names {
                if !clips
                    .iter()
                    .any(|clip| clip.name.eq_ignore_ascii_case(clip_name))
                {
                    return Err(format!(
                        "YSCD cue '{name}' layer '{}' references missing clip '{clip_name}'",
                        layer.name
                    ));
                }
            }
        }
        cues.push(YscdCue {
            stable_hash: stable_hash_from_text(&name),
            name,
            descriptor,
            clips,
        });
    }
    let dictionary = YscdDictionary { cues };
    let raw_body = encode_yscd_binary_body(&dictionary)?;
    let mut deflater = DeflateEncoder::new(Vec::new(), Compression::best());
    deflater
        .write_all(&raw_body)
        .map_err(|error| format!("YSCD deflate write failed: {error}"))?;
    let stored_body = deflater
        .finish()
        .map_err(|error| format!("YSCD deflate finish failed: {error}"))?;

    let mut metadata = ListFileHeaderMetadata {
        logical_path: logical_path.clone(),
        content_kind: "yscd_sound_cue_dictionary".to_owned(),
        authored_by: "newengine-asset-format-nef8::compile_yscd_manifest".to_owned(),
        source: source_path.to_string_lossy().replace('\\', "/"),
        build_profile: "authoring".to_owned(),
        ..Default::default()
    };
    metadata.entries = dictionary
        .cues
        .iter()
        .map(|cue| {
            AssetEntryManifest::new(
                cue.name.clone(),
                "sound_cue",
                format!("{}@{}", logical_path, cue.name),
            )
        })
        .collect();
    metadata.policy.push(
        "YSCD embeds encoded clip payloads; loose authored WAVs are not runtime dependencies"
            .to_owned(),
    );
    let metadata_bytes = serde_json::to_vec(&metadata)
        .map_err(|error| format!("YSCD header metadata encode failed: {error}"))?;
    let output = encode_list_file(ListFileEncodeRequest {
        content_kind: yscd::CONTENT_KIND,
        content_schema_version: yscd::CONTENT_SCHEMA_VERSION,
        entry_count: dictionary.cues.len() as u32,
        additional_flags: 0,
        min_size_class: 7,
        header_metadata: &metadata_bytes,
        body_stored: &stored_body,
        body_uncompressed_len: raw_body.len() as u64,
        body_raw_hash: Some(*blake3::hash(&raw_body).as_bytes()),
        stable_file_id: Some(stable_hash_from_text(&logical_path)),
        import_settings_hash: Some(stable_hash_from_text(&String::from_utf8_lossy(
            &source_bytes,
        ))),
    })?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create '{}' failed: {error}", parent.display()))?;
    }
    fs::write(&output_path, &output)
        .map_err(|error| format!("write '{}' failed: {error}", output_path.display()))?;
    let decoded = newengine_asset_format_nef8::decode_yscd_nef8(&output, &logical_path)?;
    if decoded.cues.len() != dictionary.cues.len() {
        return Err("YSCD post-write cue count mismatch".to_owned());
    }
    println!(
        "YSCD compiled source='{}' output='{}' logical='{}' cues={} embedded_clips={} bytes={}",
        source_path.display(),
        output_path.display(),
        logical_path,
        decoded.cues.len(),
        decoded
            .cues
            .iter()
            .map(|cue| cue.clips.len())
            .sum::<usize>(),
        output.len()
    );
    Ok(())
}
