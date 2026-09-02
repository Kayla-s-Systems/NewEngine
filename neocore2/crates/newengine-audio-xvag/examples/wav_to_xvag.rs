use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("wav_to_xvag failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: wav_to_xvag <input.wav|input_dir> <output.xvag|output_dir>".to_owned());
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    if input.is_file() {
        transcode_one(&input, &output)?;
        println!("{} -> {}", input.display(), output.display());
        return Ok(());
    }
    if !input.is_dir() {
        return Err(format!("input '{}' does not exist", input.display()));
    }
    fs::create_dir_all(&output)
        .map_err(|error| format!("create '{}' failed: {error}", output.display()))?;
    let mut converted = 0usize;
    transcode_tree(&input, &input, &output, &mut converted)?;
    println!("converted {converted} WAV file(s) to XVAG");
    Ok(())
}

fn transcode_tree(
    root: &Path,
    current: &Path,
    output_root: &Path,
    converted: &mut usize,
) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|error| format!("read_dir '{}' failed: {error}", current.display()))?
    {
        let entry = entry.map_err(|error| format!("directory entry failed: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            transcode_tree(root, &path, output_root, converted)?;
            continue;
        }
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("wav"))
        {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("relative path failed '{}': {error}", path.display()))?;
        let mut target = output_root.join(relative);
        target.set_extension("xvag");
        transcode_one(&path, &target)?;
        *converted += 1;
    }
    Ok(())
}

fn transcode_one(input: &Path, output: &Path) -> Result<(), String> {
    let mut reader = hound::WavReader::open(input)
        .map_err(|error| format!("open WAV '{}' failed: {error}", input.display()))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(format!(
            "WAV '{}' must be PCM16, got {:?}/{}-bit",
            input.display(),
            spec.sample_format,
            spec.bits_per_sample
        ));
    }
    if spec.channels == 0 {
        return Err(format!("WAV '{}' has zero channels", input.display()));
    }
    let samples = reader
        .samples::<i16>()
        .map(|sample| {
            sample
                .map(|value| f32::from(value) / 32768.0)
                .map_err(|error| format!("decode WAV '{}' failed: {error}", input.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let encoded =
        newengine_audio_xvag::encode_xvag_ps_adpcm(spec.sample_rate, spec.channels, &samples)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create '{}' failed: {error}", parent.display()))?;
    }
    fs::write(output, encoded)
        .map_err(|error| format!("write XVAG '{}' failed: {error}", output.display()))
}
