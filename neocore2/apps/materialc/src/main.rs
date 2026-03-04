#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_materials::binary::{encode_asset, MaterialBinaryAsset};
use newengine_materials::serde as mat_serde;

use std::path::{Path, PathBuf};

fn main() {
    if let Err(e) = main_impl() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn main_impl() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let in_path = match args.next() {
        Some(v) => v,
        None => {
            return Err(
                "usage: materialc <in.json> [--out <out.nemat>] [--name <materials/foo>]"
                    .to_string(),
            )
        }
    };

    let mut out_path: Option<String> = None;
    let mut name: Option<String> = None;

    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => {
                out_path = args.next();
                if out_path.is_none() {
                    return Err("--out expects a value".to_string());
                }
            }
            "--name" => {
                name = args.next();
                if name.is_none() {
                    return Err("--name expects a value".to_string());
                }
            }
            _ => return Err(format!("unknown argument '{a}'")),
        }
    }

    let in_path = PathBuf::from(in_path);
    if !in_path.is_file() {
        return Err(format!("input file not found: '{}'", in_path.display()));
    }

    let src = std::fs::read_to_string(&in_path)
        .map_err(|e| format!("read failed: '{}' err='{e}'", in_path.display()))?;

    let mut desc = mat_serde::from_json(&src).map_err(|e| format!("json parse failed: {e}"))?;
    desc.sanitize_in_place();

    let stem = file_stem_ascii(&in_path).ok_or_else(|| "bad input filename".to_string())?;
    let name = name.unwrap_or_else(|| format!("materials/{stem}"));

    let out_path = out_path
        .map(PathBuf::from)
        .unwrap_or_else(|| in_path.with_extension("nemat"));

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let bytes = encode_asset(&MaterialBinaryAsset {
        name,
        desc,
    })
        .map_err(|e| format!("encode failed: {e}"))?;

    std::fs::write(&out_path, bytes)
        .map_err(|e| format!("write failed: '{}' err='{e}'", out_path.display()))?;

    println!("{}", out_path.display());
    Ok(())
}

#[inline]
fn file_stem_ascii(path: &Path) -> Option<String> {
    let s = path.file_stem()?.to_string_lossy();
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    Some(s.to_string())
}
