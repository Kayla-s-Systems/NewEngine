use newengine_model_import_northstar::PakFile;
use std::{env, fs};
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
#[derive(Clone, Copy)]
struct S {
    kind: u8,
    buf: usize,
    len: usize,
    sz: [u8; 4],
    sc: [f32; 4],
    off: [f32; 4],
}
fn sd(p: &PakFile, a: usize) -> Result<S, String> {
    let mut sc = [0.; 4];
    let mut off = [0.; 4];
    for i in 0..4 {
        sc[i] = p.read_f32(a + 32 + i * 4)?;
        off[i] = p.read_f32(a + 48 + i * 4)?;
    }
    Ok(S {
        kind: p.read_u8(a + 20)?,
        buf: p.resolve_pointer(a)?.ok_or("buf")?,
        len: p.read_u32(a + 16)? as usize,
        sz: [
            p.read_u8(a + 24)?,
            p.read_u8(a + 25)?,
            p.read_u8(a + 26)?,
            p.read_u8(a + 27)?,
        ],
        sc,
        off,
    })
}
fn dec(p: &PakFile, s: S, n: usize) -> Result<Vec<[f32; 4]>, String> {
    let d = p.slice(s.buf, s.len)?;
    let mut b = BR { d, b: 0 };
    let mut o = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = [0.; 4];
        for (c, value) in v.iter_mut().enumerate() {
            let w = s.sz[c] as usize;
            if w > 0 {
                *value = b.r(w) as f32 * s.sc[c] + s.off[c];
            }
        }
        o.push(v)
    }
    Ok(o)
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
    if l > 1e-9 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0., 1., 0.]
    }
}
fn main() -> Result<(), String> {
    let src = env::args().nth(1).ok_or("pak")?;
    let p = PakFile::parse(fs::read(src).map_err(|e| e.to_string())?)?;
    let r = p.resource("GEOMETRY_1").ok_or("geom")?;
    let base = p.resource_payload(r)?;
    let count = p.read_u32(base + 8)? as usize;
    let table = p.resolve_pointer(base + 40)?.ok_or("table")?;
    for mi in 0..count {
        let subm = table + mi * 192;
        let name = p
            .resolve_pointer(subm + 32)?
            .map(|x| p.string_at(x))
            .transpose()?
            .unwrap_or_default();
        if !name.contains("LODShape0") {
            continue;
        }
        let vc = p.read_u32(subm + 136)? as usize;
        let ic = p.read_u32(subm + 140)? as usize;
        let sc = p.read_u32(subm + 144)? as usize;
        let st = p.resolve_pointer(subm + 48)?.ok_or("streams")?;
        let ib = p.resolve_pointer(subm + 64)?.ok_or("ib")?;
        let ss = (0..sc)
            .map(|j| sd(&p, st + j * 64))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(ns) = ss.iter().find(|s| s.kind == 80 && s.sz[0] > 0).copied() else {
            continue;
        };
        let ps = *ss.iter().find(|s| s.kind == 64).ok_or("pos")?;
        let pos = dec(&p, ps, vc)?;
        let nv = dec(&p, ns, vc)?;
        let idx = p.slice(ib, ic * 2)?;
        let mut neg = 0usize;
        let mut valid = 0usize;
        let mut dots = Vec::new();
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
            let f = norm(cross(
                sub(
                    [pos[b][0], pos[b][1], pos[b][2]],
                    [pos[a][0], pos[a][1], pos[a][2]],
                ),
                sub(
                    [pos[c][0], pos[c][1], pos[c][2]],
                    [pos[a][0], pos[a][1], pos[a][2]],
                ),
            ));
            let n = norm([
                nv[a][0] + nv[b][0] + nv[c][0],
                nv[a][1] + nv[b][1] + nv[c][1],
                nv[a][2] + nv[b][2] + nv[c][2],
            ]);
            let d = dot(f, n);
            if d < 0. {
                neg += 1
            }
            valid += 1;
            dots.push(d);
        }
        dots.sort_by(|a, b| a.total_cmp(b));
        let at = |q: f32| dots[((dots.len() - 1) as f32 * q).round() as usize];
        println!(
            "mesh={} neg={}/{} ({:.3}%) dot p01={:.3} p05={:.3} p10={:.3} med={:.3} name={}",
            mi,
            neg,
            valid,
            neg as f64 * 100.0 / valid as f64,
            at(0.01),
            at(0.05),
            at(0.10),
            at(0.50),
            name
        );
    }
    Ok(())
}
