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
struct LsbBitReader<'a> {
    data: &'a [u8],
    bit: usize,
}
impl<'a> LsbBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }
    fn read(&mut self, w: usize) -> Result<u32, String> {
        if w == 0 {
            return Ok(0);
        };
        if w > 32 || self.bit + w > self.data.len() * 8 {
            return Err("bitstream read out of range".into());
        }
        let mut v = 0u32;
        for i in 0..w {
            let at = self.bit + i;
            v |= (((self.data[at >> 3] >> (at & 7)) & 1) as u32) << i;
        }
        self.bit += w;
        Ok(v)
    }
}
fn desc(p: &PakFile, at: usize) -> Result<StreamDesc, String> {
    let mut sc = [0.; 4];
    let mut of = [0.; 4];
    for i in 0..4 {
        sc[i] = p.read_f32(at + 32 + i * 4)?;
        of[i] = p.read_f32(at + 48 + i * 4)?;
    }
    Ok(StreamDesc {
        kind: p.read_u8(at + 20)?,
        buffer: p.resolve_pointer(at)?.ok_or("buf")?,
        buffer_size: p.read_u32(at + 16)? as usize,
        sizes: [
            p.read_u8(at + 24)?,
            p.read_u8(at + 25)?,
            p.read_u8(at + 26)?,
            p.read_u8(at + 27)?,
        ],
        q_scale: sc,
        q_offset: of,
    })
}
fn qdecode(p: &PakFile, s: StreamDesc, n: usize) -> Result<Vec<[f32; 4]>, String> {
    let data = p.slice(s.buffer, s.buffer_size)?;
    let mut br = LsbBitReader::new(data);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = [0.; 4];
        for c in 0..4 {
            let w = s.sizes[c] as usize;
            if w > 0 {
                v[c] = br.read(w)? as f32 * s.q_scale[c] + s.q_offset[c];
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
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0., 1., 0.]
    }
}
fn quantile(mut v: Vec<f32>, q: f32) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f32 * q).round() as usize]
}
fn main() -> Result<(), String> {
    let src = env::args().nth(1).ok_or("pak")?;
    let pak = PakFile::parse(fs::read(src).map_err(|e| e.to_string())?)?;
    let r = pak.resource("GEOMETRY_1").ok_or("geom")?;
    let p = pak.resource_payload(r)?;
    let count = pak.read_u32(p + 8)? as usize;
    let table = pak.resolve_pointer(p + 40)?.ok_or("table")?;
    for si in 0..count.min(8) {
        let subm = table + si * 192;
        let name = pak
            .resolve_pointer(subm + 32)?
            .map(|x| pak.string_at(x))
            .transpose()?
            .unwrap_or_default();
        if !name.contains("LODShape0") {
            continue;
        }
        let vc = pak.read_u32(subm + 136)? as usize;
        let ic = pak.read_u32(subm + 140)? as usize;
        let sc = pak.read_u32(subm + 144)? as usize;
        let st = pak.resolve_pointer(subm + 48)?.ok_or("streams")?;
        let ib = pak.resolve_pointer(subm + 64)?.ok_or("indices")?;
        let ss = (0..sc)
            .map(|j| desc(&pak, st + j * 64))
            .collect::<Result<Vec<_>, _>>()?;
        let ps = *ss.iter().find(|s| s.kind == 64).ok_or("pos")?;
        let pos = qdecode(&pak, ps, vc)?;
        let mut acc = vec![[0.; 3]; vc];
        let bytes = pak.slice(ib, ic * 2)?;
        for t in (0..ic).step_by(3) {
            if t + 2 >= ic {
                break;
            }
            let ix = |k: usize| {
                u16::from_le_bytes([bytes[(t + k) * 2], bytes[(t + k) * 2 + 1]]) as usize
            };
            let (a, b, c) = (ix(0), ix(1), ix(2));
            if a >= vc || b >= vc || c >= vc {
                continue;
            }
            let pa = [pos[a][0], pos[a][1], pos[a][2]];
            let pb = [pos[b][0], pos[b][1], pos[b][2]];
            let pc = [pos[c][0], pos[c][1], pos[c][2]];
            let n = cross(sub(pb, pa), sub(pc, pa));
            for i in [a, b, c] {
                for k in 0..3 {
                    acc[i][k] += n[k];
                }
            }
        }
        let geom = acc.into_iter().map(norm).collect::<Vec<_>>();
        println!("MESH {si} {name}");
        for s in &ss {
            println!(
                " stream kind={} sizes={:?} scale={:?} offset={:?}",
                s.kind, s.sizes, s.q_scale, s.q_offset
            );
        }
        for kind in [75u8, 80u8] {
            if let Some(s) = ss.iter().find(|s| s.kind == kind).copied() {
                let q = qdecode(&pak, s, vc)?;
                let cand = q
                    .iter()
                    .map(|v| norm([v[0], v[1], v[2]]))
                    .collect::<Vec<_>>();
                let signed = geom
                    .iter()
                    .zip(&cand)
                    .map(|(a, b)| dot(*a, *b))
                    .collect::<Vec<_>>();
                let abs = signed.iter().map(|x| x.abs()).collect::<Vec<_>>();
                println!(" kind{} vec3 signed p10={:.4} median={:.4} p90={:.4} abs p10={:.4} median={:.4} p90={:.4} sample={:?}",kind,quantile(signed.clone(),0.1),quantile(signed.clone(),0.5),quantile(signed,0.9),quantile(abs.clone(),0.1),quantile(abs.clone(),0.5),quantile(abs,0.9),q.first());
            }
        }
    }
    Ok(())
}
