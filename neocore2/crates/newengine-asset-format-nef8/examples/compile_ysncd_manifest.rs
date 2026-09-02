use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::{
    encode_ysncd_binary_body, YsncdClip, YsncdCue, YsncdCueDescriptor, YsncdDictionary,
    YSNCD_BINARY_SCHEMA_VERSION,
};
use newengine_assets_api::{
    encode_list_file, stable_hash_from_text, AssetEntryManifest, ListFileEncodeRequest,
    ListFileHeaderMetadata, LIST_FILE_CONTENT_KIND_YSNCD,
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

fn load_runtime_clip_payload(
    physical: &Path,
    authored_codec: &str,
) -> Result<(Vec<u8>, String), String> {
    let extension = physical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "wav" {
        let mut reader = hound::WavReader::open(physical)
            .map_err(|error| format!("open WAV '{}' failed: {error}", physical.display()))?;
        let spec = reader.spec();
        if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
            return Err(format!(
                "YSNCD XVAG migration requires PCM16 WAV source '{}', got {:?}/{}-bit",
                physical.display(),
                spec.sample_format,
                spec.bits_per_sample
            ));
        }
        let samples = reader
            .samples::<i16>()
            .map(|sample| {
                sample
                    .map(|value| f32::from(value) / 32768.0)
                    .map_err(|error| format!("decode WAV '{}' failed: {error}", physical.display()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bytes =
            newengine_audio_xvag::encode_xvag_ps_adpcm(spec.sample_rate, spec.channels, &samples)?;
        return Ok((bytes, "xvag".to_owned()));
    }

    let bytes = fs::read(physical)
        .map_err(|error| format!("read audio '{}' failed: {error}", physical.display()))?;
    let codec = if authored_codec.trim().is_empty() {
        extension
    } else {
        authored_codec.trim().to_ascii_lowercase()
    };
    Ok((bytes, codec))
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err(
            "usage: compile_ysncd_manifest <source.ysncd.json> <output.ysncd> <logical/path.ysncd>"
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
    if authored.schema != "newengine.ysncd.manifest.v1" || authored.version != 1 {
        return Err(format!(
            "unsupported YSNCD authoring contract schema='{}' version={}",
            authored.schema, authored.version
        ));
    }
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    let mut cues = Vec::with_capacity(authored.cues.len());
    for cue_value in authored.cues {
        let mut object = cue_value
            .as_object()
            .cloned()
            .ok_or("YSNCD cue must be an object")?;
        let name = object
            .remove("name")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or("YSNCD cue requires string name")?;
        let clips_value = object
            .remove("clips")
            .ok_or_else(|| format!("YSNCD cue '{name}' requires clips"))?;
        let authored_clips: Vec<AuthoredClip> = serde_json::from_value(clips_value)
            .map_err(|error| format!("YSNCD cue '{name}' clips invalid: {error}"))?;
        let descriptor: YsncdCueDescriptor = serde_json::from_value(Value::Object(object))
            .map_err(|error| format!("YSNCD cue '{name}' descriptor invalid: {error}"))?;
        let mut clips = Vec::with_capacity(authored_clips.len());
        for clip in authored_clips {
            if clip.name.trim().is_empty() || clip.source.trim().is_empty() {
                return Err(format!(
                    "YSNCD cue '{name}' has clip with empty name/source"
                ));
            }
            let physical = source_dir.join(&clip.source);
            let (bytes, codec) =
                load_runtime_clip_payload(&physical, &clip.codec).map_err(|error| {
                    format!(
                        "YSNCD clip '{}' runtime payload build failed '{}': {error}",
                        clip.name,
                        physical.display()
                    )
                })?;
            clips.push(YsncdClip {
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
                        "YSNCD cue '{name}' layer '{}' references missing clip '{clip_name}'",
                        layer.name
                    ));
                }
            }
        }
        cues.push(YsncdCue {
            stable_hash: stable_hash_from_text(&name),
            name,
            descriptor,
            clips,
        });
    }
    let dictionary = YsncdDictionary { cues };
    let raw_body = encode_ysncd_binary_body(&dictionary)?;
    let mut deflater = DeflateEncoder::new(Vec::new(), Compression::best());
    deflater
        .write_all(&raw_body)
        .map_err(|error| format!("YSNCD deflate write failed: {error}"))?;
    let stored_body = deflater
        .finish()
        .map_err(|error| format!("YSNCD deflate finish failed: {error}"))?;

    let mut metadata = ListFileHeaderMetadata {
        logical_path: logical_path.clone(),
        content_kind: "ysncd_sound_cue_dictionary".to_owned(),
        authored_by: "newengine-asset-format-nef8::compile_ysncd_manifest".to_owned(),
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
        "YSNCD legacy dictionaries embed XVAG runtime clip payloads; loose authored WAVs remain source-only"
            .to_owned(),
    );
    let metadata_bytes = serde_json::to_vec(&metadata)
        .map_err(|error| format!("YSNCD header metadata encode failed: {error}"))?;
    let output = encode_list_file(ListFileEncodeRequest {
        content_kind: LIST_FILE_CONTENT_KIND_YSNCD,
        content_schema_version: YSNCD_BINARY_SCHEMA_VERSION,
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
    let decoded = newengine_asset_format_nef8::decode_ysncd_nef8(
        &output,
        &logical_path,
        LIST_FILE_CONTENT_KIND_YSNCD,
        YSNCD_BINARY_SCHEMA_VERSION,
    )?;
    if decoded.cues.len() != dictionary.cues.len() {
        return Err("YSNCD post-write cue count mismatch".to_owned());
    }
    println!(
        "YSNCD compiled source='{}' output='{}' logical='{}' cues={} embedded_clips={} bytes={}",
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
