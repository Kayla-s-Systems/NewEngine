use newengine_model_import_northstar::PakFile;
use std::{env, fs};

#[derive(Clone, Copy)]
struct S {
    kind: u8,
    buffer: usize,
    len: usize,
    sizes: [u8; 4],
    scale: [f32; 4],
    offset: [f32; 4],
}
struct BR<'a> {
    d: &'a [u8],
    b: usize,
}
impl<'a> BR<'a> {
    fn r(&mut self, w: usize) -> u32 {
        let mut v = 0;
        for i in 0..w {
            let a = self.b + i;
            v |= (((self.d[a >> 3] >> (a & 7)) & 1) as u32) << i;
        }
        self.b += w;
        v
    }
}
fn sd(p: &PakFile, a: usize) -> Result<S, String> {
    let mut scale = [0.; 4];
    let mut offset = [0.; 4];
    for i in 0..4 {
        scale[i] = p.read_f32(a + 32 + i * 4)?;
        offset[i] = p.read_f32(a + 48 + i * 4)?;
    }
    Ok(S {
        kind: p.read_u8(a + 20)?,
        buffer: p.resolve_pointer(a)?.ok_or("buf")?,
        len: p.read_u32(a + 16)? as usize,
        sizes: [
            p.read_u8(a + 24)?,
            p.read_u8(a + 25)?,
            p.read_u8(a + 26)?,
            p.read_u8(a + 27)?,
        ],
        scale,
        offset,
    })
}
fn qdecode(p: &PakFile, s: S, n: usize) -> Result<Vec<[f32; 4]>, String> {
    let d = p.slice(s.buffer, s.len)?;
    let mut br = BR { d, b: 0 };
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = [0.; 4];
        for (c, value) in v.iter_mut().enumerate() {
            let w = s.sizes[c] as usize;
            if w > 0 {
                *value = br.r(w) as f32 * s.scale[c] + s.offset[c];
            }
        }
        out.push(v);
    }
    Ok(out)
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
        [0., 1., 0.]
    }
}
fn q(mut v: Vec<f32>, p: f32) -> f32 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f32 * p).round() as usize]
}
fn sn10(v: u32) -> f32 {
    let mut i = (v & 1023) as i32;
    if i & 512 != 0 {
        i -= 1024
    }
    (i as f32 / 511.).clamp(-1., 1.)
}
fn un10(v: u32) -> f32 {
    (v & 1023) as f32 / 1023.
}
fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let tx = 2. * (y * v[2] - z * v[1]);
    let ty = 2. * (z * v[0] - x * v[2]);
    let tz = 2. * (x * v[1] - y * v[0]);
    [
        v[0] + w * tx + (y * tz - z * ty),
        v[1] + w * ty + (z * tx - x * tz),
        v[2] + w * tz + (x * ty - y * tx),
    ]
}
fn quat_norm(mut v: [f32; 4]) -> [f32; 4] {
    let l = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
    if l > 1e-9 {
        for x in &mut v {
            *x /= l
        }
    }
    v
}
fn decode_smallest3(v: u32, signed_components: bool, mode: u32) -> [f32; 4] {
    let code = (v >> 30) & 3;
    let raw = [v & 1023, (v >> 10) & 1023, (v >> 20) & 1023];
    let scale = std::f32::consts::FRAC_1_SQRT_2;
    let mut a = [0f32; 3];
    for i in 0..3 {
        a[i] = if signed_components {
            sn10(raw[i]) * scale
        } else {
            (un10(raw[i]) * 2. - 1.) * scale
        };
    }
    let omitted = (1.0 - (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]))
        .max(0.0)
        .sqrt();
    let mut out = [0f32; 4];
    let index = match mode {
        0 => code as usize,
        1 => (code ^ 3) as usize,
        _ => code as usize,
    };
    let mut k = 0;
    for (i, value) in out.iter_mut().enumerate() {
        if i == index {
            *value = omitted
        } else {
            *value = a[k];
            k += 1;
        }
    }
    quat_norm(out)
}
fn oct(mut x: f32, mut y: f32) -> [f32; 3] {
    let mut z = 1. - x.abs() - y.abs();
    if z < 0. {
        let ox = x;
        x = (1. - y.abs()) * ox.signum();
        y = (1. - ox.abs()) * y.signum();
        z = 1. - x.abs() - y.abs();
    }
    norm([x, y, z])
}
fn score(label: &str, geom: &[[f32; 3]], cand: Vec<[f32; 3]>) {
    let ds = geom
        .iter()
        .zip(&cand)
        .map(|(a, b)| dot(*a, *b))
        .collect::<Vec<_>>();
    let ad = ds.iter().map(|d| d.abs()).collect::<Vec<_>>();
    println!(
        "{label}: signed p10={:.4} med={:.4} p90={:.4}; abs p10={:.4} med={:.4} p90={:.4}",
        q(ds.clone(), 0.1),
        q(ds.clone(), 0.5),
        q(ds, 0.9),
        q(ad.clone(), 0.1),
        q(ad.clone(), 0.5),
        q(ad, 0.9)
    );
}
fn main() -> Result<(), String> {
    let src = env::args().nth(1).ok_or("pak")?;
    let pak = PakFile::parse(fs::read(src).map_err(|e| e.to_string())?)?;
    let r = pak.resource("GEOMETRY_1").ok_or("geom")?;
    let p = pak.resource_payload(r)?;
    let count = pak.read_u32(p + 8)? as usize;
    let table = pak.resolve_pointer(p + 40)?.ok_or("table")?;
    for mi in 0..count {
        let sm = table + mi * 192;
        let name = pak
            .resolve_pointer(sm + 32)?
            .map(|x| pak.string_at(x))
            .transpose()?
            .unwrap_or_default();
        if !name.contains("LODShape0") {
            continue;
        }
        let vc = pak.read_u32(sm + 136)? as usize;
        let ic = pak.read_u32(sm + 140)? as usize;
        let sc = pak.read_u32(sm + 144)? as usize;
        let st = pak.resolve_pointer(sm + 48)?.ok_or("streams")?;
        let ib = pak.resolve_pointer(sm + 64)?.ok_or("ib")?;
        let ss = (0..sc)
            .map(|j| sd(&pak, st + j * 64))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(ps) = ss.iter().find(|s| s.kind == 64).copied() else {
            continue;
        };
        let Some(k130) = ss.iter().find(|s| s.kind == 130).copied() else {
            continue;
        };
        let pos = qdecode(&pak, ps, vc)?;
        let idx = pak.slice(ib, ic * 2)?;
        let mut acc = vec![[0.; 3]; vc];
        for t in (0..ic).step_by(3) {
            if t + 2 >= ic {
                break;
            }
            let ix =
                |k: usize| u16::from_le_bytes([idx[(t + k) * 2], idx[(t + k) * 2 + 1]]) as usize;
            let (a, b, c) = (ix(0), ix(1), ix(2));
            if a >= vc || b >= vc || c >= vc {
                continue;
            }
            let pa = [pos[a][0], pos[a][1], pos[a][2]];
            let pb = [pos[b][0], pos[b][1], pos[b][2]];
            let pc = [pos[c][0], pos[c][1], pos[c][2]];
            let f = cross(sub(pb, pa), sub(pc, pa));
            for i in [a, b, c] {
                for k in 0..3 {
                    acc[i][k] += f[k];
                }
            }
        }
        let geom = acc.into_iter().map(norm).collect::<Vec<_>>();
        let raw = pak.slice(k130.buffer, vc * 4)?;
        println!(
            "\nMESH {mi} {name} kind130={:?}/{:?} kind75={:?}",
            k130.scale,
            k130.offset,
            ss.iter()
                .find(|s| s.kind == 75)
                .map(|s| (s.sizes, s.scale, s.offset))
        );
        for signed in [true, false] {
            for mode in [0u32, 1u32] {
                for axis in 0..3 {
                    let basis = match axis {
                        0 => [1., 0., 0.],
                        1 => [0., 1., 0.],
                        _ => [0., 0., 1.],
                    };
                    let cand = (0..vc)
                        .map(|i| {
                            let o = i * 4;
                            let v = u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
                            norm(quat_rotate(decode_smallest3(v, signed, mode), basis))
                        })
                        .collect();
                    score(
                        &format!("small3 signed={signed} mode={mode} axis={axis}"),
                        &geom,
                        cand,
                    );
                }
            }
        }
        if let Some(s75) = ss.iter().find(|s| s.kind == 75).copied() {
            let vals = qdecode(&pak, s75, vc)?;
            let variants = [("raw", 1.0, 0.0), ("signed2", 2.0, -1.0)];
            for (label, mul, add) in variants {
                let cand = vals
                    .iter()
                    .map(|v| oct(v[0] * mul + add, v[1] * mul + add))
                    .collect();
                score(&format!("kind75 oct {label}"), &geom, cand);
            }
        }
        if name.contains("head_lod0") || name.contains("abby_head") || name.contains("default_head")
        {
            break;
        }
    }
    Ok(())
}
