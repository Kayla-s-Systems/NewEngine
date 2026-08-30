use newengine_model_import_northstar::{decode_vram_textures, PakFile};
use std::{env, fs, path::PathBuf};
fn main() -> Result<(), String> {
    let mut a = env::args().skip(1);
    let root = PathBuf::from(a.next().ok_or("root required")?);
    let out = PathBuf::from(a.next().ok_or("output required")?);
    let mut files = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("pak"))
        })
        .collect::<Vec<_>>();
    files.sort();
    let mut text=String::from("package\tlogical_name\tsource_path\tdxgi\tformat\twidth\theight\tmips\ttype\tstream_flags\n");
    let mut count = 0usize;
    let mut packages = 0usize;
    for p in files {
        let bytes = match fs::read(&p) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let pak = match PakFile::parse(bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let textures = match decode_vram_textures(&pak) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if textures.is_empty() {
            continue;
        }
        packages += 1;
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        for t in textures {
            let src = t.source_path.replace('\t', " ").replace('\n', " ");
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t0x{:x}\n",
                name,
                t.logical_name(),
                src,
                t.format.dxgi(),
                t.format,
                t.width,
                t.height,
                t.mip_count,
                t.texture_type,
                t.stream_flags
            ));
            count += 1;
        }
    }
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&out, text).map_err(|e| e.to_string())?;
    println!(
        "VRAM_INVENTORY_OK packages={} textures={} output='{}'",
        packages,
        count,
        out.display()
    );
    Ok(())
}
