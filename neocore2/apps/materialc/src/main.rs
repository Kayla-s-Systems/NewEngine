#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_materials::serde as mat_serde;
use newengine_materials::{
    AuthoredMaterialDescriptor, AuthoredMaterialLibrary, AuthoredMaterialSurface,
    MaterialFlags, MaterialParamValue, ShadingModel,
};

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
                "usage: materialc <descriptor.json> [--out <out.material-library.json>] [--name <entry>]"
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
    let name = name.unwrap_or(stem);

    let out_path = out_path
        .map(PathBuf::from)
        .unwrap_or_else(|| in_path.with_extension("material-library.json"));

    if out_path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("nemat")).unwrap_or(false) {
        return Err("materialc no longer writes top-level NEMAT binary files; compile authoring JSON into NEF8 .nemat through the ListFile toolchain".to_owned());
    }

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let library = AuthoredMaterialLibrary {
        version: 1,
        materials: vec![AuthoredMaterialDescriptor {
            name,
            shader: if matches!(desc.shading_model, ShadingModel::Unlit) { "unlit".to_owned() } else { "pbr.default".to_owned() },
            surface: AuthoredMaterialSurface {
                blend: if desc.flags.contains(MaterialFlags::ALPHA_BLEND) { "alpha_blend".to_owned() } else { "opaque".to_owned() },
                two_sided: desc.flags.contains(MaterialFlags::DOUBLE_SIDED),
                alpha_cutoff: if desc.flags.contains(MaterialFlags::ALPHA_TEST) { Some(desc.alpha_cutoff) } else { None },
            },
            textures: Default::default(),
            params: [
                ("base_color".to_owned(), MaterialParamValue::Color(desc.base_color)),
                ("metallic".to_owned(), MaterialParamValue::Float(desc.metallic)),
                ("roughness".to_owned(), MaterialParamValue::Float(desc.roughness)),
                ("normal_scale".to_owned(), MaterialParamValue::Float(desc.normal_scale)),
                ("occlusion_strength".to_owned(), MaterialParamValue::Float(desc.occlusion_strength)),
                ("emissive".to_owned(), MaterialParamValue::Float3(desc.emissive)),
                ("emissive_strength".to_owned(), MaterialParamValue::Float(desc.emissive_strength)),
            ].into_iter().collect(),
        }],
    };

    let bytes = serde_json::to_vec_pretty(&library).map_err(|e| format!("encode failed: {e}"))?;
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
