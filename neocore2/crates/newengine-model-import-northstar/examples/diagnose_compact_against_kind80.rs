use newengine_model_import_northstar::PakFile;
use std::{env, fs};
#[derive(Clone, Copy)]
struct S {
    kind: u8,
    buf: usize,
    len: usize,
    sz: [u8; 4],
    sc: [f32; 4],
    off: [f32; 4],
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
fn qdec(p: &PakFile, s: S, n: usize) -> Result<Vec<[f32; 4]>, String> {
    let d = p.slice(s.buf, s.len)?;
    let mut br = BR { d, b: 0 };
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = [0.; 4];
        for c in 0..4 {
            let w = s.sz[c] as usize;
            if w > 0 {
                v[c] = br.r(w) as f32 * s.sc[c] + s.off[c];
            }
        }
        out.push(v)
    }
    Ok(out)
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
fn score(label: &str, gt: &[[f32; 3]], cand: Vec<[f32; 3]>) {
    let ds = gt
        .iter()
        .zip(cand)
        .map(|(a, b)| dot(*a, b))
        .collect::<Vec<_>>();
    let ad = ds.iter().map(|d| d.abs()).collect::<Vec<_>>();
    println!(
        "{label:32} signed p10={:.4} med={:.4} p90={:.4} | abs p10={:.4} med={:.4} p90={:.4}",
        q(ds.clone(), 0.1),
        q(ds.clone(), 0.5),
        q(ds, 0.9),
        q(ad.clone(), 0.1),
        q(ad.clone(), 0.5),
        q(ad, 0.9)
    );
}
fn s8(x: u8) -> f32 {
    (x as i8 as f32 / 127.).clamp(-1., 1.)
}
fn u8n(x: u8) -> f32 {
    x as f32 / 127.5 - 1.
}
fn s10(x: u32) -> f32 {
    let mut i = (x & 1023) as i32;
    if i & 512 != 0 {
        i -= 1024
    }
    (i as f32 / 511.).clamp(-1., 1.)
}
fn u10n(x: u32) -> f32 {
    (x & 1023) as f32 / 511.5 - 1.
}
fn s16(x: u16) -> f32 {
    (x as i16 as f32 / 32767.).clamp(-1., 1.)
}
fn u16n(x: u16) -> f32 {
    x as f32 / 32767.5 - 1.
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
fn qrot(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
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
fn qnorm(mut v: [f32; 4]) -> [f32; 4] {
    let l = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
    if l > 1e-9 {
        for x in &mut v {
            *x /= l
        }
    }
    v
}
fn small3(v: u32, unorm: bool, index_mode: u32, sign_mode: u32) -> [f32; 4] {
    let code = (v >> 30) & 3;
    let raw = [v & 1023, (v >> 10) & 1023, (v >> 20) & 1023];
    let scale = std::f32::consts::FRAC_1_SQRT_2;
    let mut a = [0.; 3];
    for i in 0..3 {
        a[i] = if unorm {
            u10n(raw[i]) * scale
        } else {
            s10(raw[i]) * scale
        };
    }
    let mut omitted = (1. - a.iter().map(|x| x * x).sum::<f32>()).max(0.).sqrt();
    if sign_mode == 1 && (code & 1) != 0 {
        omitted = -omitted;
    }
    let idx = match index_mode {
        0 => code as usize,
        1 => (code ^ 3) as usize,
        2 => (code >> 1) as usize,
        _ => code as usize,
    };
    let mut out = [0.; 4];
    let mut k = 0;
    for i in 0..4 {
        if i == idx {
            out[i] = omitted
        } else {
            out[i] = a[k];
            k += 1;
        }
    }
    qnorm(out)
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
        let sc = pak.read_u32(sm + 144)? as usize;
        let st = pak.resolve_pointer(sm + 48)?.ok_or("streams")?;
        let ss = (0..sc)
            .map(|j| sd(&pak, st + j * 64))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(s80) = ss
            .iter()
            .find(|s| s.kind == 80 && s.sz[0] > 0 && s.sz[1] > 0 && s.sz[2] > 0)
            .copied()
        else {
            continue;
        };
        let Some(s130) = ss.iter().find(|s| s.kind == 130).copied() else {
            continue;
        };
        let gt = qdec(&pak, s80, vc)?
            .iter()
            .map(|v| norm([v[0], v[1], v[2]]))
            .collect::<Vec<_>>();
        let raw = pak.slice(s130.buf, vc * 4)?;
        println!("\nMESH {mi} {name} vc={vc}");
        println!(
            "kind80 {:?}/{:?}; kind130 {:?}/{:?}",
            s80.sc, s80.off, s130.sc, s130.off
        );
        score(
            "130 snorm8 xyz",
            &gt,
            (0..vc)
                .map(|i| {
                    let o = i * 4;
                    norm([s8(raw[o]), s8(raw[o + 1]), s8(raw[o + 2])])
                })
                .collect(),
        );
        score(
            "130 unorm8 xyz",
            &gt,
            (0..vc)
                .map(|i| {
                    let o = i * 4;
                    norm([u8n(raw[o]), u8n(raw[o + 1]), u8n(raw[o + 2])])
                })
                .collect(),
        );
        score(
            "130 oct s16",
            &gt,
            (0..vc)
                .map(|i| {
                    let o = i * 4;
                    oct(
                        s16(u16::from_le_bytes(raw[o..o + 2].try_into().unwrap())),
                        s16(u16::from_le_bytes(raw[o + 2..o + 4].try_into().unwrap())),
                    )
                })
                .collect(),
        );
        score(
            "130 oct u16",
            &gt,
            (0..vc)
                .map(|i| {
                    let o = i * 4;
                    oct(
                        u16n(u16::from_le_bytes(raw[o..o + 2].try_into().unwrap())),
                        u16n(u16::from_le_bytes(raw[o + 2..o + 4].try_into().unwrap())),
                    )
                })
                .collect(),
        );
        for unorm in [false, true] {
            for im in 0..3 {
                for smode in 0..2 {
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
                                norm(qrot(small3(v, unorm, im, smode), basis))
                            })
                            .collect();
                        score(
                            &format!("130 q3 u={unorm} i={im} s={smode} a={axis}"),
                            &gt,
                            cand,
                        );
                    }
                }
            }
        }
        if let Some(s75) = ss.iter().find(|s| s.kind == 75).copied() {
            let vals = qdec(&pak, s75, vc)?;
            let xs = vals.iter().map(|v| v[0]).collect::<Vec<_>>();
            let ys = vals.iter().map(|v| v[1]).collect::<Vec<_>>();
            println!(
                "kind75 sizes={:?} scale={:?} offset={:?} x=[{:.4},{:.4}] y=[{:.4},{:.4}]",
                s75.sz,
                s75.sc,
                s75.off,
                xs.iter().copied().fold(f32::INFINITY, f32::min),
                xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                ys.iter().copied().fold(f32::INFINITY, f32::min),
                ys.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            );
            for (label, fx, fy) in [("75 oct raw", 1., 1.), ("75 oct x2-1", 2., 2.)] {
                let cand = vals
                    .iter()
                    .map(|v| {
                        let add = if fx == 2. { -1. } else { 0. };
                        oct(v[0] * fx + add, v[1] * fy + add)
                    })
                    .collect();
                score(label, &gt, cand);
            }
        }
        break;
    }
    Ok(())
}
