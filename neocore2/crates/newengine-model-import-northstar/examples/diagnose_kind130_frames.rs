use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-9 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0., 1., 0.]
    }
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
fn q(mut v: Vec<f32>, p: f32) -> f32 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f32 * p).round() as usize]
}
fn s8(x: u8) -> f32 {
    (x as i8 as f32 / 127.0).clamp(-1., 1.)
}
fn u8n(x: u8) -> f32 {
    x as f32 / 127.5 - 1.0
}
fn s10(x: u32) -> f32 {
    let mut i = (x & 1023) as i32;
    if i & 512 != 0 {
        i -= 1024
    }
    (i as f32 / 511.).clamp(-1., 1.)
}
fn u10(x: u32) -> f32 {
    (x & 1023) as f32 / 511.5 - 1.0
}
fn s16(x: u16) -> f32 {
    (x as i16 as f32 / 32767.).clamp(-1., 1.)
}
fn u16n(x: u16) -> f32 {
    x as f32 / 32767.5 - 1.0
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
fn quat8(v: u32, unorm: bool) -> [f32; 4] {
    let b = v.to_le_bytes();
    let mut q = if unorm {
        [u8n(b[0]), u8n(b[1]), u8n(b[2]), u8n(b[3])]
    } else {
        [s8(b[0]), s8(b[1]), s8(b[2]), s8(b[3])]
    };
    let l = (q.iter().map(|x| x * x).sum::<f32>()).sqrt();
    if l > 1e-8 {
        for x in &mut q {
            *x /= l
        }
    }
    q
}
fn decode_f16(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exp = ((bits >> 10) & 31) as u32;
    let man = (bits & 1023) as u32;
    let raw = match exp {
        0 => {
            if man == 0 {
                sign
            } else {
                let mut m = man;
                let mut e = 113u32;
                while m & 0x400 == 0 {
                    m <<= 1;
                    e -= 1;
                }
                m &= 0x3ff;
                sign | (e << 23) | (m << 13)
            }
        }
        31 => sign | 0x7f800000 | (man << 13),
        _ => sign | ((exp + 112) << 23) | (man << 13),
    };
    f32::from_bits(raw)
}

fn main() -> Result<(), String> {
    let src = env::args().nth(1).ok_or("pak")?;
    let pak = PakFile::parse(fs::read(src).map_err(|e| e.to_string())?)?;
    let r = pak.resource("GEOMETRY_1").ok_or("geom")?;
    let p = pak.resource_payload(r)?;
    let table = pak.resolve_pointer(p + 40)?.ok_or("table")?;
    let subm = table;
    let vc = pak.read_u32(subm + 136)? as usize;
    let ic = pak.read_u32(subm + 140)? as usize;
    let sc = pak.read_u32(subm + 144)? as usize;
    let st = pak.resolve_pointer(subm + 48)?.ok_or("streams")?;
    let ib = pak.resolve_pointer(subm + 64)?.ok_or("indices")?;
    let mut posbuf = None;
    let mut pos_sizes = [0u8; 4];
    let mut pos_scale = [0f32; 4];
    let mut pos_offset = [0f32; 4];
    let mut k130 = None;
    for j in 0..sc {
        let at = st + j * 64;
        let kind = pak.read_u8(at + 20)?;
        let buf = pak.resolve_pointer(at)?.ok_or("buf")?;
        if kind == 64 {
            posbuf = Some((buf, pak.read_u32(at + 16)? as usize));
            for c in 0..4 {
                pos_sizes[c] = pak.read_u8(at + 24 + c)?;
                pos_scale[c] = pak.read_f32(at + 32 + c * 4)?;
                pos_offset[c] = pak.read_f32(at + 48 + c * 4)?;
            }
        }
        if kind == 130 {
            k130 = Some(buf);
        }
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
    let (pb, psz) = posbuf.ok_or("pos")?;
    let pd = pak.slice(pb, psz)?;
    let mut br = BR { d: pd, b: 0 };
    let mut pos = Vec::with_capacity(vc);
    for _ in 0..vc {
        let mut v = [0.; 3];
        for c in 0..4 {
            let w = pos_sizes[c] as usize;
            if w > 0 {
                let raw = br.r(w) as f32;
                if c < 3 {
                    v[c] = raw * pos_scale[c] + pos_offset[c];
                }
            }
        }
        pos.push(v)
    }
    let ids = pak.slice(ib, ic * 2)?;
    let mut acc = vec![[0.; 3]; vc];
    for t in (0..ic).step_by(3) {
        if t + 2 >= ic {
            break;
        }
        let ix = |k: usize| u16::from_le_bytes([ids[(t + k) * 2], ids[(t + k) * 2 + 1]]) as usize;
        let (a, b, c) = (ix(0), ix(1), ix(2));
        let n = cross(sub(pos[b], pos[a]), sub(pos[c], pos[a]));
        for i in [a, b, c] {
            for c in 0..3 {
                acc[i][c] += n[c];
            }
        }
    }
    let geom = acc.into_iter().map(norm).collect::<Vec<_>>();
    let raw = pak.slice(k130.ok_or("kind130")?, vc * 4)?;
    let perms = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut tests: Vec<(&str, Vec<[f32; 3]>)> = Vec::new();
    for (un, label) in [(false, "snorm8"), (true, "unorm8")] {
        let base = (0..vc)
            .map(|i| {
                let b = &raw[i * 4..i * 4 + 4];
                if un {
                    norm([u8n(b[0]), u8n(b[1]), u8n(b[2])])
                } else {
                    norm([s8(b[0]), s8(b[1]), s8(b[2])])
                }
            })
            .collect::<Vec<_>>();
        for (pn, p) in perms.iter().enumerate() {
            tests.push((
                Box::leak(format!("{}-perm{}", label, pn).into_boxed_str()),
                base.iter().map(|v| [v[p[0]], v[p[1]], v[p[2]]]).collect(),
            ));
        }
    }
    for (un, label) in [(false, "snorm10"), (true, "unorm10")] {
        let base = (0..vc)
            .map(|i| {
                let o = i * 4;
                let v = u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
                if un {
                    norm([u10(v), u10(v >> 10), u10(v >> 20)])
                } else {
                    norm([s10(v), s10(v >> 10), s10(v >> 20)])
                }
            })
            .collect::<Vec<_>>();
        for (pn, p) in perms.iter().enumerate() {
            tests.push((
                Box::leak(format!("{}-perm{}", label, pn).into_boxed_str()),
                base.iter().map(|v| [v[p[0]], v[p[1]], v[p[2]]]).collect(),
            ));
        }
    }
    tests.push((
        "oct-s16",
        (0..vc)
            .map(|i| {
                let o = i * 4;
                oct(
                    s16(u16::from_le_bytes(raw[o..o + 2].try_into().unwrap())),
                    s16(u16::from_le_bytes(raw[o + 2..o + 4].try_into().unwrap())),
                )
            })
            .collect(),
    ));
    tests.push((
        "oct-u16",
        (0..vc)
            .map(|i| {
                let o = i * 4;
                oct(
                    u16n(u16::from_le_bytes(raw[o..o + 2].try_into().unwrap())),
                    u16n(u16::from_le_bytes(raw[o + 2..o + 4].try_into().unwrap())),
                )
            })
            .collect(),
    ));
    for (un, label) in [(false, "quat-s8"), (true, "quat-u8")] {
        for (axis, basis) in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
            .into_iter()
            .enumerate()
        {
            tests.push((
                Box::leak(format!("{}-axis{}", label, axis).into_boxed_str()),
                (0..vc)
                    .map(|i| {
                        let o = i * 4;
                        let v = u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
                        norm(qrot(quat8(v, un), basis))
                    })
                    .collect(),
            ));
        }
    }
    // half2 isn't expected to be a vector, but log its numerical range as a packing clue.
    let mut h0 = Vec::new();
    let mut h1 = Vec::new();
    for i in 0..vc {
        let o = i * 4;
        h0.push(decode_f16(u16::from_le_bytes(
            raw[o..o + 2].try_into().unwrap(),
        )));
        h1.push(decode_f16(u16::from_le_bytes(
            raw[o + 2..o + 4].try_into().unwrap(),
        )));
    }
    println!(
        "half2 ranges x=[{:.4},{:.4}] y=[{:.4},{:.4}]",
        h0.iter().copied().fold(f32::INFINITY, f32::min),
        h0.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        h1.iter().copied().fold(f32::INFINITY, f32::min),
        h1.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    );
    let mut scored = tests
        .into_iter()
        .map(|(name, c)| {
            let ds = geom
                .iter()
                .zip(&c)
                .map(|(a, b)| dot(*a, *b).abs())
                .collect::<Vec<_>>();
            (q(ds.clone(), 0.5), q(ds, 0.1), name)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    for x in scored.into_iter().take(12) {
        println!("{} absdot median={:.4} p10={:.4}", x.2, x.0, x.1);
    }
    Ok(())
}
