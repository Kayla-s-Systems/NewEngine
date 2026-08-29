use newengine_model_import_northstar::PakFile;
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let source = PathBuf::from(
        args.next()
            .ok_or("usage: extract_havok_collision_tagfiles INPUT-phys.pak OUTPUT_DIR")?,
    );
    let output_dir = PathBuf::from(args.next().ok_or("missing OUTPUT_DIR")?);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    let pak =
        PakFile::parse(fs::read(&source).map_err(|e| format!("read {}: {e}", source.display()))?)?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("collision");
    let mut count = 0usize;
    for resource in pak
        .resources()
        .iter()
        .filter(|r| r.kind == "COLLISION_DATA_HAVOK_BG")
    {
        let payload = pak.resource_payload(resource)?;
        let tag_at = pak.resolve_pointer(payload + 16)?.ok_or_else(|| {
            format!(
                "collision resource at 0x{:x} has no TAG0 pointer",
                resource.absolute_offset
            )
        })?;
        let header = pak.slice(tag_at, 8)?;
        let size = u32::from_be_bytes(header[0..4].try_into().unwrap()) as usize;
        if &header[4..8] != b"TAG0" {
            return Err(format!(
                "collision resource target is not TAG0 at 0x{tag_at:x}: {:02x?}",
                header
            ));
        }
        if size < 8 {
            return Err(format!("invalid TAG0 size {size}"));
        }
        let bytes = pak.slice(tag_at, size)?;
        let out = output_dir.join(format!("{stem}.{count:02}.hkx"));
        fs::write(&out, bytes).map_err(|e| format!("write {}: {e}", out.display()))?;
        println!(
            "HAVOK_TAGFILE_EXTRACT_OK index={} bytes={} resource=0x{:x} tag=0x{:x} output='{}'",
            count,
            size,
            resource.absolute_offset,
            tag_at,
            out.display()
        );
        count += 1;
    }
    if count == 0 {
        return Err("package contains no COLLISION_DATA_HAVOK_BG resources".to_owned());
    }
    Ok(())
}
