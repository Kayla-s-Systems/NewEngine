use std::collections::BTreeMap;

use crate::mesh::{mesh_from_builder, normalize_parts, push_triangle};
use crate::mtl::load_mtl_map;
use crate::parsing::{content_line, next_f32_or};
use crate::path::normalize_logical_path;
use crate::types::{ObjCorner, ObjPartBuilder};
use crate::{ObjDecodeResult, ObjPart};

pub fn decode_obj_with_mtl_loader<F>(
    logical_path: &str,
    obj_text: &str,
    target_height: f32,
    mut read_mtl: F,
) -> Result<ObjDecodeResult, String>
where
    F: FnMut(&str) -> Option<String>,
{
    let logical_path = normalize_logical_path(logical_path, false)?;
    let mut parser = ObjParser::default();
    parser.parse(obj_text);

    let mut parts = parser
        .groups
        .into_iter()
        .filter_map(|(material_slot, builder)| {
            mesh_from_builder(builder).map(|mesh| ObjPart {
                material_slot,
                mesh,
            })
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err(format!(
            "model OBJ has no renderable faces path='{logical_path}'"
        ));
    }

    normalize_parts(&mut parts, target_height);
    let materials = load_mtl_map(&logical_path, &parser.mtllibs, &mut read_mtl);
    Ok(ObjDecodeResult {
        parts,
        materials,
        mtllibs: parser.mtllibs,
    })
}

#[derive(Default)]
struct ObjParser {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    mtllibs: Vec<String>,
    current_material: String,
    groups: BTreeMap<String, ObjPartBuilder>,
}

impl ObjParser {
    fn parse(&mut self, text: &str) {
        if self.current_material.is_empty() {
            self.current_material = "default".to_owned();
        }
        for raw_line in text.lines() {
            self.parse_line(content_line(raw_line));
        }
    }

    fn parse_line(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        let mut words = line.split_whitespace();
        let Some(tag) = words.next() else {
            return;
        };
        match tag {
            "v" => self.positions.push([
                next_f32_or(&mut words, 0.0),
                next_f32_or(&mut words, 0.0),
                next_f32_or(&mut words, 0.0),
            ]),
            "vn" => self.normals.push([
                next_f32_or(&mut words, 0.0),
                next_f32_or(&mut words, 1.0),
                next_f32_or(&mut words, 0.0),
            ]),
            "vt" => {
                let u = next_f32_or(&mut words, 0.0);
                let v = next_f32_or(&mut words, 0.0);
                self.uvs.push([u, 1.0 - v]);
            }
            "mtllib" => self.mtllibs.extend(words.map(str::to_owned)),
            "usemtl" => {
                self.current_material = words.next().unwrap_or("default").trim().to_owned();
            }
            "f" => self.parse_face(words),
            _ => {}
        }
    }

    fn parse_face(&mut self, words: std::str::SplitWhitespace<'_>) {
        let corners = words
            .filter_map(|token| {
                parse_face_corner(
                    token,
                    self.positions.len(),
                    self.uvs.len(),
                    self.normals.len(),
                )
            })
            .collect::<Vec<_>>();
        if corners.len() < 3 {
            return;
        }

        let part = self
            .groups
            .entry(self.current_material.clone())
            .or_default();
        for index in 1..corners.len() - 1 {
            push_triangle(
                part,
                [corners[0], corners[index], corners[index + 1]],
                &self.positions,
                &self.uvs,
                &self.normals,
            );
        }
    }
}

fn parse_obj_index(raw: &str, len: usize) -> Option<usize> {
    let index = raw.trim().parse::<isize>().ok()?;
    match index.cmp(&0) {
        std::cmp::Ordering::Greater => {
            let zero_based = (index as usize).checked_sub(1)?;
            (zero_based < len).then_some(zero_based)
        }
        std::cmp::Ordering::Less => {
            let resolved = len as isize + index;
            (resolved >= 0 && (resolved as usize) < len).then_some(resolved as usize)
        }
        std::cmp::Ordering::Equal => None,
    }
}

fn parse_face_corner(
    token: &str,
    position_len: usize,
    uv_len: usize,
    normal_len: usize,
) -> Option<ObjCorner> {
    let mut values = token.split('/');
    let pos = parse_obj_index(values.next()?, position_len)?;
    let uv = parse_optional_index(values.next(), uv_len);
    let nrm = parse_optional_index(values.next(), normal_len);
    Some(ObjCorner { pos, uv, nrm })
}

fn parse_optional_index(value: Option<&str>, len: usize) -> Option<usize> {
    value
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| parse_obj_index(value, len))
}
