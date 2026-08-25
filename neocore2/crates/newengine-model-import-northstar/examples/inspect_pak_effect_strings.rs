use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn ascii_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        let printable = b.is_ascii_graphic() || b == b' ';
        if printable {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            if i - s >= min_len {
                out.push(String::from_utf8_lossy(&bytes[s..i]).into_owned());
            }
        }
    }
    if let Some(s) = start {
        if bytes.len() - s >= min_len {
            out.push(String::from_utf8_lossy(&bytes[s..]).into_owned());
        }
    }
    out
}

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        println!("PAK {source}");
        let mut offsets = pak
            .resources()
            .iter()
            .map(|r| r.absolute_offset)
            .collect::<Vec<_>>();
        offsets.sort_unstable();
        for r in pak.resources().iter().filter(|r| {
            r.kind.contains("EFFECT") || r.kind.contains("PARTICLE") || r.kind.contains("MATERIAL")
        }) {
            let payload = pak.resource_payload(r)?;
            let end = offsets
                .iter()
                .copied()
                .filter(|o| *o > r.absolute_offset)
                .min()
                .unwrap_or(pak.bytes().len());
            let end = end.min(payload.saturating_add(65536));
            println!(
                "RESOURCE kind='{}' name='{}' payload=0x{:x} span={}",
                r.kind,
                r.name,
                payload,
                end.saturating_sub(payload)
            );
            for s in ascii_strings(&pak.bytes()[payload..end], 4) {
                println!("  {s}");
            }
        }
    }
    Ok(())
}
