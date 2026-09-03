use newengine_model_import_northstar::PakFile;
use std::{env, fs};

#[derive(Clone, Copy)]
struct StreamDesc {
    kind: u8,
    buffer: usize,
    buffer_size: usize,
    sizes: [u8; 4],
    q_scale: [f32; 4],
    q_offset: [f32; 4],
}

fn desc(pak: &PakFile, at: usize) -> Result<StreamDesc, String> {
    let mut q_scale = [0.0; 4];
    let mut q_offset = [0.0; 4];
    for c in 0..4 {
        q_scale[c] = pak.read_f32(at + 32 + c * 4)?;
        q_offset[c] = pak.read_f32(at + 48 + c * 4)?;
    }
    Ok(StreamDesc {
        kind: pak.read_u8(at + 20)?,
        buffer: pak.resolve_pointer(at)?.ok_or("missing stream buffer")?,
        buffer_size: pak.read_u32(at + 16)? as usize,
        sizes: [
            pak.read_u8(at + 24)?,
            pak.read_u8(at + 25)?,
            pak.read_u8(at + 26)?,
            pak.read_u8(at + 27)?,
        ],
        q_scale,
        q_offset,
    })
}

fn raw_f32x3(pak: &PakFile, s: StreamDesc, n: usize) -> Result<Vec<[f32; 3]>, String> {
    let bytes = pak.slice(s.buffer, n * 12)?;
    Ok((0..n)
        .map(|i| {
            let o = i * 12;
            [
                f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()),
                f32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()),
                f32::from_le_bytes(bytes[o + 8..o + 12].try_into().unwrap()),
            ]
        })
        .collect())
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt();
    if l > 1e-12 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}
fn q(mut v: Vec<f32>, p: f32) -> f32 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f32 * p).round() as usize]
}
fn s8(x: u8) -> f32 {
    (x as i8 as f32 / 127.0).clamp(-1.0, 1.0)
}
fn u8n(x: u8) -> f32 {
    x as f32 / 127.5 - 1.0
}

fn main() -> Result<(), String> {
    let src = env::args().nth(1).ok_or("pak path")?;
    let pak = PakFile::parse(fs::read(&src).map_err(|e| e.to_string())?)?;
    let resource = pak.resource("GEOMETRY_1").ok_or("no GEOMETRY_1")?;
    let payload = pak.resource_payload(resource)?;
    let count = pak.read_u32(payload + 8)? as usize;
    let table = pak
        .resolve_pointer(payload + 40)?
        .ok_or("no submesh table")?;
    for mi in 0..count {
        let subm = table + mi * 192;
        let name = pak
            .resolve_pointer(subm + 32)?
            .map(|p| pak.string_at(p))
            .transpose()?
            .unwrap_or_default();
        if !name.contains("LODShape0") {
            continue;
        }
        let vc = pak.read_u32(subm + 136)? as usize;
        let ic = pak.read_u32(subm + 140)? as usize;
        let sc = pak.read_u32(subm + 144)? as usize;
        let streams_at = pak.resolve_pointer(subm + 48)?.ok_or("no streams")?;
        let index_at = pak.resolve_pointer(subm + 64)?.ok_or("no indices")?;
        let streams = (0..sc)
            .map(|i| desc(&pak, streams_at + i * 64))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(pos_s) = streams.iter().find(|s| s.kind == 0).copied() else {
            continue;
        };
        let positions = raw_f32x3(&pak, pos_s, vc)?;
        let ib = pak.slice(index_at, ic * 2)?;
        let mut accum = vec![[0.0f32; 3]; vc];
        for t in (0..ic).step_by(3) {
            if t + 2 >= ic {
                break;
            }
            let ix = |k: usize| u16::from_le_bytes([ib[(t + k) * 2], ib[(t + k) * 2 + 1]]) as usize;
            let (a, b, c) = (ix(0), ix(1), ix(2));
            if a >= vc || b >= vc || c >= vc {
                continue;
            }
            let f = cross(
                sub(positions[b], positions[a]),
                sub(positions[c], positions[a]),
            );
            for i in [a, b, c] {
                for k in 0..3 {
                    accum[i][k] += f[k];
                }
            }
        }
        let geom = accum.into_iter().map(norm).collect::<Vec<_>>();
        println!("\nMESH {mi} {name} vc={vc}");
        for kind in [2u8, 3u8] {
            let Some(s) = streams.iter().find(|s| s.kind == kind).copied() else {
                continue;
            };
            println!(
                " kind{kind} sizes={:?} scale={:?} offset={:?} bytes={}",
                s.sizes, s.q_scale, s.q_offset, s.buffer_size
            );
            if s.buffer_size < vc * 4 {
                continue;
            }
            let raw = pak.slice(s.buffer, vc * 4)?;
            for (label, signed) in [("snorm8", true), ("unorm8", false)] {
                let mut signed_dots = Vec::with_capacity(vc);
                let mut abs_dots = Vec::with_capacity(vc);
                for (i, geom_normal) in geom.iter().enumerate().take(vc) {
                    let o = i * 4;
                    let v = if signed {
                        norm([s8(raw[o]), s8(raw[o + 1]), s8(raw[o + 2])])
                    } else {
                        norm([u8n(raw[o]), u8n(raw[o + 1]), u8n(raw[o + 2])])
                    };
                    let d = dot(*geom_normal, v);
                    signed_dots.push(d);
                    abs_dots.push(d.abs());
                }
                println!("  {label}: signed p10={:.4} med={:.4} p90={:.4}; abs p10={:.4} med={:.4} p90={:.4}",
                    q(signed_dots.clone(),0.1),q(signed_dots.clone(),0.5),q(signed_dots,0.9),
                    q(abs_dots.clone(),0.1),q(abs_dots.clone(),0.5),q(abs_dots,0.9));
            }
            println!("  sample bytes={:02x?}", &raw[..raw.len().min(16)]);
        }
        // Only the large authored head partitions are needed for this diagnostic.
        if name.contains("head_LOD")
            || name.contains("head_lod")
            || name.contains("seattle_head")
            || name.contains("default_head")
        {
            break;
        }
    }
    Ok(())
}
