use std::{env, fs, path::PathBuf};

use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or("usage: dump_native_skin_source OUTPUT PACKAGE...")?,
    );
    let packages = args.map(PathBuf::from).collect::<Vec<_>>();
    if packages.is_empty() {
        return Err("usage: dump_native_skin_source OUTPUT PACKAGE...".into());
    }

    let mut json = String::from(
        "{\n  \"schema\": \"northstar.native-runtime-skin.v1\",\n  \"projection\": \"top8_runtime_projection\",\n  \"packages\": [\n",
    );
    let mut first_package = true;
    let mut total_meshes = 0usize;
    let mut source_max = 0u32;

    for package_path in &packages {
        let pak = PakFile::parse(fs::read(package_path)?)?;
        let decoded = decode_geometry_lod0(&pak)?;
        source_max = source_max.max(decoded.skin_loss.max_source_influences);
        if !first_package {
            json.push_str(",\n");
        }
        first_package = false;
        json.push_str("    {\n      \"path\": ");
        json.push_str(&json_string(
            &package_path.to_string_lossy().replace('\\', "/"),
        ));
        json.push_str(",\n      \"meshes\": [\n");

        for (mesh_index, mesh) in decoded.meshes.iter().enumerate() {
            if mesh_index != 0 {
                json.push_str(",\n");
            }
            total_meshes += 1;
            json.push_str("        {\n          \"name\": ");
            json.push_str(&json_string(&mesh.name));
            json.push_str(&format!(
                ",\n          \"vertex_count\": {},\n          \"source_skin_joint_domain_size\": {},\n          \"skin\": [",
                mesh.vertices.len(),
                mesh.source_skin_joint_domain_size
                    .map(|value| value as i64)
                    .unwrap_or(-1)
            ));
            if let Some(skin) = &mesh.skin {
                for (vertex_index, vertex) in skin.iter().enumerate() {
                    if vertex_index != 0 {
                        json.push(',');
                    }
                    json.push('[');
                    let influences = vertex
                        .joints
                        .iter()
                        .copied()
                        .zip(vertex.weights.iter().copied())
                        .chain(
                            vertex
                                .joints_extra
                                .iter()
                                .copied()
                                .zip(vertex.weights_extra.iter().copied()),
                        )
                        .filter(|(_, weight)| weight.is_finite() && *weight > 0.0)
                        .collect::<Vec<_>>();
                    for (influence_index, (joint, weight)) in influences.iter().enumerate() {
                        if influence_index != 0 {
                            json.push(',');
                        }
                        json.push_str(&format!("[{},{}]", joint, weight));
                    }
                    json.push(']');
                }
            }
            json.push_str("]\n        }");
        }
        json.push_str("\n      ]\n    }");
    }
    json.push_str(&format!(
        "\n  ],\n  \"summary\": {{\"packages\": {}, \"meshes\": {}, \"max_source_influences_before_top8_projection\": {}}}\n}}\n",
        packages.len(), total_meshes, source_max
    ));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, json)?;
    println!(
        "native-runtime-skin: PASS packages={} meshes={} max_source_influences={} output={}",
        packages.len(),
        total_meshes,
        source_max,
        output.display()
    );
    Ok(())
}
