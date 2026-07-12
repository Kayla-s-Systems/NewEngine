use std::collections::BTreeMap;

use crate::parsing::{content_line, indexed_f32_or};
use crate::path::{join_logical_path, logical_dir, mtl_texture_path};
use crate::ModelMaterialSource;

pub fn parse_mtl_text(base_dir: &str, text: &str) -> BTreeMap<String, ModelMaterialSource> {
    let mut materials = BTreeMap::new();
    let mut current: Option<(String, ModelMaterialSource)> = None;

    for raw_line in text.lines() {
        let line = content_line(raw_line);
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(tag) = words.next() else {
            continue;
        };
        let values = words.collect::<Vec<_>>();

        match tag {
            "newmtl" => {
                flush_material(&mut materials, &mut current);
                current = Some((
                    values.first().copied().unwrap_or("default").to_owned(),
                    ModelMaterialSource::default(),
                ));
            }
            "Kd" => with_material(&mut current, |material| {
                material.kd = [
                    indexed_f32_or(&values, 0, material.kd[0]),
                    indexed_f32_or(&values, 1, material.kd[1]),
                    indexed_f32_or(&values, 2, material.kd[2]),
                ];
            }),
            "d" => with_material(&mut current, |material| {
                material.alpha = indexed_f32_or(&values, 0, material.alpha);
            }),
            "Ns" => with_material(&mut current, |material| {
                material.ns = indexed_f32_or(&values, 0, material.ns);
            }),
            "map_Kd" => with_material(&mut current, |material| {
                material.base_color_texture = mtl_texture_path(base_dir, &values);
            }),
            "norm" | "map_Bump" | "bump" => with_material(&mut current, |material| {
                material.normal_texture = mtl_texture_path(base_dir, &values);
            }),
            _ => {}
        }
    }

    flush_material(&mut materials, &mut current);
    materials
}

pub(crate) fn load_mtl_map<F>(
    obj_path: &str,
    mtllibs: &[String],
    read_mtl: &mut F,
) -> BTreeMap<String, ModelMaterialSource>
where
    F: FnMut(&str) -> Option<String>,
{
    let base = logical_dir(obj_path);
    let mut materials = BTreeMap::new();

    for relative in mtllibs {
        let Ok(path) = join_logical_path(base, relative) else {
            newengine_ulog_api::ulog::warn!(
                "model import obj: MTL rejected relative='{}' base='{}'",
                relative,
                base
            );
            continue;
        };
        let Some(text) = read_mtl(&path) else {
            newengine_ulog_api::ulog::warn!("model import obj: MTL unavailable path='{}'", path);
            continue;
        };
        materials.extend(parse_mtl_text(base, &text));
    }

    materials
}

fn with_material(
    current: &mut Option<(String, ModelMaterialSource)>,
    update: impl FnOnce(&mut ModelMaterialSource),
) {
    if let Some((_, material)) = current {
        update(material);
    }
}

fn flush_material(
    materials: &mut BTreeMap<String, ModelMaterialSource>,
    current: &mut Option<(String, ModelMaterialSource)>,
) {
    if let Some((name, material)) = current.take() {
        materials.insert(name, material);
    }
}
